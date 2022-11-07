use std::fmt::format;
use std::vec;

use crate::error::ContractError;
use crate::state::CONFIG;
use cosmwasm_std::{
    attr, entry_point, to_binary, Addr, Attribute, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, QueryRequest, Response, StdError, StdResult, Uint128, WasmMsg, WasmQuery,
};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};
use dexter::asset::{addr_validate_to_lower, Asset, AssetInfo};
use dexter::pool::ResponseType;
use dexter::querier::query_vault_config;
use dexter::router::{
    return_swap_sim_failure, CallbackMsg, Config, ConfigResponse, ExecuteMsg, HopSwapRequest,
    InstantiateMsg, MigrateMsg, QueryMsg, SimulateMultiHopResponse, SimulatedTrade,
};
use dexter::vault::{self, SingleSwapRequest, SwapType};
/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter-router";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x      Instantiate Contract : Execute function     x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let cfg = Config {
        dexter_vault: addr_validate_to_lower(deps.api, &msg.dexter_vault)?,
    };

    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::default())
}

// ----------------x----------------x----------------x------------------x----------------x----------------
// ----------------x----------------x  Execute function :: Entry Point  x----------------x----------------
// ----------------x----------------x----------------x------------------x----------------x----------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::ExecuteMultihopSwap {
            multiswap_request,
            offer_amount,
            recipient,
            minimum_receive,
        } => execute_multihop_swap(
            deps,
            env,
            info,
            multiswap_request,
            offer_amount,
            recipient,
            minimum_receive,
        ),
        ExecuteMsg::Callback(msg) => handle_callback(deps, env, info, msg),
    }
}

fn handle_callback(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CallbackMsg,
) -> Result<Response, ContractError> {
    // Callback functions can only be called this contract itself
    if info.sender != env.contract.address {
        return Err(ContractError::InvalidMultihopSwapRequest {
            msg: "callbacks cannot be invoked externally".to_string(),
        });
    }
    match msg {
        CallbackMsg::ContinueHopSwap {
            multiswap_request,
            offer_asset,
            prev_ask_amount,
            recipient,
            minimum_receive,
        } => continue_hop_swap(
            deps,
            env,
            info,
            multiswap_request,
            offer_asset,
            prev_ask_amount,
            recipient,
            minimum_receive,
        ),
    }
}

pub fn execute_multihop_swap(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mut multiswap_request: Vec<HopSwapRequest>,
    offer_amount: Uint128,
    recipient: Option<Addr>,
    minimum_receive: Option<Uint128>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Validate the multiswap request
    if multiswap_request.len() < 1 {
        return Err(ContractError::InvalidMultihopSwapRequest {
            msg: "Multihop swap request must contain at least 1 hop".to_string(),
        });
    }

    let mut execute_msgs: Vec<CosmosMsg> = vec![];
    let current_ask_balance: Uint128;
    let mut attributes: Vec<Attribute> = vec![];

    // Get number of offer asset (Native) tokens sent with the msg
    let tokens_received: Uint128;
    if multiswap_request[0].asset_in.is_native_token() {
        let tokens_received = multiswap_request[0]
            .asset_in
            .get_sent_native_token_balance(&info);

        // Error - If offer amount is invalid
        if tokens_received.is_zero() || tokens_received < offer_amount {
            return Err(ContractError::InvalidMultihopSwapRequest {
                msg: "Invalid number of tokens sent. ".to_string(),
            });
        }
    } else {
        // If a CW20 token, we need to give allowance to the dexter Vault contract
        let allowance_msg = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: multiswap_request[0].asset_in.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                spender: config.dexter_vault.to_string(),
                amount: offer_amount,
                expires: None,
            })?,
        });
        execute_msgs.push(allowance_msg);
    }

    // Create SingleSwapRequest for the first hop
    let first_hop = multiswap_request[0].clone();
    let first_hop_swap_request = SingleSwapRequest {
        pool_id: first_hop.pool_id,
        asset_in: first_hop.asset_in.clone(),
        asset_out: first_hop.asset_out,
        swap_type: SwapType::GiveIn {},
        // Amount provided is the amount to be used for the first hop
        amount: offer_amount,
        max_spread: first_hop.max_spread,
        belief_price: first_hop.belief_price,
    };

    // Need to send native tokens if the offer asset is native token
    let coins: Vec<Coin> = if first_hop.asset_in.is_native_token() {
        vec![Coin {
            denom: first_hop.asset_in.to_string(),
            amount: offer_amount,
        }]
    } else {
        vec![]
    };

    // ExecuteMsg for the first hop
    let first_hop_execute_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.dexter_vault.to_string(),
        funds: coins,
        msg: to_binary(&vault::ExecuteMsg::Swap {
            swap_request: first_hop_swap_request.clone(),
            recipient: Some(env.contract.address.clone().to_string()),
            min_receive: None,
            max_spend: None,
        })?,
    });
    execute_msgs.push(first_hop_execute_msg);

    // Get current balance of the ask asset (Native) token
    current_ask_balance = multiswap_request[0]
        .asset_out
        .query_for_balance(&deps.querier, env.contract.address.clone())?;

    // Add Callback Msg as we need to continue with the hops
    multiswap_request.remove(0);
    let arb_chain_msg = CallbackMsg::ContinueHopSwap {
        multiswap_request: multiswap_request,
        offer_asset: first_hop_swap_request.asset_out,
        prev_ask_amount: current_ask_balance,
        recipient: recipient.unwrap_or_else(|| info.sender),
        minimum_receive: minimum_receive,
    }
    .to_cosmos_msg(&env.contract.address)?;
    execute_msgs.push(arb_chain_msg);

    Ok(Response::new())
}

pub fn continue_hop_swap(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    mut multiswap_request: Vec<HopSwapRequest>,
    offer_asset: AssetInfo,
    prev_ask_amount: Uint128,
    recipient: Addr,
    minimum_receive: Option<Uint128>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Calculate current offer asset balance
    let asset_balance =
        offer_asset.query_for_balance(&deps.querier, env.contract.address.clone())?;
    let amount_returned_prev_hop = asset_balance.checked_sub(prev_ask_amount)?;

    // ExecuteMsgs
    let mut execute_msgs: Vec<CosmosMsg> = vec![];
    let current_ask_balance: Uint128;

    // If Hop is over, check if the minimum receive amount is met and transfer the tokens to the recipient
    if multiswap_request.len() == 0 {
        if amount_returned_prev_hop < minimum_receive.unwrap() {
            return Err(ContractError::InvalidMultihopSwapRequest {
                msg: "Minimum receive amount not met. Swap failed.".to_string(),
            });
        }
        execute_msgs.push(offer_asset.into_msg(recipient, amount_returned_prev_hop)?);
    } else {
        let next_hop = multiswap_request[0].clone();

        // Asset returned from prev hop needs to match the asset to be used for the next hop
        if !offer_asset.equal(&next_hop.asset_in.clone()) {
            return Err(ContractError::InvalidMultihopSwapRequest {
                msg:
                    "Invalid multiswap request. Offer asset does not match the next hop's asset in."
                        .to_string(),
            });
        }

        // - If a CW20 token, we need to give allowance to the dexter Vault contract
        if !offer_asset.is_native_token() {
            // If a CW20 token, we need to give allowance to the dexter Vault contract
            let allowance_msg = CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: offer_asset.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                    spender: config.dexter_vault.to_string(),
                    amount: amount_returned_prev_hop,
                    expires: None,
                })?,
            });
            execute_msgs.push(allowance_msg);
        }

        // Create SingleSwapRequest for the next hop
        let next_hop_swap_request = SingleSwapRequest {
            pool_id: next_hop.pool_id,
            asset_in: next_hop.asset_in.clone(),
            asset_out: next_hop.asset_out,
            swap_type: SwapType::GiveIn {},
            // Amount returned from prev hop is to be used for the next hop
            amount: amount_returned_prev_hop,
            max_spread: next_hop.max_spread,
            belief_price: next_hop.belief_price,
        };

        // Need to send native tokens if the offer asset is native token
        let coins: Vec<Coin> = if next_hop.asset_in.is_native_token() {
            vec![Coin {
                denom: next_hop.asset_in.to_string(),
                amount: amount_returned_prev_hop,
            }]
        } else {
            vec![]
        };

        // ExecuteMsg for the next hop
        let next_hop_execute_msg = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.dexter_vault.to_string(),
            funds: coins,
            msg: to_binary(&vault::ExecuteMsg::Swap {
                swap_request: next_hop_swap_request.clone(),
                recipient: Some(env.contract.address.clone().to_string()),
                min_receive: None,
                max_spend: None,
            })?,
        });
        execute_msgs.push(next_hop_execute_msg);

        // Get current balance of the ask asset (Native) token
        current_ask_balance = multiswap_request[0]
            .asset_out
            .query_for_balance(&deps.querier, env.contract.address.clone())?;

        // Add Callback Msg as we need to continue with the hops
        multiswap_request.remove(0);
        let arb_chain_msg = CallbackMsg::ContinueHopSwap {
            multiswap_request: multiswap_request,
            offer_asset: next_hop_swap_request.asset_out,
            prev_ask_amount: current_ask_balance,
            recipient: recipient,
            minimum_receive: minimum_receive,
        }
        .to_cosmos_msg(&env.contract.address)?;
        execute_msgs.push(arb_chain_msg);
    }

    Ok(Response::new())
}

// ----------------x----------------x---------------------x-----------------------x----------------x----------------
// ----------------x----------------x  :::: Keeper::QUERIES Implementation   ::::  x----------------x----------------
// ----------------x----------------x---------------------x-----------------------x----------------x----------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_get_config(deps)?),
        QueryMsg::SimulateMultihopSwap {
            multiswap_request,
            swap_type,
            amount,
        } => to_binary(&query_simulate_multihop(
            deps,
            env,
            multiswap_request,
            swap_type,
            amount,
        )?),
    }
}

fn query_get_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        dexter_vault: config.dexter_vault.to_string(),
    })
}

/// ## Description - Returns an object of type [`SimulateMultiHopResponse`] which contains the reponse type (success or failure)
/// along with the list of [`SimulatedTrade`] objects which contains the details of each hop in the multiswap request.
///
/// ## Params
/// * **multiswap_request** is the object of type [`Vec<HopSwapRequest>`] which contains the list of hops in the multiswap request.
/// * **swap_type** is the object of type [`SwapType`] which contains the type of swap (GiveIn or GiveOut). For GiveOut swaps, we need to simulate the swaps in reverse order.
/// * **amount** is the object of type [`Uint128`] which contains the amount to be provided for GiveIn swaps and the amount to be received for GiveOut swaps.
fn query_simulate_multihop(
    deps: Deps,
    _env: Env,
    multiswap_request: Vec<HopSwapRequest>,
    swap_type: SwapType,
    amount: Uint128,
) -> StdResult<SimulateMultiHopResponse> {
    let config = CONFIG.load(deps.storage)?;
    let mut simulated_trades: Vec<SimulatedTrade> = vec![];

    // Error - If invalid request
    if multiswap_request.len() == 0 {
        return_swap_sim_failure(vec![], "Multiswap request cannot be empty".to_string());
    }

    match swap_type {
        // If we are giving in, we need to simulate the trades in the order of the hops
        SwapType::GiveIn {} => {
            // Amount to be provided for the next hop
            let mut next_amount_in = amount;
            // Token to be provided for the next hop
            let mut next_token_in = multiswap_request[0].asset_in.clone();

            // Iterate on all swap operations and get the amount of tokens that will be received
            for hop in multiswap_request.iter() {
                //  Error - If the hop routes are invalid, we return an error
                if !next_token_in.equal(&hop.asset_in) {
                    return Ok(return_swap_sim_failure(
                        vec![],
                        format!("Invalid multiswap request. Asset {} out of previous hop does not match the asset {} to be provided for next hop.", next_token_in.to_string(), hop.asset_in.to_string())));
                }

                // Get pool info
                let pool_response: dexter::vault::PoolInfoResponse =
                    deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: config.dexter_vault.clone().to_string(),
                        msg: to_binary(&dexter::vault::QueryMsg::GetPoolById {
                            pool_id: hop.pool_id,
                        })?,
                    }))?;
                // println!(
                //     "Pool ID: {:?}  | asset_in: {:?} | asset_out: {:?} | amount_provided {:?}",
                //     hop.pool_id,
                //     next_token_in.to_string(),
                //     hop.asset_out.to_string(),
                //     next_amount_in.to_string()
                // );

                // Query pool to get the amount of tokens that will be received
                let pool_swap_transition: dexter::pool::SwapResponse =
                    deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: pool_response.pool_addr.clone().unwrap().to_string(),
                        msg: to_binary(&dexter::pool::QueryMsg::OnSwap {
                            swap_type: SwapType::GiveIn {},
                            offer_asset: next_token_in.clone(),
                            ask_asset: hop.asset_out.clone(),
                            amount: next_amount_in.clone(),
                            max_spread: hop.max_spread,
                            belief_price: hop.belief_price,
                        })?,
                    }))?;

                // println!("pool_swap_transition: {:?}", pool_swap_transition);

                // If the swap gives error, return the error
                if !pool_swap_transition.response.is_success() {
                    return_swap_sim_failure(
                        simulated_trades.clone(),
                        pool_swap_transition.response.to_string(),
                    );
                }

                // Create the SimulatedTrade object and push it to the vector
                simulated_trades.push(SimulatedTrade {
                    pool_id: hop.pool_id,
                    asset_in: next_token_in.clone(),
                    asset_out: hop.asset_out.clone(),
                    offered_amount: pool_swap_transition.trade_params.amount_in,
                    received_amount: pool_swap_transition.trade_params.amount_out,
                });
                // println!("simulated_trades: {:?}", simulated_trades);

                // Set the next amount in to the amount out of the previous swap as it will be used for the next swap
                next_amount_in = pool_swap_transition.trade_params.amount_out;
                // Set the next token in to the token out of the previous swap as it will be used for the next swap
                next_token_in = hop.asset_out.clone();
                // println!(
                //     "Number of {:?} tokens returned and are to be used for next swap: {:?}\n",
                //     hop.asset_out.clone().to_string(),
                //     next_amount_in
                // );
            }
        }
        SwapType::GiveOut {} => {
            // Amount to be received for the next hop
            let mut prev_amount_out = amount;
            // Token to be transferred for the next hop
            let mut prev_token_out = multiswap_request[multiswap_request.len() - 1]
                .asset_out
                .clone();
            // We Iterate recursively as we know the last return amount
            // and need to calculate the input amounts

            for hop in multiswap_request.iter().rev() {
                //  Error - If the hop routes are invalid, we return an error
                if !prev_token_out.equal(&hop.asset_out) {
                    return Ok(return_swap_sim_failure(
                        vec![],
                        format!("Invalid multiswap request. Asset {} to be provided for next hop does not match the asset {} returned by the current hop. ", prev_token_out.to_string(), hop.asset_in.to_string())));
                }

                println!(
                    "Pool ID: {:?}  | asset_in: {:?} | asset_out: {:?} | amount_to_receive {:?}",
                    hop.pool_id,
                    hop.asset_in.to_string(),
                    hop.asset_out.to_string(),
                    prev_amount_out.to_string()
                );

                // Get pool info
                let pool_response: dexter::vault::PoolInfoResponse =
                    deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: config.dexter_vault.clone().to_string(),
                        msg: to_binary(&dexter::vault::QueryMsg::GetPoolById {
                            pool_id: hop.pool_id,
                        })?,
                    }))?;

                // Query pool to get the swap transition response
                let pool_swap_transition: dexter::pool::SwapResponse =
                    deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: pool_response.pool_addr.clone().unwrap().to_string(),
                        msg: to_binary(&dexter::pool::QueryMsg::OnSwap {
                            swap_type: SwapType::GiveOut {},
                            offer_asset: hop.asset_in.clone(),
                            ask_asset: hop.asset_out.clone(),
                            amount: prev_amount_out.clone(),
                            max_spread: hop.max_spread,
                            belief_price: hop.belief_price,
                        })?,
                    }))?;

                // If the swap gives error, return the error
                if !pool_swap_transition.response.is_success() {
                    return_swap_sim_failure(
                        simulated_trades.clone(),
                        pool_swap_transition.response.to_string(),
                    );
                }

                // Create the SimulatedTrade object and push it to the vector.
                // We need to reverse the order of the trades as we are iterating in reverse order
                simulated_trades.insert(
                    0,
                    SimulatedTrade {
                        pool_id: hop.pool_id,
                        asset_in: hop.asset_in.clone(),
                        asset_out: hop.asset_out.clone(),
                        offered_amount: pool_swap_transition.trade_params.amount_in,
                        received_amount: pool_swap_transition.trade_params.amount_out,
                    },
                );

                println!(
                    "asset_in: {:?} offered_amount: {:?} || asset_out: {:?} received_amount: {:?}",
                    hop.asset_in.to_string(),
                    pool_swap_transition.trade_params.amount_in,
                    hop.asset_out.to_string(),
                    pool_swap_transition.trade_params.amount_out
                );

                // Number of tokens provied in the current hop are received from the previous hop
                prev_amount_out = pool_swap_transition.trade_params.amount_in;
                // Token provided in current swap is the token received in the previous swap
                prev_token_out = hop.asset_in.clone();
                println!(
                    "Number of {:?} tokens to be provided for this swap and should be returned by previous swap: {:?}\n",
                    hop.asset_in.clone().to_string(),
                    prev_amount_out
                );
            }
        }
        SwapType::Custom(_) => {
            return Ok(return_swap_sim_failure(
                vec![],
                "SwapType not supported".to_string(),
            ))
        }
    }

    Ok(SimulateMultiHopResponse {
        swap_operations: simulated_trades,
        response: ResponseType::Success {},
    })
}

/// Helper Function. Returns CosmosMsg which transfers CW20 Tokens from owner to recipient. (Transfers DEX from user to itself )
fn build_swap_via_vault_msg(
    vault_addr: String,
    swap_request: SingleSwapRequest,
    recipient: Option<String>,
    min_receive: Option<Uint128>,
    max_spend: Option<Uint128>,
    coin_to_transfer: Option<Coin>,
) -> StdResult<CosmosMsg> {
    // Tokens to transfer (to do if AssetIn is NativeToken)
    let mut coins: Vec<Coin> = vec![];

    if swap_request.asset_in.is_native_token() {
        coins.push(Coin {
            denom: swap_request.asset_in.to_string(),
            amount: max_spend.unwrap(),
        });
    };

    if let Some(coin) = coin_to_transfer {
        coins.push(coin);
    }
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: vault_addr,
        funds: coins,
        msg: to_binary(&dexter::vault::ExecuteMsg::Swap {
            swap_request,
            recipient,
            min_receive,
            max_spend,
        })?,
    }))
}
