#[cfg(not(feature = "library"))]
use crate::error::ContractError;
use crate::response::MsgInstantiateContractResponse;
use crate::state::{
    ACTIVE_POOLS, CONFIG, LP_TOKEN_TO_POOL_ID, OWNERSHIP_PROPOSAL, REGISTRY, TMP_POOL_INFO,
};
use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut,
    Env, Event, MessageInfo, QueryRequest, Reply, ReplyOn, Response, StdError, StdResult, SubMsg,
    Uint128, WasmMsg, WasmQuery,
};
use protobuf::Message;
use std::collections::HashMap;
use std::collections::HashSet;

use dexter::asset::{addr_opt_validate, Asset, AssetInfo};
use dexter::helper::{
    build_transfer_cw20_from_user_msg, claim_ownership, drop_ownership_proposal,
    find_sent_native_token_balance, get_lp_token_name, get_lp_token_symbol, propose_new_owner,
};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{FeeStructs, InstantiateMsg as PoolInstantiateMsg};
use dexter::vault::{
    AllowPoolInstantiation, AssetFeeBreakup, Config, ConfigResponse, Cw20HookMsg, ExecuteMsg,
    FeeInfo, InstantiateMsg, MigrateMsg, PauseInfo, PoolConfigResponse, PoolInfo, PoolInfoResponse,
    PoolType, PoolTypeConfig, QueryMsg, SingleSwapRequest, AutoStakeImpl, TmpPoolInfo, PauseInfoUpdateType,
};

use cw2::{get_contract_version, set_contract_version};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "crates.io:dexter-vault";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
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
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Check if code id is valid
    if let Some(code_id) = msg.lp_token_code_id {
        if code_id == 0 {
            return Err(ContractError::InvalidCodeId {});
        }
    }

    if let Some(pool_creation_fee) = &msg.pool_creation_fee {
        if pool_creation_fee.amount.is_zero() {
            return Err(ContractError::InvalidPoolCreationFee {});
        }
    }

    let config = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        whitelisted_addresses: vec![],
        lp_token_code_id: msg.lp_token_code_id,
        fee_collector: addr_opt_validate(deps.api, &msg.fee_collector)?,
        auto_stake_impl: msg.auto_stake_impl,
        multistaking_address: addr_opt_validate(deps.api, &msg.multistaking_address)?,
        generator_address: addr_opt_validate(deps.api, &msg.generator_address)?,
        pool_creation_fee: msg.pool_creation_fee,
        next_pool_id: Uint128::from(1u128),
        paused: PauseInfo::default(),
    };

    let config_set: HashSet<String> = msg
        .pool_configs
        .iter()
        .map(|pc| pc.pool_type.to_string())
        .collect();

    if config_set.len() != msg.pool_configs.len() {
        return Err(ContractError::PoolTypeConfigDuplicate {});
    }

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
        // Validate dev address (if provided)
        if pc.default_fee_info.developer_addr.clone().is_some() {
            deps.api
                .addr_validate(pc.default_fee_info.developer_addr.clone().unwrap().as_str())?;
        }
        REGISTRY.save(deps.storage, pc.clone().pool_type.to_string(), pc)?;
    }
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new())
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
            generator_address,
            multistaking_address,
            paused,
        } => execute_update_config(
            deps,
            env,
            info,
            lp_token_code_id,
            fee_collector,
            pool_creation_fee,
            auto_stake_impl,
            generator_address,
            multistaking_address,
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
            is_generator_disabled,
            paused,
        } => execute_update_pool_type_config(
            deps,
            info,
            pool_type,
            allow_instantiation,
            new_fee_info,
            is_generator_disabled,
            paused,
        ),
        ExecuteMsg::AddAddressToWhitelist { address } => {
            execute_add_address_to_whitelist(deps, info, address)
        }
        ExecuteMsg::RemoveAddressFromWhitelist { address } => {
            execute_remove_address_from_whitelist(deps, info, address)
        }
        ExecuteMsg::AddToRegistry { new_pool_config } => {
            execute_add_to_registry(deps, env, info, new_pool_config)
        }
        ExecuteMsg::CreatePoolInstance {
            pool_type,
            asset_infos,
            fee_info,
            init_params,
        } => execute_create_pool_instance(
            deps,
            env,
            info,
            pool_type,
            asset_infos,
            fee_info,
            init_params,
        ),
        ExecuteMsg::UpdatePoolConfig { pool_id, fee_info, paused } => {
            execute_update_pool_config(deps, info, pool_id, fee_info, paused)
        }
        ExecuteMsg::JoinPool {
            pool_id,
            recipient,
            assets,
            lp_to_mint,
            slippage_tolerance,
            auto_stake,
        } => execute_join_pool(
            deps,
            env,
            info,
            pool_id,
            recipient,
            assets,
            lp_to_mint,
            slippage_tolerance,
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
    let amount_transferred = cw20_msg.amount;

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::ExitPool {
            pool_id,
            recipient,
            assets,
            mut burn_amount,
        } => {
            // Check if amount is valid or not
            if burn_amount.is_some() && burn_amount.unwrap() != amount_transferred {
                return Err(ContractError::InvalidAmount {});
            }
            burn_amount = Some(amount_transferred);

            let act_recepient = recipient.unwrap_or(sender.clone());
            let sender = sender.clone();

            execute_exit_pool(
                deps,
                env,
                info,
                pool_id,
                act_recepient,
                sender,
                assets,
                burn_amount,
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
/// * **generator_address** optional parameter. New address of the generator to be used for staking LP tokens via `auto_stake`
///
/// ##Executor - Only owner can execute it
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    lp_token_code_id: Option<u64>,
    fee_collector: Option<String>,
    pool_creation_fee: Option<Asset>,
    auto_stake_impl: Option<AutoStakeImpl>,
    generator_address: Option<String>,
    multistaking_address: Option<String>,
    paused: Option<PauseInfo>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

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
        config.lp_token_code_id = Some(lp_token_code_id);
    }

    // Update fee collector
    if let Some(fee_collector) = fee_collector {
        config.fee_collector = Some(deps.api.addr_validate(fee_collector.as_str())?);
    }

    // Validate the pool creation fee
    if let Some(pool_creation_fee) = &pool_creation_fee {
        if pool_creation_fee.amount.is_zero() {
            return Err(ContractError::InvalidPoolCreationFee {});
        }
    }
    config.pool_creation_fee = pool_creation_fee;

    // set auto stake implementation
    config.auto_stake_impl = auto_stake_impl;

    // Set generator only if its not set
    if !config.generator_address.is_some() {
        if let Some(generator_address) = generator_address {
            config.generator_address = Some(deps.api.addr_validate(generator_address.as_str())?);
        }
    }

    // set multistaking address only if its not set
    if !config.multistaking_address.is_some() {
        if let Some(multistaking_address) = multistaking_address {
            config.multistaking_address =
                Some(deps.api.addr_validate(multistaking_address.as_str())?);
        }
    }

    // update the pause status
    if let Some(paused) = paused {
        config.paused = paused;
    }

    // Update LP token code id
    if let Some(lp_token_code_id) = lp_token_code_id {
        // Check if code id is valid
        if lp_token_code_id == 0 {
            return Err(ContractError::InvalidCodeId {});
        }
        config.lp_token_code_id = Some(lp_token_code_id);
    }

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_config"))
}

pub fn execute_update_pause_info(
    deps: DepsMut,
    info: MessageInfo,
    update_type: PauseInfoUpdateType,
    pause_info: PauseInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if !config.whitelisted_addresses.contains(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }

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

    Ok(Response::new())
}

/// ## Description - Updates pool configuration. Returns an [`ContractError`] on failure or
/// the following [`PoolConfig`] data will be updated if successful.
///
/// ## Params
/// * **is_disabled** Optional parameter. If set to `true`, the instantiation of new pool instances will be disabled. If set to `false`, they will be enabled.
/// * **is_generator_disabled**  Optional parameter. If set to `true`, the generator will not be able to support
///
/// ## Executor
/// Only owner can execute it
pub fn execute_update_pool_type_config(
    deps: DepsMut,
    info: MessageInfo,
    pool_type: PoolType,
    allow_instantiation: Option<AllowPoolInstantiation>,
    new_fee_info: Option<FeeInfo>,
    is_generator_disabled: Option<bool>,
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
    let mut event = Event::new("dexter-vault::update_pool_config")
        .add_attribute("tx_executor", info.sender.to_string());

    // Update allow instantiation
    if let Some(allow_instantiation) = allow_instantiation {
        event = event.add_attribute("allow_instantiation", allow_instantiation.to_string());
        pool_config.allow_instantiation = allow_instantiation;
    }

    // Disable or enable integration with dexter generator
    if let Some(is_generator_disabled) = is_generator_disabled {
        pool_config.is_generator_disabled = is_generator_disabled;
        event = event.add_attribute("is_generator_disabled", is_generator_disabled.to_string());
    }

    // Update fee info
    if let Some(new_fee_info) = new_fee_info {
        if !new_fee_info.valid_fee_info() {
            return Err(ContractError::InvalidFeeInfo {});
        }
        // Validate dev address (if provided)
        if new_fee_info.developer_addr.clone().is_some() {
            deps.api
                .addr_validate(new_fee_info.developer_addr.clone().unwrap().as_str())?;
        }

        pool_config.default_fee_info = new_fee_info;
        event = event
            .add_attribute(
                "total_fee_bps",
                pool_config.default_fee_info.total_fee_bps.to_string(),
            )
            .add_attribute(
                "protocol_fee_percent",
                pool_config
                    .default_fee_info
                    .protocol_fee_percent
                    .to_string(),
            )
            .add_attribute(
                "dev_fee_percent",
                pool_config.default_fee_info.dev_fee_percent.to_string(),
            );
    }

    if let Some(paused) = paused {
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
    paused: Option<PauseInfo>
) -> Result<Response, ContractError> {
    // permission check - only Owner can update any pool config.
    let config = CONFIG.load(deps.storage)?;
    if info.sender.clone() != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut pool = ACTIVE_POOLS.load(deps.storage, &pool_id.to_string().as_bytes())?;

    // Emit Event
    let mut event = Event::new("dexter-vault::update_pool_config")
        .add_attribute("tx_executor", info.sender.to_string());

    let mut msgs: Vec<CosmosMsg> = vec![];

    // Update fee info
    if let Some(fee_info) = fee_info {
        if !fee_info.valid_fee_info() {
            return Err(ContractError::InvalidFeeInfo {});
        }
        // Validate dev address (if provided)
        if fee_info.developer_addr.clone().is_some() {
            deps.api
                .addr_validate(fee_info.developer_addr.clone().unwrap().as_str())?;
        }

        pool.fee_info = fee_info;
        event = event
            .add_attribute("total_fee_bps", pool.fee_info.total_fee_bps.to_string())
            .add_attribute(
                "protocol_fee_percent",
                pool.fee_info.protocol_fee_percent.to_string(),
            )
            .add_attribute("dev_fee_percent", pool.fee_info.dev_fee_percent.to_string());

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

        event = event.add_attribute("paused", paused.to_string());
    }

    // Save pool config
    ACTIVE_POOLS.save(deps.storage, pool_id.to_string().as_bytes(), &pool)?;

    let response = Response::new()
        .add_event(event)
        .add_messages(msgs);

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
    config.whitelisted_addresses.push(address);

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "add_address_to_whitelist"))
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
    Ok(Response::new().add_attribute("action", "remove_address_from_whitelist"))
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
    _env: Env,
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

    // Validate dev address (if provided)
    if pool_type_config
        .default_fee_info
        .developer_addr
        .clone()
        .is_some()
    {
        deps.api.addr_validate(
            pool_type_config
                .default_fee_info
                .developer_addr
                .clone()
                .unwrap()
                .as_str(),
        )?;
    }

    // Save pool config
    REGISTRY.save(
        deps.storage,
        pool_type_config.pool_type.to_string(),
        &pool_type_config,
    )?;

    // Emit Event
    let event = Event::new("dexter-vault::add_new_pool")
        .add_attribute("pool_type", pool_type_config.pool_type.to_string())
        .add_attribute("code_id", pool_type_config.code_id.to_string())
        .add_attribute(
            "developer_addr",
            pool_type_config
                .default_fee_info
                .developer_addr
                .unwrap_or(Addr::unchecked("None".to_string())),
        )
        .add_attribute(
            "allow_instantiation",
            pool_type_config.allow_instantiation.to_string(),
        )
        .add_attribute(
            "is_generator_disabled",
            pool_type_config.is_generator_disabled.to_string(),
        )
        .add_attribute(
            "total_fee_bps",
            pool_type_config.default_fee_info.total_fee_bps.to_string(),
        )
        .add_attribute(
            "protocol_fee_percent",
            pool_type_config
                .default_fee_info
                .protocol_fee_percent
                .to_string(),
        )
        .add_attribute(
            "dev_fee_percent",
            pool_type_config
                .default_fee_info
                .dev_fee_percent
                .to_string(),
        );

    Ok(Response::new().add_event(event))
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

    let mut execute_msgs = vec![];

    // Validate if fee is sent for creation of pool
    if let Some(pool_creation_fee) = config.pool_creation_fee {
        // Check if sender has sent enough funds to pay for the pool creation fee
        let fee_amount = pool_creation_fee.amount;
        match pool_creation_fee.info.clone() {
            AssetInfo::NativeToken { denom } => {
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
                        let transfer_msg = pool_creation_fee.info.clone().create_transfer_msg(
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
    }

    // Sort Assets List
    asset_infos.sort_by(|a, b| {
        a.to_string()
            .to_lowercase()
            .cmp(&b.to_string().to_lowercase())
    });

    let mut assets: Vec<Asset> = vec![];
    let mut event = Event::new("dexter-vault::add_pool");

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

    event = event.add_attribute("pool_assets", serde_json_wasm::to_string(&assets).unwrap());

    // Pool Id for the new pool instance
    let pool_id = config.next_pool_id;

    let fee_info = fee_info.unwrap_or(pool_type_config.default_fee_info);
    let tmp_pool_info = TmpPoolInfo {
        code_id: pool_type_config.code_id,
        pool_id,
        lp_token_addr: None,
        fee_info: fee_info.clone(),
        assets,
        pool_type: pool_type_config.pool_type.clone(),
        init_params
    };

    // Store the temporary Pool Info
    TMP_POOL_INFO.save(deps.storage, &tmp_pool_info)?;

    // LP Token Name
    let token_name = get_lp_token_name(pool_id.clone());
    // LP Token Symbol
    let token_symbol = get_lp_token_symbol();

    // Emit Event
    event = event
        .add_attribute("pool_type", pool_type.to_string())
        .add_attribute("pool_id", pool_id.to_string())
        .add_attribute("lp_token_name", token_name.clone())
        .add_attribute("lp_token_symbol", token_symbol.clone())
        .add_attribute("total_fee_bps", fee_info.total_fee_bps.to_string())
        .add_attribute(
            "protocol_fee_percent",
            fee_info.protocol_fee_percent.to_string(),
        )
        .add_attribute(
            "developer_fee_percent",
            fee_info.dev_fee_percent.to_string(),
        );

    // Sub Msg to initialize the LP token instance
    let init_lp_token_sub_msg: SubMsg = SubMsg {
        id: INSTANTIATE_LP_REPLY_ID,
        msg: WasmMsg::Instantiate {
            admin: None,
            code_id: config.lp_token_code_id.ok_or(ContractError::LpTokenCodeIdNotSet)?,
            msg: to_binary(&TokenInstantiateMsg {
                name: token_name,
                symbol: token_symbol,
                decimals: 6,
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
    let mut event = Event::new("dexter-vault::add_pool_reply");

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
            tmp_pool_info.lp_token_addr = Some(lp_token_addr.clone());
            // Store the temporary Pool Info
            TMP_POOL_INFO.save(deps.storage, &tmp_pool_info)?;

            response = response.add_attributes(vec![
                attr("action", "reply"),
                attr("lp_token_addr", lp_token_addr.clone()),
            ]);

            // Store LP token addr _> Pool Id mapping in the LP token map
            LP_TOKEN_TO_POOL_ID.save(
                deps.storage,
                &lp_token_addr.clone().as_bytes(),
                &tmp_pool_info.pool_id.clone(),
            )?;

            event = event.add_attribute("pool_id", tmp_pool_info.pool_id);
            event = event.add_attribute("lp_token_addr", lp_token_addr.clone());

            // Sub Msg to initialize the pool instance
            let init_pool_sub_msg: SubMsg = SubMsg {
                id: INSTANTIATE_POOL_REPLY_ID,
                msg: WasmMsg::Instantiate {
                    admin: Some( CONFIG.load(deps.storage)?.owner.to_string()),
                    code_id: tmp_pool_info.code_id,
                    msg: to_binary(&PoolInstantiateMsg {
                        pool_id: tmp_pool_info.pool_id,
                        pool_type: tmp_pool_info.pool_type,
                        vault_addr: env.contract.address,
                        lp_token_addr,
                        asset_infos: tmp_pool_info.assets.iter().map(|a| a.info.clone()).collect(),
                        fee_info: FeeStructs {
                            total_fee_bps: tmp_pool_info.fee_info.total_fee_bps,
                        },
                        init_params: tmp_pool_info.init_params,
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
            response = response.add_attributes(vec![
                attr("action", "reply"),
                attr("pool_addr", pool_addr.clone()),
            ]);

            // Save the temporary pool info as permanent pool info mapped with the Pool Id
            ACTIVE_POOLS.save(
                deps.storage,
                &tmp_pool_info.pool_id.to_string().as_bytes(),
                &PoolInfo{
                    pool_id: tmp_pool_info.pool_id,
                    pool_addr: pool_addr.clone(),
                    lp_token_addr: tmp_pool_info.lp_token_addr.unwrap(),
                    fee_info: tmp_pool_info.fee_info,
                    assets: tmp_pool_info.assets,
                    pool_type: tmp_pool_info.pool_type,
                    paused: PauseInfo::default(),
                },
            )?;

            event = event.add_attribute("pool_id", tmp_pool_info.pool_id);
            event = event.add_attribute("pool_addr", pool_addr);

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
/// * **lp_to_mint** Optional parameter. The number of LP tokens the user wants to get against the provided assets.
/// * **auto_stake** Optional parameter. If provided, the Vault will automatically stake the provided assets with the generator contract.
pub fn execute_join_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_id: Uint128,
    op_recipient: Option<String>,
    assets_in: Option<Vec<Asset>>,
    lp_to_mint: Option<Uint128>,
    slippage_tolerance: Option<Decimal>,
    auto_stake: Option<bool>,
) -> Result<Response, ContractError> {
    // Read - Vault Config
    let config = CONFIG.load(deps.storage)?;

    // Read -  Get PoolInfo {} for the pool to which liquidity is to be provided
    let mut pool_info = ACTIVE_POOLS
        .load(deps.storage, pool_id.to_string().as_bytes())
        .or(Err(ContractError::InvalidPoolId {}))?;

    // Read -  Get PoolConfig {} for the pool
    let pool_config = REGISTRY.load(deps.storage, pool_info.pool_type.to_string())?;

    if config.paused.deposit || pool_config.paused.deposit || pool_info.paused.deposit {
        return Err(ContractError::PausedDeposit {});
    }

    // Check if auto-staking (if requested), is enabled (or possible) right now
    if auto_stake.unwrap_or(false) {
        match &config.auto_stake_impl {
            None => return Err(ContractError::AutoStakeDisabled {}),
            Some(auto_stake_impl) => match auto_stake_impl {
                AutoStakeImpl::Generator => {
                    // validate generator contract address is set
                    if config.generator_address.is_none() {
                        return Err(ContractError::GeneratorAddrNotSet);
                    }
                }
                AutoStakeImpl::Multistaking => {
                    // Validate multistaking contract address is set
                    if config.multistaking_address.is_none() {
                        return Err(ContractError::MultistakingAddrNotSet);
                    }
                }
            },
        }
    }

    // Query - Query the Pool Contract to get the state transition to be handled
    // AfterJoinResponse {} is the response from the pool contract and it contains the state transition to be handled by the Vault.
    // The state transition is described via the response params as following -
    // provided_assets - Sorted list of assets to be transferred from the user to the Vault as Pool Liquidity
    // new_shares - The number of LP tokens to be minted to the user / recipient
    // response - The response type :: Success or Failure
    // fee - Optional List assets (info and amounts) to be charged as fees to the user. If it is null then no fee is charged
    //       - We calculate the protocol_fee and developer_fee and transfer it to keeper and developer respectively.
    //       - When updating pool liquidity, we subtract the protocol_fee and developer_fee from the provided_assets.
    let pool_join_transition: dexter::pool::AfterJoinResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: pool_info.pool_addr.to_string(),
            msg: to_binary(&dexter::pool::QueryMsg::OnJoinPool {
                assets_in,
                mint_amount: lp_to_mint,
                slippage_tolerance,
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
    }

    // Error - Number of Assets should match
    if pool_join_transition.provided_assets.len() != pool_info.assets.len() {
        return Err(ContractError::InvalidNumberOfAssets {});
    }

    // Response -Number of LP tokens to be minted
    let new_shares = pool_join_transition.new_shares;

    // Response - Emit Event
    let mut event = Event::new("dexter-vault::join_pool")
        .add_attribute("pool_id", pool_id.to_string())
        .add_attribute(
            "pool_addr",
            pool_info.pool_addr.to_string(),
        )
        .add_attribute("lp_tokens_minted", new_shares.to_string());

    // ExecuteMsg - Stores the list of messages to be executed
    let mut execute_msgs: Vec<CosmosMsg> = vec![];

    // HashMap - Map the fees to be charged for each token: token_identifier --> amount
    let mut fee_collection: HashMap<AssetInfo, Uint128> = HashMap::new();
    if pool_join_transition.fee.is_some() {
        fee_collection = pool_join_transition
            .fee
            .clone()
            .unwrap()
            .into_iter()
            .map(|asset| (asset.info, asset.amount))
            .collect();
    }

    let mut charged_fee_breakup: Vec<AssetFeeBreakup> = vec![];

    // Update Loop - We loop through all the assets supported by the pool and do the following,
    //              1. Calculate Fee to be charged for the asset, and net liquidity to be updated for the asset
    //              2. Update the PoolInfo {} with the new liquidity
    //              3. Create CosmosMsg to - transfer tokens to the Vault, transfer fees to the keeper and developer
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

        // Param - protocol, developer and lp fee
        let mut protocol_fee: Uint128 = Uint128::zero();
        let mut dev_fee: Uint128 = Uint128::zero();

        // Compute - calculate protocol fee and dev fee based on % of total fee
        if !total_fee.clone().is_zero() {
            (protocol_fee, dev_fee) = pool_config
                .default_fee_info
                .calculate_total_fee_breakup(total_fee.clone());
        }

        // Compute - Update fee if recipient addresses are not set
        if !pool_config.default_fee_info.developer_addr.is_some() {
            protocol_fee = protocol_fee + dev_fee;
            dev_fee = Uint128::zero();
        }
        if !config.fee_collector.is_some() {
            protocol_fee = Uint128::zero();
        }

        // If number of tokens to transfer > 0, then
        // - Update stored pool's asset balances in `PoolInfo` Struct
        // - Transfer net calculated CW20 tokens from user to the Vault
        // - Return native tokens to the user (which are to be returned)
        if !transfer_in.is_zero() || !total_fee.is_zero() {
            // Update - Update Pool Liquidity
            // - Liquidity Provided = transfer_in - protocol_fee - dev_fee
            // here,
            // transfer_in: tokens to be transferred from user to the Vault
            // protocol_fee: protocol fee to be charged and transfer to the fee_collector
            // dev_fee: developer fee to be charged and transfer to the developer_addr
            // Note: LP fees = total_fee - protocol_fee - dev_fee, pools need to charge fee in-terms of LP tokens (mint less number of LP tokens)
            //                 so inherently users are minted LP tokens equivalent to : (transfer_in - total_fee) while the actual liquidity
            //                provided is (transfer_in - total_fee + lp_fee), where lp_fee = total_fee - protocol_fee - dev_fee
            // Compute - Add all tokens to be transferred to the Vault
            stored_asset.amount = stored_asset.amount.checked_add(transfer_in)?;
            // Compute - Subtract the protocol fee from the stored asset amount
            stored_asset.amount = stored_asset.amount.checked_sub(protocol_fee)?;
            // Compute - Subtract the developer fee from the stored asset amount
            stored_asset.amount = stored_asset.amount.checked_sub(dev_fee)?;

            // Indexing - Add fee to vec to push to event later
            charged_fee_breakup.push(AssetFeeBreakup {
                asset: stored_asset.info.clone(),
                total_fee,
                protocol_fee,
                dev_fee,
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
        // ExecuteMsg -::- To transfer the dev fee to the developer address
        if !dev_fee.is_zero() {
            execute_msgs.push(stored_asset.info.clone().create_transfer_msg(
                pool_config.default_fee_info.developer_addr.clone().unwrap(),
                dev_fee,
            )?);
        }

        // Increment Index
        index = index + 1;
    }

    let provided_assets_json =
        serde_json_wasm::to_string(&pool_join_transition.provided_assets).unwrap();
    let fees_json = serde_json_wasm::to_string(&charged_fee_breakup).unwrap();

    event = event.add_attribute("provided_assets", provided_assets_json);
    event = event.add_attribute("fees", fees_json);

    // Param - LP Token recipient / beneficiary if auto_stake = True
    let recipient = deps
        .api
        .addr_validate(op_recipient.unwrap_or(info.sender.to_string()).as_str())?;

    event = event.add_attribute("recipient", recipient.to_string());

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
        recipient,
        new_shares,
        auto_stake.unwrap_or(false),
        config.auto_stake_impl.clone(),
        config.multistaking_address.clone(),
        config.generator_address.clone(),
    )?;
    for msg in mint_msgs {
        execute_msgs.push(msg);
    }

    // WRITE - Store the Updated PoolInfo state to the storage
    ACTIVE_POOLS.save(deps.storage, &pool_id.to_string().as_bytes(), &pool_info)?;

    Ok(Response::new()
        .add_messages(execute_msgs)
        .add_event(event))
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
    sender: String,
    assets_out: Option<Vec<Asset>>,
    burn_amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    // Read - Vault config
    let config = CONFIG.load(deps.storage)?;

    //  Read -  Get PoolInfo {} for the pool to which liquidity is to be provided
    let mut pool_info = ACTIVE_POOLS
        .load(deps.storage, pool_id.to_string().as_bytes())
        .or(Err(ContractError::InvalidPoolId {}))?;

    // Read -  Get PoolConfig {} for the pool
    let pool_config = REGISTRY.load(deps.storage, pool_info.pool_type.to_string())?;

    // Error - Check if the LP token sent is valid
    if info.sender != pool_info.lp_token_addr {
        return Err(ContractError::Unauthorized {});
    }

    //  Query - Query the Pool Contract to get the state transition to be handled
    // AfterExitResponse {} is the response from the pool contract and it contains the state transition to be handled by the Vault.
    // The state transition is described via the response params as following -
    // assets_out - Sorted list of assets to be transferred to the user from the Vault
    // new_shares - The number of LP tokens to be burnt
    // response - The response type :: Success or Failure
    // fee - Optional List assets (info and amounts) to be charged as fees. If it is null then no fee is charged
    //       - We calculate the protocol_fee and developer_fee and transfer it to keeper and developer respectively.
    //       - When updating pool liquidity, we add the protocol_fee and developer_fee to the assets_out.
    let pool_exit_transition: dexter::pool::AfterExitResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: pool_info.pool_addr.to_string(),
            msg: to_binary(&dexter::pool::QueryMsg::OnExitPool {
                assets_out,
                burn_amount,
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

    // Param - Number of LP shares to be returned to the user
    let lp_to_return: Uint128;

    // Error - If Lp token to burn > Lp tokens transferred by the user
    if pool_exit_transition.burn_shares > burn_amount.unwrap() {
        return Err(ContractError::InsufficientLpTokensToExit {});
    } else {
        // TODO: Somehow by the above if check we are enforcing that the burn_amount must always be provided.
        //  So, why keep burn_amount as optional?
        //  Once we are sure we don't need this for any future versions, maybe we can make it required.
        lp_to_return = burn_amount
            .unwrap()
            .checked_sub(pool_exit_transition.burn_shares)?;
    }

    //  ExecuteMsg - Stores the list of messages to be executed
    let mut execute_msgs: Vec<CosmosMsg> = vec![];

    // Response - Emit Event
    let mut event = Event::new("dexter-vault::exit_pool")
        .add_attribute("pool_id", pool_id.to_string())
        .add_attribute(
            "pool_addr",
            pool_info.pool_addr.to_string(),
        )
        .add_attribute(
            "lp_tokens_burnt",
            pool_exit_transition.burn_shares.to_string(),
        );

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
    let recipient_addr = deps.api.addr_validate(&recipient)?;

    // Response - List of assets to be transferred to the user
    let mut assets_out = vec![];

    let mut charged_fee_breakup: Vec<AssetFeeBreakup> = vec![];
    let mut liquidity_withdrawn_per_asset: Vec<Asset> = vec![];

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

        // Param - protocol, developer and lp fee
        let mut protocol_fee: Uint128 = Uint128::zero();
        let mut dev_fee: Uint128 = Uint128::zero();

        // Compute - calculate protocol fee and dev fee based on % of total fee
        if !total_fee.clone().is_zero() {
            (protocol_fee, dev_fee) = pool_config
                .default_fee_info
                .calculate_total_fee_breakup(total_fee.clone());
        }

        // Compute - Update fee if recipient addresses are not set
        if !pool_config.default_fee_info.developer_addr.is_some() {
            protocol_fee = protocol_fee + dev_fee;
            dev_fee = Uint128::zero();
        }
        if !config.fee_collector.is_some() {
            protocol_fee = Uint128::zero();
        }

        // If number of tokens to transfer > 0 or fee > 0, then
        // - Update stored pool's asset balances in `PoolInfo` Struct
        // - Transfer tokens to the user, tranfer fees
        if !to_transfer.is_zero() || !total_fee.is_zero() {
            let liquidity_withdrawn = to_transfer + protocol_fee + dev_fee;

            // Update - Update Pool Liquidity
            // - Liquidity Removed = transfer_out + protocol_fee + dev_fee
            // here,
            // to_transfer: tokens to be transferred from Vault  to the user
            // protocol_fee: protocol fee to be charged and transfer to the fee_collector
            // dev_fee: developer fee to be charged and transfer to the developer_addr
            // Note: LP fees = total_fee - protocol_fee - dev_fee, pools need to charge fee in-terms of LP tokens (burn more number of LP tokens)
            //                 so inherently users burn LP tokens equivalent to : (transfer_out + total_fee) while the actual liquidity
            //                withdrawn is (transfer_out - total_fee + lp_fee), where lp_fee = total_fee - protocol_fee - dev_fee
            // Compute - Subtract all tokens to be transferred to the User, protocol fee and developer fee
            stored_asset.amount = stored_asset.amount.checked_sub(liquidity_withdrawn)?;

            // Indexing - Collect fee data to add to add to event
            charged_fee_breakup.push(AssetFeeBreakup {
                asset: stored_asset.info.clone(),
                total_fee,
                protocol_fee,
                dev_fee,
            });

            // Indexing: Collect liquidity withdrawn to add to event
            liquidity_withdrawn_per_asset.push(Asset {
                info: stored_asset.info.clone(),
                amount: liquidity_withdrawn,
            });

            // ExecuteMsg -::- Transfer tokens from Vault to the user
            if !to_transfer.is_zero() {
                execute_msgs.push(
                    stored_asset
                        .info
                        .clone()
                        .create_transfer_msg(recipient_addr.clone(), to_transfer)?,
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
            // ExecuteMsg -::- To transfer the dev fee to the developer address
            if !dev_fee.is_zero() {
                execute_msgs.push(stored_asset.info.clone().create_transfer_msg(
                    pool_config.default_fee_info.developer_addr.clone().unwrap(),
                    dev_fee,
                )?);
            }

            let asset_out = pool_exit_transition.assets_out[index].clone();
            assets_out.push(asset_out);
        }
        // Increment Index
        index = index + 1;
    }

    let assets_out_json = serde_json_wasm::to_string(&assets_out).unwrap();
    let liquidity_withdrawan_json =
        serde_json_wasm::to_string(&liquidity_withdrawn_per_asset).unwrap();
    let fee_breakup_json = serde_json_wasm::to_string(&charged_fee_breakup).unwrap();
    event = event.add_attribute("assets_out", assets_out_json);
    event = event.add_attribute("liquidity_withdrawn", liquidity_withdrawan_json);
    event = event.add_attribute("fees", fee_breakup_json);

    event = event.add_attribute("sender", sender.clone());
    event = event.add_attribute("vault_contract_address", env.contract.address);

    // Check - Burn amount cannot be 0
    if pool_exit_transition.burn_shares.is_zero() {
        return Err(ContractError::BurnAmountZero {});
    }

    // ExecuteMsg:: Burn LP Tokens
    execute_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: pool_info.lp_token_addr.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Burn {
            amount: pool_exit_transition.burn_shares.clone(),
        })?,
        funds: vec![],
    }));

    // ExecuteMsg:: Return LP shares in case some of the LP tokens transferred are to be returned
    if !lp_to_return.is_zero() {
        execute_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: pool_info.lp_token_addr.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                amount: lp_to_return,
                recipient: sender.clone(),
            })?,
            funds: vec![],
        }));
    }

    event = event.add_attribute("recipient_addr", recipient_addr.to_string());

    // ExecuteMsg:: Updated Pool's stored liquidity state
    execute_msgs.push(build_update_pool_state_msg(
        pool_info.pool_addr.to_string(),
        pool_info.assets.clone(),
    )?);

    // WRITE - Store the Updated PoolInfo state to the storage
    ACTIVE_POOLS.save(deps.storage, &pool_id.to_string().as_bytes(), &pool_info)?;

    Ok(Response::new().add_messages(execute_msgs).add_event(event))
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
    op_recipient: Option<String>,
    min_receive: Option<Uint128>,
    max_spend: Option<Uint128>,
) -> Result<Response, ContractError> {
    // Param - recipient address
    let mut recipient = info.sender.clone();
    if !op_recipient.is_none() {
        recipient = deps.api.addr_validate(op_recipient.unwrap().as_str())?;
    }

    // Error - Amount cannot be zero
    if swap_request.amount.is_zero() {
        return Err(ContractError::InvalidAmount {});
    }

    // Error - AssetInfo's cannot be same
    if swap_request.asset_in == swap_request.asset_out {
        return Err(ContractError::SameTokenError {});
    }

    // Read - Get the Config {}
    let config = CONFIG.load(deps.storage)?;

    //  Read -  Get PoolInfo {} for the pool
    let mut pool_info = ACTIVE_POOLS
    .load(deps.storage, swap_request.pool_id.to_string().as_bytes())
    .or(Err(ContractError::InvalidPoolId {}))?;

    // Read - Get the PoolConfig {} for the pool
    let pool_config = REGISTRY.load(deps.storage, pool_info.pool_type.to_string())?;

    if config.paused.swap || pool_config.paused.swap || pool_info.paused.swap {
        return Err(ContractError::PausedSwap {});
    }

    // Indexing - Make Event for indexing support
    let mut event = Event::new("dexter-vault::swap")
        .add_attribute("pool_id", swap_request.pool_id.to_string())
        .add_attribute(
            "pool_addr",
            pool_info.pool_addr.to_string(),
        )
        .add_attribute("swap_type", swap_request.swap_type.to_string())
        .add_attribute("recipient", recipient.to_string())
        .add_attribute("sender", info.sender.clone());

    // Query - Query Pool Instance  to get the state transition to be handled
    // SwapResponse {}  is the response from the pool contract and has the following parameters,
    // * **trade_params** of type [`Trade`] - Contains `amount_in` and `amount_out` of type [`Uint128`] along-with the `spread`
    // * **response** of type [`response`] - The response type :: Success or Failure
    // * **fee** of type [`Option<Asset>`] - Optional Fee to be charged as fees to the user.  If it is null then no fee is charged
    let pool_swap_transition: dexter::pool::SwapResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: pool_info.pool_addr.to_string(),
            msg: to_binary(&dexter::pool::QueryMsg::OnSwap {
                swap_type: swap_request.swap_type,
                offer_asset: swap_request.asset_in.clone(),
                ask_asset: swap_request.asset_out.clone(),
                amount: swap_request.amount,
                max_spread: swap_request.max_spread,
                belief_price: swap_request.belief_price,
            })?,
        }))?;

    // Error - If the response is failure
    if !pool_swap_transition.response.clone().is_success() {
        return Err(ContractError::PoolQueryFailed {
            error: pool_swap_transition.response.clone().to_string(),
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
        info: swap_request.asset_in.clone(),
        amount: pool_swap_transition.trade_params.amount_in,
    };
    let ask_asset = Asset {
        info: swap_request.asset_out.clone(),
        amount: pool_swap_transition.trade_params.amount_out,
    };

    // Compute - Fee Calculation
    let mut protocol_fee = Uint128::zero();
    let mut dev_fee = Uint128::zero();
    if let Some(fee) = pool_swap_transition.fee.clone() {
        if !fee.amount.is_zero() {
            event = event.add_attribute(
                "fee_asset",
                serde_json_wasm::to_string(&fee.info).unwrap(),
            );
            event = event.add_attribute(
                "total_fee",
                fee.amount.to_string(),
            );
            // Compute - Protocol Fee and dev fee
            (protocol_fee, dev_fee) = pool_config
                .default_fee_info
                .calculate_total_fee_breakup(fee.amount);
        }
    }

    // Compute - Update fee if recipient addresses are not set
    if !pool_config.default_fee_info.developer_addr.is_some() {
        protocol_fee = protocol_fee + dev_fee;
        dev_fee = Uint128::zero();
    }
    if !config.fee_collector.is_some() {
        protocol_fee = Uint128::zero();
    }

    // Error - If the max spend amount is provided, then check if the offer asset amount is less than the max spend amount and if not then return error
    if max_spend.is_some() && max_spend.unwrap() < offer_asset.amount.clone() {
        return Err(ContractError::MaxSpendError {
            max_spend: max_spend.unwrap(),
            offer_amount: offer_asset.amount.clone(),
        });
    }

    // Error - If the min receive amount is provided, then check if the ask asset amount is greater than the min receive amount and if not then return error
    if min_receive.is_some() && min_receive.unwrap() > ask_asset.amount.clone() {
        return Err(ContractError::MinReceiveError {
            min_receive: min_receive.unwrap(),
            ask_amount: ask_asset.amount.clone(),
        });
    }

    // Indexing - Event for indexing support
    event = event
        .add_attribute(
            "offer_asset",
            serde_json_wasm::to_string(&offer_asset.info).unwrap(),
        )
        .add_attribute("offer_amount", offer_asset.amount.to_string())
        .add_attribute(
            "ask_asset",
            serde_json_wasm::to_string(&ask_asset.info).unwrap(),
        )
        .add_attribute("ask_amount", ask_asset.amount.to_string());

    // Update asset balances
    let mut offer_asset_updated: bool = false;
    let mut ask_asset_updated: bool = false;
    let mut execute_msgs: Vec<CosmosMsg> = vec![];

    // Update Loop - We loop through all the assets supported by the pool and do the following,
    for stored_asset in pool_info.assets.iter_mut() {
        // ::: Offer Asset
        if stored_asset.info.as_string() == offer_asset.info.as_string() {
            let act_amount_in = offer_asset.amount.clone();
            // ::: Update State -  Add tokens received to pool balance
            stored_asset.amount = stored_asset.amount.checked_add(act_amount_in)?;
            // ::: Update State - If fee is charged in offer asset, then subtract protocol_fee and dev_fee from pool balance
            if pool_swap_transition
                .fee
                .clone()
                .unwrap()
                .info
                .equal(&offer_asset.info.clone())
            {
                stored_asset.amount = stored_asset.amount.checked_sub(protocol_fee)?;
                stored_asset.amount = stored_asset.amount.checked_sub(dev_fee)?;
            }
            offer_asset_updated = true;

            // ExecuteMsg : Transfer offer asset from user to the vault
            if !offer_asset.is_native_token() {
                execute_msgs.push(build_transfer_cw20_from_user_msg(
                    offer_asset.info.as_string(),
                    info.sender.clone().to_string(),
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
        if stored_asset.info == ask_asset.clone().info {
            // ::: Update State -  Subtract tokens transferred to user from pool balance
            stored_asset.amount = stored_asset.amount.checked_sub(ask_asset.amount.clone())?;
            // ::: Update State - If fee is charged in ask asset, then subtract protocol_fee and dev_fee from pool balance
            if pool_swap_transition
                .fee
                .clone()
                .unwrap()
                .info
                .equal(&ask_asset.info.clone())
            {
                stored_asset.amount = stored_asset.amount.checked_sub(protocol_fee)?;
                stored_asset.amount = stored_asset.amount.checked_sub(dev_fee)?;
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

    event = event.add_attribute("protocol_fee", protocol_fee.to_string());
    event = event.add_attribute("dev_fee", dev_fee.to_string());

    // ExecuteMsg :: Protocol Fee transfer to Keeper contract
    if !protocol_fee.is_zero() {
        execute_msgs.push(
            pool_swap_transition
                .fee
                .clone()
                .unwrap()
                .info
                .create_transfer_msg(config.fee_collector.clone().unwrap(), protocol_fee)?,
        );
    }

    // ExecuteMsg :: Dev Fee transfer to Keeper contract
    if !dev_fee.is_zero() {
        execute_msgs.push(
            pool_swap_transition
                .fee
                .clone()
                .unwrap()
                .info
                .create_transfer_msg(
                    pool_config.default_fee_info.developer_addr.unwrap(),
                    dev_fee,
                )?,
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
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::QueryRegistry { pool_type } => to_binary(&query_registry(deps, pool_type)?),
        QueryMsg::IsGeneratorDisabled { lp_token_addr } => {
            to_binary(&query_is_generator_disabled(deps, lp_token_addr)?)
        }
        QueryMsg::GetPoolById { pool_id } => to_binary(&query_pool_by_id(deps, pool_id)?),
        QueryMsg::GetPoolByAddress { pool_addr } => {
            to_binary(&query_pool_by_addr(deps, pool_addr)?)
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
pub fn query_registry(deps: Deps, pool_type: PoolType) -> StdResult<PoolConfigResponse> {
    let pool_config = REGISTRY.load(deps.storage, pool_type.to_string())
        .or(Err(StdError::generic_err(ContractError::PoolTypeConfigNotFound {}.to_string())))?;
    Ok(Some(pool_config))
}

/// ## Description - Returns boolean value indicating if the genarator is disabled or not for the pool
///
/// ## Params
/// * **pool_id** is the object of type [`Uint128`]. Its the pool id for which the state is requested.
pub fn query_is_generator_disabled(deps: Deps, lp_token_addr: String) -> StdResult<bool> {
    let pool_id = LP_TOKEN_TO_POOL_ID
        .load(deps.storage, lp_token_addr.as_bytes())
        .or(Err(StdError::generic_err(ContractError::LpTokenNotFound {}.to_string())))?;

    let pool_info = ACTIVE_POOLS
        .load(deps.storage, &pool_id.to_string().as_bytes())
        .or(Err(StdError::generic_err(ContractError::InvalidPoolId {}.to_string())))?;

    let pool_type_config = REGISTRY
        .load(deps.storage, pool_info.pool_type.to_string())
        .or(Err(StdError::generic_err(ContractError::PoolTypeConfigNotFound {}.to_string())))?;
    Ok(pool_type_config.is_generator_disabled)
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
        msg: to_binary(&dexter::pool::QueryMsg::PoolId {})?,
    }))?;

    ACTIVE_POOLS.load(deps.storage, pool_id.to_string().as_bytes())
}

// ----------------x----------------x---------------------x-------------------x----------------x----------------
// ----------------x----------------x  :::: VAULT::Migration function   ::::  x----------------x----------------
// ----------------x----------------x---------------------x-------------------x----------------x----------------

/// ## Description - Used for migration of contract. Returns the default object of type [`Response`].
/// ## Params
/// * **_msg** is the object of type [`MigrateMsg`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let contract_version = get_contract_version(deps.storage)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

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
/// Mint LP token to beneficiary or auto deposit into generator if set.
/// # Params
/// * **recipient** is the object of type [`Addr`]. The recipient of the liquidity.
/// * **amount** is the object of type [`Uint128`]. The amount that will be mint to the recipient.
/// * **auto_stake** is the field of type [`bool`]. Determines whether an autostake will be performed on the generator
fn build_mint_lp_token_msg(
    _deps: Deps,
    env: Env,
    lp_token: Addr,
    recipient: Addr,
    amount: Uint128,
    auto_stake: bool,
    auto_stake_impl: Option<AutoStakeImpl>,
    multistaking_address: Option<Addr>,
    generator: Option<Addr>,
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

    let mut msgs = vec![CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_token.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Mint {
            recipient: env.contract.address.to_string(),
            amount,
        })?,
        funds: vec![],
    })];

    // Safe to do since it is validated at the caller
    let auto_stake_impl = auto_stake_impl.unwrap();
    let msg = match auto_stake_impl {
        AutoStakeImpl::Generator => {
            // Address of generator
            let generator = generator.clone().unwrap();
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: lp_token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: generator.to_string(),
                    amount,
                    msg: to_binary(&dexter::generator::Cw20HookMsg::DepositFor {
                        beneficiary: recipient,
                    })?,
                })?,
                funds: vec![],
            })
        }
        AutoStakeImpl::Multistaking => {
            // Address of multistaking
            let multistaking_address = multistaking_address.unwrap();
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: lp_token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: multistaking_address.to_string(),
                    amount,
                    msg: to_binary(&dexter::multi_staking::Cw20HookMsg::BondForBeneficiary {
                        beneficiary: recipient,
                    })?,
                })?,
                funds: vec![],
            })
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
        msg: to_binary(&dexter::pool::ExecuteMsg::UpdateLiquidity { assets })?,
    }))
}
