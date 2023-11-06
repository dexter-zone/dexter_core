#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use const_format::concatcp;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env, Event,
    MessageInfo, Response, StdError, StdResult, Storage, Uint128, WasmMsg,
};
use std::{cmp::min, collections::HashMap};

use dexter::{
    asset::AssetInfo,
    helper::{
        build_transfer_token_to_user_msg, claim_ownership, drop_ownership_proposal,
        propose_new_owner,
    },
    multi_staking::{
        AssetRewardState, AssetStakerInfo, Config, ConfigV1,
        CreatorClaimableRewardState, Cw20HookMsg, ExecuteMsg, InstantLpUnlockFee, InstantiateMsg,
        MigrateMsg, QueryMsg, RewardSchedule, TokenLockInfo, UnclaimedReward, ConfigV2_1, ConfigV2_2,
    },
};

use cw2::{get_contract_version, set_contract_version};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw_storage_plus::Item;
use dexter::asset::Asset;
use dexter::helper::EventExt;
use dexter::multi_staking::{
    RewardScheduleResponse, MAX_ALLOWED_LP_TOKENS, MAX_INSTANT_UNBOND_FEE_BP,
};

use crate::{
    error::ContractError,
    state::{
        next_reward_schedule_id, ASSET_LP_REWARD_STATE, ASSET_STAKER_INFO, CONFIG,
        CREATOR_CLAIMABLE_REWARD, LP_GLOBAL_STATE, LP_TOKEN_ASSET_REWARD_SCHEDULE,
        OWNERSHIP_PROPOSAL, REWARD_SCHEDULES, USER_BONDED_LP_TOKENS, USER_LP_TOKEN_LOCKS,
    },
};
use crate::{
    execute::{
        unbond::{instant_unbond, unbond},
        unlock::{instant_unlock, unlock},
    },
    query::query_instant_unlock_fee_tiers,
    utils::calculate_unlock_fee,
};

/// Contract name that is used for migration.
pub const CONTRACT_NAME: &str = "dexter-multi-staking";

const CONTRACT_VERSION_V1: &str = "1.0.0";
const CONTRACT_VERSION_V2: &str = "2.0.0";
const CONTRACT_VERSION_V2_1: &str = "2.1.0";
const CONTRACT_VERSION_V2_2: &str = "2.2.0";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub type ContractResult<T> = Result<T, ContractError>;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    if msg.instant_unbond_fee_bp > MAX_INSTANT_UNBOND_FEE_BP {
        return Err(ContractError::InvalidInstantUnbondFee {
            max_allowed: MAX_INSTANT_UNBOND_FEE_BP,
            received: msg.instant_unbond_fee_bp,
        });
    }

    if msg.instant_unbond_min_fee_bp > msg.instant_unbond_fee_bp {
        return Err(ContractError::InvalidInstantUnbondMinFee {
            max_allowed: msg.instant_unbond_fee_bp,
            received: msg.instant_unbond_min_fee_bp,
        });
    }

    if msg.fee_tier_interval > msg.unlock_period {
        return Err(ContractError::InvalidFeeTierInterval {
            max_allowed: msg.unlock_period,
            received: msg.fee_tier_interval,
        });
    }

    // validate keeper address
    deps.api.addr_validate(&msg.keeper_addr.to_string())?;

    CONFIG.save(
        deps.storage,
        &Config {
            keeper: msg.keeper_addr,
            unlock_period: msg.unlock_period,
            owner: deps.api.addr_validate(msg.owner.as_str())?,
            allowed_lp_tokens: vec![],
            instant_unbond_fee_bp: msg.instant_unbond_fee_bp,
            instant_unbond_min_fee_bp: msg.instant_unbond_min_fee_bp,
            fee_tier_interval: msg.fee_tier_interval,
        },
    )?;

    Ok(Response::new().add_event(
        Event::from_info(concatcp!(CONTRACT_NAME, "::instantiate"), &info)
            .add_attribute("owner", msg.owner.to_string())
            .add_attribute("unlock_period", msg.unlock_period.to_string())
            .add_attribute(
                "minimum_reward_schedule_proposal_start_delay",
                msg.minimum_reward_schedule_proposal_start_delay.to_string(),
            ),
    ))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<Response> {
    match msg {
        ExecuteMsg::UpdateConfig {
            keeper_addr,
            unlock_period,
            instant_unbond_fee_bp,
            instant_unbond_min_fee_bp,
            fee_tier_interval,
        } => update_config(
            deps,
            env,
            info,
            keeper_addr,
            unlock_period,
            instant_unbond_fee_bp,
            instant_unbond_min_fee_bp,
            fee_tier_interval,
        ),
        ExecuteMsg::AllowLpToken { lp_token } => allow_lp_token(deps, env, info, lp_token),
        ExecuteMsg::RemoveLpToken { lp_token } => remove_lp_token(deps, info, &lp_token),
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::CreateRewardSchedule {
            lp_token,
            title,
            actual_creator,
            start_block_time,
            end_block_time,
        } => {
            // only owner can create reward schedule
            let config = CONFIG.load(deps.storage)?;
            if info.sender != config.owner {
                return Err(ContractError::Unauthorized);
            }

            // Verify that no more than one asset is sent with the message for reward distribution
            if info.funds.len() != 1 {
                return Err(ContractError::InvalidNumberOfAssets {
                    correct_number: 1,
                    received_number: info.funds.len() as u8,
                });
            }

            let sent_asset = info.funds[0].clone();
            let creator = match actual_creator {
                Some(creator) => deps.api.addr_validate(&creator.to_string())?,
                None => info.sender.clone(),
            };

            create_reward_schedule(
                deps,
                env,
                info,
                lp_token,
                title,
                start_block_time,
                end_block_time,
                creator,
                Asset::new_native(sent_asset.denom, sent_asset.amount),
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

            let response = bond(deps, env, sender.clone(), sender, lp_token, amount)?;
            Ok(response.add_message(transfer_msg))
        }
        ExecuteMsg::Unbond { lp_token, amount } => unbond(deps, env, info, lp_token, amount),
        ExecuteMsg::InstantUnbond { lp_token, amount } => {
            instant_unbond(deps, env, info, lp_token, amount)
        }
        ExecuteMsg::Unlock { lp_token } => unlock(deps, env, info, lp_token),
        ExecuteMsg::InstantUnlock {
            lp_token,
            token_locks,
        } => instant_unlock(deps, &env, &info, &lp_token, token_locks),
        ExecuteMsg::Withdraw { lp_token } => withdraw(deps, env, info, lp_token),
        ExecuteMsg::ClaimUnallocatedReward { reward_schedule_id } => {
            claim_unallocated_reward(deps, env, info, reward_schedule_id)
        }
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
                CONTRACT_NAME,
            )?;
            Ok(response)
        }
        ExecuteMsg::DropOwnershipProposal {} => {
            let config: Config = CONFIG.load(deps.storage)?;

            drop_ownership_proposal(deps, info, config.owner, OWNERSHIP_PROPOSAL, CONTRACT_NAME)
                .map_err(|e| e.into())
        }
        ExecuteMsg::ClaimOwnership {} => claim_ownership(
            deps,
            info,
            env,
            OWNERSHIP_PROPOSAL,
            |deps, new_owner| {
                CONFIG.update::<_, StdError>(deps.storage, |mut v| {
                    v.owner = new_owner;
                    Ok(v)
                })?;

                Ok(())
            },
            CONTRACT_NAME,
        )
        .map_err(|e| e.into()),
    }
}

fn update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    keeper_addr: Option<Addr>,
    unlock_period: Option<u64>,
    instant_unbond_fee_bp: Option<u64>,
    instant_unbond_min_fee_bp: Option<u64>,
    fee_tier_interval: Option<u64>,
) -> ContractResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // Verify that the message sender is the owner
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized);
    }

    let mut event = Event::from_info(concatcp!(CONTRACT_NAME, "::update_config"), &info);

    if let Some(keeper_addr) = keeper_addr {
        config.keeper = keeper_addr.clone();
        event = event.add_attribute("keeper_addr", keeper_addr.to_string());
    }

    if let Some(unlock_period) = unlock_period {
        // validate if unlock period is greater than the fee tier interval, then reset the fee tier interval to unlock period as well
        if fee_tier_interval.is_some() && fee_tier_interval.unwrap() > unlock_period {
            return Err(ContractError::InvalidFeeTierInterval {
                max_allowed: unlock_period,
                received: fee_tier_interval.unwrap(),
            });
        }

        // reset the current fee tier interval to unlock period if it is greater than unlock period
        if config.fee_tier_interval > unlock_period {
            config.fee_tier_interval = unlock_period;
            event = event.add_attribute("fee_tier_interval", config.fee_tier_interval.to_string());
        }

        config.unlock_period = unlock_period;
        event = event.add_attribute("unlock_period", config.unlock_period.to_string());
    }

    if let Some(instant_unbond_fee_bp) = instant_unbond_fee_bp {
        // validate max allowed instant unbond fee which is 10%
        if instant_unbond_fee_bp > MAX_INSTANT_UNBOND_FEE_BP {
            return Err(ContractError::InvalidInstantUnbondFee {
                max_allowed: MAX_INSTANT_UNBOND_FEE_BP,
                received: instant_unbond_fee_bp,
            });
        }
        config.instant_unbond_fee_bp = instant_unbond_fee_bp;
        event = event.add_attribute(
            "instant_unbond_fee_bp",
            config.instant_unbond_fee_bp.to_string(),
        );
    }

    if let Some(instant_unbond_min_fee_bp) = instant_unbond_min_fee_bp {
        // validate min allowed instant unbond fee max value which is 10% and lesser than the instant unbond fee
        if instant_unbond_min_fee_bp > MAX_INSTANT_UNBOND_FEE_BP
            || instant_unbond_min_fee_bp > config.instant_unbond_fee_bp
        {
            return Err(ContractError::InvalidInstantUnbondMinFee {
                max_allowed: min(config.instant_unbond_fee_bp, MAX_INSTANT_UNBOND_FEE_BP),
                received: instant_unbond_min_fee_bp,
            });
        }

        config.instant_unbond_min_fee_bp = instant_unbond_min_fee_bp;
        event = event.add_attribute(
            "instant_unbond_min_fee_bp",
            config.instant_unbond_min_fee_bp.to_string(),
        );
    }

    if let Some(fee_tier_interval) = fee_tier_interval {
        // max allowed fee tier interval in equal to the unlock period.
        if fee_tier_interval > config.unlock_period {
            return Err(ContractError::InvalidFeeTierInterval {
                max_allowed: config.unlock_period,
                received: fee_tier_interval,
            });
        }

        config.fee_tier_interval = fee_tier_interval;
        event = event.add_attribute("fee_tier_interval", config.fee_tier_interval.to_string());
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_event(event))
}

/// Claim unallocated reward for a reward schedule by the creator. This is useful when there was no tokens bonded for a certain
/// time period during reward schedule and the reward schedule creator wants to claim the unallocated amount.
fn claim_unallocated_reward(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    reward_schedule_id: u64,
) -> ContractResult<Response> {
    let reward_schedule = REWARD_SCHEDULES.load(deps.storage, reward_schedule_id)?;
    let mut creator_claimable_reward_state = CREATOR_CLAIMABLE_REWARD
        .may_load(deps.storage, reward_schedule_id)?
        .unwrap_or_default();

    // Verify that the message sender is the reward schedule creator
    if info.sender != reward_schedule.creator {
        return Err(ContractError::Unauthorized);
    }

    // Verify that the reward schedule is not active
    if reward_schedule.end_block_time > env.block.time.seconds() {
        return Err(ContractError::RewardScheduleIsActive);
    }

    // Verify that the reward schedule is not already claimed
    if creator_claimable_reward_state.claimed {
        return Err(ContractError::UnallocatedRewardAlreadyClaimed);
    }

    // if no user activity happened after the last time rewards were computed for this reward schedule
    // and before the reward schedule ended, then the creator claimable reward amount would be less
    // than what it should be if there was nothing bonded for this LP token during that time.
    compute_creator_claimable_reward(
        deps.storage,
        env,
        &reward_schedule,
        &mut creator_claimable_reward_state,
    )?;

    // Verify that the reward schedule has unclaimed reward
    if creator_claimable_reward_state.amount.is_zero() {
        return Err(ContractError::NoUnallocatedReward);
    }

    // Update the reward schedule to be claimed
    creator_claimable_reward_state.claimed = true;
    CREATOR_CLAIMABLE_REWARD.save(
        deps.storage,
        reward_schedule_id,
        &creator_claimable_reward_state,
    )?;

    // Send the unclaimed reward to the reward schedule creator
    let msg = build_transfer_token_to_user_msg(
        reward_schedule.asset.clone(),
        reward_schedule.creator,
        creator_claimable_reward_state.amount,
    )?;

    let event = Event::from_info(
        concatcp!(CONTRACT_NAME, "::claim_unallocated_reward"),
        &info,
    )
    .add_attribute("reward_schedule_id", reward_schedule_id.to_string())
    .add_attribute("asset", reward_schedule.asset.as_string())
    .add_attribute("amount", creator_claimable_reward_state.amount.to_string());

    Ok(Response::new().add_event(event).add_message(msg))
}

fn compute_creator_claimable_reward(
    store: &dyn Storage,
    env: Env,
    reward_schedule: &RewardSchedule,
    creator_claimable_reward_state: &mut CreatorClaimableRewardState,
) -> ContractResult<()> {
    let lp_global_state = LP_GLOBAL_STATE
        .may_load(store, &reward_schedule.staking_lp_token)?
        .unwrap_or_default();
    let asset_state = ASSET_LP_REWARD_STATE
        .may_load(
            store,
            (
                &reward_schedule.asset.to_string(),
                &reward_schedule.staking_lp_token,
            ),
        )?
        .unwrap_or(AssetRewardState {
            reward_index: Decimal::zero(),
            last_distributed: 0,
        });
    let current_block_time = env.block.time.seconds();

    if lp_global_state.total_bond_amount.is_zero()
        && asset_state.last_distributed < reward_schedule.end_block_time
    {
        let start_time = reward_schedule.start_block_time;
        let end_time = reward_schedule.end_block_time;

        // this case is possible during the query
        if start_time > current_block_time {
            return Ok(());
        }

        // min(s.1, block_time) - max(s.0, last_distributed)
        let passed_time = std::cmp::min(end_time, current_block_time)
            - std::cmp::max(start_time, asset_state.last_distributed);

        let time = end_time - start_time;
        let distribution_amount_per_second: Decimal =
            Decimal::from_ratio(reward_schedule.amount, time);
        let distributed_amount =
            distribution_amount_per_second * Uint128::from(passed_time as u128);

        creator_claimable_reward_state.amount = creator_claimable_reward_state
            .amount
            .checked_add(distributed_amount)?;
        creator_claimable_reward_state.last_update = env.block.time.seconds();
    }

    Ok(())
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

    // To prevent out-of-gas issues in long run
    if config.allowed_lp_tokens.len() == MAX_ALLOWED_LP_TOKENS {
        return Err(ContractError::CantAllowAnyMoreLpTokens);
    }

    let lp_token = deps.api.addr_validate(lp_token.as_str())?;

    // verify that lp token is not already allowed
    if config.allowed_lp_tokens.contains(&lp_token) {
        return Err(ContractError::LpTokenAlreadyAllowed);
    }

    config.allowed_lp_tokens.push(lp_token.clone());
    CONFIG.save(deps.storage, &config)?;

    let response = Response::new().add_event(
        Event::from_info(concatcp!(CONTRACT_NAME, "::allow_lp_token"), &info)
            .add_attribute("lp_token", lp_token.to_string()),
    );
    Ok(response)
}

fn remove_lp_token(
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

    let response = Response::new().add_event(
        Event::from_info(concatcp!(CONTRACT_NAME, "::remove_lp_token"), &info)
            .add_attribute("lp_token", lp_token.to_string()),
    );

    Ok(response)
}

pub fn create_reward_schedule(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_token: Addr,
    title: String,
    start_block_time: u64,
    end_block_time: u64,
    creator: Addr,
    asset: Asset,
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
    if start_block_time <= env.block.time.seconds()
    {
        return Err(ContractError::InvalidStartBlockTime {
            start_block_time,
            current_block_time: env.block.time.seconds(),
        });
    }

    // still need to check as an LP token might have been removed after the reward schedule was proposed
    check_if_lp_token_allowed(&config, &lp_token)?;

    let mut lp_global_state = LP_GLOBAL_STATE
        .may_load(deps.storage, &lp_token)?
        .unwrap_or_default();

    if !lp_global_state.active_reward_assets.contains(&asset.info) {
        lp_global_state
            .active_reward_assets
            .push(asset.info.clone());
    }

    LP_GLOBAL_STATE.save(deps.storage, &lp_token, &lp_global_state)?;

    let reward_schedule_id = next_reward_schedule_id(deps.storage)?;

    let reward_schedule = RewardSchedule {
        title: title.clone(),
        creator: creator.clone(),
        asset: asset.info.clone(),
        amount: asset.amount,
        staking_lp_token: lp_token.clone(),
        start_block_time,
        end_block_time,
    };

    REWARD_SCHEDULES.save(deps.storage, reward_schedule_id, &reward_schedule)?;

    let mut reward_schedules_ids = LP_TOKEN_ASSET_REWARD_SCHEDULE
        .may_load(deps.storage, (&lp_token, &asset.info.to_string()))?
        .unwrap_or_default();

    reward_schedules_ids.push(reward_schedule_id);
    LP_TOKEN_ASSET_REWARD_SCHEDULE.save(
        deps.storage,
        (&lp_token, &asset.info.to_string()),
        &reward_schedules_ids,
    )?;

    Ok(Response::new().add_event(
        Event::from_sender(
            concatcp!(CONTRACT_NAME, "::create_reward_schedule"),
            &info.sender,
        )
        .add_attribute("creator", creator.to_string())
        .add_attribute("lp_token", lp_token.to_string())
        .add_attribute("title", title)
        .add_attribute("start_block_time", start_block_time.to_string())
        .add_attribute("end_block_time", end_block_time.to_string())
        .add_attribute("asset", serde_json_wasm::to_string(&asset).unwrap())
        .add_attribute("reward_schedule_id", reward_schedule_id.to_string()),
    ))
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::Bond { beneficiary_user } => {
            let token_address = info.sender;
            let cw20_sender = deps.api.addr_validate(&cw20_msg.sender)?;

            let user = if let Some(beneficiary_user) = beneficiary_user {
                deps.api.addr_validate(beneficiary_user.as_str())?
            } else {
                cw20_sender.clone()
            };

            bond(
                deps,
                env,
                cw20_sender.clone(),
                user,
                token_address,
                cw20_msg.amount,
            )
        }
        Cw20HookMsg::CreateRewardSchedule {
            lp_token,
            title,
            actual_creator,
            start_block_time,
            end_block_time,
        } => {
            // only owner can create reward schedule
            let config = CONFIG.load(deps.storage)?;
            if cw20_msg.sender != config.owner {
                return Err(ContractError::Unauthorized);
            }

            let token_addr = info.sender.clone();

            let creator = match actual_creator {
                Some(creator) => deps.api.addr_validate(&creator.to_string())?,
                None => deps.api.addr_validate(&cw20_msg.sender)?,
            };

            create_reward_schedule(
                deps,
                env,
                info,
                lp_token,
                title,
                start_block_time,
                end_block_time,
                creator,
                Asset::new_token(token_addr, cw20_msg.amount),
            )
        }
    }
}

pub fn compute_reward(
    current_block_time: u64,
    total_bond_amount: Uint128,
    state: &mut AssetRewardState,
    reward_schedules: Vec<(u64, RewardSchedule)>,
    // Current creator claimable rewards for the above reward schedule ids
    creator_claimable_reward: &mut HashMap<u64, CreatorClaimableRewardState>,
) {
    if state.last_distributed == current_block_time {
        return;
    }

    let mut distributed_amount: Uint128 = Uint128::zero();
    for (id, s) in reward_schedules.iter() {
        let start_time = s.start_block_time;
        let end_time = s.end_block_time;

        if start_time > current_block_time || end_time <= state.last_distributed {
            continue;
        }

        // min(s.1, block_time) - max(s.0, last_distributed)
        let passed_time = std::cmp::min(end_time, current_block_time)
            - std::cmp::max(start_time, state.last_distributed);

        let time = end_time - start_time;
        let distribution_amount_per_second: Decimal = Decimal::from_ratio(s.amount, time);
        distributed_amount += distribution_amount_per_second * Uint128::from(passed_time as u128);

        // This means between last distributed time and current block time, no one has bonded any assets
        // This reward value must be claimable by the reward schedule creator
        if total_bond_amount.is_zero() && state.last_distributed < current_block_time {
            // Previous function ensures we can unwrap safely here
            let current_creator_claimable_reward =
                creator_claimable_reward.get(id).cloned().unwrap();
            // don't update already claimed creator claimable rewards
            if !current_creator_claimable_reward.claimed {
                let amount = current_creator_claimable_reward.amount;
                let new_amount = amount.checked_add(distributed_amount).unwrap();
                creator_claimable_reward.insert(
                    *id,
                    CreatorClaimableRewardState {
                        claimed: false,
                        amount: new_amount,
                        last_update: current_block_time,
                    },
                );
            }
        }
    }

    state.last_distributed = current_block_time;

    if total_bond_amount.is_zero() {
        return;
    }
    state.reward_index =
        state.reward_index + Decimal::from_ratio(distributed_amount, total_bond_amount);
}

pub fn compute_staker_reward(
    bond_amount: Uint128,
    state: &AssetRewardState,
    staker_info: &mut AssetStakerInfo,
) -> StdResult<()> {
    let pending_reward =
        bond_amount * (state.reward_index.checked_sub(staker_info.reward_index)?);
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

/// This function is called when a user wants to bond their LP tokens either directly or through the vault
/// This function will update the user's bond amount and the total bond amount for the given LP token
/// ### Params:
/// **sender**: This is the address that sent the cw20 token.
/// This is not necessarily the user address since vault can bond on behalf of the user
/// **user**: This is the user address that owns the bonded tokens and will receive rewards
/// This user is elligible to withdraw the tokens after unbonding and not the sender
/// **lp_token**: The LP token address
/// **amount**: The amount of LP tokens to bond
pub fn bond(
    mut deps: DepsMut,
    env: Env,
    sender: Addr,
    user: Addr,
    lp_token: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if amount.is_zero() {
        return Err(ContractError::ZeroAmount);
    }

    let config = CONFIG.load(deps.storage)?;
    check_if_lp_token_allowed(&config, &lp_token)?;

    let current_bond_amount = USER_BONDED_LP_TOKENS
        .may_load(deps.storage, (&lp_token, &user))?
        .unwrap_or_default();

    let mut lp_global_state = LP_GLOBAL_STATE
        .may_load(deps.storage, &lp_token)?
        .unwrap_or_default();
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
            None,
        )?;
    }

    // Increase bond amount
    lp_global_state.total_bond_amount = lp_global_state.total_bond_amount.checked_add(amount)?;
    LP_GLOBAL_STATE.save(deps.storage, &lp_token, &lp_global_state)?;

    let user_updated_bond_amount = current_bond_amount.checked_add(amount)?;

    // Increase user bond amount
    USER_BONDED_LP_TOKENS.save(deps.storage, (&lp_token, &user), &user_updated_bond_amount)?;

    // even though the msg sender might be a CW20 contract,
    // in the event, we are only concerned with the actual human sender
    let event = Event::from_sender(concatcp!(CONTRACT_NAME, "::bond"), sender)
        .add_attribute("user", user)
        .add_attribute("lp_token", lp_token)
        .add_attribute("amount", amount)
        .add_attribute("total_bond_amount", lp_global_state.total_bond_amount)
        .add_attribute("user_updated_bond_amount", user_updated_bond_amount);

    response = response.add_event(event);
    Ok(response)
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
    operation_post_update: Option<
        fn(
            &Addr,
            &Addr,
            &mut AssetRewardState,
            &mut AssetStakerInfo,
            &mut Response,
        ) -> ContractResult<()>,
    >,
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

    let reward_schedule_ids = LP_TOKEN_ASSET_REWARD_SCHEDULE
        .may_load(deps.storage, (&lp_token, &asset.to_string()))?
        .unwrap_or_default();

    let mut reward_schedules = vec![];
    for id in &reward_schedule_ids {
        reward_schedules.push((*id, REWARD_SCHEDULES.load(deps.storage, *id)?.clone()));
    }

    let mut current_creator_claimable_rewards = HashMap::new();
    for id in &reward_schedule_ids {
        let reward = CREATOR_CLAIMABLE_REWARD
            .may_load(deps.storage, *id)?
            .unwrap_or_default();
        current_creator_claimable_rewards.insert(*id, reward);
    }

    compute_reward(
        current_block_time,
        total_bond_amount,
        &mut asset_state,
        reward_schedules,
        &mut current_creator_claimable_rewards,
    );
    compute_staker_reward(
        current_bond_amount,
        &mut asset_state,
        &mut asset_staker_info,
    )?;

    if let Some(operation) = operation_post_update {
        operation(
            user,
            lp_token,
            &mut asset_state,
            &mut asset_staker_info,
            response,
        )?;
    }

    ASSET_LP_REWARD_STATE.save(deps.storage, (&asset.to_string(), &lp_token), &asset_state)?;

    ASSET_STAKER_INFO.save(
        deps.storage,
        (&lp_token, &user, &asset.to_string()),
        &asset_staker_info,
    )?;

    for (id, reward) in current_creator_claimable_rewards {
        CREATOR_CLAIMABLE_REWARD.save(deps.storage, id, &reward)?;
    }

    Ok(())
}

fn withdraw_pending_reward(
    user: &Addr,
    lp_token: &Addr,
    _asset_reward_state: &mut AssetRewardState,
    asset_staker_info: &mut AssetStakerInfo,
    response: &mut Response,
) -> ContractResult<()> {
    let pending_reward = asset_staker_info.pending_reward;

    if pending_reward > Uint128::zero() {
        let event = Event::from_sender(concatcp!(CONTRACT_NAME, "::withdraw_reward"), user)
            .add_attribute("lp_token", lp_token)
            .add_attribute("asset", asset_staker_info.asset.to_string())
            .add_attribute("amount", pending_reward);

        let res = response
            .clone()
            .add_message(build_transfer_token_to_user_msg(
                asset_staker_info.asset.clone(),
                user.clone(),
                pending_reward,
            )?)
            .add_event(event);
        *response = res;
    }

    asset_staker_info.pending_reward = Uint128::zero();

    Ok(())
}

pub fn withdraw(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_token: Addr,
) -> ContractResult<Response> {
    let mut response = Response::new();
    let current_bonded_amount = USER_BONDED_LP_TOKENS
        .may_load(deps.storage, (&lp_token, &info.sender))?
        .unwrap_or_default();

    let lp_global_state = LP_GLOBAL_STATE.load(deps.storage, &lp_token)?;

    for asset in &lp_global_state.active_reward_assets {
        update_staking_rewards(
            asset,
            &lp_token,
            &info.sender,
            lp_global_state.total_bond_amount,
            current_bonded_amount,
            env.block.time.seconds(),
            &mut deps,
            &mut response,
            Some(withdraw_pending_reward),
        )?;
    }

    // At each withdraw, we withdraw all earned rewards by the user.
    // If we keep a track of the reward at the subgraph level, then that much data can really suffice.
    response = response.add_event(
        Event::from_info(concatcp!(CONTRACT_NAME, "::withdraw"), &info)
            .add_attribute("lp_token", lp_token.clone()),
    );
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
        }
        QueryMsg::InstantUnlockFee {
            user,
            lp_token,
            token_lock,
        } => {
            let config = CONFIG.load(deps.storage)?;
            // validate if token lock actually exists
            let token_locks = USER_LP_TOKEN_LOCKS
                .may_load(deps.storage, (&lp_token, &user))?
                .unwrap_or_default();

            let exists = token_locks.iter().any(|lock| *lock == token_lock.clone());
            if !exists {
                return Err(ContractError::TokenLockNotFound);
            }

            let (fee_bp, unlock_fee) =
                calculate_unlock_fee(&token_lock, env.block.time.seconds(), &config);

            let instant_lp_unlock_fee = InstantLpUnlockFee {
                time_until_lock_expiry: token_lock
                    .unlock_time
                    .checked_sub(env.block.time.seconds())
                    .unwrap_or_default(),
                unlock_amount: token_lock.amount,
                unlock_fee_bp: fee_bp,
                unlock_fee,
            };

            to_binary(&instant_lp_unlock_fee).map_err(ContractError::from)
        }
        QueryMsg::InstantUnlockFeeTiers {} => {
            let config = CONFIG.load(deps.storage)?;
            let min_fee = config.instant_unbond_min_fee_bp;
            let max_fee = config.instant_unbond_fee_bp;

            let unlock_period = config.unlock_period;
            let fee_tiers = query_instant_unlock_fee_tiers(
                config.fee_tier_interval,
                unlock_period,
                min_fee,
                max_fee,
            );

            to_binary(&fee_tiers).map_err(ContractError::from)
        }
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
                        last_distributed: 0,
                    });

                let reward_schedule_ids = LP_TOKEN_ASSET_REWARD_SCHEDULE
                    .may_load(deps.storage, (&lp_token, &asset.to_string()))?
                    .unwrap_or_default();

                let mut reward_schedules = vec![];
                for id in &reward_schedule_ids {
                    reward_schedules.push((*id, REWARD_SCHEDULES.load(deps.storage, *id)?.clone()));
                }

                let mut current_creator_claimable_rewards = HashMap::new();
                for id in &reward_schedule_ids {
                    let reward = CREATOR_CLAIMABLE_REWARD
                        .may_load(deps.storage, *id)?
                        .unwrap_or_default();
                    current_creator_claimable_rewards.insert(*id, reward);
                }

                compute_reward(
                    block_time,
                    lp_global_state.total_bond_amount,
                    &mut asset_state,
                    reward_schedules,
                    &mut current_creator_claimable_rewards,
                );
                compute_staker_reward(
                    current_bonded_amount,
                    &mut asset_state,
                    &mut asset_staker_info,
                )?;

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
            let reward_schedule_ids = LP_TOKEN_ASSET_REWARD_SCHEDULE
                .may_load(deps.storage, (&lp_token, &asset.to_string()))?
                .unwrap_or_default();

            let mut reward_schedules = vec![];
            for id in &reward_schedule_ids {
                reward_schedules.push(RewardScheduleResponse {
                    id: *id,
                    reward_schedule: REWARD_SCHEDULES.load(deps.storage, *id)?.clone(),
                });
            }
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
            })
            .map_err(ContractError::from)
        }
        QueryMsg::RawTokenLocks { lp_token, user } => {
            let locks = USER_LP_TOKEN_LOCKS
                .may_load(deps.storage, (&lp_token, &user))?
                .unwrap_or_default();

            to_binary(&locks).map_err(ContractError::from)
        }
        QueryMsg::RewardState { lp_token, asset } => {
            let reward_state =
                ASSET_LP_REWARD_STATE.may_load(deps.storage, (&asset.to_string(), &lp_token))?;

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
            let reward_state =
                ASSET_STAKER_INFO.may_load(deps.storage, (&lp_token, &user, &asset.to_string()))?;

            match reward_state {
                Some(reward_state) => to_binary(&reward_state).map_err(ContractError::from),
                None => Err(ContractError::NoUserRewardState),
            }
        }
        QueryMsg::CreatorClaimableReward { reward_schedule_id } => {
            let reward_schedule = REWARD_SCHEDULES.load(deps.storage, reward_schedule_id)?;
            let mut creator_claimable_reward = CREATOR_CLAIMABLE_REWARD
                .may_load(deps.storage, reward_schedule_id)?
                .unwrap_or_default();

            compute_creator_claimable_reward(
                deps.storage,
                env,
                &reward_schedule,
                &mut creator_claimable_reward,
            )?;

            to_binary(&creator_claimable_reward).map_err(ContractError::from)
        }
        QueryMsg::Config {} => {
            let config = CONFIG.load(deps.storage)?;
            to_binary(&config).map_err(ContractError::from)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> ContractResult<Response> {
    match msg {
        MigrateMsg::V3FromV1 {
            keeper_addr,
            instant_unbond_fee_bp,
            instant_unbond_min_fee_bp,
            fee_tier_interval
        } => {
            // verify if we are running on V1 right now
            let contract_version = get_contract_version(deps.storage)?;
            if contract_version.version != CONTRACT_VERSION_V1 {
                return Err(ContractError::InvalidContractVersionForUpgrade {
                    upgrade_version: CONTRACT_VERSION.to_string(),
                    expected: CONTRACT_VERSION_V1.to_string(),
                    actual: contract_version.version,
                });
            }

            // validate input for upgrade
            if instant_unbond_fee_bp > MAX_INSTANT_UNBOND_FEE_BP {
                return Err(ContractError::InvalidInstantUnbondFee {
                    max_allowed: MAX_INSTANT_UNBOND_FEE_BP,
                    received: instant_unbond_fee_bp,
                });
            }

            if instant_unbond_min_fee_bp > instant_unbond_fee_bp {
                return Err(ContractError::InvalidInstantUnbondMinFee {
                    max_allowed: instant_unbond_fee_bp,
                    received: instant_unbond_min_fee_bp,
                });
            }

            let config_v1: ConfigV1 = Item::new("config").load(deps.storage)?;

            // valiate fee tier interval
            if fee_tier_interval > config_v1.unlock_period {
                return Err(ContractError::InvalidFeeTierInterval {
                    max_allowed: config_v1.unlock_period,
                    received: fee_tier_interval,
                });
            }

            // copy fields from v1 to v2
            let config = Config {
                owner: config_v1.owner,
                allowed_lp_tokens: config_v1.allowed_lp_tokens,
                unlock_period: config_v1.unlock_period,
                keeper: deps.api.addr_validate(&keeper_addr.to_string())?,
                instant_unbond_fee_bp,
                instant_unbond_min_fee_bp,
                fee_tier_interval,
            };

            set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
            CONFIG.save(deps.storage, &config)?;
        },
        MigrateMsg::V3FromV2 { keeper_addr } => {
            let contract_version = get_contract_version(deps.storage)?;
            // if version is v2 or v2.1, apply the changes.
            if contract_version.version == CONTRACT_VERSION_V2 || contract_version.version == CONTRACT_VERSION_V2_1 {
                let config_v2: ConfigV2_1 = Item::new("config").load(deps.storage)?;
                let config = Config {
                    owner: config_v2.owner,
                    allowed_lp_tokens: config_v2.allowed_lp_tokens,
                    unlock_period: config_v2.unlock_period,
                    keeper: keeper_addr,
                    instant_unbond_fee_bp: config_v2.instant_unbond_fee_bp,
                    instant_unbond_min_fee_bp: config_v2.instant_unbond_min_fee_bp,
                    fee_tier_interval: config_v2.fee_tier_interval,
                };

                CONFIG.save(deps.storage, &config)?;
                set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
            } else {
                return Err(ContractError::InvalidContractVersionForUpgrade {
                    upgrade_version: CONTRACT_VERSION.to_string(),
                    expected: CONTRACT_VERSION_V2.to_string(),
                    actual: contract_version.version,
                });
            }
        }

        MigrateMsg::V3FromV2_2 {} => {
            let contract_version = get_contract_version(deps.storage)?;
            // if version if v2.2 apply the changes and return
            if contract_version.version == CONTRACT_VERSION_V2_2 {
                let config_v2: ConfigV2_2 = Item::new("config").load(deps.storage)?;
                let config = Config {
                    owner: config_v2.owner,
                    allowed_lp_tokens: config_v2.allowed_lp_tokens,
                    unlock_period: config_v2.unlock_period,
                    keeper: config_v2.keeper,
                    instant_unbond_fee_bp: config_v2.instant_unbond_fee_bp,
                    instant_unbond_min_fee_bp: config_v2.instant_unbond_min_fee_bp,
                    fee_tier_interval: config_v2.fee_tier_interval,
                };

                CONFIG.save(deps.storage, &config)?;

                // set the contract version to v3
                set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

            } else {
                return Err(ContractError::InvalidContractVersionForUpgrade {
                    upgrade_version: CONTRACT_VERSION.to_string(),
                    expected: CONTRACT_VERSION_V2_2.to_string(),
                    actual: contract_version.version,
                });
            }
        }
    }

    Ok(Response::default())
}
