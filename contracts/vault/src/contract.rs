#[cfg(not(feature = "library"))]
use itertools::Itertools;
use crate::error::ContractError;
use crate::response::MsgInstantiateContractResponse;
use crate::state::{
    ACTIVE_POOLS, CONFIG, LP_TOKEN_TO_POOL_ID, OWNERSHIP_PROPOSAL, REGISTRY, TMP_POOL_INFO,
    DEFUNCT_POOLS, REFUNDED_USERS,
};
use cosmwasm_std::{
    entry_point, from_json, to_json_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut,
    Env, Event, MessageInfo, QueryRequest, Reply, ReplyOn, Response, StdError, StdResult, SubMsg,
    Uint128, WasmMsg, WasmQuery, QuerierWrapper,
};
use protobuf::Message;
use std::collections::HashMap;
use std::collections::HashSet;
use const_format::concatcp;

use dexter::asset::{addr_opt_validate, Asset, AssetInfo};
use dexter::helper::{build_transfer_cw20_from_user_msg, claim_ownership, DEFAULT_LIMIT, drop_ownership_proposal, EventExt, find_sent_native_token_balance, get_lp_token_name, get_lp_token_symbol, MAX_LIMIT, propose_new_owner};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{FeeStructs, InstantiateMsg as PoolInstantiateMsg};
use dexter::vault::{AllowPoolInstantiation, AssetFeeBreakup, AutoStakeImpl, Config, ConfigResponse, Cw20HookMsg, ExecuteMsg, FeeInfo, InstantiateMsg, MigrateMsg, PauseInfo, PoolTypeConfigResponse, PoolInfo, PoolInfoResponse, PoolType, PoolTypeConfig, QueryMsg, SingleSwapRequest, TmpPoolInfo, PoolCreationFee, PauseInfoUpdateType, ExitType, NativeAssetPrecisionInfo, DefunctPoolInfo};

use cw2::{get_contract_version, set_contract_version};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};
use dexter::pool;

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter-vault";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CONTRACT_VERSION_V1: &str = "1.0.0";

/// A `reply` call code ID of sub-message.
const INSTANTIATE_LP_REPLY_ID: u64 = 1;
const INSTANTIATE_POOL_REPLY_ID: u64 = 2;

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x      Instantiate Contract : Execute function     x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// ## Description
/// Creates a new contract with the specified parameters in the [`InstantiateMsg`].
/// Returns the [`Response`] with the specified attributes if the operation was successful, or a [`ContractError`] if the contract was not created
///
/// ## Params
/// * **msg** is a message of type [`InstantiateMsg`] which contains the basic settings for creating a contract
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config_set: HashSet<String> = msg
        .pool_configs
        .iter()
        .map(|pc| pc.pool_type.to_string())
        .collect();

    if config_set.len() != msg.pool_configs.len() {
        return Err(ContractError::PoolTypeConfigDuplicate {});
    }
    
    let mut event = Event::from_info(concatcp!(CONTRACT_NAME, "::instantiate"), &info)
        .add_attribute("owner", msg.owner.clone())
        .add_attribute("pool_configs", serde_json_wasm::to_string(&msg.pool_configs).unwrap());

    // Check if code id is valid
    if let Some(code_id) = msg.lp_token_code_id {
        if code_id == 0 {
            return Err(ContractError::InvalidCodeId {});
        }
        event = event.add_attribute("lp_token_code_id", code_id.to_string());
    }

    if let Some(fee_collector) = &msg.fee_collector {
        event = event.add_attribute("fee_collector", fee_collector.clone());
    }

    let pool_creation_fee = &msg.pool_creation_fee;
    if let PoolCreationFee::Enabled { fee } = &pool_creation_fee {
        if fee.amount == Uint128::zero() {
            return Err(ContractError::InvalidPoolCreationFee);
        }
    }
    event = event.add_attribute("pool_creation_fee", serde_json_wasm::to_string(&pool_creation_fee).unwrap());

    if let AutoStakeImpl::Multistaking { contract_addr } = &msg.auto_stake_impl {
        deps.api.addr_validate(contract_addr.as_str())?;
    }
    event = event.add_attribute("auto_stake_impl", serde_json_wasm::to_string(&msg.auto_stake_impl).unwrap());

    let config = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        whitelisted_addresses: vec![],
        lp_token_code_id: msg.lp_token_code_id,
        fee_collector: addr_opt_validate(deps.api, &msg.fee_collector)?,
        auto_stake_impl: msg.auto_stake_impl,
        pool_creation_fee: msg.pool_creation_fee,
        next_pool_id: Uint128::from(1u128),
        paused: PauseInfo::default(),
    };

    // Save Pool Config info
    for pc in msg.pool_configs.iter() {
        // Check if code id is valid
        if pc.code_id == 0 {
            return Err(ContractError::InvalidCodeId {});
        }
        // validate fee bps limits
        if !pc.default_fee_info.valid_fee_info() {
            return Err(ContractError::InvalidFeeInfo {});
        }
        REGISTRY.save(deps.storage, pc.clone().pool_type.to_string(), pc)?;
    }
    

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_event(event))
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
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::UpdateConfig {
            lp_token_code_id,
            fee_collector,
            pool_creation_fee,
            auto_stake_impl,
            paused,
        } => execute_update_config(
            deps,
            info,
            lp_token_code_id,
            fee_collector,
            pool_creation_fee,
            auto_stake_impl,
            paused,
        ),
        ExecuteMsg::UpdatePauseInfo {
            update_type,
            pause_info
        } => execute_update_pause_info(
            deps,
            info,
            update_type,
            pause_info,
        ),
        ExecuteMsg::UpdatePoolTypeConfig {
            pool_type,
            allow_instantiation,
            new_fee_info,
            paused,
        } => execute_update_pool_type_config(
            deps,
            info,
            pool_type,
            allow_instantiation,
            new_fee_info,
            paused,
        ),
        ExecuteMsg::AddAddressToWhitelist { address } => {
            execute_add_address_to_whitelist(deps, info, address)
        }
        ExecuteMsg::RemoveAddressFromWhitelist { address } => {
            execute_remove_address_from_whitelist(deps, info, address)
        }
        ExecuteMsg::AddToRegistry { new_pool_type_config } => {
            execute_add_to_registry(deps, info, new_pool_type_config)
        }
        ExecuteMsg::CreatePoolInstance {
            pool_type,
            asset_infos,
            native_asset_precisions,
            fee_info,
            init_params,
        } => execute_create_pool_instance(
            deps,
            env,
            info,
            pool_type,
            asset_infos,
            native_asset_precisions,
            fee_info,
            init_params,
        ),
        ExecuteMsg::UpdatePoolConfig {
            pool_id,
            fee_info,
            paused,
        } => execute_update_pool_config(deps, info, pool_id, fee_info, paused),
        ExecuteMsg::UpdatePoolParams { pool_id, params } => {
            execute_update_pool_params(deps, info, pool_id, params)
        },
        ExecuteMsg::JoinPool {
            pool_id,
            recipient,
            assets,
            min_lp_to_receive,
            auto_stake,
        } => execute_join_pool(
            deps,
            env,
            info,
            pool_id,
            recipient,
            assets,
            min_lp_to_receive,
            auto_stake,
        ),
        ExecuteMsg::Swap {
            swap_request,
            recipient,
            min_receive,
            max_spend,
        } => execute_swap(
            deps,
            env,
            info,
            swap_request,
            recipient,
            min_receive,
            max_spend,
        ),
        ExecuteMsg::ProposeNewOwner { new_owner, expires_in } => {
            let config: Config = CONFIG.load(deps.storage)?;
            propose_new_owner(
                deps,
                info,
                env,
                new_owner,
                expires_in,
                config.owner,
                OWNERSHIP_PROPOSAL,
                CONTRACT_NAME
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
        ExecuteMsg::DefunctPool { pool_id } => {
            execute_defunct_pool(deps, env, info, pool_id)
        }
        ExecuteMsg::ProcessRefundBatch { pool_id, user_addresses } => {
            execute_process_refund_batch(deps, env, info, pool_id, user_addresses)
        }
    }
}

/// ## Description
/// Receives a message of type [`Cw20ReceiveMsg`] and processes it depending on the received template.
/// If the template is not found in the received message, then an [`ContractError`] is returned,
/// otherwise returns the [`Response`] with the specified attributes if the operation was successful
/// ## Params
/// * **deps** is the object of type [`DepsMut`].
/// * **env** is the object of type [`Env`].
/// * **info** is the object of type [`MessageInfo`].
/// * **cw20_msg** is the object of type [`Cw20ReceiveMsg`].
pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let sender = cw20_msg.sender;
    let lp_received = cw20_msg.amount;

    match from_json(&cw20_msg.msg)? {
        Cw20HookMsg::ExitPool {
            pool_id,
            recipient,
            exit_type
        } => {
            execute_exit_pool(
                deps,
                env,
                info,
                pool_id,
                recipient.unwrap_or(sender.clone()),
                exit_type,
                sender,
                lp_received,
            )
        }
    }
}

// ----------------x----------------x----------------x-----------------------x----------------x----------------
// ----------------x----------------x  Execute :: Functional implementation  x----------------x----------------
// ----------------x----------------x----------------x-----------------------x----------------x----------------

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
pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    lp_token_code_id: Option<u64>,
    fee_collector: Option<String>,
    pool_creation_fee: Option<PoolCreationFee>,
    auto_stake_impl: Option<AutoStakeImpl>,
    paused: Option<PauseInfo>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    let mut event = Event::from_info(concatcp!(CONTRACT_NAME, "::update_config"), &info);

    // permission check
    if info.sender.clone() != config.owner {
        return Err(ContractError::Unauthorized {});
    }

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

pub fn execute_update_pause_info(
    deps: DepsMut,
    info: MessageInfo,
    update_type: PauseInfoUpdateType,
    pause_info: PauseInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // either vault owner or whitelisted address can update the pause info
    if info.sender != config.owner && !config.whitelisted_addresses.contains(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }

    let event = Event::from_info(concatcp!(CONTRACT_NAME, "::update_pause_info"), &info)
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

/// ## Description - Updates pool configuration. Returns an [`ContractError`] on failure or
/// the following [`PoolConfig`] data will be updated if successful.
///
/// ## Params
/// * **is_disabled** Optional parameter. If set to `true`, the instantiation of new pool instances will be disabled. If set to `false`, they will be enabled.
///
/// ## Executor
/// Only owner can execute it
pub fn execute_update_pool_type_config(
    deps: DepsMut,
    info: MessageInfo,
    pool_type: PoolType,
    allow_instantiation: Option<AllowPoolInstantiation>,
    new_fee_info: Option<FeeInfo>,
    paused: Option<PauseInfo>,
) -> Result<Response, ContractError> {
    // permission check - only Owner can update any pool config.
    let config = CONFIG.load(deps.storage)?;
    if info.sender.clone() != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut pool_config = REGISTRY
        .load(deps.storage, pool_type.to_string())
        .map_err(|_| ContractError::PoolTypeConfigNotFound {})?;

    // Emit Event
    let mut event = Event::from_info(concatcp!(CONTRACT_NAME, "::update_pool_type_config"), &info)
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

fn execute_update_pool_config(
    deps: DepsMut,
    info: MessageInfo,
    pool_id: Uint128,
    fee_info: Option<FeeInfo>,
    paused: Option<PauseInfo>,
) -> Result<Response, ContractError> {
    // permission check - only Owner can update any pool config.
    let config = CONFIG.load(deps.storage)?;
    if info.sender.clone() != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut pool = ACTIVE_POOLS.load(deps.storage, &pool_id.to_string().as_bytes())?;

    // Emit Event
    let mut event = Event::from_info(concatcp!(CONTRACT_NAME, "::update_pool_config"), &info)
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
            msg: to_json_binary(&dexter::pool::ExecuteMsg::UpdateFee {
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


fn execute_update_pool_params(
    deps: DepsMut,
    info: MessageInfo,
    pool_id: Uint128,
    params: Binary,
) -> Result<Response, ContractError> {
    // permission check - only Owner can update any pool config.
    let config = CONFIG.load(deps.storage)?;
    if info.sender.clone() != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let pool = ACTIVE_POOLS.load(deps.storage, &pool_id.to_string().as_bytes())?;

    // Emit Event
    let event = Event::from_info(concatcp!(CONTRACT_NAME, "::update_pool_params"), &info)
        .add_attribute("pool_id", pool_id);

    // create pool update config message and send it to the pool contract
    let msg = WasmMsg::Execute {
        contract_addr: pool.pool_addr.to_string(),
        funds: vec![],
        msg: to_json_binary(&dexter::pool::ExecuteMsg::UpdateConfig {
            params,
        })?,
    };

    let response = Response::new()
        .add_event(event)
        .add_message(msg);

    Ok(response)
}


fn execute_add_address_to_whitelist(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender.clone() != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Validate address
    let address = deps.api.addr_validate(address.as_str())?;

    // check if address to be added is the owner
    if address == config.owner {
        return Err(ContractError::CannotAddOwnerToWhitelist);
    }

    // check if address is already whitelisted
    if config.whitelisted_addresses.contains(&address) {
        return Err(ContractError::AddressAlreadyWhitelisted);
    }

    // Add address to whitelist
    config.whitelisted_addresses.push(address.clone());

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_event(
        Event::from_info(concatcp!(CONTRACT_NAME, "::add_address_to_whitelist"), &info)
            .add_attribute("address", address.to_string())
    ))
}

fn execute_remove_address_from_whitelist(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender.clone() != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Validate address
    let address = deps.api.addr_validate(address.as_str())?;

    // check if address is already whitelisted
    if !config.whitelisted_addresses.contains(&address) {
        return Err(ContractError::AddressNotWhitelisted);
    }

    // Remove address from whitelist
    config.whitelisted_addresses.retain(|x| x != &address);

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_event(
        Event::from_info(concatcp!(CONTRACT_NAME, "::remove_address_from_whitelist"), &info)
            .add_attribute("address", address.to_string())
    ))
}

//--------x---------------x--------------x-----
//--------x  Execute :: Create Pool      x-----
//--------x---------------x--------------x-----

/// ## Description - Adds a new pool with a new [`PoolType`] Key. Returns an [`ContractError`] on failure or
/// returns the poolType and the code ID for the pool contract which is used for instantiation.
///
/// ## Params
/// * **new_pool_config** is the object of type [`PoolConfig`]. Contains configuration parameters for the new pool.
///
/// * Executor** Only owner can execute this function
pub fn execute_add_to_registry(
    deps: DepsMut,
    info: MessageInfo,
    pool_type_config: PoolTypeConfig,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    // permission check : Only owner can execute it
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

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
        Event::from_info(concatcp!(CONTRACT_NAME, "::add_to_registry"), &info)
            .add_attribute("pool_type_config", serde_json_wasm::to_string(&pool_type_config).unwrap())
    ))
}

/// ## Description - Creates a new pool with the specified parameters in the `asset_infos` variable. Returns an [`ContractError`] on failure or
/// returns the address of the contract if the creation was successful.
///
/// ## Params
/// * **pool_type** is the object of type [`PoolType`].
/// * **asset_infos** is a vector consisting of type [`AssetInfo`].
/// * **lp_token_name** is the name of the LP token to be used for instantiating new LP tokens along-with the Pools.
/// * **lp_token_symbol** is the symbol of the LP token to be used for instantiating new LP tokens along-with the Pools.
/// * **init_params** is the object of type [`Binary`] which contains any custom params required by the Pool instance for its initialization.
pub fn execute_create_pool_instance(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_type: PoolType,
    mut asset_infos: Vec<AssetInfo>,
    native_asset_precisions: Vec<NativeAssetPrecisionInfo>,
    fee_info: Option<FeeInfo>,
    init_params: Option<Binary>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Get current pool's config from stored pool configs
    let pool_type_config = REGISTRY
        .load(deps.storage, pool_type.to_string())
        .map_err(|_| ContractError::PoolTypeConfigNotFound {})?;

    // Check if creation is allowed
    match pool_type_config.allow_instantiation {
        AllowPoolInstantiation::OnlyWhitelistedAddresses => {
            // Check if sender is whitelisted
            if info.sender != config.owner && !config.whitelisted_addresses.contains(&info.sender) {
                return Err(ContractError::Unauthorized {});
            }
        }
        AllowPoolInstantiation::Nobody => {
            return Err(ContractError::PoolTypeCreationDisabled);
        }
        AllowPoolInstantiation::Everyone => {}
    }

    // Validate if the native asset precision has been provided for all native assets
    let native_asset_denoms = asset_infos.iter().filter_map(|a| match a {
        AssetInfo::NativeToken { denom } => Some(denom),
        _ => None,
    }).sorted().collect_vec();
    
    let denoms_of_precisions_supplied = native_asset_precisions.iter().map(|k| &k.denom).sorted().collect_vec();

    if native_asset_denoms != denoms_of_precisions_supplied {
        return Err(ContractError::InvalidNativeAssetPrecisionList);
    }
    
    // We only support precisions upto 18 decimal places, reject if any asset has precision greater than 18
    if native_asset_precisions.iter().any(|p| p.precision > 18) {
        return Err(ContractError::UnsupportedPrecision);
    }

    let mut execute_msgs = vec![];

    // Validate if fee is sent for creation of pool
    if let PoolCreationFee::Enabled { fee } = config.pool_creation_fee {
        let fee_amount = fee.amount;
        match fee.info.clone() {
            AssetInfo::NativeToken { denom } => {
                // Check if sender has sent enough funds to pay for the pool creation fee
                let tokens_received = find_sent_native_token_balance(&info, &denom);

                if tokens_received < fee_amount {
                    return Err(ContractError::InsufficientNativeTokensSent {
                        denom,
                        sent: tokens_received,
                        needed: fee_amount,
                    });
                } else if tokens_received > fee_amount {
                    // refund the extra tokens
                    if tokens_received > fee_amount {
                        let transfer_msg = fee.info.clone().create_transfer_msg(
                            info.sender.clone(),
                            tokens_received.checked_sub(fee_amount)?,
                        )?;
                        execute_msgs.push(transfer_msg);
                    }
                }
            }
            AssetInfo::Token { contract_addr } => {
                if !fee_amount.is_zero() {
                    execute_msgs.push(build_transfer_cw20_from_user_msg(
                        contract_addr.to_string(),
                        info.sender.clone().to_string(),
                        env.contract.address.to_string(),
                        fee_amount,
                    )?);
                }
            }
        }

        let fee_collector = config
            .fee_collector
            .ok_or(ContractError::FeeCollectorNotSet)?;
        
        // Withdraw the pool creation fee to the fee collector address
        let withdraw_msg = fee.info.clone().create_transfer_msg(
            fee_collector,
            fee_amount,
        )?;

        execute_msgs.push(withdraw_msg);
    }

    // Sort Assets List
    asset_infos.sort();

    let mut assets: Vec<Asset> = vec![];

    // Check asset definitions and make sure no asset is repeated
    let mut previous_asset: String = "".to_string();
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

    // Pool Id for the new pool instance
    let pool_id = config.next_pool_id;

    let fee_info = fee_info.unwrap_or(pool_type_config.default_fee_info);
    // validate fee bps limits
    if !fee_info.valid_fee_info() {
        return Err(ContractError::InvalidFeeInfo {});
    }
    let tmp_pool_info = TmpPoolInfo {
        code_id: pool_type_config.code_id,
        pool_id,
        lp_token_addr: None,
        fee_info: fee_info.clone(),
        assets,
        pool_type: pool_type_config.pool_type.clone(),
        init_params: init_params.clone(),
        native_asset_precisions: native_asset_precisions.clone(),
    };

    // Store the temporary Pool Info
    TMP_POOL_INFO.save(deps.storage, &tmp_pool_info)?;

    // LP Token Name
    let token_name = get_lp_token_name(pool_id.clone());
    // LP Token Symbol
    let token_symbol = get_lp_token_symbol();

    // Emit Event
    let event = Event::from_info(concatcp!(CONTRACT_NAME, "::create_pool_instance"), &info)
        .add_attribute("pool_type", pool_type.to_string())
        .add_attribute("asset_infos", serde_json_wasm::to_string(&asset_infos).unwrap())
        .add_attribute("native_asset_precisions", serde_json_wasm::to_string(&native_asset_precisions).unwrap())
        .add_attribute("fee_info", serde_json_wasm::to_string(&fee_info).unwrap())
        .add_attribute("init_params", serde_json_wasm::to_string(&init_params).unwrap())
        .add_attribute("code_id", pool_type_config.code_id.to_string())// useful to know the code_id with which the pool was created
        .add_attribute("pool_id", pool_id.to_string())
        .add_attribute("lp_token_name", token_name.clone())
        .add_attribute("lp_token_symbol", token_symbol.clone());

    // Sub Msg to initialize the LP token instance
    let init_lp_token_sub_msg: SubMsg = SubMsg {
        id: INSTANTIATE_LP_REPLY_ID,
        msg: WasmMsg::Instantiate {
            admin: None,
            code_id: config
                .lp_token_code_id
                .ok_or(ContractError::LpTokenCodeIdNotSet)?,
            msg: to_json_binary(&TokenInstantiateMsg {
                name: token_name,
                symbol: token_symbol,
                decimals: Decimal::DECIMAL_PLACES as u8,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: env.contract.address.clone().to_string(),
                    cap: None,
                }),
                marketing: None,
            })?,
            funds: vec![],

            label: String::from("Dexter LP token"),
        }
        .into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    };

    Ok(Response::new()
        .add_submessages([init_lp_token_sub_msg])
        .add_messages(execute_msgs)
        .add_event(event))
}

/// # Description
/// The entry point to the contract for processing the reply from the submessage
/// # Params
/// * **msg** is the object of type [`Reply`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    // Load stored temporary pool info
    let mut tmp_pool_info = TMP_POOL_INFO.load(deps.storage)?;
    let event;

    // Parse the reply from the submessage
    let data = msg.result.unwrap().data.unwrap();
    let res: MsgInstantiateContractResponse =
        Message::parse_from_bytes(data.as_slice()).map_err(|_| {
            StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
        })?;

    let mut response = Response::new();

    match msg.id {
        // Reply from the submessage to instantiate the LP token instance
        INSTANTIATE_LP_REPLY_ID => {
            // Update the LP token address in the temporary pool info
            let lp_token_addr = deps.api.addr_validate(res.get_contract_address())?;

            event = Event::new(concatcp!(CONTRACT_NAME, "::reply::lp_token_init"))
                .add_attribute("pool_id", tmp_pool_info.pool_id.to_string())
                .add_attribute("lp_token_addr", lp_token_addr.to_string());
            
                tmp_pool_info.lp_token_addr = Some(lp_token_addr.clone());
            // Store the temporary Pool Info
            TMP_POOL_INFO.save(deps.storage, &tmp_pool_info)?;

            // Store LP token addr _> Pool Id mapping in the LP token map
            LP_TOKEN_TO_POOL_ID.save(
                deps.storage,
                &lp_token_addr.clone().as_bytes(),
                &tmp_pool_info.pool_id.clone(),
            )?;

            // Sub Msg to initialize the pool instance
            let init_pool_sub_msg: SubMsg = SubMsg {
                id: INSTANTIATE_POOL_REPLY_ID,
                msg: WasmMsg::Instantiate {
                    admin: Some(CONFIG.load(deps.storage)?.owner.to_string()),
                    code_id: tmp_pool_info.code_id,
                    msg: to_json_binary(&PoolInstantiateMsg {
                        pool_id: tmp_pool_info.pool_id,
                        pool_type: tmp_pool_info.pool_type,
                        vault_addr: env.contract.address,
                        lp_token_addr,
                        asset_infos: tmp_pool_info
                            .assets
                            .iter()
                            .map(|a| a.info.clone())
                            .collect(),
                        fee_info: FeeStructs {
                            total_fee_bps: tmp_pool_info.fee_info.total_fee_bps,
                        },
                        init_params: tmp_pool_info.init_params,
                        native_asset_precisions: tmp_pool_info.native_asset_precisions
                    })?,
                    funds: vec![],
                    label: "dexter-pool-".to_string() + &tmp_pool_info.pool_id.to_string(),
                }
                .into(),
                gas_limit: None,
                reply_on: ReplyOn::Success,
            };
            response = response.add_submessage(init_pool_sub_msg);
        }
        // Reply from the submessage to instantiate the pool instance
        INSTANTIATE_POOL_REPLY_ID => {
            let pool_addr = deps.api.addr_validate(res.get_contract_address())?;

            event = Event::new(concatcp!(CONTRACT_NAME, "::reply::pool_init"))
                .add_attribute("pool_id", tmp_pool_info.pool_id.to_string())
                .add_attribute("pool_addr", pool_addr.to_string());

            // Save the temporary pool info as permanent pool info mapped with the Pool Id
            ACTIVE_POOLS.save(
                deps.storage,
                &tmp_pool_info.pool_id.to_string().as_bytes(),
                &PoolInfo {
                    pool_id: tmp_pool_info.pool_id,
                    pool_addr: pool_addr.clone(),
                    lp_token_addr: tmp_pool_info.lp_token_addr.unwrap(),
                    fee_info: tmp_pool_info.fee_info,
                    assets: tmp_pool_info.assets,
                    pool_type: tmp_pool_info.pool_type,
                    paused: PauseInfo::default(),
                },
            )?;

            // Update the next pool id in the config and save it
            let mut config = CONFIG.load(deps.storage)?;
            config.next_pool_id = config.next_pool_id.checked_add(Uint128::from(1u128))?;
            CONFIG.save(deps.storage, &config)?;
        }
        _ => {
            return Err(ContractError::InvalidSubMsgId {});
        }
    }

    Ok(response.add_event(event))
}

//--------x---------------x--------------x-----x-----
//--------x    Execute :: Join / Exit Pool     x-----
//--------x---------------x--------------x-----x-----

/// ## Description - Entry point for a user to Join a pool supported by the Vault. User can join by providing the pool id and either the number of assets to be provided or the LP tokens to be minted to the user.
/// The  number of assets or LP tokens to be minted to the user is decided by the pool contract 's math computations. Vault contract
/// is responsible for the the transfer of assets and minting of LP tokens to the user.
///
/// ## Params
/// * **pool_id** is the id of the pool to be joined.
/// * **op_recipient** Optional parameter. If provided, the Vault will transfer the LP tokens to the provided address.
/// * **assets_in** Optional parameter. It is the list of assets the user is willing to provide to join the pool
/// * **min_lp_to_receive** Optional parameter. The minimum number of LP tokens the user wants to receive against the provided assets.
/// * **auto_stake** Optional parameter. If provided, the Vault will automatically stake the provided assets with the multistaking contract.
pub fn execute_join_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_id: Uint128,
    recipient: Option<String>,
    assets: Option<Vec<Asset>>,
    min_lp_to_receive: Option<Uint128>,
    auto_stake: Option<bool>,
) -> Result<Response, ContractError> {
    // Read - Vault Config
    let config = CONFIG.load(deps.storage)?;

    // Read -  Get PoolInfo {} for the pool to which liquidity is to be provided
    // This also validates the pool exists and is not defunct
    let mut pool_info = validate_pool_exists_and_not_defunct(deps.storage, pool_id)?;

    // Read -  Get PoolConfig {} for the pool
    let pool_config = REGISTRY.load(deps.storage, pool_info.pool_type.to_string())?;

    if config.paused.deposit || pool_config.paused.deposit || pool_info.paused.deposit {
        return Err(ContractError::PausedDeposit {});
    }

    // Check if auto-staking (if requested), is enabled (or possible) right now
    if auto_stake.unwrap_or(false) {
        if let AutoStakeImpl::None = &config.auto_stake_impl {
            return Err(ContractError::AutoStakeDisabled);    
        }
    }

    // Query - Query the Pool Contract to get the state transition to be handled
    // AfterJoinResponse {} is the response from the pool contract and it contains the state transition to be handled by the Vault.
    // The state transition is described via the response params as following -
    // provided_assets - Sorted list of assets to be transferred from the user to the Vault as Pool Liquidity
    // new_shares - The number of LP tokens to be minted to the user / recipient
    // response - The response type :: Success or Failure
    // fee - Optional List assets (info and amounts) to be charged as fees to the user. If it is null then no fee is charged
    //       - We calculate the protocol_fee and transfer it to keeper.
    //       - When updating pool liquidity, we subtract the protocol_fee from the provided_assets.
    let pool_join_transition: dexter::pool::AfterJoinResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: pool_info.pool_addr.to_string(),
            msg: to_json_binary(&dexter::pool::QueryMsg::OnJoinPool {
                assets_in: assets,
                mint_amount: None,
            })?,
        }))?;

    // Error - If the response is failure or LP tokens to mint = 0
    if !pool_join_transition.response.is_success() {
        return Err(ContractError::PoolQueryFailed {
            error: pool_join_transition.response.to_string(),
        });
    } else if pool_join_transition.new_shares.is_zero() {
        return Err(ContractError::PoolQueryFailed {
            error: "LP tokens to mint cannot be 0".to_string(),
        });
    } else if min_lp_to_receive.is_some() && pool_join_transition.new_shares.lt(&min_lp_to_receive.unwrap()) {
        return Err(ContractError::MinReceiveError{
            min_receive: min_lp_to_receive.unwrap(),
            ask_amount: pool_join_transition.new_shares,
        });
    }

    // Error - Number of Assets should match
    if pool_join_transition.provided_assets.len() != pool_info.assets.len() {
        return Err(ContractError::InvalidNumberOfAssets {});
    }

    // Response -Number of LP tokens to be minted
    let new_shares = pool_join_transition.new_shares;

    // ExecuteMsg - Stores the list of messages to be executed
    let mut execute_msgs: Vec<CosmosMsg> = vec![];

    // HashMap - Map the fees to be charged for each token: token_identifier --> amount
    let fee_collection: HashMap<AssetInfo, Uint128> = pool_join_transition
        .fee
        .unwrap_or(vec![])
        .into_iter()
        .map(|asset| (asset.info, asset.amount))
        .collect();

    let mut charged_fee_breakup: Vec<AssetFeeBreakup> = vec![];

    // Update Loop - We loop through all the assets supported by the pool and do the following,
    //              1. Calculate Fee to be charged for the asset, and net liquidity to be updated for the asset
    //              2. Update the PoolInfo {} with the new liquidity
    //              3. Create CosmosMsg to - transfer tokens to the Vault, transfer fees to the keeper
    let mut index = 0;
    for stored_asset in pool_info.assets.iter_mut() {
        // Error - The returned list of assets needs to be sorted in the same order as the stored list of assets
        if stored_asset.info.clone() != pool_join_transition.provided_assets[index].info {
            return Err(ContractError::InvalidSequenceOfAssets {});
        }
        // Param - Number of tokens for this asset to be transferred to the Vault
        let transfer_in = pool_join_transition.provided_assets[index].amount;

        // Param - Fee to be charged in these tokens
        let total_fee = fee_collection
            .get(&stored_asset.info.clone())
            .copied()
            .unwrap_or(Uint128::zero());

        // Param - protocol and lp fee
        let mut protocol_fee: Uint128 = Uint128::zero();

        // Compute - calculate protocol fee based on % of total fee
        if !total_fee.clone().is_zero() {
            protocol_fee = pool_info
                .fee_info
                .calculate_total_fee_breakup(total_fee.clone());
        }

        // Compute - Update fee if recipient addresses are not set
        if !config.fee_collector.is_some() {
            protocol_fee = Uint128::zero();
        }

        // If number of tokens to transfer > 0, then
        // - Update stored pool's asset balances in `PoolInfo` Struct
        // - Transfer net calculated CW20 tokens from user to the Vault
        // - Return native tokens to the user (which are to be returned)
        if !transfer_in.is_zero() || !total_fee.is_zero() {
            // Update - Update Pool Liquidity
            // - Liquidity Provided = transfer_in - protocol_fee
            // here,
            // transfer_in: tokens to be transferred from user to the Vault
            // protocol_fee: protocol fee to be charged and transfer to the fee_collector
            // Note: LP fees = total_fee - protocol_fee, pools need to charge fee in-terms of LP tokens (mint less number of LP tokens)
            //                 so inherently users are minted LP tokens equivalent to : (transfer_in - total_fee) while the actual liquidity
            //                provided is (transfer_in - total_fee + lp_fee), where lp_fee = total_fee - protocol_fee
            // Compute - Add all tokens to be transferred to the Vault
            stored_asset.amount = stored_asset.amount.checked_add(transfer_in)?;
            // Compute - Subtract the protocol fee from the stored asset amount
            stored_asset.amount = stored_asset.amount.checked_sub(protocol_fee)?;

            // Indexing - Add fee to vec to push to event later
            charged_fee_breakup.push(AssetFeeBreakup {
                asset_info: stored_asset.info.clone(),
                total_fee,
                protocol_fee,
            });

            // ExecuteMsg -::- Transfer tokens from user to the Vault
            // If token is native, then,
            // - Return extra sent native tokens to the user (if any)
            // - Error : If  not enough native tokens are sent
            if !stored_asset.info.is_native_token() {
                if !transfer_in.is_zero() {
                    execute_msgs.push(build_transfer_cw20_from_user_msg(
                        stored_asset.info.as_string(),
                        info.sender.clone().to_string(),
                        env.contract.address.to_string(),
                        transfer_in,
                    )?);
                }
            } else {
                // Get number of native tokens that were sent alongwith the tx
                let tokens_received =
                    find_sent_native_token_balance(&info, &stored_asset.info.as_string());

                // ExecuteMsg -::- Return the extra native tokens sent by the user to the Vault
                if tokens_received > transfer_in {
                    execute_msgs.push(stored_asset.info.clone().create_transfer_msg(
                        info.sender.clone(),
                        tokens_received.checked_sub(transfer_in)?,
                    )?);
                }
                // Error - If the number of tokens transferred are less than the number of tokens required
                else if tokens_received < transfer_in {
                    return Err(ContractError::InsufficientNativeTokensSent {
                        denom: stored_asset.info.to_string(),
                        sent: tokens_received,
                        needed: transfer_in,
                    });
                }
            }
        }

        // ExecuteMsg -::- To transfer the protocol fee to the fee collector
        if !protocol_fee.is_zero() {
            execute_msgs.push(
                stored_asset
                    .info
                    .clone()
                    .create_transfer_msg(config.fee_collector.clone().unwrap(), protocol_fee)?,
            );
        }

        // Increment Index
        index = index + 1;
    }

    // Param - LP Token recipient / beneficiary if auto_stake = false
    let recipient = deps
        .api
        .addr_validate(recipient.unwrap_or(info.sender.to_string()).as_str())?;

    // ExecuteMsg:: Updated Pool's stored liquidity state
    execute_msgs.push(build_update_pool_state_msg(
        pool_info.pool_addr.to_string(),
        pool_info.assets.clone(),
    )?);

    // ExecuteMsg:: Mint LP Tokens
    let mint_msgs = build_mint_lp_token_msg(
        deps.as_ref(),
        env.clone(),
        pool_info.lp_token_addr.clone(),
        recipient.clone(),
        new_shares,
        auto_stake.unwrap_or(false),
        config.auto_stake_impl.clone()
    )?;
    for msg in mint_msgs {
        execute_msgs.push(msg);
    }

    // WRITE - Store the Updated PoolInfo state to the storage
    ACTIVE_POOLS.save(deps.storage, &pool_id.to_string().as_bytes(), &pool_info)?;

    // Response - Emit Event
    Ok(Response::new().add_messages(execute_msgs).add_event(
        Event::from_info(concatcp!(CONTRACT_NAME, "::join_pool"), &info)
            .add_attribute("pool_id", pool_id.to_string())
            .add_attribute("recipient", recipient.to_string())
            .add_attribute("assets", serde_json_wasm::to_string(&pool_join_transition.provided_assets).unwrap())
            .add_attribute("min_lp_to_receive", min_lp_to_receive.unwrap_or(Uint128::zero()).to_string())
            .add_attribute("auto_stake", auto_stake.unwrap_or(false).to_string())
            .add_attribute("lp_tokens_minted", new_shares.to_string())
            .add_attribute("fees", serde_json_wasm::to_string(&charged_fee_breakup).unwrap())
            .add_attribute("pool_addr", pool_info.pool_addr.to_string()) // TODO: do we really need this here?
    ))
}

/// ## Description - Entry point for a user to Exit a pool supported by the Vault. User can exit by providing the pool id and either the number of assets to be returned or the LP tokens to be burnt.
/// The  number of assets to be returned or LP tokens to be burnt are decided by the pool contract 's math computations. Vault contract
/// is responsible for the the transfer of assets and burning of LP tokens only
///
/// ## Params
/// * **pool_id** is the id of the pool to be joined.
/// * **op_recipient** Optional parameter. If provided, the Vault will transfer the assets to the provided address.
/// * **assets_out** Optional parameter. It is the list of assets the user wants to get back when exiting the pool
/// * **burn_amount** Optional parameter. The number of LP tokens the user wants to burn for the underlying assets.
pub fn execute_exit_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_id: Uint128,
    recipient: String,
    exit_type: ExitType,
    sender: String,
    lp_received: Uint128,
) -> Result<Response, ContractError> {
    // Read - Vault config
    let config = CONFIG.load(deps.storage)?;

    //  Read -  Get PoolInfo {} for the pool to which liquidity is to be provided
    // This also validates the pool exists and is not defunct
    let mut pool_info = validate_pool_exists_and_not_defunct(deps.storage, pool_id)?;

    // Read - Get the PoolConfig {} for the pool
    let pool_config = REGISTRY.load(deps.storage, pool_info.pool_type.to_string())?;

    // Error - Check if the LP token sent is valid
    if info.sender != pool_info.lp_token_addr {
        return Err(ContractError::Unauthorized {});
    }

    let query_exit_type: pool::ExitType;
    let mut min_assets_out_map: HashMap<String, Uint128> = HashMap::new();

    // Check if exit_type is valid or not
    match exit_type.clone() {
        ExitType::ExactLpBurn { lp_to_burn, min_assets_out } => {
            // ensure we have received exact lp tokens as the user wants to burn
            if lp_to_burn != lp_received {
                return Err(ContractError::ReceivedUnexpectedLpTokens {
                    expected: lp_to_burn,
                    received: lp_received,
                });
            }
            // more validation on lp_to_burn should happen in each pool's query

            // Check - user should specify all the pool assets in min_assets_out if specifying at all
            if let Some(min_assets_out) = min_assets_out {
                min_assets_out.into_iter().for_each(|a| {
                   min_assets_out_map.insert(a.info.to_string(), a.amount);
                });

                for a in pool_info.assets.clone() {
                    if min_assets_out_map.get(a.info.to_string().as_str()).is_none() {
                        return  Err(ContractError::MismatchedAssets {});
                    }
                }
            }

            query_exit_type = pool::ExitType::ExactLpBurn(lp_to_burn);
        }
        ExitType::ExactAssetsOut { max_lp_to_burn, assets_out } => {

            // Validate if exit is paused for imbalanced withdrawals
            if config.paused.imbalanced_withdraw || pool_config.paused.imbalanced_withdraw || pool_info.paused.imbalanced_withdraw {
                return Err(ContractError::ImbalancedExitPaused);
            }

            // validate assets_out => this should happen in each pool's exit query

            // ensure we have received at least as much lp tokens as the maximum user wants to burn
            if max_lp_to_burn.is_some() && max_lp_to_burn.unwrap() > lp_received {
                return Err(ContractError::ReceivedUnexpectedLpTokens {
                    expected: max_lp_to_burn.unwrap(),
                    received: lp_received
                });
            }

            query_exit_type = pool::ExitType::ExactAssetsOut(assets_out);
        }
    }

    //  Query - Query the Pool Contract to get the state transition to be handled
    // AfterExitResponse {} is the response from the pool contract and it contains the state transition to be handled by the Vault.
    // The state transition is described via the response params as following -
    // assets_out - Sorted list of assets to be transferred to the user from the Vault
    // new_shares - The number of LP tokens to be burnt
    // response - The response type :: Success or Failure
    // fee - Optional List assets (info and amounts) to be charged as fees. If it is null then no fee is charged
    //       - We calculate the protocol_fee and transfer it to keeper.
    //       - When updating pool liquidity, we add the protocol_fee to the assets_out.
    let pool_exit_transition: dexter::pool::AfterExitResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: pool_info.pool_addr.to_string(),
            msg: to_json_binary(&dexter::pool::QueryMsg::OnExitPool {
               exit_type: query_exit_type
            })?,
        }))?;

    // Error - If the response is failure
    if !pool_exit_transition.response.is_success() {
        return Err(ContractError::PoolQueryFailed {
            error: pool_exit_transition.response.to_string(),
        });
    }

    // Error - Number of Assets should match
    if pool_exit_transition.assets_out.len() != pool_info.assets.len() {
        return Err(ContractError::InvalidNumberOfAssets {});
    }

    // Check - Burn amount cannot be 0
    if pool_exit_transition.burn_shares.is_zero() {
        return Err(ContractError::BurnAmountZero {});
    }

    // Check - ExitType validations
    match exit_type {
        ExitType::ExactLpBurn { lp_to_burn, min_assets_out } => {
            if pool_exit_transition.burn_shares != lp_to_burn {
                return Err(ContractError::PoolExitTransitionLpToBurnMismatch {
                    expected_to_burn: lp_to_burn,
                    actual_burn: pool_exit_transition.burn_shares,
                });
            }
            if let Some(_) = min_assets_out {
                for a in pool_exit_transition.assets_out.clone() {
                    let min_amount = min_assets_out_map.get(a.info.to_string().as_str()).unwrap();
                    if a.amount.lt(min_amount) {
                        return Err(ContractError::MinAssetOutError {
                            return_amount: a.amount,
                            min_receive: *min_amount,
                            asset_info: a.info,
                        });
                    }
                }
            }
        }
        ExitType::ExactAssetsOut { assets_out, max_lp_to_burn } => {
            let assets_out_map: HashMap<String, Uint128> = assets_out
                .iter()
                .filter(|a| a.amount.gt(&Uint128::zero()))
                .map(|a| (a.info.to_string(), a.amount))
                .collect();
            for a in pool_exit_transition.assets_out.clone() {
                let asset_out_amount = assets_out_map
                    .get(a.info.to_string().as_str())
                    .cloned()
                    .unwrap_or(Uint128::zero());
                if a.amount != asset_out_amount {
                    return Err(ContractError::PoolExitTransitionAssetsOutMismatch {
                        expected_assets_out: serde_json_wasm::to_string(&assets_out).unwrap(),
                        actual_assets_out: serde_json_wasm::to_string(&pool_exit_transition.assets_out).unwrap(),
                    });
                }
            }
            if let Some(max_lp_to_burn) = max_lp_to_burn {
                if pool_exit_transition.burn_shares > max_lp_to_burn {
                    return Err(ContractError::MaxLpToBurnError {
                        burn_amount: pool_exit_transition.burn_shares,
                        max_lp_to_burn,
                    })
                }
            }
        }
    }

    // Param - Number of LP shares to be returned to the user
    let lp_to_return: Uint128 = lp_received
        .checked_sub(pool_exit_transition.burn_shares)
        .map_err(|_| {
            return ContractError::InsufficientLpTokensToExit {};
        })?;

    //  ExecuteMsg - Stores the list of messages to be executed
    let mut execute_msgs: Vec<CosmosMsg> = vec![];

    // Response - Emit Event

    // HashMap - Map the fees to be charged for each token: token_identifier --> amount
    let mut fee_collection: HashMap<AssetInfo, Uint128> = HashMap::new();
    if pool_exit_transition.fee.is_some() {
        fee_collection = pool_exit_transition
            .fee
            .clone()
            .unwrap()
            .into_iter()
            .map(|asset| (asset.info, asset.amount))
            .collect();
    }

    // Param - address to which tokens are to be transferred
    let recipient = deps.api.addr_validate(&recipient)?;

    // Response - List of assets to be transferred to the user
    let mut assets_out = vec![];

    let mut charged_fee_breakup: Vec<AssetFeeBreakup> = vec![];

    // Update asset balances & transfer tokens WasmMsgs
    let mut index = 0;
    for stored_asset in pool_info.assets.iter_mut() {
        // Error - The returned list of assets needs to be sorted in the same order as the stored list of assets
        if stored_asset.info != pool_exit_transition.assets_out[index].info.clone() {
            return Err(ContractError::InvalidSequenceOfAssets {});
        }

        // Param - Number of tokens for this asset to be transferred to the recipient: As instructed by the Pool Math
        let to_transfer = pool_exit_transition.assets_out[index].amount.clone();

        // Param - Fee to be charged in these tokens
        let total_fee = fee_collection
            .get(&stored_asset.info.clone())
            .copied()
            .unwrap_or(Uint128::zero());

        // Param - protocol and lp fee
        let mut protocol_fee: Uint128 = Uint128::zero();

        // Compute - calculate protocol fee based on % of total fee
        if !total_fee.is_zero() && config.fee_collector.is_some() {
            protocol_fee = pool_info
                .fee_info
                .calculate_total_fee_breakup(total_fee.clone());
        }

        // If number of tokens to transfer > 0 or fee > 0, then
        // - Update stored pool's asset balances in `PoolInfo` Struct
        // - Transfer tokens to the user, tranfer fees
        if !to_transfer.is_zero() || !total_fee.is_zero() {
            let liquidity_withdrawn = to_transfer + protocol_fee;

            // Update - Update Pool Liquidity
            // - Liquidity Removed = transfer_out + protocol_fee
            // here,
            // to_transfer: tokens to be transferred from Vault  to the user
            // protocol_fee: protocol fee to be charged and transfer to the fee_collector
            // Note: LP fees = total_fee - protocol_fee, pools need to charge fee in-terms of LP tokens (burn more number of LP tokens)
            //                 so inherently users burn LP tokens equivalent to : (transfer_out + total_fee) while the actual liquidity
            //                withdrawn is (transfer_out - total_fee + lp_fee), where lp_fee = total_fee - protocol_fee
            // Compute - Subtract all tokens to be transferred to the User and protocol fee
            stored_asset.amount = stored_asset.amount.checked_sub(liquidity_withdrawn)?;

            // Indexing - Collect fee data to add to add to event
            charged_fee_breakup.push(AssetFeeBreakup {
                asset_info: stored_asset.info.clone(),
                total_fee,
                protocol_fee,
            });

            // ExecuteMsg -::- Transfer tokens from Vault to the user
            if !to_transfer.is_zero() {
                execute_msgs.push(
                    stored_asset
                        .info
                        .clone()
                        .create_transfer_msg(recipient.clone(), to_transfer)?,
                );
            }

            // ExecuteMsg -::- To transfer the protocol fee to the fee collector
            if !protocol_fee.is_zero() {
                execute_msgs.push(
                    stored_asset
                        .info
                        .clone()
                        .create_transfer_msg(config.fee_collector.clone().unwrap(), protocol_fee)?,
                );
            }

            let asset_out = pool_exit_transition.assets_out[index].clone();
            assets_out.push(asset_out);
        }
        // Increment Index
        index = index + 1;
    }

    // ExecuteMsg:: Burn LP Tokens
    execute_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: pool_info.lp_token_addr.to_string(),
        msg: to_json_binary(&Cw20ExecuteMsg::Burn {
            amount: pool_exit_transition.burn_shares.clone(),
        })?,
        funds: vec![],
    }));

    // ExecuteMsg:: Return LP shares in case some of the LP tokens transferred are to be returned
    if !lp_to_return.is_zero() {
        execute_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: pool_info.lp_token_addr.to_string(),
            msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                amount: lp_to_return,
                recipient: sender.clone(),
            })?,
            funds: vec![],
        }));
    }

    // ExecuteMsg:: Updated Pool's stored liquidity state
    execute_msgs.push(build_update_pool_state_msg(
        pool_info.pool_addr.to_string(),
        pool_info.assets.clone(),
    )?);

    // WRITE - Store the Updated PoolInfo state to the storage
    ACTIVE_POOLS.save(deps.storage, &pool_id.to_string().as_bytes(), &pool_info)?;

    Ok(Response::new().add_messages(execute_msgs).add_event(
        // can't use info here as the msg sender would be the cw20 contract in that case
        Event::from_sender(concatcp!(CONTRACT_NAME, "::exit_pool"), sender.clone())
            .add_attribute("pool_id", pool_id.to_string())
            .add_attribute("recipient", recipient.to_string())
            .add_attribute("lp_tokens_burnt", pool_exit_transition.burn_shares.to_string())
            .add_attribute("assets_out", serde_json_wasm::to_string(&assets_out).unwrap())
            .add_attribute("fees", serde_json_wasm::to_string(&charged_fee_breakup).unwrap())
            .add_attribute("pool_addr", pool_info.pool_addr.to_string())
            .add_attribute("vault_contract_address", env.contract.address)
    ))
}

//--------x---------------x--------------x-----x-----
//--------x    Execute :: Swap Tx Execution    x-----
//--------x---------------x--------------x-----x-----

/// ## Description - Entry point for a swap tx between offer and ask assets. The swap request details are passed in [`SingleSwapRequest`] Type parameter.
/// User needs to provide offer and ask asset info 's, the SwapType ( GiveIn or GiveOut ) and the amount of tokens to be swapped (ask )
/// The  number of tokens to be swapped against are decided by the pool contract 's math computations.
///
/// ## Params
/// * **swap_request** of type [`SingleSwapRequest`] which consists of the following fields: pool_id of type [`Uint128`], asset_in of type [`AssetInfo`], asset_out of type [`AssetInfo`], swap_type of type SwapType, amount of type [`Uint128`]
/// * **op_recipient** Optional parameter. Recipient address of the swap tx. If not provided, then the default value is the sender address.
/// * **min_receive** Optional parameter. Minimum tokens to receive if swap is of type GiveIn
/// * **max_spend** Optional parameter. Maximum tokens to spend if swap is of type GiveOut
pub fn execute_swap(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    swap_request: SingleSwapRequest,
    recipient: Option<String>,
    min_receive: Option<Uint128>,
    max_spend: Option<Uint128>,
) -> Result<Response, ContractError> {
    // Param - recipient address
    let recipient = recipient
        .map(|r| deps.api.addr_validate(r.as_str()).unwrap())
        .unwrap_or(info.sender.clone());

    // Error - Amount cannot be zero
    if swap_request.amount.is_zero() {
        return Err(ContractError::AmountCannotBeZero {});
    }

    // Error - AssetInfo's cannot be same
    if swap_request.asset_in == swap_request.asset_out {
        return Err(ContractError::SameTokenError {});
    }

    // Read - Get the Config {}
    let config = CONFIG.load(deps.storage)?;

    //  Read -  Get PoolInfo {} for the pool
    // This also validates the pool exists and is not defunct
    let mut pool_info = validate_pool_exists_and_not_defunct(deps.storage, swap_request.pool_id)?;

    // Read - Get the PoolConfig {} for the pool
    let pool_config = REGISTRY.load(deps.storage, pool_info.pool_type.to_string())?;

    if config.paused.swap || pool_config.paused.swap || pool_info.paused.swap {
        return Err(ContractError::PausedSwap {});
    }

    // Query - Query Pool Instance  to get the state transition to be handled
    // SwapResponse {}  is the response from the pool contract and has the following parameters,
    // * **trade_params** of type [`Trade`] - Contains `amount_in` and `amount_out` of type [`Uint128`] along-with the `spread`
    // * **response** of type [`response`] - The response type :: Success or Failure
    // * **fee** of type [`Option<Asset>`] - Optional Fee to be charged as fees to the user.  If it is null then no fee is charged
    let pool_swap_transition: dexter::pool::SwapResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: pool_info.pool_addr.to_string(),
            msg: to_json_binary(&dexter::pool::QueryMsg::OnSwap {
                swap_type: swap_request.swap_type.clone(),
                offer_asset: swap_request.asset_in.clone(),
                ask_asset: swap_request.asset_out.clone(),
                amount: swap_request.amount
            })?,
        }))?;

    // Error - If the response is failure
    if !pool_swap_transition.response.is_success() {
        return Err(ContractError::PoolQueryFailed {
            error: pool_swap_transition.response.to_string(),
        });
    }

    // Error - If any of the amount_in / amount_out is zero
    if pool_swap_transition.trade_params.amount_in.is_zero()
        || pool_swap_transition.trade_params.amount_out.is_zero()
    {
        return Err(ContractError::SwapAmountZero {});
    }

    // Params - Create offer and ask assets
    let offer_asset = Asset {
        info: swap_request.asset_in,
        amount: pool_swap_transition.trade_params.amount_in,
    };
    let ask_asset = Asset {
        info: swap_request.asset_out,
        amount: pool_swap_transition.trade_params.amount_out,
    };

    // Indexing - Make Event for indexing support
    let mut event = Event::from_info(concatcp!(CONTRACT_NAME, "::swap"), &info)
        .add_attribute("pool_id", swap_request.pool_id.to_string())
        .add_attribute("pool_addr", pool_info.pool_addr.to_string())
        .add_attribute("asset_in", serde_json_wasm::to_string(&offer_asset).unwrap())
        .add_attribute("asset_out", serde_json_wasm::to_string(&ask_asset).unwrap())
        .add_attribute("swap_type", swap_request.swap_type.to_string());
   
   
    event = event.add_attribute("recipient", recipient.to_string());
    if min_receive.is_some() {
        event = event.add_attribute("min_receive", min_receive.unwrap().to_string());
    }
    if max_spend.is_some() {
        event = event.add_attribute("max_spend", max_spend.unwrap().to_string())
    }

    // Compute - Fee Calculation
    let mut protocol_fee = Uint128::zero();
    if let Some(fee) = pool_swap_transition.fee.clone() {
        if !fee.amount.is_zero() {
            // Compute - Protocol Fee if recipient address is set
            if config.fee_collector.is_some() {
                protocol_fee = pool_info
                    .fee_info
                    .calculate_total_fee_breakup(fee.amount);
            }

            event = event.add_attribute("fee_asset", serde_json_wasm::to_string(&fee.info).unwrap())
                .add_attribute("total_fee", fee.amount.to_string())
                .add_attribute("protocol_fee", protocol_fee.to_string());
        }
    }

    // Error - If the max spend amount is provided, then check if the offer asset amount is less than the max spend amount and if not then return error
    if max_spend.is_some() && max_spend.unwrap() < offer_asset.amount {
        return Err(ContractError::MaxSpendError {
            max_spend: max_spend.unwrap(),
            offer_amount: offer_asset.amount,
        });
    }

    // Error - If the min receive amount is provided, then check if the ask asset amount is greater than the min receive amount and if not then return error
    if min_receive.is_some() && min_receive.unwrap() > ask_asset.amount {
        return Err(ContractError::MinReceiveError {
            min_receive: min_receive.unwrap(),
            ask_amount: ask_asset.amount,
        });
    }

    // Update asset balances
    let mut offer_asset_updated: bool = false;
    let mut ask_asset_updated: bool = false;
    let mut execute_msgs: Vec<CosmosMsg> = vec![];

    // Update Loop - We loop through all the assets supported by the pool and do the following,
    for stored_asset in pool_info.assets.iter_mut() {
        // ::: Offer Asset
        if stored_asset.info.as_string() == offer_asset.info.as_string() {
            let act_amount_in = offer_asset.amount;
            // ::: Update State -  Add tokens received to pool balance
            stored_asset.amount = stored_asset.amount.checked_add(act_amount_in)?;
            // ::: Update State - If fee is charged in offer asset, then subtract protocol_fee from pool balance
            if pool_swap_transition
                .fee
                .clone()
                .unwrap()
                .info
                .equal(&offer_asset.info)
            {
                stored_asset.amount = stored_asset.amount.checked_sub(protocol_fee)?;
            }
            offer_asset_updated = true;

            // ExecuteMsg : Transfer offer asset from user to the vault
            if !offer_asset.is_native_token() {
                execute_msgs.push(build_transfer_cw20_from_user_msg(
                    offer_asset.info.as_string(),
                    info.sender.to_string(),
                    env.contract.address.to_string(),
                    act_amount_in,
                )?);
            } else {
                // Get number of offer asset (Native) tokens sent with the msg
                let tokens_received = offer_asset.info.get_sent_native_token_balance(&info);

                // Error - If number of tokens sent are less than what the pool expects, return error
                if tokens_received < act_amount_in {
                    return Err(ContractError::InsufficientTokensSent {});
                }
                // ExecuteMsg - If number of tokens sent are more than what the pool expects, return additional tokens sent
                if tokens_received > act_amount_in {
                    execute_msgs.push(stored_asset.info.clone().create_transfer_msg(
                        info.sender.clone(),
                        tokens_received.checked_sub(act_amount_in)?,
                    )?);
                }
            }
        }

        // ::: Ask Asset
        if stored_asset.info == ask_asset.info {
            // ::: Update State -  Subtract tokens transferred to user from pool balance
            stored_asset.amount = stored_asset.amount.checked_sub(ask_asset.amount)?;
            // ::: Update State - If fee is charged in ask asset, then subtract protocol_fee and dev_fee from pool balance
            if pool_swap_transition
                .fee
                .clone()
                .unwrap()
                .info
                .equal(&ask_asset.info)
            {
                stored_asset.amount = stored_asset.amount.checked_sub(protocol_fee)?;
            }
            ask_asset_updated = true;

            // ExecuteMsg : Transfer tokens from Vault to the recipient
            execute_msgs.push(ask_asset.clone().into_msg(recipient.clone())?);
        }

        // if we have updated both offer & ask asset, no need to iterate further and waste gas
        if offer_asset_updated && ask_asset_updated {
            break;
        }
    }

    // Error - Error if something is wrong with state update operations
    if !offer_asset_updated || !ask_asset_updated {
        return Err(ContractError::MismatchedAssets {});
    }

    // WRITE - Update pool state
    ACTIVE_POOLS.save(
        deps.storage,
        &swap_request.pool_id.to_string().as_bytes(),
        &pool_info,
    )?;

    // ExecuteMsg :: Update Pool Instance state
    execute_msgs.push(build_update_pool_state_msg(
        pool_info.pool_addr.to_string(),
        pool_info.assets,
    )?);

    // ExecuteMsg :: Protocol Fee transfer to Keeper contract
    if !protocol_fee.is_zero() {
        execute_msgs.push(
            pool_swap_transition
                .fee
                .unwrap()
                .info
                .create_transfer_msg(config.fee_collector.unwrap(), protocol_fee)?,
        );
    }

    Ok(Response::new().add_messages(execute_msgs).add_event(event))
}

// ----------------x----------------x---------------------x-----------------------x----------------x----------------
// ----------------x----------------x  :::: VAULT::QUERIES Implementation   ::::  x----------------x----------------
// ----------------x----------------x---------------------x-----------------------x----------------x----------------

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
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::QueryRegistry { pool_type } => to_json_binary(&query_registry(deps, pool_type)?),
        QueryMsg::GetPoolById { pool_id } => to_json_binary(&query_pool_by_id(deps, pool_id)?),
        QueryMsg::Pools { start_after, limit } => to_json_binary(&query_pools(deps, start_after, limit)?),
        QueryMsg::GetPoolByAddress { pool_addr } => {
            to_json_binary(&query_pool_by_addr(deps, pool_addr)?)
        }
        QueryMsg::GetPoolByLpTokenAddress { lp_token_addr } => {
            to_json_binary(&query_pool_by_lp_token_addr(deps, lp_token_addr)?)
        }
        QueryMsg::GetDefunctPoolInfo { pool_id } => {
            let defunct_pool_info = DEFUNCT_POOLS
                .may_load(deps.storage, pool_id.to_string().as_bytes())?;
            to_json_binary(&defunct_pool_info)
        }
        QueryMsg::IsUserRefunded { pool_id, user } => {
            let is_refunded = REFUNDED_USERS
                .has(deps.storage, (pool_id.to_string().as_bytes(), user.as_str()));
            to_json_binary(&is_refunded)
        }
    }
}

/// ## Description - Returns the stored Vault Configuration settings in custom [`ConfigResponse`] structure
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(config)
}

/// ## Description - Returns the [`PoolType`]'s Configuration settings  in custom [`PoolConfigResponse`] structure
///
/// ## Params
/// * **pool_type** is the object of type [`PoolType`]. Its the pool type for which the configuration is requested.
pub fn query_registry(deps: Deps, pool_type: PoolType) -> StdResult<PoolTypeConfigResponse> {
    let pool_config =
        REGISTRY
            .load(deps.storage, pool_type.to_string())
            .or(Err(StdError::generic_err(
                ContractError::PoolTypeConfigNotFound {}.to_string(),
            )))?;
    Ok(Some(pool_config))
}

/// ## Description - Returns the current stored state of the queried pools
///
/// ## Params
/// * **start_after** is the object of type [`Uint128`]. Its the pool id after which you want to query the pool states.
/// * **limit** is the number of pools you want in a page.
pub fn query_pools(deps: Deps, start_after: Option<Uint128>, limit: Option<u32>) -> StdResult<Vec<PoolInfoResponse>> {
    let config = CONFIG.load(deps.storage)?;

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let start = start_after.unwrap_or_default().u128() + 1u128;

    let mut end = start + Uint128::from(limit).u128();
    if end > config.next_pool_id.u128() {
        end = config.next_pool_id.u128();
    }

    let mut response: Vec<PoolInfoResponse>= vec![];
    for pool_id in start..end {
        response.push(ACTIVE_POOLS.load(deps.storage, Uint128::from(pool_id).to_string().as_bytes())?);
    }

    Ok(response)
}

/// ## Description - Returns the current stored state of the Pool in custom [`PoolInfoResponse`] structure
///
/// ## Params
/// * **pool_id** is the object of type [`Uint128`]. Its the pool id for which the state is requested.
pub fn query_pool_by_id(deps: Deps, pool_id: Uint128) -> StdResult<PoolInfoResponse> {
    ACTIVE_POOLS.load(deps.storage, pool_id.to_string().as_bytes())
}

/// ## Description - Returns the current stored state of the Pool in custom [`PoolInfoResponse`] structure
///
/// ## Params
/// * **pool_addr** is the object of type [`String`]. Its the pool address for which the state is requested.
pub fn query_pool_by_addr(deps: Deps, pool_addr: String) -> StdResult<PoolInfoResponse> {
    let pool_id: Uint128 = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: pool_addr.to_string(),
        msg: to_json_binary(&dexter::pool::QueryMsg::PoolId {})?,
    }))?;

    ACTIVE_POOLS.load(deps.storage, pool_id.to_string().as_bytes())
}

/// ## Description - Returns the current stored state of the Pool in custom [`PoolInfoResponse`] structure
/// 
/// ## Params
/// * **lp_token_addr** is the object of type [`String`]. Its the lp token address for which the state is requested.
pub fn query_pool_by_lp_token_addr(deps: Deps, lp_token_addr: String) -> StdResult<PoolInfoResponse> {
    let pool_id = LP_TOKEN_TO_POOL_ID.load(deps.storage, lp_token_addr.as_bytes())?;
    ACTIVE_POOLS.load(deps.storage, pool_id.to_string().as_bytes())
}

// ----------------x----------------x---------------------x-------------------x----------------x----------------
// ----------------x----------------x  :::: VAULT::Migration function   ::::  x----------------x----------------
// ----------------x----------------x---------------------x-------------------x----------------x----------------

/// ## Description - Used for migration of contract. Returns the default object of type [`Response`].
/// ## Params
/// * **_msg** is the object of type [`MigrateMsg`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    let contract_version = get_contract_version(deps.storage)?;
    
    match msg {
        MigrateMsg::V1_1 { updated_pool_type_configs } => {
            // validate contract name
            if contract_version.contract != CONTRACT_NAME {
                return Err(ContractError::InvalidContractNameForMigration { 
                    expected: CONTRACT_NAME.to_string(),
                    actual: contract_version.contract,
                 });
            }

            // validate that current version is v1.0
            if contract_version.version != CONTRACT_VERSION_V1 {
                return Err(ContractError::InvalidContractVersionForUpgrade { 
                    upgrade_version: CONTRACT_VERSION.to_string(),
                    expected: CONTRACT_VERSION_V1.to_string(),
                    actual: contract_version.version,
                 });
            }

            // update pool type configs to new values. This makes sure we instantiate new pools with the new configs particularly the 
            // Code ID for each pool type which has been updated to a new value with the new version of the pool contracts
            for pool_type_config in updated_pool_type_configs {
                 // Check if code id is valid
                if pool_type_config.code_id == 0 {
                    return Err(ContractError::InvalidCodeId {});
                }
                // validate fee bps limits
                if !pool_type_config.default_fee_info.valid_fee_info() {
                    return Err(ContractError::InvalidFeeInfo {});
                }
                REGISTRY.save(deps.storage, pool_type_config.pool_type.to_string(), &pool_type_config)?;
            }

            set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
        }
    }
    
    Ok(Response::new()
        .add_attribute("previous_contract_name", &contract_version.contract)
        .add_attribute("previous_contract_version", &contract_version.version)
        .add_attribute("new_contract_name", CONTRACT_NAME)
        .add_attribute("new_contract_version", CONTRACT_VERSION))
}

// ----------------x----------------x---------------------x-------------------x----------------x-----
// ----------------x----------------x  :::: helper functions  ::::  x----------------x---------------
// ----------------x----------------x---------------------x-------------------x----------------x-----

/// # Description
/// Mint LP token to beneficiary or auto deposit into multistaking if set.
/// # Params
/// * **recipient** is the object of type [`Addr`]. The recipient of the liquidity.
/// * **amount** is the object of type [`Uint128`]. The amount that will be mint to the recipient.
/// * **auto_stake** is the field of type [`bool`]. Determines whether an autostake will be performed on the multistaking
fn build_mint_lp_token_msg(
    _deps: Deps,
    env: Env,
    lp_token: Addr,
    recipient: Addr,
    amount: Uint128,
    auto_stake: bool,
    auto_stake_impl: AutoStakeImpl,
) -> Result<Vec<CosmosMsg>, ContractError> {
    // If no auto-stake - just mint to recipient
    if !auto_stake {
        return Ok(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: lp_token.to_string(),
            msg: to_json_binary(&Cw20ExecuteMsg::Mint {
                recipient: recipient.to_string(),
                amount,
            })?,
            funds: vec![],
        })]);
    }

    let mut msgs = vec![CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_token.to_string(),
        msg: to_json_binary(&Cw20ExecuteMsg::Mint {
            recipient: env.contract.address.to_string(),
            amount,
        })?,
        funds: vec![],
    })];

    // Safe to do since it is validated at the caller
    let msg = match auto_stake_impl {
        AutoStakeImpl::Multistaking { contract_addr} => {
            // Address of multistaking
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: lp_token.to_string(),
                msg: to_json_binary(&Cw20ExecuteMsg::Send {
                    contract: contract_addr.to_string(),
                    amount,
                    msg: to_json_binary(&dexter::multi_staking::Cw20HookMsg::Bond {
                        beneficiary_user: Some(recipient),
                    })?,
                })?,
                funds: vec![],
            })
        }
        AutoStakeImpl::None => {
            return Err(ContractError::AutoStakeDisabled)
        }
    };

    msgs.push(msg);
    Ok(msgs)
}

pub fn build_update_pool_state_msg(
    pool_address: String,
    assets: Vec<Asset>,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: pool_address,
        funds: vec![],
        msg: to_json_binary(&dexter::pool::ExecuteMsg::UpdateLiquidity { assets })?,
    }))
}

// ----------------x----------------x---------------------x-------------------x----------------x-----
// ----------------x----------------x  :::: Defunct Pool Execute Functions  ::::  x----------------x---
// ----------------x----------------x---------------------x-------------------x----------------x-----

/// Validates that there are no active reward schedules for the given LP token
fn validate_no_active_reward_schedules(
    querier: &QuerierWrapper,
    lp_token: &Addr,
    current_time: u64,
    auto_stake_impl: &AutoStakeImpl,
) -> Result<(), ContractError> {
    // Only check if multistaking is enabled
    if let AutoStakeImpl::Multistaking { contract_addr } = auto_stake_impl {
        // Get common reward assets to check (native tokens and known CW20s)
        let common_reward_assets = vec![
            // Common native tokens
            dexter::asset::AssetInfo::NativeToken { denom: "uxprt".to_string() },
            dexter::asset::AssetInfo::NativeToken { denom: "uatom".to_string() },
            dexter::asset::AssetInfo::NativeToken { denom: "uusdc".to_string() },
            dexter::asset::AssetInfo::NativeToken { denom: "uosmo".to_string() },
            // Add more common assets as needed
        ];

        // Check each common asset for active reward schedules
        for asset in common_reward_assets {
            let reward_schedules: Vec<dexter::multi_staking::RewardScheduleResponse> = querier
                .query_wasm_smart(
                    contract_addr,
                    &dexter::multi_staking::QueryMsg::RewardSchedules {
                        lp_token: lp_token.clone(),
                        asset: asset.clone(),
                    },
                )
                .unwrap_or_default(); // If query fails, assume no schedules

            // Check if any reward schedule is currently active
            for schedule_response in reward_schedules {
                let schedule = schedule_response.reward_schedule;
                if schedule.start_block_time <= current_time && current_time < schedule.end_block_time {
                    return Err(ContractError::PoolHasActiveRewardSchedules);
                }
            }
        }
    }

    Ok(())
}

/// Makes a pool defunct - stops all operations and captures current state for refunds
pub fn execute_defunct_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_id: Uint128,
) -> Result<Response, ContractError> {
    // Validate caller is authorized
    validate_authorized_caller(deps.storage, &info.sender)?;

    // Validate pool exists and is not already defunct
    let pool_info = validate_pool_exists_and_not_defunct(deps.storage, pool_id)?;

    // Get config to check for multistaking and validate no active reward schedules
    let config = CONFIG.load(deps.storage)?;
    validate_no_active_reward_schedules(
        &deps.querier,
        &pool_info.lp_token_addr,
        env.block.time.seconds(),
        &config.auto_stake_impl,
    )?;

    // Query current pool assets from the pool contract
    let pool_config: dexter::pool::ConfigResponse = deps.querier.query_wasm_smart(
        &pool_info.pool_addr,
        &dexter::pool::QueryMsg::Config {},
    )?;
    let pool_assets = pool_config.assets;

    // Get total LP token supply
    let total_lp_supply = query_total_lp_supply(&deps.querier, &pool_info.lp_token_addr)?;

    // Create defunct pool info
    let defunct_pool_info = DefunctPoolInfo {
        pool_id,
        lp_token_addr: pool_info.lp_token_addr.clone(),
        total_assets_at_defunct: pool_assets,
        total_lp_supply_at_defunct: total_lp_supply,
        defunct_timestamp: env.block.time.seconds(),
        total_refunded_lp_tokens: Uint128::zero(),
    };

    // Save defunct pool info
    DEFUNCT_POOLS.save(deps.storage, pool_id.to_string().as_bytes(), &defunct_pool_info)?;

    // Remove from active pools
    ACTIVE_POOLS.remove(deps.storage, pool_id.to_string().as_bytes());

    let event = Event::from_info(concatcp!(CONTRACT_NAME, "::defunct_pool"), &info)
        .add_attribute("pool_id", pool_id.to_string())
        .add_attribute("lp_token_addr", pool_info.lp_token_addr.to_string())
        .add_attribute("total_lp_supply", total_lp_supply.to_string())
        .add_attribute("defunct_timestamp", env.block.time.seconds().to_string());

    Ok(Response::new().add_event(event))
}

/// Processes refunds for a batch of users from a defunct pool
pub fn execute_process_refund_batch(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    pool_id: Uint128,
    user_addresses: Vec<String>,
) -> Result<Response, ContractError> {
    // Validate caller is authorized
    validate_authorized_caller(deps.storage, &info.sender)?;

    // Validate pool is defunct
    let mut defunct_pool_info = validate_pool_is_defunct(deps.storage, pool_id)?;

    // Get config to check for multistaking address
    let config = CONFIG.load(deps.storage)?;
    
    let multistaking_addr = match &config.auto_stake_impl {
        AutoStakeImpl::Multistaking { contract_addr } => Some(contract_addr),
        AutoStakeImpl::None => None,
    };

    let mut messages = Vec::new();
    let mut refunded_lp_total = Uint128::zero();
    let mut batch_entries = Vec::new();

    // Process each user
    for user_addr_str in &user_addresses {
        let user_addr = deps.api.addr_validate(user_addr_str)?;
        
        // Check if user already refunded
        validate_user_not_refunded(deps.storage, pool_id, user_addr_str)?;

        // Calculate user's total LP tokens (direct + multistaking)
        let user_total_lp = if let Some(multistaking_addr) = multistaking_addr {
            calculate_user_total_lp_tokens(
                &deps.querier,
                multistaking_addr,
                &defunct_pool_info.lp_token_addr,
                &user_addr,
            )?
        } else {
            query_user_direct_lp_balance(&deps.querier, &defunct_pool_info.lp_token_addr, &user_addr)?
        };

        // Skip users with zero LP tokens
        if user_total_lp.is_zero() {
            continue;
        }

        // Calculate proportional refund
        let refund_assets = calculate_proportional_refund(
            defunct_pool_info.total_lp_supply_at_defunct,
            user_total_lp,
            &defunct_pool_info.total_assets_at_defunct,
        )?;

        // Create transfer messages for each asset
        for asset in &refund_assets {
            match &asset.info {
                dexter::asset::AssetInfo::Token { contract_addr } => {
                    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: contract_addr.to_string(),
                        funds: vec![],
                        msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                            recipient: user_addr.to_string(),
                            amount: asset.amount,
                        })?,
                    }));
                }
                dexter::asset::AssetInfo::NativeToken { denom } => {
                    messages.push(CosmosMsg::Bank(cosmwasm_std::BankMsg::Send {
                        to_address: user_addr.to_string(),
                        amount: vec![cosmwasm_std::Coin {
                            denom: denom.clone(),
                            amount: asset.amount,
                        }],
                    }));
                }
            }
        }

        // Mark user as refunded
        REFUNDED_USERS.save(deps.storage, (pool_id.to_string().as_bytes(), user_addr_str), &true)?;

        // Track total refunded LP tokens
        refunded_lp_total += user_total_lp;

        // Add to batch entries for event
        batch_entries.push(dexter::vault::RefundBatchEntry {
            user: user_addr,
            total_lp_tokens: user_total_lp,
            refund_assets,
        });
    }

    // Update total refunded LP tokens
    defunct_pool_info.total_refunded_lp_tokens += refunded_lp_total;
    DEFUNCT_POOLS.save(deps.storage, pool_id.to_string().as_bytes(), &defunct_pool_info)?;

    let event = Event::from_info(concatcp!(CONTRACT_NAME, "::process_refund_batch"), &info)
        .add_attribute("pool_id", pool_id.to_string())
        .add_attribute("batch_size", user_addresses.len().to_string())
        .add_attribute("refunded_lp_tokens", refunded_lp_total.to_string())
        .add_attribute("total_refunded_lp_tokens", defunct_pool_info.total_refunded_lp_tokens.to_string())
        .add_attribute("batch_entries", serde_json_wasm::to_string(&batch_entries).unwrap());

    Ok(Response::new().add_messages(messages).add_event(event))
}

// ----------------x----------------x---------------------x-------------------x----------------x-----
// ----------------x----------------x  :::: Defunct Pool Helper Functions  ::::  x----------------x---
// ----------------x----------------x---------------------x-------------------x----------------x-----

/// Validates that a pool exists and is not already defunct
fn validate_pool_exists_and_not_defunct(
    storage: &dyn cosmwasm_std::Storage,
    pool_id: Uint128,
) -> Result<PoolInfo, ContractError> {
    // Check if pool is already defunct
    if DEFUNCT_POOLS.has(storage, pool_id.to_string().as_bytes()) {
        return Err(ContractError::PoolAlreadyDefunct);
    }

    // Load pool info (this will fail if pool doesn't exist)
    let pool_info = ACTIVE_POOLS
        .load(storage, pool_id.to_string().as_bytes())
        .map_err(|_| ContractError::InvalidPoolId {})?;

    Ok(pool_info)
}

/// Validates that a pool is defunct
fn validate_pool_is_defunct(
    storage: &dyn cosmwasm_std::Storage,
    pool_id: Uint128,
) -> Result<DefunctPoolInfo, ContractError> {
    DEFUNCT_POOLS
        .load(storage, pool_id.to_string().as_bytes())
        .map_err(|_| ContractError::PoolNotDefunct)
}

/// Validates that the caller is authorized (owner or whitelisted)
fn validate_authorized_caller(
    storage: &dyn cosmwasm_std::Storage,
    caller: &Addr,
) -> Result<(), ContractError> {
    let config = CONFIG.load(storage)?;
    
    if caller == config.owner || config.whitelisted_addresses.contains(caller) {
        Ok(())
    } else {
        Err(ContractError::Unauthorized {})
    }
}

/// Validates that the user has not been refunded yet
fn validate_user_not_refunded(
    storage: &dyn cosmwasm_std::Storage,
    pool_id: Uint128,
    user: &str,
) -> Result<(), ContractError> {
    if REFUNDED_USERS.has(storage, (pool_id.to_string().as_bytes(), user)) {
        return Err(ContractError::UserAlreadyRefunded);
    }
    Ok(())
}

/// Calculates the proportional refund amount for a user based on their LP token holdings
fn calculate_proportional_refund(
    total_lp_tokens: Uint128,
    user_lp_tokens: Uint128,
    pool_assets: &[Asset],
) -> Result<Vec<Asset>, ContractError> {
    if total_lp_tokens.is_zero() {
        return Ok(vec![]);
    }

    let mut refund_assets = Vec::new();
    
    for asset in pool_assets {
        let refund_amount = asset.amount
            .checked_mul(user_lp_tokens)
            .map_err(|e| ContractError::Std(StdError::overflow(e)))?
            .checked_div(total_lp_tokens)
            .map_err(|e| ContractError::Std(StdError::divide_by_zero(e)))?;
        
        if !refund_amount.is_zero() {
            refund_assets.push(Asset {
                info: asset.info.clone(),
                amount: refund_amount,
            });
        }
    }
    
    Ok(refund_assets)
}

/// Queries the multistaking contract for user's bonded LP tokens
fn query_user_bonded_lp_tokens(
    querier: &cosmwasm_std::QuerierWrapper,
    multistaking_addr: &Addr,
    lp_token: &Addr,
    user: &Addr,
) -> Result<Uint128, ContractError> {
    let bonded_amount: Uint128 = querier.query_wasm_smart(
        multistaking_addr,
        &dexter::multi_staking::QueryMsg::BondedLpTokens {
            lp_token: lp_token.clone(),
            user: user.clone(),
        },
    )?;
    Ok(bonded_amount)
}

/// Queries the multistaking contract for user's locked LP tokens (unbonded but still in unlock period)
fn query_user_locked_lp_tokens(
    querier: &cosmwasm_std::QuerierWrapper,
    multistaking_addr: &Addr,
    lp_token: &Addr,
    user: &Addr,
) -> Result<Uint128, ContractError> {
    let token_lock_info: dexter::multi_staking::TokenLockInfo = querier.query_wasm_smart(
        multistaking_addr,
        &dexter::multi_staking::QueryMsg::TokenLocks {
            lp_token: lp_token.clone(),
            user: user.clone(),
            block_time: None,
        },
    )?;
    
    let locked_amount = token_lock_info.locks
        .iter()
        .fold(Uint128::zero(), |acc, lock| acc + lock.amount);
    
    Ok(locked_amount + token_lock_info.unlocked_amount)
}

/// Queries the CW20 contract for user's direct LP token balance
fn query_user_direct_lp_balance(
    querier: &cosmwasm_std::QuerierWrapper,
    lp_token: &Addr,
    user: &Addr,
) -> Result<Uint128, ContractError> {
    let balance: cw20::BalanceResponse = querier.query_wasm_smart(
        lp_token,
        &cw20::Cw20QueryMsg::Balance {
            address: user.to_string(),
        },
    )?;
    Ok(balance.balance)
}

/// Gets the total LP token supply from the CW20 contract
fn query_total_lp_supply(
    querier: &cosmwasm_std::QuerierWrapper,
    lp_token: &Addr,
) -> Result<Uint128, ContractError> {
    let token_info: cw20::TokenInfoResponse = querier.query_wasm_smart(
        lp_token,
        &cw20::Cw20QueryMsg::TokenInfo {},
    )?;
    Ok(token_info.total_supply)
}

/// Calculates the total LP tokens a user owns across all states (direct + multistaking)
fn calculate_user_total_lp_tokens(
    querier: &cosmwasm_std::QuerierWrapper,
    multistaking_addr: &Addr,
    lp_token: &Addr,
    user: &Addr,
) -> Result<Uint128, ContractError> {
    let direct_balance = query_user_direct_lp_balance(querier, lp_token, user)?;
    let bonded_balance = query_user_bonded_lp_tokens(querier, multistaking_addr, lp_token, user)?;
    let locked_balance = query_user_locked_lp_tokens(querier, multistaking_addr, lp_token, user)?;
    
    Ok(direct_balance + bonded_balance + locked_balance)
}
