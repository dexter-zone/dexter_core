use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut,
    Env, Event, MessageInfo, QueryRequest, Reply, ReplyOn, Response, StdError, StdResult, SubMsg,
    Uint128, WasmMsg, WasmQuery,
};
use protobuf::Message;
use std::collections::HashSet;

use crate::error::ContractError;
use crate::response::MsgInstantiateContractResponse;
use crate::state::{
    ACTIVE_POOLS, CONFIG, LP_TOKEN_TO_POOL_ID, OWNERSHIP_PROPOSAL, REGISTRY, TMP_POOL_INFO,
};

use dexter::asset::{addr_opt_validate, addr_validate_to_lower, Asset, AssetInfo};
use dexter::helper::{
    build_send_native_asset_msg, build_transfer_cw20_from_user_msg, build_transfer_cw20_token_msg,
    build_transfer_token_to_user_msg, claim_ownership, drop_ownership_proposal,
    find_sent_native_token_balance, get_lp_token_name, get_lp_token_symbol, is_valid_name,
    is_valid_symbol, propose_new_owner,
};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{FeeStructs, InstantiateMsg as PoolInstantiateMsg};
use dexter::vault::{
    Config, ConfigResponse, Cw20HookMsg, ExecuteMsg, FeeInfo, InstantiateMsg, MigrateMsg,
    PoolConfig, PoolConfigResponse, PoolInfo, PoolInfoResponse, PoolType, QueryMsg,
    SingleSwapRequest,
};

use cw2::{get_contract_version, set_contract_version};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter-vault";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// A `reply` call code ID of sub-message.
const INSTANTIATE_POOL_REPLY_ID: u64 = 1;
const INSTANTIATE_LP_REPLY_ID: u64 = 2;

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

    let config = Config {
        owner: addr_validate_to_lower(deps.api, &msg.owner)?,
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
        if !pc.fee_info.valid_fee_info() {
            return Err(ContractError::InvalidFeeInfo {});
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
            new_fee_info,
            is_generator_disabled,
        } => execute_update_pool_config(
            deps,
            info,
            pool_type,
            is_disabled,
            new_fee_info,
            is_generator_disabled,
        ),
        ExecuteMsg::AddToRegistry { new_pool_config } => {
            execute_add_to_registry(deps, env, info, new_pool_config)
        }
        ExecuteMsg::CreatePoolInstance {
            pool_type,
            asset_infos,
            lp_token_name,
            lp_token_symbol,
            init_params,
        } => execute_create_pool_instance(
            deps,
            env,
            pool_type,
            asset_infos,
            lp_token_name,
            lp_token_symbol,
            init_params,
        ),
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
        } => execute_swap(deps, env, info, swap_request, recipient),
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
            burn_amount,
        } => {
            // Check if amount is valid or not
            if burn_amount.is_some() && burn_amount.unwrap() > amount_transferred {
                return Err(ContractError::InvalidAmount {});
            }

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
    generator_address: Option<String>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender.clone() != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Update fee collector
    if let Some(fee_collector) = fee_collector {
        config.fee_collector = Some(addr_validate_to_lower(deps.api, fee_collector.as_str())?);
    }

    // Set generator only if its not set
    if !config.generator_address.is_some() {
        if let Some(generator_address) = generator_address {
            config.generator_address = Some(addr_validate_to_lower(
                deps.api,
                generator_address.as_str(),
            )?);
        }
    }

    // Update LP token code id
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
/// * **is_disabled** Optional parameter. If set to `true`, the instantiation of new pool instances will be disabled. If set to `false`, they will be enabled.
/// * **is_generator_disabled**  Optional parameter. If set to `true`, the generator will not be able to support
///
/// ## Executor
/// Only owner or the Pool's developer address can execute it
pub fn execute_update_pool_config(
    deps: DepsMut,
    info: MessageInfo,
    pool_type: PoolType,
    is_disabled: Option<bool>,
    new_fee_info: Option<FeeInfo>,
    is_generator_disabled: Option<bool>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut pool_config = REGISTRY
        .load(deps.storage, pool_type.to_string())
        .map_err(|_| ContractError::PoolConfigNotFound {})?;

    // permission check :: If developer address is set then only developer can call this function
    if pool_config.fee_info.developer_addr.is_some() {
        if info.sender.clone() != pool_config.fee_info.developer_addr.clone().unwrap() {
            return Err(ContractError::Unauthorized {});
        }
    }
    // permission check :: If developer address is not set then only owner can call this function
    else {
        if info.sender.clone() != config.owner {
            return Err(ContractError::Unauthorized {});
        }
    }

    // Disable or enable pool instances creation
    if let Some(is_disabled) = is_disabled {
        pool_config.is_disabled = is_disabled;
    }

    // Disable or enable integration with dexter generator
    if let Some(is_generator_disabled) = is_generator_disabled {
        pool_config.is_generator_disabled = is_generator_disabled;
    }

    // Update fee info
    if let Some(new_fee_info) = new_fee_info {
        if !new_fee_info.valid_fee_info() {
            return Err(ContractError::InvalidFeeInfo {});
        }
        pool_config.fee_info = new_fee_info;
    }

    // Save pool config
    REGISTRY.save(
        deps.storage,
        pool_config.pool_type.to_string(),
        &pool_config,
    )?;

    Ok(Response::new().add_attribute("action", "update_pool_config"))
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
    new_pool_config: PoolConfig,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    // permission check : Only owner can execute it
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if new_pool_config.code_id == 0 {
        return Err(ContractError::InvalidCodeId {});
    }

    // Check :: If pool type is already registered
    let mut pool_config = REGISTRY
        .load(deps.storage, new_pool_config.pool_type.to_string())
        .unwrap_or_default();
    if pool_config.code_id != 0u64 {
        return Err(ContractError::PoolTypeAlreadyExists {});
    }

    // Set pool config
    pool_config = new_pool_config;

    // validate fee bps limits
    if !pool_config.fee_info.valid_fee_info() {
        return Err(ContractError::InvalidFeeInfo {});
    }

    // Save pool config
    REGISTRY.save(
        deps.storage,
        pool_config.pool_type.to_string(),
        &pool_config,
    )?;

    // Emit Event
    let event = Event::new("dexter-vault::add_new_pool")
        .add_attribute("pool_type", pool_config.pool_type.to_string())
        .add_attribute("code_id", pool_config.code_id.to_string());
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
    pool_type: PoolType,
    mut asset_infos: Vec<AssetInfo>,
    lp_token_name: Option<String>,
    lp_token_symbol: Option<String>,
    init_params: Option<Binary>,
) -> Result<Response, ContractError> {
    // Sort Assets List
    asset_infos.sort_by(|a, b| {
        a.to_string()
            .to_lowercase()
            .cmp(&b.to_string().to_lowercase())
    });

    let mut assets: Vec<Asset> = vec![];
    let mut event = Event::new("dexter-vault::add_pool");

    // Check asset definations and make sure no asset is repeated
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

    if !lp_token_name.clone().is_none() && !is_valid_name(lp_token_name.as_ref().unwrap()) {
        return Err(ContractError::InvalidLpTokenName {});
    }
    if !lp_token_symbol.is_none() && !is_valid_symbol(&lp_token_symbol.as_ref().unwrap()) {
        return Err(ContractError::InvalidLpTokenSymbol {});
    }

    let config = CONFIG.load(deps.storage)?;

    // Get current pool's config from stored pool configs
    let pool_config = REGISTRY
        .load(deps.storage, pool_type.to_string())
        .map_err(|_| ContractError::PoolConfigNotFound {})?;

    // Check if pool config is disabled
    if pool_config.is_disabled {
        return Err(ContractError::PoolConfigDisabled {});
    }

    // Pool Id for the new pool instance
    let pool_id = config.next_pool_id;

    let tmp_pool_info = PoolInfo {
        pool_id: pool_id,
        pool_addr: None,
        lp_token_addr: None,
        assets: assets,
        pool_type: pool_config.pool_type.clone(),
        developer_addr: pool_config.fee_info.clone().developer_addr,
    };

    // Store the temporary Pool Info
    TMP_POOL_INFO.save(deps.storage, &tmp_pool_info)?;

    // LP Token Name
    let token_name = get_lp_token_name(pool_id.clone(), lp_token_name.clone());
    // LP Token Symbol
    let token_symbol = get_lp_token_symbol(lp_token_symbol.clone());

    // Emit Event
    event = event
        .add_attribute("pool_type", pool_type.to_string())
        .add_attribute("pool_id", pool_id.to_string())
        .add_attribute("lp_token_name", token_name.clone())
        .add_attribute("lp_token_symbol", token_symbol.clone())
        .add_attribute(
            "total_fee_bps",
            pool_config.fee_info.total_fee_bps.clone().to_string(),
        )
        .add_attribute(
            "protocol_fee_percent",
            pool_config
                .fee_info
                .protocol_fee_percent
                .clone()
                .to_string(),
        )
        .add_attribute(
            "developer_fee_percent",
            pool_config.fee_info.dev_fee_percent.clone().to_string(),
        );

    // Sub Msg to initialize the pool instance
    let init_pool_sub_msg: SubMsg = SubMsg {
        id: INSTANTIATE_POOL_REPLY_ID,
        msg: WasmMsg::Instantiate {
            admin: Some(config.owner.to_string()),
            code_id: pool_config.code_id,
            msg: to_binary(&PoolInstantiateMsg {
                pool_id: pool_id,
                pool_type: pool_config.pool_type,
                vault_addr: env.contract.address.clone(),
                asset_infos: asset_infos.clone(),
                fee_info: FeeStructs {
                    total_fee_bps: pool_config.fee_info.total_fee_bps,
                },
                init_params,
            })?,
            funds: vec![],
            label: "dexter-pool-".to_string() + &pool_id.to_string(),
        }
        .into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    };

    // Sub Msg to initialize the LP token instance
    let init_lp_token_sub_msg: SubMsg = SubMsg {
        id: INSTANTIATE_LP_REPLY_ID,
        msg: WasmMsg::Instantiate {
            admin: None,
            code_id: config.lp_token_code_id,
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
        .add_submessages([init_pool_sub_msg, init_lp_token_sub_msg])
        .add_event(event))
}

/// # Description
/// The entry point to the contract for processing the reply from the submessage
/// # Params
/// * **msg** is the object of type [`Reply`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
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
        // Reply from the submessage to instantiate the pool instance
        INSTANTIATE_POOL_REPLY_ID => {
            // Update the pool address in the temporary pool info
            tmp_pool_info.pool_addr = Some(addr_validate_to_lower(
                deps.api,
                res.get_contract_address(),
            )?);
            response = response.add_attributes(vec![
                attr("action", "reply"),
                attr("pool_addr", tmp_pool_info.clone().pool_addr.unwrap()),
            ]);

            event = event.add_attribute("pool_id", tmp_pool_info.pool_id);

            // Store the temporary Pool Info
            TMP_POOL_INFO.save(deps.storage, &tmp_pool_info)?;
            event = event.add_attribute("pool_addr", tmp_pool_info.clone().pool_addr.unwrap());
        }
        // Reply from the submessage to instantiate the LP token instance
        INSTANTIATE_LP_REPLY_ID => {
            // Update the LP token address in the temporary pool info
            tmp_pool_info.lp_token_addr = Some(addr_validate_to_lower(
                deps.api,
                res.get_contract_address(),
            )?);
            response = response.add_attributes(vec![
                attr("action", "reply"),
                attr(
                    "lp_token_addr",
                    tmp_pool_info.lp_token_addr.clone().unwrap(),
                ),
            ]);
            response = response.add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: tmp_pool_info.pool_addr.clone().unwrap().to_string(),
                funds: vec![],
                msg: to_binary(&dexter::pool::ExecuteMsg::SetLpToken {
                    lp_token_addr: tmp_pool_info.lp_token_addr.clone().unwrap(),
                })?,
            }));
            // Store the temporary Pool Info
            TMP_POOL_INFO.save(deps.storage, &tmp_pool_info)?;
            // Store LP token addr _> Pool Id mapping in the LP token map
            LP_TOKEN_TO_POOL_ID.save(
                deps.storage,
                &tmp_pool_info.lp_token_addr.clone().unwrap().as_bytes(),
                &tmp_pool_info.pool_id.clone(),
            )?;
            event = event.add_attribute(
                "lp_token_addr",
                tmp_pool_info.clone().lp_token_addr.unwrap(),
            );

            // Update the next pool id in the config and save it
            let mut config = CONFIG.load(deps.storage)?;
            config.next_pool_id = config.next_pool_id.checked_add(Uint128::from(1u128))?;
            CONFIG.save(deps.storage, &config)?;
        }
        _ => {
            return Err(ContractError::InvalidSubMsgId {});
        }
    }

    // Save the temporary pool info as permanent pool info mapped with the Pool Id
    ACTIVE_POOLS.save(
        deps.storage,
        &tmp_pool_info.pool_id.to_string().as_bytes(),
        &tmp_pool_info,
    )?;

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
/// * **auto_stakes** Optional parameter. If provided, the Vault will automatically stake the provided assets with the generator contract.
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
    let config = CONFIG.load(deps.storage)?;

    // Load the pool info from the storage
    let mut pool_info = ACTIVE_POOLS
        .load(deps.storage, pool_id.to_string().as_bytes())
        .expect("Invalid Pool Id");

    // Query Pool Instance for Math Operations --> Returns response type (success or failure), number of LP shares to be minted and a `sorted` list of Assets which are to be transferred to the Vault by the user
    let after_join_res: dexter::pool::AfterJoinResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: pool_info.pool_addr.clone().unwrap().to_string(),
            msg: to_binary(&dexter::pool::QueryMsg::OnJoinPool {
                assets_in: assets_in.clone(),
                mint_amount: lp_to_mint,
                slippage_tolerance: slippage_tolerance.clone(),
            })?,
        }))?;

    // If the response is failure
    if !after_join_res.response.is_success() || after_join_res.new_shares.is_zero() {
        return Err(ContractError::PoolQueryFailed {
            error: after_join_res.response.to_string(),
        });
    }

    // Number of Assets should match
    if after_join_res.provided_assets.len() != pool_info.assets.len() {
        return Err(ContractError::InvalidNumberOfAssets {});
    }

    // Number of LP tokens to be minted
    let new_shares = after_join_res.new_shares;

    // Emit Event
    let mut event = Event::new("dexter-vault::join_pool")
        .add_attribute("pool_id", pool_id.to_string())
        .add_attribute(
            "pool_addr",
            pool_info.pool_addr.clone().unwrap().to_string(),
        )
        .add_attribute("lp_tokens_minted", new_shares.to_string());

    // Fee Calculation
    let mut protocol_fee = Uint128::zero();
    let mut dev_fee = Uint128::zero();
    if after_join_res.fee.clone().is_some() && !after_join_res.fee.clone().unwrap().amount.is_zero()
    {
        event = event.add_attribute(
            "fee_asset",
            serde_json_wasm::to_string(&after_join_res.fee.clone().unwrap().info.to_string())
                .unwrap(),
        );
        event = event.add_attribute(
            "total_fee",
            after_join_res.fee.clone().unwrap().amount.to_string(),
        );
        let pool_config = REGISTRY.load(deps.storage, pool_info.pool_type.to_string())?;
        (protocol_fee, dev_fee) = pool_config
            .fee_info
            .calculate_total_fee_breakup(after_join_res.fee.clone().unwrap().amount.clone());

        // Protocol fee = 0 if keeper address is not set
        if !config.fee_collector.is_some() {
            protocol_fee = Uint128::zero();
        }
    }

    // // Dev fee = 0 is dev receiver is not set
    if !pool_info.developer_addr.is_some() {
        dev_fee = Uint128::zero();
    }

    let mut execute_msgs: Vec<CosmosMsg> = vec![];

    // Update asset balances
    let mut index = 0;
    for stored_asset in pool_info.assets.iter_mut() {
        // the returned list of assets needs to be sorted in the same order as the stored list of assets
        if stored_asset.info != after_join_res.provided_assets[index].info {
            return Err(ContractError::InvalidSequenceOfAssets {});
        }
        // Number of tokens to be transferred to the Vault
        let to_transfer = after_join_res.provided_assets[index].amount;

        // If number of tokens to transfer > 0, then
        // - Update stored pool's asset balances in `PoolInfo` Struct
        // - Transfer net calculated CW20 tokens from user to the Vault
        // - Return native tokens to the user (which are to be returned)
        if !to_transfer.is_zero() {
            let mut act_asset_in = to_transfer;
            if after_join_res.fee.is_some()
                && !after_join_res.fee.clone().unwrap().amount.is_zero()
                && after_join_res
                    .fee
                    .clone()
                    .unwrap()
                    .info
                    .eq(&stored_asset.info.clone())
            {
                act_asset_in = to_transfer.checked_sub(protocol_fee + dev_fee).unwrap();
            }
            // PoolInfo State update - Add number of tokens to be transferred to the stored pool state
            stored_asset.amount = stored_asset.amount.checked_add(act_asset_in)?;
            // Token Transfers
            if !stored_asset.info.is_native_token() {
                // Transfer Number of CW tokens = Pool Math instructs that the user needs to provide this number of tokens to the Vault
                execute_msgs.push(build_transfer_cw20_from_user_msg(
                    stored_asset.info.as_string(),
                    info.sender.clone().to_string(),
                    env.contract.address.to_string(),
                    to_transfer,
                )?);
            } else {
                // Get number of native tokens that were sent
                let tokens_sent =
                    find_sent_native_token_balance(&info, &stored_asset.info.as_string());
                // Return the extra native tokens sent by the user to the Vault
                if tokens_sent > after_join_res.provided_assets[index].amount {
                    execute_msgs.push(build_send_native_asset_msg(
                        info.sender.clone(),
                        &after_join_res.provided_assets[index].info.as_string(),
                        tokens_sent.checked_sub(after_join_res.provided_assets[index].amount)?,
                    )?);
                }
                // Return error if insufficient number of tokens were sent
                else if tokens_sent < after_join_res.provided_assets[index].amount {
                    return Err(ContractError::InsufficientNativeTokensSent {
                        denom: after_join_res.provided_assets[index].info.to_string(),
                        sent: tokens_sent,
                        needed: after_join_res.provided_assets[index].amount,
                    });
                }
            }
        }
        // Increment Index
        index = index + 1;
    }
    event = event.add_attribute(
        "provided_assets",
        serde_json_wasm::to_string(&after_join_res.provided_assets).unwrap(),
    );

    let config = CONFIG.load(deps.storage)?;

    // LP Token recipient
    let recipient: Addr;
    if auto_stake.is_some() && auto_stake.unwrap() {
        recipient = config
            .generator_address
            .clone()
            .expect("Generator address not set");
    } else {
        recipient = addr_validate_to_lower(
            deps.api,
            op_recipient.unwrap_or(info.sender.to_string()).as_str(),
        )?;
    }
    event = event.add_attribute("recipient", recipient.to_string());

    // Pool State Update Execution :: Send Updated pool state to the Pool Contract so it can do its internal computes
    execute_msgs.push(build_update_pool_state_msg(
        pool_info.pool_addr.clone().unwrap().to_string(),
        pool_info.assets.clone(),
    )?);

    // Mint LP Tokens
    let mint_msgs = build_mint_lp_token_msg(
        deps.as_ref(),
        env.clone(),
        pool_info.lp_token_addr.clone().unwrap(),
        recipient,
        new_shares,
        config.generator_address.clone(),
        auto_stake.unwrap_or(false),
    )?;
    for msg in mint_msgs {
        execute_msgs.push(msg);
    }

    // Execute Msg :: Protocol Fee transfer to Keeper contract
    if !protocol_fee.is_zero() {
        execute_msgs.push(build_transfer_token_to_user_msg(
            after_join_res.fee.clone().unwrap().info.clone(),
            config.fee_collector.clone().unwrap(),
            protocol_fee.clone(),
        )?);
        event = event.add_attribute("protocol_fee", protocol_fee.to_string())
    }

    // Execute Msg :: Dev Fee transfer to dev address
    if !dev_fee.is_zero() {
        execute_msgs.push(build_transfer_token_to_user_msg(
            after_join_res.fee.clone().unwrap().info.clone(),
            pool_info.developer_addr.clone().unwrap(),
            dev_fee.clone(),
        )?);
        event = event.add_attribute("dev_fee", dev_fee.to_string())
    }

    // Save the updated pool state to the storage
    ACTIVE_POOLS.save(deps.storage, &pool_id.to_string().as_bytes(), &pool_info)?;

    Ok(Response::new()
        .add_messages(execute_msgs)
        .add_attribute("action", "dexter-vault/execute/join_pool")
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
    let config = CONFIG.load(deps.storage)?;

    // Load the pool info from the storage
    let mut pool_info = ACTIVE_POOLS
        .load(deps.storage, pool_id.to_string().as_bytes())
        .expect("Invalid Pool Id");

    // Check if the LP token sent is valid
    if info.sender != pool_info.lp_token_addr.clone().unwrap() {
        return Err(ContractError::Unauthorized {});
    }

    // Query Pool Instance for Math Operations --> Returns response type (success or failure), number of LP shares to be burned and the `sorted` list of Assets which are to be transfred to the user
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
        return Err(ContractError::PoolQueryFailed {
            error: after_burn_res.response.to_string(),
        });
    }

    // Number of LP shares to be returned to the user
    let lp_to_return: Uint128;

    // Check : Lp token to burn > Lp tokens transferred by the user
    if after_burn_res.burn_shares > burn_amount.unwrap() {
        return Err(ContractError::InsufficientLpTokensToExit {});
    } else {
        lp_to_return = burn_amount
            .unwrap()
            .checked_sub(after_burn_res.burn_shares)?;
    }

    let mut execute_msgs: Vec<CosmosMsg> = vec![];

    let mut event = Event::new("dexter-vault::exit_pool")
        .add_attribute("pool_id", pool_id.to_string())
        .add_attribute(
            "pool_addr",
            pool_info.pool_addr.clone().unwrap().to_string(),
        )
        .add_attribute("lp_tokens_burnt", after_burn_res.burn_shares.to_string());

    // Fee Calculation
    let mut protocol_fee = Uint128::zero();
    let mut dev_fee = Uint128::zero();
    if after_burn_res.fee.clone().is_some() && !after_burn_res.fee.clone().unwrap().amount.is_zero()
    {
        event = event.add_attribute(
            "fee_asset",
            serde_json_wasm::to_string(&after_burn_res.fee.clone().unwrap().info.to_string())
                .unwrap(),
        );
        event = event.add_attribute(
            "total_fee",
            after_burn_res.fee.clone().unwrap().amount.to_string(),
        );
        let pool_config = REGISTRY.load(deps.storage, pool_info.pool_type.to_string())?;
        (protocol_fee, dev_fee) = pool_config
            .fee_info
            .calculate_total_fee_breakup(after_burn_res.fee.clone().unwrap().amount.clone());

        // Protocol fee = 0 if keeper address is not set
        if !config.fee_collector.is_some() {
            protocol_fee = Uint128::zero();
        }
    }

    // recipient address
    let recipient_addr = addr_validate_to_lower(deps.api, &recipient)?;

    let mut assets_out = vec![];

    // Update asset balances & transfer tokens WasmMsgs
    let mut index = 0;
    for stored_asset in pool_info.assets.iter_mut() {
        // If sequence of tokens doesn't match
        if stored_asset.info != after_burn_res.assets_out[index].info.clone() {
            return Err(ContractError::InvalidSequenceOfAssets {});
        }
        // Number of tokens to be transferred to the recipient: As instructed by the Pool Math
        let to_transfer = after_burn_res.assets_out[index].amount.clone();
        // Asset amount to actually account for after deducting the fees
        let mut act_asset_out = to_transfer;
        if after_burn_res.fee.is_some()
            && !after_burn_res.fee.clone().unwrap().amount.is_zero()
            && after_burn_res
                .fee
                .clone()
                .unwrap()
                .info
                .eq(&stored_asset.info.clone())
        {
            act_asset_out = to_transfer.checked_add(protocol_fee + dev_fee).unwrap();
        }
        // If number of tokens to transfer > 0, then
        // - Update stored pool's asset balances in `PoolInfo` Struct
        // - Transfer tokens to the recipient
        if !to_transfer.is_zero() {
            // PoolInfo State update -
            stored_asset.amount = stored_asset.amount.checked_sub(act_asset_out)?;
            // Token Transfers
            if !stored_asset.info.is_native_token() {
                // Transfer Number of CW tokens the Pool Math instructs to return
                execute_msgs.push(build_transfer_cw20_token_msg(
                    recipient_addr.clone(),
                    stored_asset.info.as_string(),
                    to_transfer,
                )?);
            } else {
                // Transfer Number of Native tokens the Pool Math instructs to return
                execute_msgs.push(build_send_native_asset_msg(
                    recipient_addr.clone(),
                    &after_burn_res.assets_out[index].info.as_string(),
                    to_transfer,
                )?);
            }

            let asset_out = after_burn_res.assets_out[index].clone();
            assets_out.push(asset_out);
        }
        // Increment Index
        index = index + 1;
    }

    let assets_out_json = serde_json_wasm::to_string(&assets_out).unwrap();
    event = event.add_attribute("assets_out", assets_out_json);
    event = event.add_attribute("sender", sender);
    event = event.add_attribute("vault_contract_address", env.contract.address);
    // Burn LP Tokens
    execute_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: pool_info.lp_token_addr.clone().unwrap().to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Burn {
            amount: after_burn_res.burn_shares.clone(),
        })?,
        funds: vec![],
    }));

    // Return LP shares in case some of the LP tokens transferred are to be returned
    if !lp_to_return.is_zero() {
        execute_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: pool_info.lp_token_addr.clone().unwrap().to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                amount: lp_to_return,
                recipient: info.sender.to_string(),
            })?,
            funds: vec![],
        }));
    }

    // Execute Msg :: Protocol Fee transfer to Keeper contract
    if !protocol_fee.is_zero() {
        execute_msgs.push(build_transfer_token_to_user_msg(
            after_burn_res.fee.clone().unwrap().info.clone(),
            config.fee_collector.clone().unwrap(),
            protocol_fee.clone(),
        )?);
        event = event.add_attribute("protocol_fee", protocol_fee.to_string())
    }

    // Execute Msg :: Dev Fee transfer to dev address
    if !dev_fee.is_zero() {
        execute_msgs.push(build_transfer_token_to_user_msg(
            after_burn_res.fee.clone().unwrap().info.clone(),
            pool_info.developer_addr.clone().unwrap(),
            dev_fee.clone(),
        )?);
        event = event.add_attribute("dev_fee", dev_fee.to_string())
    }

    event = event.add_attribute("recipient_addr", recipient_addr.to_string());

    // Pool State Update Execution :: Send Updated pool state to the Pool Contract so it can do its internal computes
    execute_msgs.push(build_update_pool_state_msg(
        pool_info.pool_addr.clone().unwrap().to_string(),
        pool_info.assets.clone(),
    )?);
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
/// * **limit** Optional parameter. Minimum tokens to receive if swap is of type GiveIn or maximum tokens to give if swap is of type GiveOut. If not provided, then the default value is 0.
/// * **op_recipient** Optional parameter. Recipient address of the swap tx. If not provided, then the default value is the sender address.
pub fn execute_swap(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    swap_request: SingleSwapRequest,
    op_recipient: Option<String>,
) -> Result<Response, ContractError> {
    // Load Pool Info from Storage
    let mut pool_info = ACTIVE_POOLS
        .load(deps.storage, swap_request.pool_id.to_string().as_bytes())
        .expect("Invalid Pool Id");

    let config = CONFIG.load(deps.storage)?;

    // Amount cannot be zero
    if swap_request.amount.is_zero() {
        return Err(ContractError::InvalidAmount {});
    }

    // AssetInfo's cannot be same
    if swap_request.asset_in == swap_request.asset_out {
        return Err(ContractError::SameTokenError {});
    }

    // Make Event for indexing support
    let mut event = Event::new("dexter-vault::swap")
        .add_attribute("pool_id", swap_request.pool_id.to_string())
        .add_attribute(
            "pool_addr",
            pool_info.pool_addr.clone().unwrap().to_string(),
        )
        .add_attribute("swap_type", swap_request.swap_type.to_string());

    // Query Pool Instance for Math Operations --> Returns response type (success or failure), and the Trade struct containing trade related info
    let swap_response: dexter::pool::SwapResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: pool_info.pool_addr.clone().unwrap().to_string(),
            msg: to_binary(&dexter::pool::QueryMsg::OnSwap {
                swap_type: swap_request.swap_type,
                offer_asset: swap_request.asset_in.clone(),
                ask_asset: swap_request.asset_out.clone(),
                amount: swap_request.amount,
                max_spread: swap_request.max_spread,
                belief_price: swap_request.belief_price,
            })?,
        }))?;

    // If the response is failure
    if !swap_response.response.clone().is_success() {
        return Err(ContractError::PoolQueryFailed {
            error: swap_response.response.clone().to_string(),
        });
    }

    // Create offer and ask assets
    let offer_asset = Asset {
        info: swap_request.asset_in.clone(),
        amount: swap_response.trade_params.amount_in,
    };
    let ask_asset = Asset {
        info: swap_request.asset_out.clone(),
        amount: swap_response.trade_params.amount_out,
    };

    // Fee Calculation
    let mut protocol_fee = Uint128::zero();
    let mut dev_fee = Uint128::zero();
    if swap_response.fee.clone().is_some() && !swap_response.fee.clone().unwrap().amount.is_zero() {
        event = event.add_attribute(
            "fee_asset",
            serde_json_wasm::to_string(&swap_response.fee.clone().unwrap().info).unwrap(),
        );
        event = event.add_attribute(
            "total_fee",
            swap_response.fee.clone().unwrap().amount.to_string(),
        );
        let pool_config = REGISTRY.load(deps.storage, pool_info.pool_type.to_string())?;
        (protocol_fee, dev_fee) = pool_config
            .fee_info
            .calculate_total_fee_breakup(swap_response.fee.clone().unwrap().amount);
    }

    // Protocol fee = 0 if keeper address is not set
    if !config.fee_collector.is_some() {
        protocol_fee = Uint128::zero();
    }

    // // Dev fee = 0 is dev receiver is not set
    if !pool_info.developer_addr.is_some() {
        dev_fee = Uint128::zero();
    }

    // Event for indexing support
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

    // recipient address
    let mut recipient = info.sender.clone();
    if !op_recipient.is_none() {
        recipient = addr_validate_to_lower(
            deps.api,
            op_recipient
                .unwrap_or(info.sender.clone().to_string())
                .as_str(),
        )?;
    }

    event = event.add_attribute("recipient", recipient.to_string());
    event = event.add_attribute("sender", info.sender.clone());

    // Update asset balances
    let mut index = 0;
    let mut offer_asset_updated: bool = false;
    let mut ask_asset_updated: bool = false;
    let mut execute_msgs: Vec<CosmosMsg> = vec![];

    // Execute Swap Msgs and state update operations
    for stored_asset in pool_info.assets.iter_mut() {
        // Update state : Offer Asset
        if stored_asset.info == offer_asset.info {
            let mut act_amount_in = offer_asset.amount.clone();
            // If fee is charged in offer asset, then deduct the fee from offer asset amount to be sent to pool as it is transferred to Keeper and dev as part of the fee
            if swap_response
                .fee
                .clone()
                .unwrap()
                .info
                .equal(&offer_asset.info.clone())
            {
                act_amount_in = act_amount_in.checked_sub(protocol_fee + dev_fee)?;
            }
            stored_asset.amount = stored_asset.amount.checked_add(act_amount_in)?;
            offer_asset_updated = true;

            // Execute Msgs : Transfer offer asset from user to the vault
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
        if stored_asset.info == ask_asset.clone().info {
            let mut act_amount_out = ask_asset.amount.clone();
            // If fee is charged in offer asset, then deduct the fee from offer asset amount to be sent to pool as it is transferred to Keeper and dev as part of the fee
            if swap_response
                .fee
                .clone()
                .unwrap()
                .info
                .equal(&ask_asset.info.clone())
            {
                act_amount_out = act_amount_out.checked_add(protocol_fee + dev_fee)?;
            }
            // Update state : Ask Asset :: Fee charged in Ask Asset
            stored_asset.amount = stored_asset.amount.checked_sub(act_amount_out)?;
            ask_asset_updated = true;

            // Execute Msgs : Transfer tokens from Vault to the recipient
            execute_msgs.push(ask_asset.clone().into_msg(recipient.clone())?);
        }
        // Increment Index
        index = index + 1;
    }

    // Error if something is wrong with state update operations
    if !offer_asset_updated || !ask_asset_updated {
        return Err(ContractError::MismatchedAssets {});
    }

    // Update PoolInfo stored state
    ACTIVE_POOLS.save(
        deps.storage,
        &swap_request.pool_id.to_string().as_bytes(),
        &pool_info,
    )?;

    // Execute Msgs :: Update Pool Instance state
    execute_msgs.push(build_update_pool_state_msg(
        pool_info.pool_addr.unwrap().to_string(),
        pool_info.assets,
    )?);

    // Execute Msg :: Protocol Fee transfer to Keeper contract
    if !protocol_fee.is_zero() {
        execute_msgs.push(build_transfer_token_to_user_msg(
            swap_response.fee.clone().unwrap().info.clone(),
            config.fee_collector.clone().unwrap(),
            protocol_fee.clone(),
        )?);
        event = event.add_attribute("protocol_fee", protocol_fee.to_string())
    }

    // Execute Msg :: Dev Fee transfer to Keeper contract
    if !dev_fee.is_zero() {
        execute_msgs.push(build_transfer_token_to_user_msg(
            swap_response.fee.clone().unwrap().info.clone(),
            pool_info.developer_addr.unwrap(),
            dev_fee.clone(),
        )?);
        event = event.add_attribute("dev_fee", dev_fee.to_string())
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
        QueryMsg::QueryRegistry { pool_type } => to_binary(&query_rigistery(deps, pool_type)?),
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
    let resp = ConfigResponse {
        owner: config.owner,
        lp_token_code_id: config.lp_token_code_id,
        fee_collector: config.fee_collector,
        generator_address: config.generator_address,
    };
    Ok(resp)
}

/// ## Description - Returns the [`PoolType`]'s Configuration settings  in custom [`PoolConfigResponse`] structure
///
/// ## Params
/// * **pool_type** is the object of type [`PoolType`]. Its the pool type for which the configuration is requested.
pub fn query_rigistery(deps: Deps, pool_type: PoolType) -> StdResult<PoolConfigResponse> {
    let pool_config = REGISTRY
        .load(deps.storage, pool_type.to_string())
        .unwrap_or_default();
    Ok(pool_config)
}

/// ## Description - Returns boolean value indicating if the genarator is disabled or not for the pool
///
/// ## Params
/// * **pool_id** is the object of type [`Uint128`]. Its the pool id for which the state is requested.
pub fn query_is_generator_disabled(deps: Deps, lp_token_addr: String) -> StdResult<bool> {
    let pool_id = LP_TOKEN_TO_POOL_ID
        .may_load(deps.storage, lp_token_addr.as_bytes())?
        .expect("The LP token address does not belong to any pool");

    let pool_info = ACTIVE_POOLS
        .load(deps.storage, &pool_id.to_string().as_bytes())
        .expect("Invalid Pool Id");

    let pool_config = REGISTRY
        .load(deps.storage, pool_info.pool_type.to_string())
        .unwrap_or_default();
    Ok(pool_config.is_generator_disabled)
}

/// ## Description - Returns the current stored state of the Pool in custom [`PoolInfoResponse`] structure
///
/// ## Params
/// * **pool_id** is the object of type [`Uint128`]. Its the pool id for which the state is requested.
pub fn query_pool_by_id(deps: Deps, pool_id: Uint128) -> StdResult<PoolInfoResponse> {
    let pool_info = ACTIVE_POOLS
        .load(deps.storage, pool_id.to_string().as_bytes())
        .unwrap();
    Ok(pool_info)
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
    let pool_info = ACTIVE_POOLS
        .load(deps.storage, pool_id.to_string().as_bytes())
        .unwrap();
    Ok(pool_info)
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
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: lp_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: generator.unwrap().to_string(),
                amount,
                msg: to_binary(&dexter::generator::Cw20HookMsg::DepositFor(recipient))?,
            })?,
            funds: vec![],
        }),
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
