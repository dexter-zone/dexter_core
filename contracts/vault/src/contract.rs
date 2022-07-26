use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, Event,
    MessageInfo, Order, QueryRequest, Reply, ReplyOn, Response, StdError, StdResult, SubMsg,
    Uint128, WasmMsg, WasmQuery,
};

use crate::error::ContractError;

use crate::state::{CONFIG, OWNERSHIP_PROPOSAL, POOLS, POOL_CONFIGS, TMP_POOL_INFO};

use crate::response::MsgInstantiateContractResponse;
use dexter::asset::{addr_opt_validate, addr_validate_to_lower, Asset, AssetInfo};
// use dexter::generator::Cw20HookMsg as GeneratorHookMsg;
use dexter::helper::build_transfer_cw20_from_user_msg;
use dexter::vault::{
    Config, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg,
    PoolConfigResponse, PoolInfo, PoolInfoResponse, PoolType, QueryMsg, SingleSwapRequest,
};

use cw2::{get_contract_version, set_contract_version};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use dexter::helper::{
    build_send_native_asset_msg, build_transfer_cw20_token_msg, claim_ownership,
    drop_ownership_proposal, propose_new_owner,
};
use dexter::pool::InstantiateMsg as PoolInstantiateMsg;
use protobuf::Message;
use std::collections::HashSet;

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter-vault";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// A `reply` call code ID of sub-message.
const INSTANTIATE_POOL_REPLY_ID: u64 = 1;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        owner: addr_validate_to_lower(deps.api, &msg.owner)?,
        pool_configs: msg.pool_configs.clone(),
        lp_token_code_id: msg.lp_token_code_id,
        fee_collector: addr_opt_validate(deps.api, &msg.fee_collector)?,
        generator_address: addr_opt_validate(deps.api, &msg.generator_address)?,
        next_pool_id: Uint128::from(1u128),
    };

    let config_set: HashSet<String> = msg
        .pool_configs
        .iter()
        .map(|pc| pc.pool_type.to_string())
        .collect();

    if config_set.len() != msg.pool_configs.len() {
        return Err(ContractError::PoolConfigDuplicate {});
    }

    // Save Pool Config info
    for pc in msg.pool_configs.iter() {
        // validate fee bps limits
        if !pc.valid_fee_bps() {
            return Err(ContractError::InvalidFeeBps {});
        }
        POOL_CONFIGS.save(deps.storage, pc.clone().pool_type.to_string(), pc)?;
    }
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::UpdateConfig {
            lp_token_code_id,
            fee_collector,
            generator_address,
        } => execute_update_config(
            deps,
            env,
            info,
            lp_token_code_id,
            fee_collector,
            generator_address,
        ),
        ExecuteMsg::UpdatePoolConfig {
            pool_type,
            is_disabled,
            is_generator_disabled,
        } => execute_update_pool_config(deps, info, pool_type, is_disabled, is_generator_disabled),
        ExecuteMsg::CreatePool {
            pool_type,
            asset_infos,
            lp_token_name,
            lp_token_symbol,
            pool_manager,
            init_params
        } => execute_create_pool(
            deps,
            env,
            pool_type,
            asset_infos,
            lp_token_name,
            lp_token_symbol,
            pool_manager,
        ),
        ExecuteMsg::JoinPool {
            pool_id,
            recepient,
            assets,
            lp_to_mint,
            auto_stake,
        } => execute_join_pool(deps, env, info, pool_id, recepient, assets, lp_to_mint,auto_stake),
        ExecuteMsg::Swap {
            swap_request,
            limit,
            deadline,
            recepient,
        } => execute_swap(deps, env, info, swap_request, limit, deadline, recepient),
        // TO DO
        // ExecuteMsg::BatchSwap {
        //     swap_kind,
        //     batch_swap_steps,
        //     assets,
        //     limit,
        //     deadline,
        // } => execute_batchswap(
        //     deps,
        //     env,
        //     swap_kind,
        //     batch_swap_steps,
        //     assets,
        //     limit,
        //     deadline,
        // ),
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
            )
            .map_err(|e| e.into())
        }
        ExecuteMsg::DropOwnershipProposal {} => {
            let config: Config = CONFIG.load(deps.storage)?;

            drop_ownership_proposal(deps, info, config.owner, OWNERSHIP_PROPOSAL)
                .map_err(|e| e.into())
        }
        ExecuteMsg::ClaimOwnership {} => {
            claim_ownership(deps, info, env, OWNERSHIP_PROPOSAL, |deps, new_owner| {
                CONFIG.update::<_, StdError>(deps.storage, |mut v| {
                    v.owner = new_owner;
                    Ok(v)
                })?;

                Ok(())
            })
            .map_err(|e| e.into())
        }
    }
}

/// ## Description
/// Receives a message of type [`Cw20ReceiveMsg`] and processes it depending on the received template.
/// If the template is not found in the received message, then an [`ContractError`] is returned,
/// otherwise returns the [`Response`] with the specified attributes if the operation was successful
/// ## Params
/// * **deps** is the object of type [`DepsMut`].
///
/// * **env** is the object of type [`Env`].
///
/// * **info** is the object of type [`MessageInfo`].
///
/// * **cw20_msg** is the object of type [`Cw20ReceiveMsg`].
pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::ExitPool {
            pool_id,
            recepient,
            assets,
            burn_amount,
        } => execute_exit_pool(deps, env, info, pool_id, recepient, assets, burn_amount),
    }
}

/// ## Description - Updates general settings. Returns an [`ContractError`] on failure or the following [`Config`]
/// data will be updated if successful.
///
/// ## Params
/// * **param** is the object of type [`UpdateConfig`] that contains information to update.
///
/// ##Executor - Only owner can execute it
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    lp_token_code_id: Option<u64>,
    fee_collector: Option<String>,
    generator_address: Option<String>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender.clone() != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(fee_collector) = fee_collector {
        config.fee_collector = Some(addr_validate_to_lower(deps.api, fee_collector.as_str())?);
    }

    if let Some(generator_address) = generator_address {
        config.generator_address = Some(addr_validate_to_lower(
            deps.api,
            generator_address.as_str(),
        )?);
    }

    if let Some(lp_token_code_id) = lp_token_code_id {
        config.lp_token_code_id = lp_token_code_id;
    }

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_config"))
}

/// ## Description - Updates pool configuration. Returns an [`ContractError`] on failure or
/// the following [`PoolConfig`] data will be updated if successful.
///
/// ## Params
/// * **pool_config** is the object of type [`PoolConfig`] that contains information to update.
///
/// ## Executor
/// Only owner can execute it
pub fn execute_update_pool_config(
    deps: DepsMut,
    info: MessageInfo,
    pool_type: PoolType,
    is_disabled: Option<bool>,
    is_generator_disabled: Option<bool>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender.clone() != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut pool_config = POOL_CONFIGS.load(deps.storage, pool_type.to_string())?;

    if let Some(is_disabled) = is_disabled {
        pool_config.is_disabled = is_disabled;
    }

    if let Some(is_generator_disabled) = is_generator_disabled {
        pool_config.is_generator_disabled = is_generator_disabled;
    }

    POOL_CONFIGS.save(
        deps.storage,
        pool_config.pool_type.to_string(),
        &pool_config,
    )?;

    Ok(Response::new().add_attribute("action", "update_pool_config"))
}

/// ## Description - Creates a new pool with the specified parameters in the `asset_infos` variable. Returns an [`ContractError`] on failure or
/// returns the address of the contract if the creation was successful.
///
/// ## Params
/// * **pool_type** is the object of type [`PoolType`].
/// * **asset_infos** is an array with two items the type of [`AssetInfo`].
pub fn execute_create_pool(
    deps: DepsMut,
    env: Env,
    pool_type: PoolType,
    mut asset_infos: Vec<AssetInfo>,
    lp_token_name: Option<String>,
    lp_token_symbol: Option<String>,
    pool_manager: Option<String>,
    init_params: Option<Binary>,
) -> Result<Response, ContractError> {
    // Sort Assets List
    asset_infos.sort_by(|a, b| {
        a.to_string()
            .to_lowercase()
            .cmp(&b.to_string().to_lowercase())
    });

    let mut assets: Vec<Asset> = vec![];

    // Check asset definations and make sure no asset is repeated
    let mut previous_asset: String;
    for asset in asset_infos.iter() {
        asset.check(deps.api)?; // Asset naming should be lower case
        if previous_asset == asset.as_string() {
            return Err(ContractError::RepeatedAssets {});
        }
        previous_asset = asset.as_string();

        assets.push(Asset {
            info: asset.to_owned(),
            amount: Uint128::zero(),
        });
    }

    let config = CONFIG.load(deps.storage)?;

    // Get pool type from config
    let pool_config = POOL_CONFIGS
        .load(deps.storage, pool_type.to_string())
        .map_err(|_| ContractError::PoolConfigNotFound {})?;

    // Check if pool config is disabled
    if pool_config.is_disabled {
        return Err(ContractError::PoolConfigDisabled {});
    }

    let pool_id = config.next_pool_id;

    let tmp_pool_info = PoolInfo {
        pool_id: pool_id,
        pool_addr: None,
        lp_token_addr: None,
        assets: assets,
        pool_type: pool_config.pool_type.clone(),
        dev_addr_bps: pool_config.fee_info.clone().dev_addr_bps,
        pool_manager: pool_manager,
    };

    TMP_POOL_INFO.save(deps.storage, &tmp_pool_info)?;

    let sub_msg: Vec<SubMsg> = vec![SubMsg {
        id: INSTANTIATE_POOL_REPLY_ID,
        msg: WasmMsg::Instantiate {
            admin: Some(config.owner.to_string()),
            code_id: pool_config.code_id,
            msg: to_binary(&PoolInstantiateMsg {
                pool_id: pool_id,
                pool_type: pool_config.pool_type,
                vault_addr: env.contract.address,
                asset_infos: asset_infos.clone(),
                fee_info: pool_config.fee_info,
                lp_token_code_id: config.lp_token_code_id,
                lp_token_name: lp_token_name,
                lp_token_symbol,
                init_params
            })?,
            funds: vec![],
            label: "dexter-pool-".to_string() + &pool_id.to_string(),
        }
        .into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    }];

    Ok(Response::new()
        .add_submessages(sub_msg)
        .add_attributes(vec![
            attr("action", "create_pool"),
            attr("pool_type", pool_type.to_string()),
        ]))
}

/// # Description
/// The entry point to the contract for processing the reply from the submessage
/// # Params
/// * **msg** is the object of type [`Reply`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    let tmp_pool_info = TMP_POOL_INFO.load(deps.storage)?;

    let data = msg.result.unwrap().data.unwrap();
    let res: MsgInstantiateContractResponse =
        Message::parse_from_bytes(data.as_slice()).map_err(|_| {
            StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
        })?;

    let pool_contract = addr_validate_to_lower(deps.api, res.get_contract_address())?;
    POOLS.save(
        deps.storage,
        &tmp_pool_info.pool_id.to_string().as_bytes(),
        &tmp_pool_info,
    )?;

    let mut config = CONFIG.load(deps.storage)?;
    config.next_pool_id = config.next_pool_id.checked_add(Uint128::from(1u128))?;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "register"),
        attr("pool_contract_addr", pool_contract),
    ]))
}

pub fn execute_join_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_id: Uint128,
    op_recepient: Option<String>,
    mut assets_in: Vec<Asset>,
    lp_to_mint: Option<Uint128>,
    auto_stake: Option<bool>,
) -> Result<Response, ContractError> {
    let mut pool_info = POOLS
        .load(deps.storage, pool_id.to_string().as_bytes())
        .expect("Invalid Pool Id");

    let mut missing_assets: Vec<Asset> = vec![];

    // If some assets are omitted then add them explicitly with 0 deposit
    pool_info.assets.iter().for_each(|(asset_info, amount)| {
        if !assets_in.iter().any(|asset| asset.info.eq(asset_info)) {
            missing_assets.push(
                Asset {
                    amount: Uint128::zero(),
                    info: asset_info.clone(),
                }                
            );
        }
    });    
    assets_in.extend(missing_assets);


    // assert slippage tolerance
    // assert_slippage_tolerance(slippage_tolerance, &deposits, &pools)?;

    // Query Pool Instance for Math Operations --> Returns response type (success or failure), number of LP shares to be minted and the list of Assets which are to be returned
    let after_join_res: dexter::pool::AfterJoinResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: pool_info.pool_addr.clone().unwrap().to_string(),
            msg: to_binary(&dexter::pool::QueryMsg::OnJoinPool {
                assets_in: Some(assets_in.clone()),
                lp_to_mint: Some(mint_amount)

            })?,
        }))?;

    // If the response is failure
    if !after_join_res.response.is_success() {
        return Err(ContractError::PoolQueryFailed {});
    }

    // Number of Assets should match
    if pool_info.assets.len() != assets_in.len()
        || after_join_res.return_assets.len() != pool_info.assets.len()
    {
        return Err(ContractError::InvalidNumberOfAssets {});
    }

    // Number of LP tokens to be minted
    let new_shares = after_join_res.new_shares;

    // Sort the assets
    assets_in.sort_by(|a, b| {
        a.info
            .to_string()
            .to_lowercase()
            .cmp(&b.info.to_string().to_lowercase())
    });

    let mut execute_msgs: Vec<CosmosMsg> = vec![];

    // Update asset balances
    let mut index = 0;
    for stored_asset in pool_info.assets.iter() {
        if stored_asset.info != assets_in[index].info
            || stored_asset.info != after_join_res.return_assets[index].info
        {
            return Err(ContractError::InvalidSequenceOfAssets {});
        }
        // Number of tokens to be transferred to the Vault
        let to_transfer = assets_in[index]
            .amount
            .checked_sub(after_join_res.return_assets[index].amount)?;

        // If number of tokens to transfer > 0, then
        // - Update stored pool's asset balances in `PoolInfo` Struct
        // - Transfer net calculated CW20 tokens from user to the Vault
        // - Return native tokens to the user (which are to be returned)
        if !to_transfer.is_zero() {
            // PoolInfo State update -
            stored_asset.amount = stored_asset.amount.checked_add(to_transfer)?;
            // Token Transfers
            if !stored_asset.info.is_native_token() {
                // Transfer Number of CW tokens = User wants to provide - Pool Math instructs to return
                execute_msgs.push(build_transfer_cw20_from_user_msg(
                    stored_asset.info.as_string(),
                    op_recepient
                        .clone()
                        .unwrap_or(info.sender.clone().to_string()),
                    info.sender.to_string(),
                    to_transfer,
                )?);
            } else {
                // Check if correct number of native tokens were sent
                assets_in[index].assert_sent_native_token_balance(&info)?;
                // If native tokens to return > 0, send native tokens back to sender
                if !after_join_res.return_assets[index].amount.is_zero() {
                    execute_msgs.push(build_send_native_asset_msg(
                        info.sender.clone(),
                        &assets_in[index].info.as_string(),
                        after_join_res.return_assets[index].amount,
                    )?);
                }
            }
        }
        // Increment Index
        index = index + 1;
    }

    let config = CONFIG.load(deps.storage)?;

    // LP Token recepient
    let recepient: Addr;
    if auto_stake.is_some() && auto_stake.unwrap() {
        recepient = config
            .generator_address
            .clone()
            .expect("Generator address not set");
    } else {
        recepient = addr_validate_to_lower(
            deps.api,
            op_recepient.unwrap_or(info.sender.to_string()).as_str(),
        )?;
    }

    // Mint LP Tokens
    let mint_msgs = build_mint_lp_token_msg(
        deps.as_ref(),
        env.clone(),
        pool_info.lp_token_addr.clone().unwrap(),
        recepient,
        new_shares,
        config.generator_address.clone(),
        auto_stake.unwrap_or(false),
    )?;
    for msg in mint_msgs {
        execute_msgs.push(msg);
    }

    // Pool State Update Execution :: Send Updated pool state to the Pool Contract so it can do its internal computes
    execute_msgs.push(build_update_pool_state_msg(
        pool_info.pool_addr.clone().unwrap().to_string(),
        pool_info.assets.clone(),
    )?);
    POOLS.save(deps.storage, &pool_id.to_string().as_bytes(), &pool_info)?;

    // Emit Event
    let event = Event::new("dexter-vault::join_pool")
        .add_attribute("pool_id", pool_id.to_string())
        .add_attribute("pool_addr", pool_info.pool_addr.unwrap().to_string())
        .add_attribute("lp_tokens_minted", new_shares.to_string());

    Ok(Response::new()
        .add_messages(execute_msgs)
        .add_attribute("action", "dexter-vault/execute/join_pool")
        .add_event(event))
}

pub fn execute_exit_pool(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    pool_id: Uint128,
    op_recepient: Option<String>,
    mut assets_out: Option<Vec<Asset>>,
    burn_amount: Uint128,
) -> Result<Response, ContractError> {
    let mut pool_info = POOLS
        .load(deps.storage, pool_id.to_string().as_bytes())
        .expect("Invalid Pool Id");

    // If some assets are omitted then add them explicitly with 0 deposit
    pool_info.assets.iter().for_each(|(asset_info, amount)| {
        if !assets_in.iter().any(|asset| asset.info.eq(asset_info)) {
            missing_assets.push(
                Asset {
                    amount: Uint128::zero(),
                    info: asset_info.clone(),
                }                
            );
        }
    });    
    assets_in.extend(missing_assets);

    // assert slippage tolerance
    // assert_slippage_tolerance(slippage_tolerance, &deposits, &pools)?;

    // Query Pool Instance for Math Operations --> Returns response type (success or failure), number of LP shares to be burned and the list of Assets which are to be returned
    let after_burn_res: dexter::pool::AfterExitResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: pool_info.pool_addr.clone().unwrap().to_string(),
            msg: to_binary(&dexter::pool::QueryMsg::OnExitPool {
                assets_out: assets_out.clone(),
                burn_amount,
            })?,
        }))?;

    // If the response is failure
    if !after_burn_res.response.is_success() {
        return Err(ContractError::PoolQueryFailed {});
    }

    // Check : Lp token to burn > Lp tokens transferred by the user
    if after_burn_res.burn_shares > burn_amount {
        return Err(ContractError::InsufficientLpTokensToExit {});
    }

    // Sort the assets
    if !assets_out.is_none() {
        assets_out.unwrap().sort_by(|a, b| {
            a.info
                .to_string()
                .to_lowercase()
                .cmp(&b.info.to_string().to_lowercase())
        });
    }

    let mut execute_msgs: Vec<CosmosMsg> = vec![];

    let mut event = Event::new("dexter-vault::exit_pool")
        .add_attribute("pool_id", pool_id.to_string())
        .add_attribute(
            "pool_addr",
            pool_info.pool_addr.clone().unwrap().to_string(),
        )
        .add_attribute("lp_tokens_burnt", after_burn_res.burn_shares.to_string());

    // Recepient address
    let mut recepient = info.sender.clone();
    if !op_recepient.is_none() {
        recepient = addr_validate_to_lower(
            deps.api,
            op_recepient.unwrap_or(info.sender.to_string()).as_str(),
        )?;
    }

    // Update asset balances
    let mut index = 0;
    for stored_asset in pool_info.assets.iter() {
        // If sequence of tokens doesn't match
        if stored_asset.info != after_burn_res.assets_out[index].info {
            return Err(ContractError::InvalidSequenceOfAssets {});
        }
        // Number of tokens to be transferred to the recepient: As instructed by the Pool Math
        let to_transfer = after_burn_res.assets_out[index].amount;

        // If number of tokens to transfer > 0, then
        // - Update stored pool's asset balances in `PoolInfo` Struct
        // - Transfer tokens to the recepient
        if !to_transfer.is_zero() {
            // PoolInfo State update -
            stored_asset.amount = stored_asset.amount.checked_add(to_transfer)?;
            // Token Transfers
            if !stored_asset.info.is_native_token() {
                // Transfer Number of CW tokens the Pool Math instructs to return
                execute_msgs.push(build_transfer_cw20_token_msg(
                    recepient.clone(),
                    stored_asset.info.as_string(),
                    to_transfer,
                )?);
            } else {
                // Transfer Number of Native tokens the Pool Math instructs to return
                execute_msgs.push(build_send_native_asset_msg(
                    recepient.clone(),
                    &after_burn_res.assets_out[index].info.as_string(),
                    after_burn_res.assets_out[index].amount,
                )?);
            }
            // Add attribute to event for indexing support
            event = event.add_attribute(
                after_burn_res.assets_out[index].info.as_string(),
                to_transfer.to_string(),
            );
        }
        // Increment Index
        index = index + 1;
    }

    // Burn LP Tokens
    execute_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: pool_info.lp_token_addr.clone().unwrap().to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Burn {
            amount: after_burn_res.burn_shares,
        })?,
        funds: vec![],
    }));

    // Return LP shares in case some of the LP tokens transferred are to be returned
    let to_return = burn_amount.checked_sub(after_burn_res.burn_shares)?;
    if !to_return.is_zero() {
        execute_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: pool_info.lp_token_addr.clone().unwrap().to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                amount: to_return,
                recepient: info.sender,
            })?,
            funds: vec![],
        }));
    }

    // Pool State Update Execution :: Send Updated pool state to the Pool Contract so it can do its internal computes
    execute_msgs.push(build_update_pool_state_msg(
        pool_info.pool_addr.clone().unwrap().to_string(),
        pool_info.assets.clone(),
    )?);
    POOLS.save(deps.storage, &pool_id.to_string().as_bytes(), &pool_info)?;

    Ok(Response::new().add_messages(execute_msgs).add_event(event))
}

pub fn execute_swap(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    swap_request: SingleSwapRequest,
    limit: Option<Uint128>,
    deadline: Option<Uint128>,
    op_recepient: Option<String>,
) -> Result<Response, ContractError> {
    let mut pool_info = POOLS
        .load(deps.storage, swap_request.pool_id.to_string().as_bytes())
        .expect("Invalid Pool Id");

    // If some assets are omitted then add them explicitly with 0 deposit
    pool_info.assets.iter().for_each(|(asset_info, _)| {
        if !assets_in.iter().any(|asset| asset.info.eq(asset_info)) {
            missing_assets.push(
                Asset {
                    amount: Uint128::zero(),
                    info: asset_info.clone(),
                }                
            );
        }
    });    
    assets_in.extend(missing_assets);

    if deadline.is_some() {
        return Err(ContractError::DeadlineExpired {});
    }

    if swap_request.amount.is_zero() {
        return Err(ContractError::InvalidAmount {});
    }

    if swap_request.asset_in == swap_request.asset_out {
        return Err(ContractError::SameTokenError {});
    }

    let mut event = Event::new("dexter-vault::swap")
        .add_attribute("pool_id", swap_request.pool_id.to_string())
        .add_attribute(
            "pool_addr",
            pool_info.pool_addr.clone().unwrap().to_string(),
        )
        .add_attribute("swap_type", swap_request.swap_type.to_string());

    // Query Pool Instance for Math Operations --> Returns response type (success or failure), number of LP shares to be burned and the list of Assets which are to be returned
    // Calculate new balances and swap amount, fees etc now
    let swap_response: dexter::pool::SwapResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: pool_info.pool_addr.clone().unwrap().to_string(),
            msg: to_binary(&dexter::pool::QueryMsg::OnSwap {
                swap_type: swap_request.swap_type,
                offer_asset: swap_request.asset_in.clone(),
                ask_asset: swap_request.asset_out.clone(),
                amount: swap_request.amount,
            })?,
        }))?;

    // If the response is failure
    if !swap_response.response.is_success() {
        return Err(ContractError::PoolQueryFailed {});
    }

    // // check max spread limit if exist
    // assert_max_spread(
    //     belief_price,
    //     max_spread,
    //     offer_amount,
    //     return_amount + commission_amount,
    //     spread_amount,
    // )?;

    let offer_asset = Asset {
        info: swap_request.asset_in.clone(),
        amount: swap_response.trade_params.amount_in,
    };
    let ask_asset = Asset {
        info: swap_request.asset_out.clone(),
        amount: swap_response.trade_params.amount_out,
    };

    event = event
        .add_attribute("offer_asset", offer_asset.info.to_string())
        .add_attribute("offer_amount", offer_asset.amount.to_string())
        .add_attribute("ask_asset", ask_asset.info.to_string())
        .add_attribute("ask_amount", ask_asset.amount.to_string());

    // Recepient address
    let mut recepient = info.sender.clone();
    if !op_recepient.is_none() {
        recepient = addr_validate_to_lower(
            deps.api,
            op_recepient
                .unwrap_or(info.sender.clone().to_string())
                .as_str(),
        )?;
    }

    // Update asset balances
    let mut index = 0;
    let mut offer_asset_updated: bool = false;
    let mut ask_asset_updated: bool = false;
    let mut execute_msgs: Vec<CosmosMsg> = vec![];

    for stored_asset in pool_info.assets.iter() {
        // Update state : Offer Asset
        if stored_asset.info == offer_asset.info {
            stored_asset.amount = stored_asset.amount.checked_add(offer_asset.amount)?;
            offer_asset_updated = true;
            // Execute Msgs : Transfer tokens from user to the vault
            if !offer_asset.is_native_token() {
                // Transfer CW20 tokens from user to the Vault
                execute_msgs.push(build_transfer_cw20_from_user_msg(
                    offer_asset.info.as_string(),
                    info.sender.clone().to_string(),
                    env.contract.address.to_string(),
                    offer_asset.amount,
                )?);
            } else {
                // Get number of offer asset (Native) tokens sent with the msg
                let native_tokens_sent = offer_asset.info.get_sent_native_token_balance(&info);
                // If number of tokens sent are less than what the pool expects, return error
                if native_tokens_sent < offer_asset.amount {
                    return Err(ContractError::InsufficientTokensSent {});
                }
                // If number of tokens sent are more than what the pool expects, return additional tokens sent
                if native_tokens_sent > offer_asset.amount {
                    let extra = native_tokens_sent.checked_sub(offer_asset.amount)?;
                    execute_msgs.push(build_send_native_asset_msg(
                        info.sender.clone(),
                        &offer_asset.info.as_string(),
                        extra,
                    )?);
                }
            }
        }
        // Update state : Ask Asset
        if stored_asset.info == ask_asset.info {
            stored_asset.amount = stored_asset.amount.checked_sub(
                ask_asset.amount
                    + swap_response.trade_params.protocol_fee
                    + swap_response.trade_params.dev_fee,
            )?;
            ask_asset_updated = true;
            // Execute Msgs : Transfer tokens from Vault to the recepient
            if !ask_asset.is_native_token() {
                // Transfer CW20 tokens from Vault to the recepient
                execute_msgs.push(build_transfer_cw20_token_msg(
                    recepient.clone(),
                    ask_asset.info.as_string(),
                    ask_asset.amount,
                )?);
            }
            // Transfer Native tokens from Vault to the recepient
            else {
                execute_msgs.push(build_send_native_asset_msg(
                    recepient.clone(),
                    &ask_asset.info.as_string(),
                    ask_asset.amount,
                )?);
            }
        }
        // Increment Index
        index = index + 1;
    }

    // Error is something is wrong with state update operations
    if !offer_asset_updated || !ask_asset_updated {
        return Err(ContractError::MismatchedAssets {});
    }

    // Update PoolInfo stored state
    POOLS.save(
        deps.storage,
        &swap_request.pool_id.to_string().as_bytes(),
        &pool_info,
    )?;

    execute_msgs.push(build_update_pool_state_msg(
        pool_info.pool_addr.unwrap().to_string(),
        pool_info.assets,
    )?);

    let config = CONFIG.load(deps.storage)?;

    // transfer ask asset as Fee to Keeper Contract and Developer Address
    if !ask_asset.info.is_native_token() {
        // Execute Msg :: Protocol Fee transfer to Keeper contract
        if !swap_response.trade_params.protocol_fee.is_zero() && config.fee_collector.is_some() {
            execute_msgs.push(build_transfer_cw20_token_msg(
                config.fee_collector.unwrap(),
                ask_asset.info.as_string(),
                swap_response.trade_params.protocol_fee,
            )?);
            event = event.add_attribute(
                "protocol_fee",
                swap_response.trade_params.protocol_fee.to_string(),
            )
        }
        // Execute Msg :: Dev Fee transfer
        if !swap_response.trade_params.dev_fee.is_zero() && pool_info.dev_addr_bps.is_some() {
            execute_msgs.push(build_transfer_cw20_token_msg(
                pool_info.dev_addr_bps.unwrap(),
                ask_asset.info.as_string(),
                swap_response.trade_params.dev_fee,
            )?);
            event = event.add_attribute("dev_fee", swap_response.trade_params.dev_fee.to_string())
        }
    } else {
        // Execute Msg :: Protocol Fee transfer to dv contract
        if !swap_response.trade_params.protocol_fee.is_zero() && config.fee_collector.is_some() {
            execute_msgs.push(build_send_native_asset_msg(
                config.fee_collector.unwrap(),
                &ask_asset.info.as_string(),
                swap_response.trade_params.protocol_fee,
            )?);
            event = event.add_attribute(
                "protocol_fee",
                swap_response.trade_params.protocol_fee.to_string(),
            )
        }
        if !swap_response.trade_params.dev_fee.is_zero() && pool_info.dev_addr_bps.is_some() {
            execute_msgs.push(build_send_native_asset_msg(
                pool_info.dev_addr_bps.unwrap(),
                &ask_asset.info.as_string(),
                swap_response.trade_params.dev_fee,
            )?);
            event = event.add_attribute("dev_fee", swap_response.trade_params.dev_fee.to_string())
        }
    }

    Ok(Response::new().add_messages(execute_msgs).add_event(event))
}

/// ## Description - Available the query messages of the contract.
/// ## Params
/// * **msg** is the object of type [`QueryMsg`].
///
/// ## Queries
/// * **QueryMsg::Config {}** Returns controls settings that specified in custom [`ConfigResponse`] structure.
/// * **QueryMsg::Pair { asset_infos }** Returns the [`PoolInfo`] object with the specified input parameters
/// * **QueryMsg::Pairs { start_after, limit }** Returns an array that contains items of [`PoolInfo`] according to the specified input parameters.
/// * **QueryMsg::FeeInfo { pool_type }** Returns the settings specified in the custom structure [`FeeInfoResponse`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::PoolConfig { pool_type } => to_binary(&query_pool_config(deps, pool_type)?),
        // TO-DO
        QueryMsg::GetPoolById { pool_id } => to_binary(&query_pool_by_id(deps, pool_id)?),
        // TO-DO
        QueryMsg::GetPoolByAddress { pool_addr } => {
            to_binary(&query_pool_by_addr(deps, pool_addr)?)
        }
    }
}

/// ## Description - Returns controls settings that specified in custom [`ConfigResponse`] structure
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let resp = ConfigResponse {
        owner: config.owner,
        lp_token_code_id: config.lp_token_code_id,
        fee_collector: config.fee_collector,
        generator_address: config.generator_address,
        pool_configs: POOL_CONFIGS
            .range(deps.storage, None, None, Order::Ascending)
            .map(|item| {
                let (_, cfg) = item.unwrap();
                cfg
            })
            .collect(),
    };
    Ok(resp)
}

pub fn query_pool_config(deps: Deps, pool_type: PoolType) -> StdResult<PoolConfigResponse> {
    let pool_config = POOL_CONFIGS.load(deps.storage, pool_type.to_string())?;
    Ok(pool_config)
}

pub fn query_pool_by_id(deps: Deps, pool_id: Uint128) -> StdResult<PoolInfoResponse> {
    let pool_info = POOLS
        .load(deps.storage, pool_id.to_string().as_bytes())
        .unwrap();
    Ok(pool_info)
}

pub fn query_pool_by_addr(deps: Deps, pool_addr: Addr) -> StdResult<PoolInfoResponse> {
    let pool_id: Uint128 = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: pool_addr.to_string(),
        msg: to_binary(&dexter::pool::QueryMsg::PoolId {})?,
    }))?;
    let pool_info = POOLS
        .load(deps.storage, pool_id.to_string().as_bytes())
        .unwrap();
    Ok(pool_info)
}

/// ## Description - Used for migration of contract. Returns the default object of type [`Response`].
/// ## Params
/// * **_msg** is the object of type [`MigrateMsg`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    let contract_version = get_contract_version(deps.storage)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new()
        .add_attribute("previous_contract_name", &contract_version.contract)
        .add_attribute("previous_contract_version", &contract_version.version)
        .add_attribute("new_contract_name", CONTRACT_NAME)
        .add_attribute("new_contract_version", CONTRACT_VERSION))
}

/// # Description
/// Mint LP token to beneficiary or auto deposit into generator if set.
/// # Params
/// * **recipient** is the object of type [`Addr`]. The recipient of the liquidity.
/// * **amount** is the object of type [`Uint128`]. The amount that will be mint to the recipient.
/// * **auto_stake** is the field of type [`bool`]. Determines whether an autostake will be performed on the generator
fn build_mint_lp_token_msg(
    deps: Deps,
    env: Env,
    lp_token: Addr,
    recipient: Addr,
    amount: Uint128,
    generator: Option<Addr>,
    auto_stake: bool,
) -> Result<Vec<CosmosMsg>, ContractError> {
    // If no auto-stake - just mint to recipient
    if !auto_stake {
        return Ok(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: lp_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: recipient.to_string(),
                amount,
            })?,
            funds: vec![],
        })]);
    }

    Ok(vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: lp_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: env.contract.address.to_string(),
                amount,
            })?,
            funds: vec![],
        }),
        // CosmosMsg::Wasm(WasmMsg::Execute {
        //     contract_addr: lp_token.to_string(),
        //     msg: to_binary(&Cw20ExecuteMsg::Send {
        //         contract: generator.unwrap().to_string(),
        //         amount,
        //         msg: to_binary(&GeneratorHookMsg::DepositFor(recipient))?,
        //     })?,
        //     funds: vec![],
        // }),
    ])
}

pub fn build_update_pool_state_msg(
    pool_address: String,
    assets: Vec<Asset>,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: pool_address,
        funds: vec![],
        msg: to_binary(&dexter::pool::ExecuteMsg::UpdateLiquidity { assets })?,
    }))
}

// pub fn query_batch_swap(deps: Deps, swap_kind: SwapType,
// batch_swap_steps: Vec<BatchSwapStep>,
// assets: Vec<Asset>,) -> StdResult<PoolInfo> {
//     let pool_addr = POOLS.load(deps.storage, &pool_key(&asset_infos))?;
//     query_pool_info(deps, &pool_addr)
// }
