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
        ASSET_STAKER_INFO, CONFIG, LP_ACTIVE_REWARD_ASSETS, OWNERSHIP_PROPOSAL, REWARD_SCHEDULES,
        REWARD_STATES, USER_LP_TOKEN_LOCKS,
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
            denom,
            amount,
            start_block_time,
            end_block_time,
        } => {
            // Verify that the asset for reward was sent with the message
            if info.funds.len() != 1 {
                return Err(ContractError::InvalidNumberOfAssets {
                    correct_number: 1,
                    received_number: info.funds.len() as u8,
                });
            }

            let sender = info.sender.clone();
            let sent_asset = info.funds[0].clone();

            if sent_asset.denom == denom {
                let asset = AssetInfo::NativeToken {
                    denom: denom.clone(),
                };
                // verify that enough amount was sent
                if sent_asset.amount >= amount {
                    let mut response = add_reward_factory(
                        deps,
                        env,
                        info,
                        lp_token,
                        asset.clone(),
                        amount,
                        start_block_time,
                        end_block_time,
                    )?;

                    let extra_amount = sent_asset.amount.checked_sub(amount)?;

                    if extra_amount > Uint128::zero() {
                        response = response.add_message(build_transfer_token_to_user_msg(
                            asset,
                            sender,
                            extra_amount,
                        )?);
                    }

                    Ok(response)
                } else {
                    Err(ContractError::LessAmountReceived {
                        asset: denom,
                        correct_amount: amount,
                        received_amount: sent_asset.amount,
                    })
                }
            } else {
                Err(ContractError::InvalidAsset {
                    correct_asset: denom,
                    received_asset: sent_asset.denom,
                })
            }
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

pub fn add_reward_factory(
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

    let mut lp_active_reward_assets = LP_ACTIVE_REWARD_ASSETS
        .may_load(deps.storage, &lp_token)?
        .unwrap_or_default();

    if !lp_active_reward_assets.contains(&asset) {
        lp_active_reward_assets.push(asset);
    }

    LP_ACTIVE_REWARD_ASSETS.save(deps.storage, &lp_token, &lp_active_reward_assets)?;

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
        }
        Cw20HookMsg::AddRewardSchedule {
            lp_token,
            start_block_time,
            end_block_time,
        } => {
            let token_address = deps.api.addr_validate(info.sender.as_str())?;
            add_reward_factory(
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
    state: &mut AssetRewardState,
    reward_schedules: Vec<RewardSchedule>,
) {
    if state.total_bond_amount.is_zero() {
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
        state.reward_index + Decimal::from_ratio(distributed_amount, state.total_bond_amount);
}

pub fn compute_staker_reward(
    state: &AssetRewardState,
    staker_info: &mut AssetStakerInfo,
) -> StdResult<()> {
    let pending_reward = (staker_info.bond_amount * state.reward_index)
        .checked_sub(staker_info.bond_amount * staker_info.reward_index)?;

    staker_info.reward_index = state.reward_index;
    staker_info.pending_reward += pending_reward;
    Ok(())
}

pub fn increase_bond_amount(
    state: &mut AssetRewardState,
    staker_info: &mut AssetStakerInfo,
    amount: Uint128,
) -> StdResult<()> {
    staker_info.bond_amount = staker_info.bond_amount.checked_add(amount)?;
    state.total_bond_amount = state.total_bond_amount.checked_add(amount)?;
    Ok(())
}

pub fn decrease_bond_amount(
    state: &mut AssetRewardState,
    staker_info: &mut AssetStakerInfo,
    amount: Uint128,
) -> StdResult<()> {
    staker_info.bond_amount = staker_info.bond_amount.checked_sub(amount)?;
    state.total_bond_amount = state.total_bond_amount.checked_sub(amount)?;
    Ok(())
}

fn check_if_lp_token_allowed(config: &Config, lp_token: &Addr) -> ContractResult<()> {
    if !config.allowed_lp_tokens.contains(lp_token) {
        return Err(ContractError::LpTokenNotAllowed);
    }
    Ok(())
}

pub fn bond(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    lp_token: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    check_if_lp_token_allowed(&config, &lp_token)?;

    let lp_active_reward_assets = LP_ACTIVE_REWARD_ASSETS
        .may_load(deps.storage, &lp_token)?
        .unwrap_or_default();

    for asset in lp_active_reward_assets {
        let mut asset_staker_info = ASSET_STAKER_INFO
            .may_load(deps.storage, (&lp_token, &sender, &asset.to_string()))?
            .unwrap_or(AssetStakerInfo {
                asset: asset.clone(),
                bond_amount: Uint128::zero(),
                pending_reward: Uint128::zero(),
                reward_index: Decimal::zero(),
            });

        let mut asset_state = REWARD_STATES
            .may_load(deps.storage, &asset.to_string())?
            .unwrap_or(AssetRewardState {
                total_bond_amount: Uint128::zero(),
                reward_index: Decimal::zero(),
                last_distributed: env.block.time.seconds(),
            });

        let reward_schedules = REWARD_SCHEDULES
            .may_load(
                deps.storage,
                (&lp_token, &asset_staker_info.asset.to_string()),
            )?
            .unwrap_or_default();

        compute_reward(env.block.time.seconds(), &mut asset_state, reward_schedules);
        compute_staker_reward(&mut asset_state, &mut asset_staker_info)?;
        increase_bond_amount(&mut asset_state, &mut asset_staker_info, amount)?;

        REWARD_STATES.save(
            deps.storage,
            &asset_staker_info.asset.to_string(),
            &asset_state,
        )?;

        ASSET_STAKER_INFO.save(
            deps.storage,
            (&lp_token, &sender, &asset_staker_info.asset.to_string()),
            &asset_staker_info,
        )?;
    }

    Ok(Response::default())
}

pub fn unbond(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    lp_token: Addr,
    amount: Uint128,
) -> ContractResult<Response> {
    // We don't have to check for LP token allowed here, because there's a scenario that we allowed bonding
    // for an asset earlier and then we remove the LP token from the list of allowed LP tokens. In this case
    // we still want to allow unbonding.

    let lp_active_reward_assets = LP_ACTIVE_REWARD_ASSETS
        .may_load(deps.storage, &lp_token)?
        .unwrap_or_default();

    let response = Response::new();

    for asset in lp_active_reward_assets {
        let mut asset_staker_info = ASSET_STAKER_INFO
            .may_load(deps.storage, (&lp_token, &sender, &asset.to_string()))?
            .unwrap_or(AssetStakerInfo {
                asset: asset.clone(),
                bond_amount: Uint128::zero(),
                pending_reward: Uint128::zero(),
                reward_index: Decimal::zero(),
            });

        let mut asset_state = REWARD_STATES.load(deps.storage, &asset.to_string())?;

        let reward_schedules = REWARD_SCHEDULES
            .may_load(
                deps.storage,
                (&lp_token, &asset_staker_info.asset.to_string()),
            )?
            .unwrap_or_default();

        compute_reward(env.block.time.seconds(), &mut asset_state, reward_schedules);
        compute_staker_reward(&mut asset_state, &mut asset_staker_info)?;
        decrease_bond_amount(&mut asset_state, &mut asset_staker_info, amount)?;

        REWARD_STATES.save(
            deps.storage,
            &asset_staker_info.asset.to_string(),
            &asset_state,
        )?;

        ASSET_STAKER_INFO.save(
            deps.storage,
            (&lp_token, &sender, &asset_staker_info.asset.to_string()),
            &asset_staker_info,
        )?;
    }

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

pub fn unlock(deps: DepsMut, env: Env, sender: Addr, lp_token: Addr) -> ContractResult<Response> {
    let locks = USER_LP_TOKEN_LOCKS
        .may_load(deps.storage, (&lp_token, &sender))?
        .unwrap_or_default();

    let mut response = Response::new();

    let mut unlocked_amount = Uint128::zero();

    for token_lock in locks.iter() {
        if token_lock.unlock_time <= env.block.time.seconds() {
            unlocked_amount += token_lock.amount;
        }
    }

    let unlocks = locks
        .iter()
        .filter(|lock| lock.unlock_time > env.block.time.seconds())
        .cloned()
        .collect::<Vec<_>>();

    USER_LP_TOKEN_LOCKS.save(deps.storage, (&lp_token, &sender), &unlocks)?;

    if unlocked_amount > Uint128::zero() {
        response = response.add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: lp_token.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: sender.to_string(),
                amount: unlocked_amount,
            })?,
        }));
    }

    Ok(response)
}

pub fn withdraw(
    deps: DepsMut,
    env: Env,
    sender: &Addr,
    lp_token: Addr,
) -> ContractResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    check_if_lp_token_allowed(&config, &lp_token)?;

    let lp_active_reward_assets = LP_ACTIVE_REWARD_ASSETS
        .may_load(deps.storage, &lp_token)?
        .unwrap_or_default();

    let mut transfer_msgs: Vec<CosmosMsg> = vec![];
    for asset in lp_active_reward_assets {
        let mut asset_staker_info = ASSET_STAKER_INFO
            .may_load(deps.storage, (&lp_token, &sender, &asset.to_string()))?
            .unwrap_or(AssetStakerInfo {
                asset: asset.clone(),
                bond_amount: Uint128::zero(),
                pending_reward: Uint128::zero(),
                reward_index: Decimal::zero(),
            });

        let mut asset_state = REWARD_STATES.load(deps.storage, &asset.to_string())?;

        let reward_schedules = REWARD_SCHEDULES
            .may_load(
                deps.storage,
                (&lp_token, &asset_staker_info.asset.to_string()),
            )?
            .unwrap_or_default();

        compute_reward(env.block.time.seconds(), &mut asset_state, reward_schedules);
        compute_staker_reward(&mut asset_state, &mut asset_staker_info)?;

        transfer_msgs.push(build_transfer_token_to_user_msg(
            asset,
            sender.clone(),
            asset_staker_info.pending_reward,
        )?);
        asset_staker_info.pending_reward = Uint128::zero();

        REWARD_STATES.save(
            deps.storage,
            &asset_staker_info.asset.to_string(),
            &asset_state,
        )?;

        ASSET_STAKER_INFO.save(
            deps.storage,
            (&lp_token, &sender, &asset_staker_info.asset.to_string()),
            &asset_staker_info,
        )?;
    }

    Ok(Response::default().add_messages(transfer_msgs))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    match msg {
        QueryMsg::UnclaimedRewards {
            lp_token,
            user,
            block_time,
        } => {
            let assets_for_lp = LP_ACTIVE_REWARD_ASSETS
                .may_load(deps.storage, &lp_token)?
                .unwrap_or_default();

            let mut reward_info = vec![];
            for asset in assets_for_lp {
                let mut asset_staker_info = ASSET_STAKER_INFO
                    .may_load(deps.storage, (&lp_token, &user, &asset.to_string()))?
                    .unwrap_or(AssetStakerInfo {
                        asset: asset.clone(),
                        bond_amount: Uint128::zero(),
                        pending_reward: Uint128::zero(),
                        reward_index: Decimal::zero(),
                    });

                let block_time = block_time.unwrap_or(env.block.time.seconds());

                if block_time < env.block.time.seconds() {
                    return Err(ContractError::BlockTimeInPast);
                }

                let mut asset_state = REWARD_STATES.load(deps.storage, &asset.to_string())?;

                let reward_schedules = REWARD_SCHEDULES
                    .may_load(
                        deps.storage,
                        (&lp_token, &asset_staker_info.asset.to_string()),
                    )?
                    .unwrap_or_default();

                compute_reward(block_time, &mut asset_state, reward_schedules);
                compute_staker_reward(&mut asset_state, &mut asset_staker_info)?;

                reward_info.push(UnclaimedReward {
                    asset: asset.clone(),
                    amount: asset_staker_info.pending_reward,
                });
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
    }
}
