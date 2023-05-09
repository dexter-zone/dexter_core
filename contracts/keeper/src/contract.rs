#[cfg(not(feature = "library"))]
use crate::error::ContractError;
use crate::state::{CONFIG, OWNERSHIP_PROPOSAL};

use const_format::concatcp;
use cosmwasm_std::{Addr, Binary, Deps, DepsMut, entry_point, Env, Event, MessageInfo, Response, StdError, StdResult, to_binary, Uint128, CosmosMsg, WasmMsg, Coin, Decimal};
use cw2::{set_contract_version, get_contract_version};
use cw20::{Cw20ExecuteMsg};
use cw_storage_plus::Item;
use dexter::asset::{Asset, AssetInfo};
use dexter::keeper::{BalancesResponse, Config, ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, ConfigV1};
use dexter::helper::{claim_ownership, drop_ownership_proposal, EventExt, propose_new_owner};
use dexter::querier::query_token_balance;
use dexter::vault::{Cw20HookMsg, PoolInfo, ExitType, SingleSwapRequest};

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter-keeper";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const CONTRACT_VERSION_V1: &str = "1.0.0";

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x      Instantiate Contract : Execute function     x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// ## Description
/// Creates a new contract with the specified parameters in [`InstantiateMsg`].
/// Returns a default object of type [`Response`] if the operation was successful,
/// or a [`ContractError`] if the contract was not created.
///
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
/// * **_env** is an object of type [`Env`].
/// * **_info** is an object of type [`MessageInfo`].
/// * **msg** is a message of type [`InstantiateMsg`] which contains the parameters used for creating the contract.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let cfg = Config {
        owner: deps.api.addr_validate(msg.owner.as_str())?,
        vault_address: deps.api.addr_validate(msg.vault_address.as_str())?,
    };

    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new().add_event(
        Event::from_info(concatcp!(CONTRACT_NAME, "::instantiate"), &info)
            .add_attribute("owner", msg.owner.to_string())
    ))
}

// ----------------x----------------x----------------x------------------x----------------x----------------
// ----------------x----------------x  Execute function :: Entry Point  x----------------x----------------
// ----------------x----------------x----------------x------------------x----------------x----------------

/// ## Description
/// Exposes execute functions available in the contract.
/// ## Params
/// * **deps** is an object of type [`Deps`].
/// * **env** is an object of type [`Env`].
/// * **_info** is an object of type [`MessageInfo`].
/// * **msg** is an object of type [`ExecuteMsg`].
///
/// * **ExecuteMsg::UpdateConfig {
///             vault_contract,
///             staking_contract,
///         }** Updates general contract settings stores in the [`Config`].
///
/// * **ExecuteMsg::DistributeFees {}** Private method used by the contract to distribute collected Fees.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Withdraw {
            asset,
            amount,
            recipient,
        } => withdraw(deps, env, info, asset, amount, recipient),
        ExecuteMsg::ExitLPTokens { lp_token_address, amount } => {
            exit_lp_tokens(deps, env, info, lp_token_address, amount)
        }
        ExecuteMsg::SwapAsset { offer_asset, ask_asset_info, min_ask_amount, pool_id } => {
            swap_asset(deps, env, info, pool_id, offer_asset, ask_asset_info, min_ask_amount)
        }
        ExecuteMsg::ProposeNewOwner { owner, expires_in } => {
            let config: Config = CONFIG.load(deps.storage)?;
            propose_new_owner(
                deps,
                info,
                env,
                owner,
                expires_in,
                config.owner,
                OWNERSHIP_PROPOSAL,
                CONTRACT_NAME,
            )
            .map_err(|e| e.into())
        }
        ExecuteMsg::DropOwnershipProposal {} => {
            let config: Config = CONFIG.load(deps.storage)?;

            drop_ownership_proposal(deps, info, config.owner, OWNERSHIP_PROPOSAL, CONTRACT_NAME)
                .map_err(|e| e.into())
        }
        ExecuteMsg::ClaimOwnership {} => {
            claim_ownership(deps, info, env, OWNERSHIP_PROPOSAL, |deps, new_owner| {
                CONFIG.update::<_, StdError>(deps.storage, |mut v| {
                    v.owner = new_owner;
                    Ok(v)
                })?;

                Ok(())
            }, CONTRACT_NAME)
            .map_err(|e| e.into())
        }
    }
}

/// ## Description
/// Withdraws the specified amount of the specified asset from the contract.
/// Returns a [`ContractError`] on failure.
fn withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    asset: AssetInfo,
    amount: Uint128,
    recipient: Option<Addr>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Validate if we have enough balance
    let balance = asset.query_for_balance(&deps.querier, env.contract.address.clone())?;
    if balance < amount {
        return Err(ContractError::InsufficientBalance);
    }

    // Send the funds to the recipient or to the owner if no recipient is specified
    let recipient = recipient.unwrap_or(config.owner);
    let recipient = deps.api.addr_validate(recipient.as_str())?;
    let transfer_msg = asset.create_transfer_msg(recipient.clone(), amount)?;

    Ok(Response::new()
        .add_message(transfer_msg)
        .add_event(
            Event::from_info(concatcp!(CONTRACT_NAME, "::withdraw"), &info)
                .add_attribute("asset", serde_json_wasm::to_string(&asset).unwrap())
                .add_attribute("amount", amount.to_string())
                .add_attribute("recipient", recipient.to_string())
        )
    )
}

fn create_dexter_exit_pool_msg(
    deps: DepsMut,
    _env: &Env,
    vault_address: Addr,
    lp_token_address: Addr,
    amount: Uint128,
    sender: Addr,
    recipient: Option<Addr>
) -> Result<CosmosMsg, ContractError> {

    let recipient = recipient.unwrap_or(sender.clone());
    let recipient = recipient.to_string();

    let config  = CONFIG.load(deps.storage)?;

    let pool_info: PoolInfo = deps.querier.query_wasm_smart(
        config.vault_address.to_string(),
        &dexter::vault::QueryMsg::GetPoolByLpTokenAddress {
            lp_token_addr: lp_token_address.to_string(),
        }
    )?;

    let msg = Cw20ExecuteMsg::Send {
        contract: vault_address.to_string(),
        amount,
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: pool_info.pool_id,
            recipient: Some(recipient),
            exit_type: ExitType::ExactLpBurn { 
                lp_to_burn: amount,
                min_assets_out: None 
            }
        })?,
    };

    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_token_address.to_string(),
        funds: vec![],
        msg: to_binary(&msg)?,
    }))
}


/// Exits the specified amount of LP tokens using the specific Pool in the Dexter.
/// This is done so keeper mostly holds the base assets rather than LP tokens
fn exit_lp_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_token_address: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // query the lp token balance using CW20 query
    let lp_token_address = deps.api.addr_validate(lp_token_address.as_str())?;
    let lp_token_balance = query_token_balance(
        &deps.querier,
        lp_token_address.clone(),
        env.contract.address.clone(),
    )?;

    // Validate if we have enough balance as much as the owner wants to exit
    if lp_token_balance < amount {
        return Err(ContractError::InsufficientBalance);
    }

    // Create a dexter exit pool message and return the exited funds to the keeper itself
    let tranfer_msg = create_dexter_exit_pool_msg(
        deps,
        &env,
        config.vault_address,
        lp_token_address.clone(),
        amount,
        env.contract.address.clone(),
        Some(env.contract.address.clone()),
    )?;

    Ok(Response::new()
        .add_message(tranfer_msg)
        .add_event(
            Event::from_info(concatcp!(CONTRACT_NAME, "::exit_lp_tokens"), &info)
                .add_attribute("lp_token_address", lp_token_address.to_string())
                .add_attribute("amount", amount.to_string())
        )
    )
}

/// Swaps the specified amount of the specified asset for another asset using the Dexter protocol.
/// Returns a [`ContractError`] on failure.
fn swap_asset(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_id: Uint128,
    offer_asset: Asset,
    ask_asset_info: AssetInfo,
    min_receive: Option<Uint128>
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Validate if we have enough balance
    let balance = offer_asset.query_for_balance(&deps.querier, &env.contract.address)?;
    if balance < offer_asset.amount {
        return Err(ContractError::InsufficientBalance);
    }

    // Create a dexter swap message and return the swapped funds to the keeper itself
    let swap_msg = dexter::vault::ExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id,
            asset_in: offer_asset.info.clone(),
            asset_out: ask_asset_info.clone(),
            swap_type: dexter::vault::SwapType::GiveIn {},
            amount: offer_asset.amount,
            max_spread: Some(Decimal::from_ratio(5u128, 100u128)),
            belief_price: None,
        },
        recipient: Some(env.contract.address.to_string()),
        min_receive,
        max_spend: None,
    };

    let swap_send_funds = if let AssetInfo::NativeToken { denom } = &offer_asset.info {
        vec![Coin {
            denom: denom.clone(),
            amount: offer_asset.amount,
        }]
    } else {
        vec![]
    };

    let cosmos_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.vault_address.to_string(),
        funds: swap_send_funds,
        msg: to_binary(&swap_msg)?,
    });

    Ok(Response::new()
        .add_message(cosmos_msg)
        .add_event(
            Event::from_info(concatcp!(CONTRACT_NAME, "::swap_asset"), &info)
                .add_attribute("pool_id", pool_id.to_string())
                .add_attribute("offer_asset", offer_asset.to_string())
                .add_attribute("ask_asset_info", ask_asset_info.to_string())
                .add_attribute("min_receive", min_receive.unwrap_or_default().to_string())
        )
    )
}


// ----------------x----------------x---------------------x-----------------------x----------------x----------------
// ----------------x----------------x  :::: Keeper::QUERIES Implementation   ::::  x----------------x----------------
// ----------------x----------------x---------------------x-----------------------x----------------x----------------

/// ## Description
/// Exposes all the queries available in the contract.
///
/// # Params
/// * **deps** is an object of type [`DepsMut`].
/// * **env** is an object of type [`Env`].
/// * **msg** is an object of type [`QueryMsg`].
///
/// ## Queries
/// * **QueryMsg::Config {}** Returns the Keeper contract configuration using a [`ConfigResponse`] object.
/// * **QueryMsg::Balances { assets }** Returns the balances of certain tokens accrued by the Keeper
///
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_get_config(deps)?),
        QueryMsg::Balances { assets } => to_binary(&query_get_balances(deps, env, assets)?),
    }
}

/// ## Description
/// Returns information about the Keeper configuration using a [`ConfigResponse`] object.
/// ## Params
/// * **deps** is an object of type [`Deps`].
fn query_get_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: config.owner,
        vault_address: config.vault_address,
    })
}

/// ## Description
/// Returns Keeper's fee token balances for specific tokens using a [`ConfigResponse`] object.
/// ## Params
/// * **deps** is an object of type [`Deps`].
/// * **env** is an object of type [`Env`].
/// * **assets** is a vector that contains objects of type [`AssetInfo`]. These are the assets for which we query the Keeper's balances.
fn query_get_balances(deps: Deps, env: Env, assets: Vec<AssetInfo>) -> StdResult<BalancesResponse> {
    let mut resp = BalancesResponse { balances: vec![] };

    for a in assets {
        // Get balance
        let balance = a.query_for_balance(&deps.querier, env.contract.address.clone())?;
        if !balance.is_zero() {
            resp.balances.push(Asset {
                info: a,
                amount: balance,
            })
        }
    }

    Ok(resp)
}

// --------x--------x--------x--------x--------x--------x---
// --------x--------x Migrate Function   x--------x---------
// --------x--------x--------x--------x--------x--------x---

/// ## Description
/// Used for migration of contract. Returns the default object of type [`Response`].
/// ## Params
/// * **_deps** is the object of type [`DepsMut`].
/// * **_env** is the object of type [`Env`].
/// * **_msg** is the object of type [`MigrateMsg`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> StdResult<Response> {
    match msg {
        MigrateMsg::V2 {
            vault_address,
        } => {
            // verify if we are running on V1 right now
            let contract_version = get_contract_version(deps.storage)?;
            if contract_version.version != CONTRACT_VERSION_V1 {
                return Err(StdError::generic_err(format!(
                    "V2 upgrade is only supported over contract version {}. Current version is {}",
                    CONTRACT_VERSION_V1,
                    contract_version.version
                )));
            }

            // validate vault address
            let vault_address = deps.api.addr_validate(&vault_address)?;

            // if vault address is provided, check if it is valid by querying the config and parsing the response
            let _config: dexter::vault::ConfigResponse = deps.querier.query_wasm_smart(
                &vault_address,
                &dexter::vault::QueryMsg::Config {},
            )?;
        
            // update contract version
            set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

            // Load older config
            let config_v1: ConfigV1 = Item::new("config").load(deps.storage)?;
            let config = Config {
                owner: config_v1.owner,
                vault_address: vault_address.clone(),
            };

            CONFIG.save(deps.storage, &config)?;
        }
    }

    Ok(Response::default())
}
