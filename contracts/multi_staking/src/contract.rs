#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, Uint128, WasmMsg,
};

use dexter::{
    asset::AssetInfo,
    helper::{
        build_transfer_token_to_user_msg, claim_ownership, drop_ownership_proposal,
        propose_new_owner,
    },
    multi_staking::{
        AssetRewardState, AssetStakerInfo, Config, Cw20HookMsg, ExecuteMsg, InstantiateMsg,
        QueryMsg, RewardSchedule, TokenLock, TokenLockInfo, UnclaimedReward,
    },
};

use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use crate::{
    error::ContractError,
    state::{
        ASSET_STAKER_INFO, CONFIG, OWNERSHIP_PROPOSAL, REWARD_SCHEDULES,
        USER_LP_TOKEN_LOCKS, USER_BONDED_LP_TOKENS, LP_GLOBAL_STATE, ASSET_LP_REWARD_STATE,
    },
};
type ContractResult<T> = Result<T, ContractError>;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    CONFIG.save(
        deps.storage,
        &Config {
            unlock_period: msg.unlock_period,
            owner: msg.owner,
            allowed_lp_tokens: vec![],
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<Response> {
    match msg {
        ExecuteMsg::AllowLpToken { lp_token } => allow_lp_token(deps, env, info, lp_token),
        ExecuteMsg::RemoveLpToken { lp_token } => {
            remove_lp_token_from_allowed_list(deps, info, &lp_token)
        }
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::AddRewardSchedule {
            lp_token,
            start_block_time,
            end_block_time,
        } => {
            // Verify that no more than one asset is sent with the message for reward distribution
            if info.funds.len() != 1 {
                return Err(ContractError::InvalidNumberOfAssets {
                    correct_number: 1,
                    received_number: info.funds.len() as u8,
                });
            }

            let sent_asset = info.funds[0].clone();

            add_reward_schedule(
                deps,
                env,
                info,
                lp_token,
                AssetInfo::NativeToken { 
                    denom: sent_asset.denom,
                },
                sent_asset.amount,
                start_block_time,
                end_block_time,
            )
        }
        ExecuteMsg::Bond { lp_token, amount } => {
            let sender = info.sender;

            // Transfer the lp token to the contract
            let transfer_msg = CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: lp_token.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: sender.to_string(),
                    recipient: env.contract.address.to_string(),
                    amount,
                })?,
            });

            let response = bond(deps, env, sender, lp_token, amount)?;
            Ok(response.add_message(transfer_msg))
        }
        ExecuteMsg::Unbond { lp_token, amount } => unbond(deps, env, info.sender, lp_token, amount),
        ExecuteMsg::Unlock { lp_token } => unlock(deps, env, info.sender, lp_token),
        ExecuteMsg::Withdraw { lp_token } => withdraw(deps, env, &info.sender, lp_token),
        ExecuteMsg::ProposeNewOwner { owner, expires_in } => {
            let config = CONFIG.load(deps.storage)?;
            let response = propose_new_owner(
                deps,
                info,
                env,
                owner.to_string(),
                expires_in,
                config.owner,
                OWNERSHIP_PROPOSAL,
            )?;
            Ok(response)
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

fn allow_lp_token(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    lp_token: Addr,
) -> Result<Response, ContractError> {
    // validate if owner sent the message
    let mut config = CONFIG.load(deps.storage)?;
    if config.owner != info.sender {
        return Err(ContractError::Unauthorized);
    }

    // verify that lp token is not already allowed
    if config.allowed_lp_tokens.contains(&lp_token) {
        return Err(ContractError::LpTokenAlreadyAllowed);
    }

    config.allowed_lp_tokens.push(lp_token);
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::default())
}

fn remove_lp_token_from_allowed_list(
    deps: DepsMut,
    info: MessageInfo,
    lp_token: &Addr,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    // validate if owner sent the message
    if config.owner != info.sender {
        return Err(ContractError::Unauthorized);
    }

    config.allowed_lp_tokens.retain(|x| x != lp_token);
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

pub fn add_reward_schedule(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    lp_token: Addr,
    asset: AssetInfo,
    amount: Uint128,
    start_block_time: u64,
    end_block_time: u64,
) -> ContractResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    check_if_lp_token_allowed(&config, &lp_token)?;

    // validate block times
    if start_block_time >= end_block_time {
        return Err(ContractError::InvalidBlockTimes {
            start_block_time,
            end_block_time,
        });
    }

    if start_block_time < env.block.time.seconds() {
        return Err(ContractError::BlockTimeInPast);
    }

    let mut lp_global_state = LP_GLOBAL_STATE
        .may_load(deps.storage, &lp_token)?
        .unwrap_or_default();

    if !lp_global_state.active_reward_assets.contains(&asset) {
        lp_global_state.active_reward_assets.push(asset.clone());
    }

    LP_GLOBAL_STATE.save(deps.storage, &lp_token, &lp_global_state)?;

    let reward_schedule = RewardSchedule {
        asset: asset.clone(),
        amount,
        staking_lp_token: lp_token.clone(),
        start_block_time,
        end_block_time,
    };

    let mut reward_schedules = REWARD_SCHEDULES
        .may_load(deps.storage, (&lp_token, &asset.to_string()))?
        .unwrap_or_default();

    reward_schedules.push(reward_schedule);

    REWARD_SCHEDULES.save(
        deps.storage,
        (&lp_token, &asset.to_string()),
        &reward_schedules,
    )?;

    Ok(Response::default())
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::Bond {} => {
            let token_address = deps.api.addr_validate(info.sender.as_str())?;
            let cw20_sender = deps.api.addr_validate(&cw20_msg.sender)?;
            bond(deps, env, cw20_sender, token_address, cw20_msg.amount)
        },
        Cw20HookMsg::BondForBeneficiary { beneficiary } => {
            let token_address = deps.api.addr_validate(info.sender.as_str())?;
            bond(deps, env, beneficiary, token_address, cw20_msg.amount)
        }
        Cw20HookMsg::AddRewardSchedule {
            lp_token,
            start_block_time,
            end_block_time,
        } => {
            let token_address = deps.api.addr_validate(info.sender.as_str())?;
            add_reward_schedule(
                deps,
                env,
                info,
                lp_token,
                AssetInfo::Token {
                    contract_addr: token_address,
                },
                cw20_msg.amount,
                start_block_time,
                end_block_time,
            )
        }
    }
}

pub fn compute_reward(
    current_block_time: u64,
    total_bond_amount: Uint128,
    state: &mut AssetRewardState,
    reward_schedules: Vec<RewardSchedule>,
) {
    if total_bond_amount.is_zero() {
        state.last_distributed = current_block_time;
        return;
    }

    let mut distributed_amount: Uint128 = Uint128::zero();
    for s in reward_schedules.iter() {
        let start_time = s.start_block_time;
        let end_time = s.end_block_time;

        if start_time > current_block_time || end_time < state.last_distributed {
            continue;
        }

        // min(s.1, block_time) - max(s.0, last_distributed)
        let passed_time = std::cmp::min(end_time, current_block_time)
            - std::cmp::max(start_time, state.last_distributed);
 
        let time = end_time - start_time;
        let distribution_amount_per_second: Decimal = Decimal::from_ratio(s.amount, time);
        distributed_amount += distribution_amount_per_second * Uint128::from(passed_time as u128);
    }

    state.last_distributed = current_block_time;
    state.reward_index =
        state.reward_index + Decimal::from_ratio(distributed_amount, total_bond_amount);
}

pub fn compute_staker_reward(
    bond_amount: Uint128,
    state: &AssetRewardState,
    staker_info: &mut AssetStakerInfo,
) -> StdResult<()> {
    let pending_reward = bond_amount * (state.reward_index.checked_sub(staker_info.reward_index)?);
    staker_info.reward_index = state.reward_index;
    staker_info.pending_reward = staker_info.pending_reward.checked_add(pending_reward)?;
    Ok(())
}

fn check_if_lp_token_allowed(config: &Config, lp_token: &Addr) -> ContractResult<()> {
    if !config.allowed_lp_tokens.contains(lp_token) {
        return Err(ContractError::LpTokenNotAllowed);
    }
    Ok(())
}

pub fn bond(
    mut deps: DepsMut,
    env: Env,
    user: Addr,
    lp_token: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    check_if_lp_token_allowed(&config, &lp_token)?;

    let current_bond_amount = USER_BONDED_LP_TOKENS
        .may_load(deps.storage, (&lp_token, &user))?
        .unwrap_or_default();

    let mut lp_global_state = LP_GLOBAL_STATE.may_load(deps.storage, &lp_token)?.unwrap_or_default();
    let mut response = Response::default();

    for asset in &lp_global_state.active_reward_assets {
        update_staking_rewards(
            asset,
            &lp_token,
            &user,
            lp_global_state.total_bond_amount,
            current_bond_amount,
            env.block.time.seconds(),
            &mut deps,
            &mut response,
            None
        )?;
    }

    // Increase bond amount
    lp_global_state.total_bond_amount = lp_global_state.total_bond_amount.checked_add(amount)?;
    LP_GLOBAL_STATE.save(deps.storage, &lp_token, &lp_global_state)?;

    // Increase user bond amount
    USER_BONDED_LP_TOKENS.save(
        deps.storage,
        (&lp_token, &user),
        &(current_bond_amount.checked_add(amount)?),
    )?;

    Ok(response)
}

/// Unbond LP tokens
pub fn unbond(
    mut deps: DepsMut,
    env: Env,
    sender: Addr,
    lp_token: Addr,
    amount: Uint128,
) -> ContractResult<Response> {
    // We don't have to check for LP token allowed here, because there's a scenario that we allowed bonding
    // for an asset earlier and then we remove the LP token from the list of allowed LP tokens. In this case
    // we still want to allow unbonding.
    let mut response = Response::new();

    let current_bond_amount = USER_BONDED_LP_TOKENS
        .may_load(deps.storage, (&lp_token, &sender))?
        .unwrap_or_default();

    let mut lp_global_state = LP_GLOBAL_STATE.load(deps.storage, &lp_token)?;
    for asset in &lp_global_state.active_reward_assets {
        update_staking_rewards(
            asset,
            &lp_token,
            &sender,
            lp_global_state.total_bond_amount,
            current_bond_amount,
            env.block.time.seconds(),
            &mut deps,
            &mut response,
            None
        )?;
    }

    // Decrease bond amount
    lp_global_state.total_bond_amount = lp_global_state.total_bond_amount.checked_sub(amount)?;
    LP_GLOBAL_STATE.save(deps.storage, &lp_token, &lp_global_state)?;

    USER_BONDED_LP_TOKENS.save(
        deps.storage,
        (&lp_token, &sender),
        &(current_bond_amount.checked_sub(amount)?),
    )?;

    // Start unlocking clock for the user's LP Tokens
    let mut unlocks = USER_LP_TOKEN_LOCKS
        .may_load(deps.storage, (&lp_token, &sender))?
        .unwrap_or_default();

    let config = CONFIG.load(deps.storage)?;

    unlocks.push(TokenLock {
        unlock_time: env.block.time.seconds() + config.unlock_period,
        amount,
    });

    USER_LP_TOKEN_LOCKS.save(deps.storage, (&lp_token, &sender), &unlocks)?;

    Ok(response)
}

pub fn calculate_bonded_amount(
    deps: Deps,
    env: Env,
    sender: Addr,
    lp_token: Addr,
) -> ContractResult<Uint128> {
    let config = CONFIG.load(deps.storage)?;
    check_if_lp_token_allowed(&config, &lp_token)?;

    let current_bond_amount = USER_BONDED_LP_TOKENS
        .may_load(deps.storage, (&lp_token, &sender))?
        .unwrap_or_default();
    
    // deduct amount that is unlocked and due for withdrawal from bonded amount
    let mut unlocked_amount = Uint128::zero();
    let unlocks = USER_LP_TOKEN_LOCKS
        .may_load(deps.storage, (&lp_token, &sender))?
        .unwrap_or_default();
    let mut new_unlocks = vec![];
    for lock in unlocks.iter() {
        if lock.unlock_time <= env.block.time.seconds() {
            unlocked_amount += lock.amount;
        } else {
            new_unlocks.push(lock.clone());
        }
    }

    let bonded_amount = current_bond_amount.checked_sub(unlocked_amount)?;
    Ok(bonded_amount)
}

pub fn update_staking_rewards(
    asset: &AssetInfo,
    lp_token: &Addr,
    user: &Addr,
    total_bond_amount: Uint128,
    current_bond_amount: Uint128,
    current_block_time: u64,
    deps: &mut DepsMut,
    response: &mut Response,
    operation_post_update: Option<fn(&Addr, &mut AssetRewardState, &mut AssetStakerInfo, &mut Response) -> ContractResult<()>>,
) -> ContractResult<()> {
    let mut asset_staker_info = ASSET_STAKER_INFO
        .may_load(deps.storage, (&lp_token, &user, &asset.to_string()))?
        .unwrap_or(AssetStakerInfo {
            asset: asset.clone(),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::zero(),
        });

    let mut asset_state = ASSET_LP_REWARD_STATE
        .may_load(deps.storage, (&asset.to_string(), &lp_token))?
        .unwrap_or(AssetRewardState {
            reward_index: Decimal::zero(),
            last_distributed: 0,
        });

    let reward_schedules = REWARD_SCHEDULES
        .may_load(
            deps.storage,
            (&lp_token, &asset.to_string()),
        )?
        .unwrap_or_default();

    compute_reward(current_block_time, total_bond_amount, &mut asset_state, reward_schedules);
    compute_staker_reward(current_bond_amount, &mut asset_state, &mut asset_staker_info)?;

    if let Some(operation) = operation_post_update {
        operation(user, &mut asset_state, &mut asset_staker_info, response)?;
    }

    ASSET_LP_REWARD_STATE.save(
        deps.storage,
        (&asset.to_string(), &lp_token),
        &asset_state,
    )?;

    ASSET_STAKER_INFO.save(
        deps.storage,
        (&lp_token, &user, &asset.to_string()),
        &asset_staker_info,
    )?;

    Ok(())
}

pub fn unlock(deps: DepsMut, env: Env, sender: Addr, lp_token: Addr) -> ContractResult<Response> {
    let locks = USER_LP_TOKEN_LOCKS
        .may_load(deps.storage, (&lp_token, &sender))?
        .unwrap_or_default();

    let mut response = Response::new();
    let total_unlocked_amount = locks
        .iter()
        .filter(|lock| lock.unlock_time <= env.block.time.seconds())
        .fold(Uint128::zero(), |acc, lock| acc + lock.amount);

    let updated_unlocks = locks
        .into_iter()
        .filter(|lock| lock.unlock_time > env.block.time.seconds())
        .collect::<Vec<TokenLock>>();

    USER_LP_TOKEN_LOCKS.save(deps.storage, (&lp_token, &sender), &updated_unlocks)?;
    
    if total_unlocked_amount.is_zero() {
        return Ok(response);
    } 

    response = response.add_message(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_token.to_string(),
        funds: vec![],
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: sender.to_string(),
            amount: total_unlocked_amount,
        })?,
    }));
    
    Ok(response)
}

fn withdraw_pending_reward(
    user: &Addr,
    asset_reward_state: &mut AssetRewardState,
    asset_staker_info: &mut AssetStakerInfo,
    response: &mut Response,
) -> ContractResult<()> {
    let pending_reward = asset_staker_info.pending_reward;
    
    if pending_reward > Uint128::zero() {
        let res = response.clone().add_message(
            build_transfer_token_to_user_msg(
                asset_staker_info.asset.clone(),
                user.clone(),
                pending_reward,
            )?,
        );
        *response = res;
    }

    asset_staker_info.pending_reward = Uint128::zero();
    asset_staker_info.reward_index = asset_reward_state.reward_index;
    Ok(())
}

pub fn withdraw(
    mut deps: DepsMut,
    env: Env,
    sender: &Addr,
    lp_token: Addr,
) -> ContractResult<Response> {
    let mut response = Response::new();
    let current_bonded_amount = USER_BONDED_LP_TOKENS
        .may_load(deps.storage, (&lp_token, &sender))?
        .unwrap_or_default();
    
    let lp_global_state = LP_GLOBAL_STATE.load(deps.storage, &lp_token)?;

    for asset in &lp_global_state.active_reward_assets {
        update_staking_rewards(
            asset,
            &lp_token,
            &sender,
            lp_global_state.total_bond_amount,
            current_bonded_amount,
            env.block.time.seconds(),
            &mut deps,
            &mut response,
            Some(withdraw_pending_reward)
        )?;
    }

    Ok(response)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    match msg {
        QueryMsg::BondedLpTokens { lp_token, user } => {
            let bonded_amount = USER_BONDED_LP_TOKENS
                .may_load(deps.storage, (&lp_token, &user))?
                .unwrap_or_default();
            to_binary(&bonded_amount).map_err(ContractError::from)
        },
        QueryMsg::UnclaimedRewards {
            lp_token,
            user,
            block_time,
        } => {

            let current_bonded_amount = USER_BONDED_LP_TOKENS
                .may_load(deps.storage, (&lp_token, &user))?
                .unwrap_or_default();
            
            let lp_global_state = LP_GLOBAL_STATE.load(deps.storage, &lp_token)?;

            let mut reward_info = vec![];
            let block_time = block_time.unwrap_or(env.block.time.seconds());

            if block_time < env.block.time.seconds() {
                return Err(ContractError::BlockTimeInPast);
            }

            for asset in lp_global_state.active_reward_assets {
                let mut asset_staker_info = ASSET_STAKER_INFO
                    .may_load(deps.storage, (&lp_token, &user, &asset.to_string()))?
                    .unwrap_or(AssetStakerInfo {
                        asset: asset.clone(),
                        pending_reward: Uint128::zero(),
                        reward_index: Decimal::zero(),
                    });

                let mut asset_state = ASSET_LP_REWARD_STATE
                    .may_load(deps.storage, (&asset.to_string(), &lp_token))?
                    .unwrap_or(AssetRewardState {
                        reward_index: Decimal::zero(),
                        last_distributed: block_time,
                    });

                let reward_schedules = REWARD_SCHEDULES
                    .may_load(
                        deps.storage,
                        (&lp_token, &asset.to_string()),
                    )?
                    .unwrap_or_default();

                compute_reward(block_time, lp_global_state.total_bond_amount, &mut asset_state, reward_schedules);
                compute_staker_reward(current_bonded_amount, &mut asset_state, &mut asset_staker_info)?;
                
                if asset_staker_info.pending_reward > Uint128::zero() {
                    reward_info.push(UnclaimedReward {
                        asset: asset.clone(),
                        amount: asset_staker_info.pending_reward,
                    });
                }
            }

            to_binary(&reward_info).map_err(ContractError::from)
        }
        QueryMsg::AllowedLPTokensForReward {} => {
            let config = CONFIG.load(deps.storage)?;
            let allowed_lp_tokens = config.allowed_lp_tokens;
            to_binary(&allowed_lp_tokens).map_err(ContractError::from)
        }
        QueryMsg::Owner {} => {
            let config = CONFIG.load(deps.storage)?;
            to_binary(&config.owner).map_err(ContractError::from)
        }
        QueryMsg::RewardSchedules { lp_token, asset } => {
            let reward_schedules = REWARD_SCHEDULES
                .may_load(deps.storage, (&lp_token, &asset.to_string()))?
                .unwrap_or_default();
            to_binary(&reward_schedules).map_err(ContractError::from)
        }
        QueryMsg::TokenLocks {
            lp_token,
            user,
            block_time,
        } => {
            let mut locks = USER_LP_TOKEN_LOCKS
                .may_load(deps.storage, (&lp_token, &user))?
                .unwrap_or_default();

            let mut unlocked_amount = Uint128::zero();
            let mut filtered_locks = vec![];

            let block_time = block_time.unwrap_or(env.block.time.seconds());
            for lock in locks.iter_mut() {
                if lock.unlock_time < block_time {
                    unlocked_amount += lock.amount;
                    lock.amount = Uint128::zero();
                } else {
                    filtered_locks.push(lock.clone());
                }
            }

            to_binary(&TokenLockInfo {
                unlocked_amount,
                locks: filtered_locks,
            }).map_err(ContractError::from)
        }
        QueryMsg::RewardState { lp_token, asset } => {
            let reward_state = ASSET_LP_REWARD_STATE
                .may_load(deps.storage, (&asset.to_string(), &lp_token))?;

            match reward_state {
                Some(reward_state) => to_binary(&reward_state).map_err(ContractError::from),
                None => Err(ContractError::NoRewardState),
            }
        }
        QueryMsg::StakerInfo {
            lp_token,
            asset,
            user,
        } => {
            let reward_state = ASSET_STAKER_INFO
                .may_load(deps.storage, (&lp_token, &user, &asset.to_string()))?;

            match reward_state {
                Some(reward_state) => to_binary(&reward_state).map_err(ContractError::from),
                None => Err(ContractError::NoUserRewardState),
            }
        }

    }
}