use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut,
    Env, MessageInfo, Order, QuerierWrapper, Reply, ReplyOn, Response, StdError, StdResult, SubMsg,
    Uint128, Uint64, WasmMsg, Attribute
};
use std::collections::{HashMap, HashSet};
use protobuf::Message;

use cw2::{get_contract_version, set_contract_version};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg, MinterResponse};


use dexter::asset::{addr_validate_to_lower, token_asset_info, Asset, AssetInfo,};
use dexter::DecimalCheckedOps;
use dexter::generator::{UserInfo, PoolInfo , RestrictedVector, UnbondingInfo};
use dexter::querier::{query_token_balance,config_info_by_pool};
use dexter::helper:: { claim_ownership, drop_ownership_proposal, propose_new_owner, };
use dexter::{
    vault::{ConfigResponse as VaultConfigResponse, QueryMsg as VaultQueryMsg, PoolConfig},
    generator::{
        ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, PendingTokenResponse,
        PoolInfoResponse, PoolLengthResponse, QueryMsg, RewardInfoResponse, ExecuteOnReply, Config
    },
    generator_proxy::{
        Cw20HookMsg as ProxyCw20HookMsg, ExecuteMsg as ProxyExecuteMsg, QueryMsg as ProxyQueryMsg,
    },
    vesting::ExecuteMsg as VestingExecuteMsg,
};

use crate::state::{
    accumulate_pool_proxy_rewards, update_proxy_asset, update_user_balance, CONFIG,
    OWNERSHIP_PROPOSAL, POOL_INFO, PROXY_REWARD_ASSET,
    TMP_USER_ACTION, USER_INFO,
};
use crate::error::ContractError;


/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter-generator";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const INIT_REWARDS_HOLDER_ID: u64 = 1;


// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x      Instantiate Contract : Execute function     x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------


/// ## Description
/// Creates a new contract with the specified parameters in the [`InstantiateMsg`] struct.
/// Returns a default object of type [`Response`] if the operation was successful,
/// or a [`ContractError`] if the contract was not created.
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **_env** is an object of type [`Env`].
///
/// * **_info** is an object of type [`MessageInfo`].
/// * **msg** is a message of type [`InstantiateMsg`] which contains the parameters used for creating the contract.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let allowed_reward_proxies: Vec<Addr> = vec![];

    let mut config = Config {
        owner: addr_validate_to_lower(deps.api, &msg.owner)?,
        vault: addr_validate_to_lower(deps.api, &msg.vault)?,
        guardian: None,
        dex_token: None,
        tokens_per_block: msg.tokens_per_block,
        total_alloc_point: Uint128::zero(),
        start_block: msg.start_block,
        allowed_reward_proxies,
        vesting_contract: addr_validate_to_lower(deps.api, &msg.vesting_contract)?,
        active_pools: vec![],
        checkpoint_generator_limit: None,
        unbonding_period: msg.unbonding_period,
    };

    if let Some(guardian) = msg.guardian {
        config.guardian = Some(addr_validate_to_lower(deps.api, &guardian)?);
    }

    // Save the config to the storage.
    CONFIG.save(deps.storage, &config)?;
    TMP_USER_ACTION.save(deps.storage, &None)?;

    Ok(Response::default())
}


// ----------------x----------------x----------------x------------------x----------------x----------------
// ----------------x----------------x  Execute function :: Entry Point  x----------------x----------------
// ----------------x----------------x----------------x------------------x----------------x----------------



/// ## Description
/// Exposes execute functions available in the contract.
/// ## Params
/// * **deps** is an object of type [`Deps`].
///
/// * **env** is an object of type [`Env`].
///
/// * **info** is an object of type [`MessageInfo`].
///
/// * **msg** is an object of type [`ExecuteMsg`].
///
/// ## Queries
/// * **ExecuteMsg::UpdateConfig {
///             vesting_contract,
///             guardian,
///             checkpoint_generator_limit,
///         }** Changes the address of the Generator vesting or Generator guardian contract, or updates the generator limit.
///
/// * **ExecuteMsg::SetupPools { pools }** Setting up a new list of pools with allocation points.
///
/// * **UpdatePool {
///             lp_token,
///         }** Update the given pool's
///
/// * **ExecuteMsg::ClaimRewards { lp_token }** Updates reward and returns it to user.
///
/// * **ExecuteMsg::Unstake { lp_token, amount }** Unstake LP tokens from the Generator.
///
/// * **ExecuteMsg::EmergencyUnstake { lp_token }** Unstake LP tokens without caring about reward claiming.
/// TO BE USED IN EMERGENCY SITUATIONS ONLY.
///
/// * **ExecuteMsg::SetAllowedRewardProxies { proxies }** Sets the list of allowed reward proxy contracts
/// that can interact with the Generator contract.
///
/// * **ExecuteMsg::SendOrphanProxyReward {
///             recipient,
///             lp_token,
///         }** Sends orphan proxy rewards to another address.
///
/// * **ExecuteMsg::Receive(msg)** Receives a message of type [`Cw20ReceiveMsg`] and processes
/// it depending on the received template.
///
/// * **ExecuteMsg::SetTokensPerBlock { amount }** Sets a new amount of DEX that's distributed per block among all active generators.
///
/// * **ExecuteMsg::ProposeNewOwner { owner, expires_in }** Creates a new request to change contract ownership.
/// Only the current owner can call this.
///
/// * **ExecuteMsg::DropOwnershipProposal {}** Removes a request to change contract ownership.
/// Only the current owner can call this.
///
/// * **ExecuteMsg::ClaimOwnership {}** Claims contract ownership. Only the newly proposed owner
/// can call this.
///
/// * **ExecuteMsg::DeactivatePool { lp_token }** Sets the allocation point to zero for specified
/// LP token.
///
/// * **ExecuteMsg::DeactivatePools { pool_types }** Sets the allocation point to zero for each pool
/// by the pool type
///
/// * **ExecuteMsg::CheckpointUserBoost { user, generators }** Updates the boost emissions for
/// specified user and generators
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig {
            vesting_contract,
            guardian,
            checkpoint_generator_limit,
            unbonding_period
        } => execute_update_config(
            deps,
            info,
            vesting_contract,
            guardian,
            checkpoint_generator_limit,
            unbonding_period
        ),
        ExecuteMsg::SetupPools { pools } => execute_setup_pools(deps, env, info, pools),
        ExecuteMsg::SetAllowedRewardProxies { proxies } => {
            set_allowed_reward_proxies(deps, info, proxies)
        },
        ExecuteMsg::UpdateAllowedProxies { add, remove } => {
            update_allowed_proxies(deps, info, add, remove)
        },
        ExecuteMsg::ClaimRewards { lp_tokens } => {
            let mut lp_tokens_addr: Vec<Addr> = vec![];
            for lp_token in &lp_tokens {
                lp_tokens_addr.push(addr_validate_to_lower(deps.api, lp_token)?);
            }
            update_rewards_and_execute(
                deps,
                env,
                None,
                ExecuteOnReply::ClaimRewards {
                    lp_tokens: lp_tokens_addr,
                    account: info.sender,
                },
            )
        }
        ExecuteMsg::Unstake { lp_token, amount } => {
            let lp_token = addr_validate_to_lower(deps.api, &lp_token)?;

            update_rewards_and_execute(
                deps.branch(),
                env,
                Some(lp_token.clone()),
                ExecuteOnReply::Unstake {
                    lp_token,
                    account: info.sender,
                    amount,
                },
            )
        },
        ExecuteMsg::EmergencyUnstake { lp_token } => emergency_unstake(deps, env, info, lp_token),
        ExecuteMsg::Unlock { lp_token } => unlock(deps, env, info, lp_token),
        ExecuteMsg::SendOrphanProxyReward {
            recipient,
            lp_token,
        } => send_orphan_proxy_rewards(deps, info, recipient, lp_token),
        ExecuteMsg::SetTokensPerBlock { amount } => {
            let cfg = CONFIG.load(deps.storage)?;
            if info.sender != cfg.owner {
                return Err(ContractError::Unauthorized {});
            }

            update_rewards_and_execute(
                deps,
                env,
                None,
                ExecuteOnReply::SetTokensPerBlock { amount },
            )
        },
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
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
        },
        ExecuteMsg::DeactivatePool { lp_token } => {
            let cfg = CONFIG.load(deps.storage)?;
            if info.sender != cfg.vault {
                return Err(ContractError::Unauthorized {});
            }
            let lp_token_addr = addr_validate_to_lower(deps.api, &lp_token)?;
            let active_pools: Vec<Addr> =
                cfg.active_pools.iter().map(|pool| pool.0.clone()).collect();
            mass_update_pools(deps.branch(), &env, &cfg, &active_pools)?;
            deactivate_pool(deps, lp_token_addr)
        }
    }
}



/// ## Description
/// Receives a message of type [`Cw20ReceiveMsg`] and processes it depending on the received template.
/// If the template is not found in the received message, then a [`ContractError`] is returned,
/// otherwise returns the [`Response`] with the specified attributes if the operation was successful
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **env** is an object of type [`Env`].
///
/// * **info** is an object of type [`MessageInfo`].
///
/// * **cw20_msg** is an object of type [`Cw20ReceiveMsg`]. This is the CW20 message to process.
fn receive_cw20(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let amount = cw20_msg.amount;
    let lp_token = info.sender;
    let cfg = CONFIG.load(deps.storage)?;

    if !POOL_INFO.has(deps.storage, &lp_token) {
        let vault_cfg: VaultConfigResponse = deps
            .querier
            .query_wasm_smart(cfg.vault.clone(), &VaultQueryMsg::Config {})?;

        create_pool(deps.branch(), &env, &lp_token, &cfg, &vault_cfg)?;
    }

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::Deposit {} => update_rewards_and_execute(
            deps,
            env,
            Some(lp_token.clone()),
            ExecuteOnReply::Deposit {
                lp_token,
                account: Addr::unchecked(cw20_msg.sender),
                amount,
            },
        ),
        Cw20HookMsg::DepositFor(beneficiary) => update_rewards_and_execute(
            deps,
            env,
            Some(lp_token.clone()),
            ExecuteOnReply::Deposit {
                lp_token,
                account: beneficiary,
                amount,
            },
        ),
    }
}


// ----------------x----------------x----------------x-----------------------x----------------x----------------
// ----------------x----------------x  Execute :: Functional implementation  x----------------x----------------
// ----------------x----------------x----------------x-----------------------x----------------x----------------


/// ## Description
/// Sets a new Generator vesting contract address. Returns a [`ContractError`] on failure or the [`CONFIG`]
/// data will be updated with the new vesting contract address.
///
/// ## Params
/// * **vesting_contract** is an [`Option`] field object of type [`String`]. This is the new vesting contract address.
/// * **guardian** is an [`Option`] field object of type [`String`]. This is the new generator guardian address.
/// * **checkpoint_generator_limit** is an [`Option`] field object of type [`u32`]. This are the max number of generators allowed
///
/// ##Executor
/// Only the owner can execute this.
pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    vesting_contract: Option<String>,
    guardian: Option<String>,
    checkpoint_generator_limit: Option<u32>,
    unbonding_period: Option<u64>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(vesting_contract) = vesting_contract {
        config.vesting_contract = addr_validate_to_lower(deps.api, vesting_contract.as_str())?;
    }

    if let Some(guardian) = guardian {
        config.guardian = Some(addr_validate_to_lower(deps.api, guardian.as_str())?);
    }

    if let Some(generator_limit) = checkpoint_generator_limit {
        config.checkpoint_generator_limit = Some(generator_limit);
    }

    if let Some(unbonding_period) = unbonding_period {
        config.unbonding_period = unbonding_period;
    }

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_config"))
}


//--------x---------------x--------------x----------------x-----
//--------x  Execute :: Pool setup, update and deactivate    x-----
//--------x---------------x--------------x----------------x-----


/// ## Description
/// Returns a [`ContractError`] on failure, otherwise it creates a new generator and adds it to [`POOL_INFO`]
/// (if it does not exist yet) and updates total allocation points (in [`Config`]).
///
/// ## Params
/// * **pools** is a vector of set that contains LP token address and allocation point.
///
/// ##Executor
/// Can only be called by the owner 
pub fn execute_setup_pools(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pools: Vec<(String, Uint128)>,
) -> Result<Response, ContractError> {

    let mut cfg = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != cfg.owner  {
        return Err(ContractError::Unauthorized {});
    }

    let pools_set: HashSet<String> = pools.clone().into_iter().map(|pc| pc.0).collect();
    if pools_set.len() != pools.len() {
        return Err(ContractError::PoolDuplicate {});
    }

    let mut setup_pools: Vec<(Addr, Uint128)> = vec![];

    // Validation check on pools, add them to setup_poools if they are valid
    for (addr, alloc_point) in pools {
        let pool_addr = addr_validate_to_lower(deps.api, &addr)?;
        setup_pools.push((pool_addr, alloc_point));
    }

    let vault_cfg: VaultConfigResponse = deps
        .querier
        .query_wasm_smart(cfg.vault.clone(), &VaultQueryMsg::Config {})?;

    // Update rewards state for currently supported pools
    let prev_pools: Vec<Addr> = cfg.active_pools.iter().map(|pool| pool.0.clone()).collect();
    mass_update_pools(deps.branch(), &env, &cfg, &prev_pools)?;

    // Add new pools to the list of active pools after checking if its not already supported
    for (lp_token, _) in &setup_pools {
        if !POOL_INFO.has(deps.storage, lp_token) {
            create_pool(deps.branch(), &env, lp_token, &cfg, &vault_cfg)?;
        }
    }

    // Update allo_points and active pools
    cfg.total_alloc_point = setup_pools.iter().map(|(_, alloc_point)| alloc_point).sum();
    cfg.active_pools = setup_pools;

    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new().add_attribute("action", "setup_pools"))
}




/// ## Description
/// Sets the allocation points to zero for the generator associated with the specified LP token. Recalculates total allocation points.
pub fn deactivate_pool(deps: DepsMut, lp_token: Addr) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;

    // Gets old allocation points for the pool and subtracts them from total allocation points
    let old_alloc_point = get_alloc_point(&cfg.active_pools, &lp_token);
    cfg.total_alloc_point = cfg.total_alloc_point.checked_sub(old_alloc_point)?;

    // Sets the pool allocation points to zero
    for pool in &mut cfg.active_pools {
        if pool.0 == lp_token {
            pool.1 = Uint128::zero();
            break;
        }
    }

    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new().add_attribute("action", "deactivate_pool"))
}


/// Sets a new amount of DEX distributed per block among all active generators. Before that, we
/// will need to update all pools in order to correctly account for accrued rewards. Returns a [`ContractError`] on failure,
/// otherwise returns a [`Response`] with the specified attributes if the operation was successful.
/// # Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **amount** is the object of type [`Uint128`]. Sets a new count of tokens per block.
fn set_tokens_per_block(
    mut deps: DepsMut,
    env: Env,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;

    let pools: Vec<Addr> = cfg.active_pools.iter().map(|pool| pool.0.clone()).collect();

    mass_update_pools(deps.branch(), &env, &cfg, &pools)?;

    cfg.tokens_per_block = amount;
    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new().add_attribute("action", "set_tokens_per_block"))
}


//--------x---------------x--------------x---------------x-------------------
//--------x  Execute :: Add / remove proxies from the allowed proxies list   x-----
//--------x---------------x--------------x---------------x-------------------


/// ## Description
/// Sets the allowed reward proxies that can interact with the Generator contract. Returns a [`ContractError`] on
/// failure, otherwise returns a [`Response`] with the specified attributes if the operation was successful.
/// # Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **info** is an object of type [`MessageInfo`].
///
/// * **proxies** is an array that contains objects of type [`String`].
/// This is the full list of allowed proxies that can interact with the Generator.
fn set_allowed_reward_proxies(
    deps: DepsMut,
    info: MessageInfo,
    proxies: Vec<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut allowed_reward_proxies: Vec<Addr> = vec![];
    for proxy in proxies {
        allowed_reward_proxies.push(addr_validate_to_lower(deps.api, &proxy)?);
    }

    CONFIG.update::<_, StdError>(deps.storage, |mut v| {
        v.allowed_reward_proxies = allowed_reward_proxies;
        Ok(v)
    })?;

    Ok(Response::new().add_attribute("action", "set_allowed_reward_proxies"))
}


/// Add or remove proxy contracts to and from the proxy contract whitelist. Returns a [`ContractError`] on failure.
fn update_allowed_proxies(
    deps: DepsMut,
    info: MessageInfo,
    add: Option<Vec<String>>,
    remove: Option<Vec<String>>,
) -> Result<Response, ContractError> {
    if add.is_none() && remove.is_none() {
        return Err(ContractError::Std(StdError::generic_err(
            "Need to provide add or remove parameters",
        )));
    }

    let mut cfg = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Remove proxies
    if let Some(remove_proxies) = remove {
        for remove_proxy in remove_proxies {
            let index = cfg
                .allowed_reward_proxies
                .iter()
                .position(|x| *x.as_str() == remove_proxy.as_str().to_lowercase())
                .ok_or_else(|| {
                    StdError::generic_err(
                        "Can't remove proxy contract. It is not found in allowed list.",
                    )
                })?;
            cfg.allowed_reward_proxies.remove(index);
        }
    }

    // Add new proxies
    if let Some(add_proxies) = add {
        for add_proxy in add_proxies {
            let proxy_addr = addr_validate_to_lower(deps.api, &add_proxy)?;
            if !cfg.allowed_reward_proxies.contains(&proxy_addr) {
                cfg.allowed_reward_proxies.push(proxy_addr);
            }
        }
    }

    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::default().add_attribute("action", "update_allowed_proxies"))
}


//--------x---------------x--------------x-----
//--------x  Execute :: Update Rewards and Execute, & Reply Callback  x-----
//--------x---------------x--------------x-----


/// ## Description
/// Updates the amount of accrued rewards for a specific generator (if specified in input parameters), otherwise updates rewards for
/// all pools that are in [`POOL_INFO`]. Returns a [`ContractError`] on failure, otherwise returns a [`Response`] object with
/// the specified attributes.
///
/// ## Params
/// * **update_single_pool** is an [`Option`] field object of type [`Addr`]. This indicates whether a single generator
/// should be updated and if yes, which one.
///
/// * **on_reply** is an object of type [`ExecuteOnReply`]. This is the action to be performed on reply.
fn update_rewards_and_execute(
    mut deps: DepsMut,
    env: Env,
    update_single_pool: Option<Addr>,
    on_reply: ExecuteOnReply,
) -> Result<Response, ContractError> {
    // Store temporary user action in the storage
    TMP_USER_ACTION.update(deps.storage, |v| {
        if v.is_some() {
            Err(StdError::generic_err("Repetitive reply definition!"))
        } else {
            Ok(Some(on_reply))
        }
    })?;

    // Update rewards for all pools if update_single_pool is not specified
    let mut pools: Vec<(Addr, PoolInfo)> = vec![];
    match update_single_pool {
        Some(lp_token) => {
            let pool = POOL_INFO.load(deps.storage, &lp_token)?;
            pools = vec![(lp_token, pool)];
        }
        None => {
            let config = CONFIG.load(deps.storage)?;

            for (lp_token, _) in config.active_pools {
                pools.push((lp_token.clone(), POOL_INFO.load(deps.storage, &lp_token)?))
            }
        }
    }

    // Update rewards for all pools
    let mut messages: Vec<SubMsg> = vec![];
    for (lp_token, mut pool) in pools {
        if let Some(reward_proxy) = pool.reward_proxy.clone() {
            messages.append(&mut get_proxy_rewards(
                deps.branch(),
                &lp_token,
                &mut pool,
                &reward_proxy,
            )?);
        }
    }

    // Execute user action on reply
    if let Some(last) = messages.last_mut() {
        last.reply_on = ReplyOn::Success;
        Ok(Response::new().add_submessages(messages))
    } else {
        process_after_update(deps, env)
    }
}



/// ## Description
/// The entry point to the contract for processing replies from submessages.
/// 
/// # Params
/// * **_msg** is an object of type [`Reply`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        _ => process_after_update(deps, env),
    }
}

/// ## Description
/// Loads an action from [`TMP_USER_ACTION`] and executes it. Returns a [`ContractError`]
/// on failure, otherwise returns a [`Response`] with the specified attributes if the operation was successful.
/// # Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **env** is an object of type [`Env`].
fn process_after_update(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    match TMP_USER_ACTION.load(deps.storage)? {
        Some(action) => {
            TMP_USER_ACTION.save(deps.storage, &None)?;
            match action {
                ExecuteOnReply::ClaimRewards { lp_tokens, account } => {
                    claim_rewards(deps, env, lp_tokens, account)
                }
                ExecuteOnReply::Deposit {
                    lp_token,
                    account,
                    amount,
                } => deposit(deps, env, lp_token, account, amount),
                ExecuteOnReply::Unstake {
                    lp_token,
                    account,
                    amount,
                } => unstake(deps, env, lp_token, account, amount),
                ExecuteOnReply::SetTokensPerBlock { amount } => {
                    set_tokens_per_block(deps, env, amount)
                }
            }
        }
        None => Ok(Response::default()),
    }
}



//--------x---------------x--------------x-----
//--------x  Execute :: Deposit, Unstake, Claim Rewards  x-----
//--------x---------------x--------------x-----


/// ## Description
/// Deposit LP tokens in a generator to receive token emissions. Returns a [`ContractError`] on
/// failure, otherwise returns a [`Response`] with the specified attributes if the operation was successful.
/// 
/// # Params
/// * **lp_token** is an object of type [`Addr`]. This is the LP token to deposit.
/// * **beneficiary** is an object of type [`Addr`]. This is the address that will take ownership of the staked LP tokens.
/// * **amount** is an object of type [`Uint128`]. This is the amount of LP tokens to deposit.
pub fn deposit(
    deps: DepsMut,
    env: Env,
    lp_token: Addr,
    beneficiary: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {

    let user = USER_INFO.load(deps.storage, (&lp_token, &beneficiary)).unwrap_or_default();
    let cfg = CONFIG.load(deps.storage)?;
    let mut pool = POOL_INFO.load(deps.storage, &lp_token)?;

    accumulate_rewards_per_share(&deps.querier, &env, &lp_token, &mut pool, &cfg)?;

    // Send pending rewards (if any) to the depositor
    let send_rewards_msg = send_pending_rewards(deps.as_ref(), &cfg, &pool, &user, &beneficiary)?;

    // If a reward proxy is set - send LP tokens to the proxy
    let transfer_msg = if !amount.is_zero() && pool.reward_proxy.is_some() {
        vec![WasmMsg::Execute {
            contract_addr: lp_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: pool.reward_proxy.clone().unwrap().to_string(),
                msg: to_binary(&ProxyCw20HookMsg::Deposit {})?,
                amount,
            })?,
            funds: vec![],
        }]
    } else {
        vec![]
    };

    // Update user's LP token balance
    let updated_amount = user.amount.checked_add(amount)?;
    let mut user = update_user_balance(user, &pool, updated_amount)?;

    POOL_INFO.save(deps.storage, &lp_token, &pool)?;
    USER_INFO.save(deps.storage, (&lp_token, &beneficiary), &user)?;

    Ok(Response::new()
        .add_messages(send_rewards_msg)
        .add_messages(transfer_msg)
        .add_attribute("action", "deposit")
        .add_attribute("amount", amount))
}


/// ## Description
/// Unstake LP tokens from a generator. Returns a [`ContractError`] on
/// failure, otherwise returns a [`Response`] with the specified attributes if the operation was successful.
/// 
/// # Params
/// * **lp_token** is an object of type [`Addr`]. This is the LP token to withdraw.
/// * **account** is an object of type [`Addr`]. This is the user whose LP tokens we withdraw.
/// * **amount** is an object of type [`Uint128`]. This is the amount of LP tokens to withdraw.
pub fn unstake(
    deps: DepsMut,
    env: Env,
    lp_token: Addr,
    account: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {

    // Load user
    let user = USER_INFO.load(deps.storage, (&lp_token, &account)).unwrap_or_default();
    if user.amount < amount {
        return Err(ContractError::BalanceTooSmall {});
    }

    let cfg = CONFIG.load(deps.storage)?;
    let mut pool = POOL_INFO.load(deps.storage, &lp_token)?;

    accumulate_rewards_per_share(&deps.querier, &env, &lp_token, &mut pool, &cfg)?;

    // Send pending rewards to the user
    let send_rewards_msg = send_pending_rewards(deps.as_ref(), &cfg, &pool.clone(), &user, &account)?;
    let mut msgs: Vec<WasmMsg> = vec![];

    // Instantiate the transfer call for the LP token
    if pool.reward_proxy.clone().is_some() {
        msgs.push( WasmMsg::Execute {
            contract_addr: pool.reward_proxy.clone().unwrap().to_string(),
            funds: vec![],
            msg: to_binary(&ProxyExecuteMsg::Withdraw {
                account: account.to_string(),
                amount,
            })?,
        });
    }

    // Update user's balance
    let updated_amount = user.amount.checked_sub(amount)?;
    let mut user = update_user_balance(user, &pool.clone(), updated_amount)?;

    // Create unbonding period 
    let unbonding_period = UnbondingInfo {
        amount,
        unlock_timstamp: env.block.time.seconds() + cfg.unbonding_period,
    };

    // Save the unbonding period
    user.unbonding_periods.push( unbonding_period );

    POOL_INFO.save(deps.storage, &lp_token, &pool)?;
    USER_INFO.save(deps.storage, (&lp_token, &account), &user)?;

    Ok(Response::new()
        .add_messages(send_rewards_msg)
        .add_messages(msgs)
        .add_attribute("action", "withdraw")
        .add_attribute("amount", amount))
}



/// ## Description
/// Updates the amount of accrued rewards for a specific generator. Returns a [`ContractError`] on
/// failure, otherwise returns a [`Response`] with the specified attributes if the operation was successful.
/// 
/// # Params
/// * **lp_token** is the object of type [`Addr`]. Sets the liquidity pool to be updated.
/// * **account** is the object of type [`Addr`].
pub fn claim_rewards(
    mut deps: DepsMut,
    env: Env,
    lp_tokens: Vec<Addr>,
    account: Addr,
) -> Result<Response, ContractError> {

    let response = Response::default();
    let cfg = CONFIG.load(deps.storage)?;

    mass_update_pools(deps.branch(), &env, &cfg, &lp_tokens)?;

    let mut send_rewards_msg: Vec<WasmMsg> = vec![];
    for lp_token in &lp_tokens {
        let mut pool = POOL_INFO.load(deps.storage, lp_token)?;
        let user = USER_INFO.load(deps.storage, (lp_token, &account))?;

        // ExecuteMsg to send rewards to the user
        send_rewards_msg.append(&mut send_pending_rewards(
            deps.as_ref(),
            &cfg,
            &pool,
            &user,
            &account,
        )?);

        // Update user's amount
        let amount = user.amount;
        let mut user = update_user_balance(user, &pool, amount)?;

        // Update state
        USER_INFO.save(deps.storage, (lp_token, &account), &user)?;
        POOL_INFO.save(deps.storage, lp_token, &pool)?;
    }

    Ok(response
        .add_attribute("action", "claim_rewards")
        .add_messages(send_rewards_msg))
}

/// ## Description
/// Withdraw LP tokens without caring about rewards. TO BE USED IN EMERGENCY SITUATIONS ONLY.
/// Returns a [`ContractError`] on failure, otherwise returns a [`Response`] with the
/// specified attributes if the operation was successful.
/// 
/// # Params
/// * **lp_token** is an object of type [`String`]. This is the LP token to withdraw.
pub fn unlock(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_token: String,
) -> Result<Response, ContractError> {
    let lp_token = addr_validate_to_lower(deps.api, &lp_token)?;
    let mut user = USER_INFO.load(deps.storage, (&lp_token, &info.sender))?;
    let unbonding_sessions = user.unbonding_periods;

    let mut attributes = vec![attr("action", "unlock")];

    let mut unlock_msgs: Vec<WasmMsg> = vec![];
    let mut rem_unbonding_sessions : Vec<UnbondingInfo> = vec![];
    for session in unbonding_sessions.iter() {
        // Check if session can be unlocked
        if session.unlock_timstamp < env.block.time.seconds() {
            // ExecuteMsg to send LP Tokens to the user
            unlock_msgs.push(WasmMsg::Execute {
                contract_addr: lp_token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: info.sender.to_string(),
                    amount: session.amount,
                })?,
                funds: vec![],
            });
            attributes.push(Attribute::new("amount", session.amount));
        }
        else {
            rem_unbonding_sessions.push( UnbondingInfo { amount: session.amount, unlock_timstamp: session.unlock_timstamp} );
        }
    }

    // Save the unbonding period
    user.unbonding_periods = rem_unbonding_sessions;
    USER_INFO.save(deps.storage, (&lp_token, &info.sender), &user)?;

    Ok(Response::new()
        .add_messages(unlock_msgs)
        .add_attribute("action", "unlock")
        .add_attributes(attributes))
}



//--------x---------------x--------------x-----
//--------x  Execute :: Emergency Unstake, Transfer Orphan proxy Rewards  x-----
//--------x---------------x--------------x-----


/// ## Description
/// Unstake LP tokens without caring about rewards. TO BE USED IN EMERGENCY SITUATIONS ONLY.
/// Returns a [`ContractError`] on failure, otherwise returns a [`Response`] with the
/// specified attributes if the operation was successful.
/// 
/// # Params
/// * **lp_token** is an object of type [`String`]. This is the LP token to withdraw.
pub fn emergency_unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_token: String,
) -> Result<Response, ContractError> {
    let lp_token = addr_validate_to_lower(deps.api, &lp_token)?;
    let cfg = CONFIG.load(deps.storage)?;

    let mut pool = POOL_INFO.load(deps.storage, &lp_token)?;
    let user = USER_INFO.load(deps.storage, (&lp_token, &info.sender))?;

    // Instantiate the transfer call for the LP token
    let mut transfer_msgs: Vec<WasmMsg> = vec![];
    if let Some(proxy) = &pool.reward_proxy {
        let accumulated_proxy_rewards: HashMap<_, _> = accumulate_pool_proxy_rewards(&pool, &user)?
            .into_iter()
            .collect();
        // All users' proxy rewards become orphaned
        pool.orphan_proxy_rewards = pool
            .orphan_proxy_rewards
            .inner_ref()
            .iter()
            .map(|(addr, amount)| {
                let user_amount = accumulated_proxy_rewards
                    .get(addr)
                    .cloned()
                    .unwrap_or_default();
                let amount = amount.checked_add(user_amount)?;
                Ok((addr.clone(), amount))
            })
            .collect::<StdResult<Vec<_>>>()?
            .into();

        transfer_msgs.push( WasmMsg::Execute {
            contract_addr: proxy.to_string(),
            msg: to_binary(&ProxyExecuteMsg::EmergencyWithdraw {
                account: info.sender.to_string(),
                amount: user.amount,
            })?,
            funds: vec![],
        });
    } 
    // else {
    //     transfer_msg = WasmMsg::Execute {
    //         contract_addr: lp_token.to_string(),
    //         msg: to_binary(&Cw20ExecuteMsg::Transfer {
    //             recipient: info.sender.to_string(),
    //             amount: user.amount,
    //         })?,
    //         funds: vec![],
    //     };
    // }

    // Update user's balance
    let mut user = update_user_balance(user, &pool,  Uint128::zero())?;

    // Create unbonding period 
    let unbonding_period = UnbondingInfo {
        amount: user.amount,
        unlock_timstamp: env.block.time.seconds() + cfg.unbonding_period,
    };

    // Save the unbonding period
    user.unbonding_periods.push( unbonding_period );    

    // Change the user's balance
    USER_INFO.save(deps.storage, (&lp_token, &info.sender), &user)?;
    POOL_INFO.save(deps.storage, &lp_token, &pool)?;

    Ok(Response::new()
        .add_messages(transfer_msgs)
        .add_attribute("action", "emergency_withdraw")
        .add_attribute("amount", user.amount))
}



/// ## Description
/// Sends orphaned proxy rewards (which are left behind by emergency withdrawals) to another address.
/// Returns an [`ContractError`] on failure, otherwise returns the [`Response`] with the specified
/// attributes if the operation was successful.
/// 
/// # Params
/// * **recipient** is an object of type [`String`]. This is the recipient of the orphaned rewards.
/// * **lp_token** is an object of type [`String`]. This is the LP token whose orphaned rewards we send out.
/// 
/// # Executor - Only owner is allowed to call this function.
fn send_orphan_proxy_rewards(
    deps: DepsMut,
    info: MessageInfo,
    recipient: String,
    lp_token: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    };

    let lp_token = addr_validate_to_lower(deps.api, &lp_token)?;
    let recipient = addr_validate_to_lower(deps.api, &recipient)?;

    let mut pool = POOL_INFO.load(deps.storage, &lp_token)?;

    if pool.orphan_proxy_rewards.inner_ref().is_empty() {
        return Err(ContractError::ZeroOrphanRewards {});
    }

    // Transfer orphan rewards to the recipient
    let submessages = pool
        .orphan_proxy_rewards
        .inner_ref()
        .iter()
        .filter(|(_, value)| !value.is_zero())
        .map(|(proxy, amount)| {
                let msg = SubMsg::new(WasmMsg::Execute {
                        contract_addr: proxy.to_string(),
                        funds: vec![],
                        msg: to_binary(&ProxyExecuteMsg::SendRewards {
                            account: recipient.to_string(),
                            amount: *amount,
                        })?,
                    });
                    Ok(msg)
            })
        .collect::<StdResult<Vec<_>>>()?;

    // Clear the orphaned proxy rewards
    pool.orphan_proxy_rewards = Default::default();

    POOL_INFO.save(deps.storage, &lp_token, &pool)?;

    Ok(Response::new()
        .add_submessages(submessages)
        .add_attribute("action", "send_orphan_rewards")
        .add_attribute("recipient", recipient)
        .add_attribute("lp_token", lp_token.to_string()))
}




// ----------------x----------------x---------------------x-----------------------x----------------x----------------
// ----------------x----------------x  :::: Generator::QUERIES Implementation   ::::  x----------------x----------------
// ----------------x----------------x---------------------x-----------------------x----------------x----------------




/// ## Description
/// Exposes all the queries available in the contract.
/// ## Params
/// * **deps** is an object of type [`Deps`].
///
/// * **_env** is an object of type [`Env`].
///
/// * **msg** is an object of type [`QueryMsg`].
///
/// ## Queries
/// * **QueryMsg::PoolLength {}** Returns the amount of instantiated generators using a [`PoolLengthResponse`] object.
///
/// * **QueryMsg::Deposit { lp_token, user }** Returns the amount of LP tokens staked by a user in a specific generator.
///
/// * **QueryMsg::PendingToken { lp_token, user }** Returns the amount of pending rewards a user earned using
/// a [`PendingTokenResponse`] object.
///
/// * **QueryMsg::Config {}** Returns the Generator contract configuration using a [`ConfigResponse`] object.
///
/// * **QueryMsg::RewardInfo { lp_token }** Returns reward information about a specific generator
/// using a [`RewardInfoResponse`] object.
///
/// * **QueryMsg::OrphanProxyRewards { lp_token }** Returns the amount of orphaned proxy rewards for a specific generator.
///
/// * **QueryMsg::PoolInfo { lp_token }** Returns general information about a generator using a [`PoolInfoResponse`] object.
///
/// * **QueryMsg::SimulateFutureReward { lp_token, future_block }** Returns the amount of token rewards a generator will
/// distribute up to a future block.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Config {} => Ok(to_binary(&query_config(deps)?)?),
        QueryMsg::ActivePoolLength {} => Ok(to_binary(&active_pool_length(deps)?)?),
        QueryMsg::PoolLength {} => Ok(to_binary(&pool_length(deps)?)?),
        QueryMsg::Deposit { lp_token, user } => {
            Ok(to_binary(&query_deposit(deps, lp_token, user)?)?)
        }
        QueryMsg::PendingToken { lp_token, user } => {
            Ok(to_binary(&pending_token(deps, env, lp_token, user)?)?)
        }
        QueryMsg::RewardInfo { lp_token } => Ok(to_binary(&query_reward_info(deps, lp_token)?)?),
        QueryMsg::OrphanProxyRewards { lp_token } => {
            Ok(to_binary(&query_orphan_proxy_rewards(deps, lp_token)?)?)
        }
        QueryMsg::PoolInfo { lp_token } => Ok(to_binary(&query_pool_info(deps, env, lp_token)?)?),
        QueryMsg::SimulateFutureReward {
            lp_token,
            future_block,
        } => Ok(to_binary(&query_simulate_future_reward(
            deps,
            env,
            lp_token,
            future_block,
        )?)?),
    }
}


/// ## Description
/// Returns a [`ContractError`] on failure, otherwise returns information about a generator's
/// configuration using a [`ConfigResponse`] object .
/// ## Params
/// * **deps** is an object of type [`Deps`].
fn query_config(deps: Deps) -> Result<ConfigResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: config.owner,
        vault: config.vault,
        dex_token: config.dex_token,
        tokens_per_block: config.tokens_per_block,
        total_alloc_point: config.total_alloc_point,
        start_block: config.start_block,
        allowed_reward_proxies: config.allowed_reward_proxies,
        vesting_contract: config.vesting_contract,
        active_pools: config.active_pools,
        guardian: config.guardian,
        checkpoint_generator_limit: config.checkpoint_generator_limit,
    })
}


/// ## Description
/// Returns a [`ContractError`] on failure, otherwise returns the amount of active generators
/// using a [`PoolLengthResponse`] object.
pub fn active_pool_length(deps: Deps) -> Result<PoolLengthResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    Ok(PoolLengthResponse {
        length: config.active_pools.len(),
    })
}


/// Returns a [`ContractError`] on failure, otherwise returns the amount of instantiated generators
/// using a [`PoolLengthResponse`] object.
/// ## Params
/// * **deps** is an object of type [`Deps`].
pub fn pool_length(deps: Deps) -> Result<PoolLengthResponse, ContractError> {
    let length = POOL_INFO
        .keys(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .count();
    Ok(PoolLengthResponse { length })
}


/// ## Description
/// Returns a [`ContractError`] on failure, otherwise returns the amount of LP tokens a user staked in a specific generator.
/// 
/// ## Params
/// * **lp_token** is an object of type [`String`]. This is the LP token for which we query the user's balance for.
/// * **user** is an object of type [`String`]. This is the user whose balance we query.
pub fn query_deposit(deps: Deps, lp_token: String, user: String) -> Result<Uint128, ContractError> {
    let lp_token = addr_validate_to_lower(deps.api, &lp_token)?;
    let user = addr_validate_to_lower(deps.api, &user)?;

    let user_info = USER_INFO
        .load(deps.storage, (&lp_token, &user))
        .unwrap_or_default();
    Ok(user_info.amount)
}


/// ## Description
/// Calculates and returns the pending token rewards for a specific user. Returns a [`ContractError`] on failure, otherwise returns
/// information in a [`PendingTokenResponse`] object.
/// 
/// ## Params
/// * **lp_token** is an object of type [`String`]. This is the LP token staked by the user whose pending rewards we calculate.
/// * **user** is an object of type [`String`]. This is the user for which we fetch the amount of pending token rewards.
// View function to see pending DEX on frontend.
pub fn pending_token(
    deps: Deps,
    env: Env,
    lp_token: String,
    user: String,
) -> Result<PendingTokenResponse, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    let lp_token = addr_validate_to_lower(deps.api, &lp_token)?;
    let user = addr_validate_to_lower(deps.api, &user)?;

    let pool = POOL_INFO.load(deps.storage, &lp_token)?;
    let user_info = USER_INFO.load(deps.storage, (&lp_token, &user)).unwrap_or_default();

    let mut pending_on_proxy = None;
    let mut lp_supply: Uint128 = Uint128::zero();

    if let Some(proxy) = &pool.reward_proxy {
        let lp_supply: Uint128 = deps
            .querier
            .query_wasm_smart(proxy, &ProxyQueryMsg::Deposit {})?;

        if !lp_supply.is_zero() {
            let proxy_rewards = accumulate_pool_proxy_rewards(&pool, &user_info)?
                .into_iter()
                .map(|(proxy_addr, mut reward)| {
                    // Add reward pending on proxy
                    if proxy_addr.eq(proxy) {
                        let res: Option<Uint128> = deps
                            .querier
                            .query_wasm_smart(proxy, &ProxyQueryMsg::PendingToken {})?;
                        if let Some(token_rewards) = res {
                            let share = user_info
                                .amount
                                .multiply_ratio(token_rewards, lp_supply);
                            reward = reward.checked_add(share)?;
                        }
                    }
                    let info = PROXY_REWARD_ASSET.load(deps.storage, &proxy_addr)?;
                    Ok(Asset {
                        info,
                        amount: reward,
                    })
                })
                .collect::<StdResult<Vec<_>>>()?;

            pending_on_proxy = Some(proxy_rewards);
        }
    } else {
        lp_supply = query_token_balance(
            &deps.querier,
            lp_token.clone(),
            env.contract.address.clone(),
        )?;
    }

    let mut acc_per_share = pool.accumulated_rewards_per_share;
    if env.block.height > pool.last_reward_block.u64() && !lp_supply.is_zero() {
        let alloc_point = get_alloc_point(&cfg.active_pools, &lp_token);
        let token_rewards = calculate_rewards(&env, &pool, &alloc_point, &cfg)?;
        let share = Decimal::from_ratio(token_rewards, lp_supply);
        acc_per_share = pool.accumulated_rewards_per_share.checked_add(share)?;
    }

    // we should calculate rewards by virtual amount
    let pending = acc_per_share
        .checked_mul_uint128(user_info.amount)?
        .checked_sub(user_info.reward_debt)?;

    Ok(PendingTokenResponse {
        pending,
        pending_on_proxy,
    })
}


/// ## Description
/// Returns a [`ContractError`] on failure, otherwise returns reward information for a specific generator
/// using a [`RewardInfoResponse`] object.
/// 
/// ## Params
/// * **lp_token** is an object of type [`String`]. This is the LP token whose generator we query for reward information.
fn query_reward_info(deps: Deps, lp_token: String) -> Result<RewardInfoResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let lp_token = addr_validate_to_lower(deps.api, &lp_token)?;
    let pool = POOL_INFO.load(deps.storage, &lp_token)?;

    let proxy_reward_token = match pool.reward_proxy {
        Some(proxy) => {
            let res: Addr = deps
                .querier
                .query_wasm_smart(&proxy, &ProxyQueryMsg::RewardInfo {})?;
            Some(res)
        }
        None => None,
    };

    Ok(RewardInfoResponse {
        base_reward_token: config.dex_token,
        proxy_reward_token,
    })
}


/// Returns a [`ContractError`] on failure, otherwise returns a vector of pairs (asset, amount),
/// where 'asset' is an object of type [`AssetInfo`] and 'amount' is amount of orphaned proxy rewards for a specific generator.
/// ## Params
/// * **deps** is an object of type [`Deps`].
///
/// * **lp_token** is an object of type [`String`]. This is the LP token whose generator we query for orphaned rewards.
fn query_orphan_proxy_rewards(
    deps: Deps,
    lp_token: String,
) -> Result<Vec<(AssetInfo, Uint128)>, ContractError> {
    let lp_token = addr_validate_to_lower(deps.api, &lp_token)?;

    let pool = POOL_INFO.load(deps.storage, &lp_token)?;
    if pool.reward_proxy.is_some() {
        let orphan_rewards = pool
            .orphan_proxy_rewards
            .inner_ref()
            .iter()
            .map(|(proxy, amount)| {
                let asset = PROXY_REWARD_ASSET.load(deps.storage, proxy)?;
                Ok((asset, *amount))
            })
            .collect::<StdResult<Vec<_>>>()?;
        Ok(orphan_rewards)
    } else {
        Err(ContractError::PoolDoesNotHaveAdditionalRewards {})
    }
}

/// ## Description
/// Returns a [`ContractError`] on failure, otherwise returns a generator's
/// configuration using a [`PoolInfoResponse`] object.
/// ## Params
/// * **deps** is an object of type [`Deps`].
///
/// * **env** is an object of type [`Env`].
///
/// * **lp_token** is an object of type [`String`]. This is the LP token whose generator we query.
fn query_pool_info(
    deps: Deps,
    env: Env,
    lp_token: String,
) -> Result<PoolInfoResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let lp_token = addr_validate_to_lower(deps.api, &lp_token)?;
    let pool = POOL_INFO.load(deps.storage, &lp_token)?;

    let lp_supply: Uint128;
    let mut pending_on_proxy = None;
    let mut pending_dex_rewards = Uint128::zero();

    // If proxy rewards are live for this LP token, calculate its pending proxy rewards
    match &pool.reward_proxy {
        Some(proxy) => {
            lp_supply = deps
                .querier
                .query_wasm_smart(proxy, &ProxyQueryMsg::Deposit {})?;

            // If LP tokens are staked via a proxy contract, fetch current pending proxy rewards
            if !lp_supply.is_zero() {
                let res: Uint128 = deps
                    .querier
                    .query_wasm_smart(proxy, &ProxyQueryMsg::PendingToken {})?;

                if !res.is_zero() {
                    pending_on_proxy = Some(res);
                }
            }
        }
        None => {
            lp_supply = query_token_balance(
                &deps.querier,
                lp_token.clone(),
                env.contract.address.clone(),
            )?;
        }
    }

    let alloc_point = get_alloc_point(&config.active_pools, &lp_token);

    // Calculate pending DEX rewards
    if env.block.height > pool.last_reward_block.u64() && !lp_supply.is_zero() {
        pending_dex_rewards = calculate_rewards(&env, &pool, &alloc_point, &config)?;
    }

    // Calculate DEX tokens being distributed per block to this LP token pool
    let dex_tokens_per_block = config
        .tokens_per_block
        .checked_mul(alloc_point)?
        .checked_div(config.total_alloc_point)
        .unwrap_or_else(|_| Uint128::zero());

    Ok(PoolInfoResponse {
        alloc_point,
        dex_tokens_per_block,
        last_reward_block: pool.last_reward_block.u64(),
        current_block: env.block.height,
        pending_astro_rewards: pending_dex_rewards,
        reward_proxy: pool.reward_proxy,
        pending_proxy_rewards: pending_on_proxy,
        accumulated_proxy_rewards_per_share: pool
            .accumulated_proxy_rewards_per_share
            .inner_ref()
            .clone(),
        proxy_reward_balance_before_update: pool.proxy_reward_balance_before_update,
        orphan_proxy_rewards: pool.orphan_proxy_rewards.inner_ref().clone(),
        lp_supply,
        global_reward_index: pool.accumulated_rewards_per_share,
    })
}

/// ## Description
/// Returns a [`ContractError`] on failure, otherwise returns the total amount of DEX tokens distributed for
/// a specific generator up to a certain block in the future.
/// 
/// ## Params
/// * **lp_token** is an object of type [`Addr`]. This is the LP token for which we query the amount of future DEX rewards.
pub fn query_simulate_future_reward(
    deps: Deps,
    env: Env,
    lp_token: String,
    future_block: u64,
) -> Result<Uint128, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    let lp_token = addr_validate_to_lower(deps.api, &lp_token)?;
    let alloc_point = get_alloc_point(&cfg.active_pools, &lp_token);
    let n_blocks = Uint128::from(future_block)
        .checked_sub(env.block.height.into())
        .unwrap_or_else(|_| Uint128::zero());

    let simulated_reward = n_blocks
        .checked_mul(cfg.tokens_per_block)?
        .checked_mul(alloc_point)?
        .checked_div(cfg.total_alloc_point)
        .unwrap_or_else(|_| Uint128::zero());

    Ok(simulated_reward)
}



// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x      Helper functions           x----------------x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------



/// ## Description
/// Calculates and returns the amount of accrued rewards since the last reward checkpoint for a specific generator.
/// 
/// ## Params
/// * **pool** is an object of type [`PoolInfo`]. This is the generator for which we calculate accrued rewards.
/// * **alloc_point** is the object of type [`Uint64`].
/// * **cfg** is an object of type [`Config`]. This is the Generator contract configuration.
pub fn calculate_rewards(
    env: &Env,
    pool: &PoolInfo,
    alloc_point: &Uint128,
    cfg: &Config,
) -> StdResult<Uint128> {
    let n_blocks = Uint128::from(env.block.height).checked_sub(pool.last_reward_block.into())?;

    let r = if !cfg.total_alloc_point.is_zero() {
        n_blocks
            .checked_mul(cfg.tokens_per_block)?
            .checked_mul(*alloc_point)?
            .checked_div(cfg.total_alloc_point)?
    } else {
        Uint128::zero()
    };

    Ok(r)
}

/// ## Description
/// Gets allocation point of the pool.
/// 
/// ## Params
/// * **pools** is a vector of set that contains LP token address and allocation point.
/// * **lp_token** is an object of type [`Addr`].
pub fn get_alloc_point(pools: &[(Addr, Uint128)], lp_token: &Addr) -> Uint128 {
    pools
        .iter()
        .find_map(|(addr, alloc_point)| {
            if addr == lp_token {
                return Some(*alloc_point);
            }
            None
        })
        .unwrap_or_else(Uint128::zero)
}

/// ## Description
/// Creates pool if it is allowed in the vault.
/// ## Params
/// * **lp_token** is an object of type [`Addr`]. This is the
pub fn create_pool(
    deps: DepsMut,
    env: &Env,
    lp_token: &Addr,
    cfg: &Config,
    vault_cfg: &VaultConfigResponse,
) -> Result<PoolInfo, ContractError> {
    let pool_info = config_info_by_pool(&deps.querier, lp_token.clone().to_string())?;

    // Create Pool 
    POOL_INFO.save(
        deps.storage,
        lp_token,
        &PoolInfo {
            last_reward_block: cfg.start_block.max(Uint64::from(env.block.height)),
            reward_proxy: None,
            accumulated_proxy_rewards_per_share: Default::default(),
            proxy_reward_balance_before_update: Uint128::zero(),
            orphan_proxy_rewards: Default::default(),
            accumulated_rewards_per_share: Decimal::zero()
        },
    )?;

    Ok(POOL_INFO.load(deps.storage, lp_token)?)
}



/// ## Description
/// Fetches accrued proxy rewards. Snapshots the old amount of rewards that are still unclaimed. Returns a [`ContractError`]
/// on failure, otherwise returns a vector that contains objects of type [`SubMsg`].
///
/// ## Params
/// * **lp_token** is an object of type [`Addr`]. This is the LP token for which we fetch the latest amount of accrued proxy rewards.
/// * **pool** is an object of type [`PoolInfo`]. This is the generator associated with the `lp_token`.
/// * **reward_proxy** is an object of type [`Addr`]. This is the address of the dual rewards proxy for the target LP/generator.
fn get_proxy_rewards(
    deps: DepsMut,
    lp_token: &Addr,
    pool: &mut PoolInfo,
    reward_proxy: &Addr,
) -> Result<Vec<SubMsg>, ContractError> {

    // Fetch the amount of accrued rewards
    let reward_amount: Uint128 = deps
        .querier
        .query_wasm_smart(reward_proxy, &ProxyQueryMsg::Reward {})?;

    pool.proxy_reward_balance_before_update = reward_amount;
    POOL_INFO.save(deps.storage, lp_token, pool)?;

    let msg = ProxyQueryMsg::PendingToken {};
    let res: Uint128 = deps.querier.query_wasm_smart(reward_proxy, &msg)?;

    Ok(if !res.is_zero() {
        vec![SubMsg::new(WasmMsg::Execute {
            contract_addr: reward_proxy.to_string(),
            funds: vec![],
            msg: to_binary(&ProxyExecuteMsg::UpdateRewards {})?,
        })]
    } else {
        vec![]
    })
}

/// ## Description
/// Updates the amount of accrued rewards for all generators. Returns a [`ContractError`] on failure, otherwise
/// returns a [`Response`] with the specified attributes if the operation was successful.
/// 
/// # Params
/// * **cfg** is the object of type [`Config`].
/// * **lp_tokens** is the list of type [`Addr`].
pub fn mass_update_pools(
    deps: DepsMut,
    env: &Env,
    cfg: &Config,
    lp_tokens: &[Addr],
) -> Result<(), ContractError> {
    for lp_token in lp_tokens {
        let mut pool = POOL_INFO.load(deps.storage, lp_token)?;
        accumulate_rewards_per_share(&deps.querier, env, lp_token, &mut pool, cfg)?;
        POOL_INFO.save(deps.storage, lp_token, &pool)?;
    }

    Ok(())
}



/// ## Description
/// Accrues the amount of rewards distributed for each staked LP token in a specific generator.
/// Also update reward variables for the given generator.
/// 
/// # Params
/// * **lp_token** is an object of type [`Addr`]. This is the LP token whose rewards per share we update.
/// * **pool** is an object of type [`PoolInfo`]. This is the generator associated with the `lp_token`
/// * **cfg** is an object of type [`Config`]. This is the contract config.
/// * **deposited** is an [`Option`] field object of type [`Uint128`]. This is the total amount of LP
/// tokens deposited in the target generator.
pub fn accumulate_rewards_per_share(
    querier: &QuerierWrapper,
    env: &Env,
    lp_token: &Addr,
    pool: &mut PoolInfo,
    cfg: &Config,
) -> StdResult<()> {

    let mut lp_supply : Uint128 = Uint128::zero();

    // Update reward share for proxy rewards 
    if let Some(proxy) = &pool.reward_proxy {
        let proxy_lp_supply: Uint128 =
            querier.query_wasm_smart(proxy, &ProxyQueryMsg::Deposit {})?;

        if !proxy_lp_supply.is_zero() {
            let reward_amount: Uint128 =
                querier.query_wasm_smart(proxy, &ProxyQueryMsg::Reward {})?;

            let token_rewards =
                reward_amount.checked_sub(pool.proxy_reward_balance_before_update)?;

            let share = Decimal::from_ratio(token_rewards, proxy_lp_supply);
            pool.accumulated_proxy_rewards_per_share
                .update(proxy, share)?;
            pool.proxy_reward_balance_before_update = reward_amount;
        }
        // Lp token supply
        lp_supply = querier.query_wasm_smart(proxy, &ProxyQueryMsg::Deposit {})?
    }

    if lp_supply.is_zero() {
        let bal_res :BalanceResponse  = querier.query_wasm_smart(
            &env.contract.address,
            &cw20::Cw20QueryMsg::Balance {
                address: env.contract.address.to_string(),
            },
        )?;
        lp_supply = bal_res.balance;
    }

    if env.block.height > pool.last_reward_block.u64() {
        if !lp_supply.is_zero() {
            let alloc_point = get_alloc_point(&cfg.active_pools, lp_token);
            let token_rewards = calculate_rewards(env, pool, &alloc_point, cfg)?;

            let share = Decimal::from_ratio(token_rewards, lp_supply);
            pool.accumulated_rewards_per_share = pool.accumulated_rewards_per_share.checked_add(share)?;
        }

        pool.last_reward_block = Uint64::from(env.block.height);
    }

    Ok(())
}



/// ## Description
/// Distributes pending proxy rewards for a specific staker.
/// Returns a [`ContractError`] on failure, otherwise returns a vector that
/// contains objects of type [`SubMsg`].
/// 
/// # Params
/// * **cfg** is an object of type [`Config`].
/// * **pool** is an object of type [`PoolInfo`]. This is the generator where the staker is staked.
/// * **user** is an object of type [`UserInfo`]. This is the staker for which we claim accrued proxy rewards.
/// * **to** is an object of type [`Addr`]. This is the address that will receive the proxy rewards.
pub fn send_pending_rewards(
    deps: Deps,
    cfg: &Config,
    pool: &PoolInfo,
    user: &UserInfo,
    to: &Addr,
) -> Result<Vec<WasmMsg>, ContractError> {
    if user.amount.is_zero() {
        return Ok(vec![]);
    }

    let mut messages = vec![];

    // Calculate amount of DEX rewards to send
    let pending_rewards = pool
        .accumulated_rewards_per_share
        .checked_mul_uint128(user.amount)?
        .checked_sub(user.reward_debt)?;

    // Send proxy rewards Msg
    if !pending_rewards.is_zero() {
        messages.push(WasmMsg::Execute {
            contract_addr: cfg.vesting_contract.to_string(),
            msg: to_binary(&VestingExecuteMsg::Claim {
                recipient: Some(to.to_string()),
                amount: Some(pending_rewards),
            })?,
            funds: vec![],
        });
    }

    // Calculate pending proxy rewards 
    let proxy_rewards = accumulate_pool_proxy_rewards(pool, user)?;

    // Send proxy rewards Msg
    for (proxy, pending_proxy_rewards) in proxy_rewards {
        if !pending_proxy_rewards.is_zero() {
            match &pool.reward_proxy {
                Some(reward_proxy) if reward_proxy.as_str() == proxy.as_str() => {
                    messages.push(WasmMsg::Execute {
                        contract_addr: proxy.to_string(),
                        funds: vec![],
                        msg: to_binary(&ProxyExecuteMsg::SendRewards {
                            account: to.to_string(),
                            amount: pending_proxy_rewards,
                        })?,
                    });
                }
                _ => { }
            }
        }
    }

    Ok(messages)
}


// ----------------x----------------x-----------------x----------------x----------------x----------------
// ----------------x----------------x  :::: Migration function   ::::  x----------------x----------------
// ----------------x----------------x-----------------x----------------x----------------x----------------

/// ## Description
/// Used for contract migration. Returns a default object of type [`Response`].
/// 
/// ## Params
/// * **msg** is an object of type [`MigrateMsg`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(mut deps: DepsMut, env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    let contract_version = get_contract_version(deps.storage)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let mut response = Response::new();
    Ok(response
        .add_attribute("previous_contract_name", &contract_version.contract)
        .add_attribute("previous_contract_version", &contract_version.version)
        .add_attribute("new_contract_name", CONTRACT_NAME)
        .add_attribute("new_contract_version", CONTRACT_VERSION))
}