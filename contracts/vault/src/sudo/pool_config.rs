
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



/// ## Description - Updates pool configuration. Returns an [`ContractError`] on failure or
/// the following [`PoolConfig`] data will be updated if successful.
///
/// ## Params
/// * **is_disabled** Optional parameter. If set to `true`, the instantiation of new pool instances will be disabled. If set to `false`, they will be enabled.
pub fn sudo_update_pool_type_config(
    deps: DepsMut,
    pool_type: PoolType,
    allow_instantiation: Option<AllowPoolInstantiation>,
    new_fee_info: Option<FeeInfo>,
    paused: Option<PauseInfo>,
) -> Result<Response, ContractError> {
    // permission check - only Owner can update any pool config.
    let config = CONFIG.load(deps.storage)?;

    let mut pool_config = REGISTRY
        .load(deps.storage, pool_type.to_string())
        .map_err(|_| ContractError::PoolTypeConfigNotFound {})?;

    // Emit Event
    let mut event = Event::from_sudo(concatcp!(CONTRACT_NAME, "::update_pool_type_config"))
        .add_attribute("pool_type", pool_type.to_string());

    // Update allow instantiation
    if let Some(allow_instantiation) = allow_instantiation {
        event = event.add_attribute("allow_instantiation", allow_instantiation.to_string());
        pool_config.allow_instantiation = allow_instantiation;
    }

    // Update fee info
    if let Some(new_fee_info) = new_fee_info {
        if !new_fee_info.valid_fee_info() {
            return Err(ContractError::InvalidFeeInfo {});
        }

        pool_config.default_fee_info = new_fee_info;
        event = event.add_attribute(
            "new_fee_info",
            serde_json_wasm::to_string(&pool_config.default_fee_info).unwrap(),
        );
    }

    if let Some(paused) = paused {
        event = event.add_attribute("paused", serde_json_wasm::to_string(&paused).unwrap());
        pool_config.paused = paused;
    }

    // Save pool config
    REGISTRY.save(
        deps.storage,
        pool_config.pool_type.to_string(),
        &pool_config,
    )?;

    Ok(Response::new().add_event(event))
}


pub fn sudo_update_pool_params(
    deps: DepsMut,
    pool_id: Uint128,
    params: Binary,
) -> Result<Response, ContractError> {
    // permission check - only Owner can update any pool config.
    let config = CONFIG.load(deps.storage)?;

    let pool = ACTIVE_POOLS.load(deps.storage, &pool_id.to_string().as_bytes())?;

    // Emit Event
    let event = Event::from_sudo(concatcp!(CONTRACT_NAME, "::update_pool_params"))
        .add_attribute("pool_id", pool_id);

    // create pool update config message and send it to the pool contract
    let msg = WasmMsg::Execute {
        contract_addr: pool.pool_addr.to_string(),
        funds: vec![],
        msg: to_binary(&dexter::pool::ExecuteMsg::UpdateConfig {
            params,
        })?,
    };

    let response = Response::new()
        .add_event(event)
        .add_message(msg);

    Ok(response)
}

pub fn sudo_update_pool_config(
    deps: DepsMut,
    pool_id: Uint128,
    fee_info: Option<FeeInfo>,
    paused: Option<PauseInfo>,
) -> Result<Response, ContractError> {
    // permission check - only Owner can update any pool config.
    let config = CONFIG.load(deps.storage)?;

    let mut pool = ACTIVE_POOLS.load(deps.storage, &pool_id.to_string().as_bytes())?;

    // Emit Event
    let mut event = Event::from_sudo(concatcp!(CONTRACT_NAME, "::update_pool_config"))
        .add_attribute("pool_id", pool_id);

    let mut msgs: Vec<CosmosMsg> = vec![];

    // Update fee info
    if let Some(fee_info) = fee_info {
        if !fee_info.valid_fee_info() {
            return Err(ContractError::InvalidFeeInfo {});
        }

        pool.fee_info = fee_info;
        event = event.add_attribute("fee_info", serde_json_wasm::to_string(&pool.fee_info).unwrap());

        // update total fee in the actual pool contract by sending a wasm message
        msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: pool.pool_addr.to_string(),
            funds: vec![],
            msg: to_binary(&dexter::pool::ExecuteMsg::UpdateFee {
                total_fee_bps: pool.fee_info.total_fee_bps.clone(),
            })?,
        }));
    }

    // update pause status
    if let Some(paused) = paused {
        pool.paused = paused.clone();
        event = event.add_attribute(
            "paused",
            serde_json_wasm::to_string(&pool.paused).unwrap(),
        );
    }

    // Save pool config
    ACTIVE_POOLS.save(deps.storage, pool_id.to_string().as_bytes(), &pool)?;

    let response = Response::new().add_event(event).add_messages(msgs);

    Ok(response)
}

