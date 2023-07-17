use itertools::Itertools;
use crate::contract::CONTRACT_NAME;
use crate::error::ContractError;
use crate::response::MsgInstantiateContractResponse;
use crate::state::{
    ACTIVE_POOLS, CONFIG, LP_TOKEN_TO_POOL_ID, OWNERSHIP_PROPOSAL, REGISTRY, TMP_POOL_INFO,
};
use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut,
    Env, Event, MessageInfo, QueryRequest, Reply, ReplyOn, Response, StdError, StdResult, SubMsg,
    Uint128, WasmMsg, WasmQuery,
};
use protobuf::Message;
use std::collections::HashMap;
use std::collections::HashSet;
use const_format::concatcp;

use dexter::asset::{addr_opt_validate, Asset, AssetInfo};
use dexter::helper::{build_transfer_cw20_from_user_msg, claim_ownership, drop_ownership_proposal, EventExt, find_sent_native_token_balance, get_lp_token_name, get_lp_token_symbol, propose_new_owner};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{FeeStructs, InstantiateMsg as PoolInstantiateMsg};
use dexter::vault::{AllowPoolInstantiation, AssetFeeBreakup, AutoStakeImpl, Config, ConfigResponse, Cw20HookMsg, ExecuteMsg, FeeInfo, InstantiateMsg, MigrateMsg, PauseInfo, PoolTypeConfigResponse, PoolInfo, PoolInfoResponse, PoolType, PoolTypeConfig, QueryMsg, SingleSwapRequest, TmpPoolInfo, PoolCreationFee, PauseInfoUpdateType, ExitType, NativeAssetPrecisionInfo, SudoMsg};



//--------x---------------x--------------x-----
//--------x  Execute :: Config Updates   x-----
//--------x---------------x--------------x-----

/// ## Description - Updates general settings. Returns an [`ContractError`] on failure or the following [`Config`]
/// data will be updated if successful.
///
/// ## Params
/// * **lp_token_code_id** optional parameter. The new id of the LP token code to be used for instantiating new LP tokens along-with the Pools
/// * **fee_collector** optional parameter. The new address of the fee collector to be used for collecting fees from LP tokens
///
/// ##Executor - Only owner can execute it
pub fn sudo_update_config(
    deps: DepsMut,
    lp_token_code_id: Option<u64>,
    fee_collector: Option<String>,
    pool_creation_fee: Option<PoolCreationFee>,
    auto_stake_impl: Option<AutoStakeImpl>,
    paused: Option<PauseInfo>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    let mut event = Event::from_sudo(concatcp!(CONTRACT_NAME, "::update_config"));

    // Update LP token code id
    if let Some(lp_token_code_id) = lp_token_code_id {
        // Check if code id is valid
        if lp_token_code_id == 0 {
            return Err(ContractError::InvalidCodeId {});
        }
        event = event.add_attribute("lp_token_code_id", lp_token_code_id.to_string());
        config.lp_token_code_id = Some(lp_token_code_id);
    }

    // Update fee collector
    if let Some(fee_collector) = fee_collector {
        event = event.add_attribute("fee_collector", fee_collector.clone());
        config.fee_collector = Some(deps.api.addr_validate(fee_collector.as_str())?);
    }

    // Validate the pool creation fee
    if let Some(pool_creation_fee) = pool_creation_fee {
        if let PoolCreationFee::Enabled { fee } = &pool_creation_fee {
            if fee.amount == Uint128::zero() {
                return Err(ContractError::InvalidPoolCreationFee);
            }
        }
        config.pool_creation_fee = pool_creation_fee;
        event = event.add_attribute("pool_creation_fee", serde_json_wasm::to_string(&config.pool_creation_fee).unwrap());
    }

    // set auto stake implementation
    if let Some(auto_stake_impl) = auto_stake_impl {
        if let AutoStakeImpl::Multistaking { contract_addr } = &auto_stake_impl {
            deps.api.addr_validate(contract_addr.as_str())?;
        }
        config.auto_stake_impl = auto_stake_impl;
        event = event.add_attribute("auto_stake_impl", serde_json_wasm::to_string(&config.auto_stake_impl).unwrap());
    }

    // update the pause status
    if let Some(paused) = paused {
        event = event.add_attribute("paused", serde_json_wasm::to_string(&paused).unwrap());
        config.paused = paused;
    }

    CONFIG.save(deps.storage, &config)?;
    
    let response = Response::default().add_event(event);
    Ok(response)
}


/// ## Description - Adds a new pool with a new [`PoolType`] Key. Returns an [`ContractError`] on failure or
/// returns the poolType and the code ID for the pool contract which is used for instantiation.
///
/// ## Params
/// * **new_pool_config** is the object of type [`PoolConfig`]. Contains configuration parameters for the new pool.
///
/// * Executor** Only owner can execute this function
pub fn sudo_add_to_registry(
    deps: DepsMut,
    pool_type_config: PoolTypeConfig,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;


    // Check if code id is valid
    if pool_type_config.code_id == 0 {
        return Err(ContractError::InvalidCodeId {});
    }

    // Check :: If pool type is already registered
    match REGISTRY.load(deps.storage, pool_type_config.pool_type.to_string()) {
        Ok(_) => return Err(ContractError::PoolTypeAlreadyExists {}),
        Err(_) => {}
    }

    // validate fee bps limits
    if !pool_type_config.default_fee_info.valid_fee_info() {
        return Err(ContractError::InvalidFeeInfo {});
    }

    // Save pool config
    REGISTRY.save(
        deps.storage,
        pool_type_config.pool_type.to_string(),
        &pool_type_config,
    )?;

    Ok(Response::new().add_event(
        Event::from_sudo(concatcp!(CONTRACT_NAME, "::add_to_registry"))
            .add_attribute("pool_type_config", serde_json_wasm::to_string(&pool_type_config).unwrap())
    ))
}


pub fn sudo_update_pause_info(
    deps: DepsMut,
    update_type: PauseInfoUpdateType,
    pause_info: PauseInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let event = Event::from_sudo(concatcp!(CONTRACT_NAME, "::update_pause_info"))
        .add_attribute("update_type", serde_json_wasm::to_string(&update_type).unwrap())
        .add_attribute("pause_info", serde_json_wasm::to_string(&pause_info).unwrap());

    match update_type {
        PauseInfoUpdateType::PoolId(pool_id) => {
            let mut pool = ACTIVE_POOLS.
                load(deps.storage, pool_id.to_string().as_bytes())
                .map_err(|_| ContractError::InvalidPoolId {})?;
            pool.paused = pause_info;
            ACTIVE_POOLS.save(deps.storage, pool_id.to_string().as_bytes(), &pool)?;
        }
        PauseInfoUpdateType::PoolType(pool_type) => {
            let mut pool_type_config = REGISTRY
                .load(deps.storage, pool_type.to_string())
                .map_err(|_| ContractError::PoolTypeConfigNotFound {})?;
            pool_type_config.paused = pause_info;
            REGISTRY.save(deps.storage, pool_type.to_string(), &pool_type_config)?;
        }
    }

    Ok(Response::new().add_event(event))
}
