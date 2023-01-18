#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use std::collections::{HashSet, HashMap};
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, Uint128, WasmMsg, Order, Storage, Event
};

use dexter::{
    asset::AssetInfo,
    helper::{
        build_transfer_token_to_user_msg, claim_ownership, drop_ownership_proposal,
        propose_new_owner,
    },
    multi_staking::{
        AssetRewardState, AssetStakerInfo, Config, Cw20HookMsg, ExecuteMsg, InstantiateMsg,
        QueryMsg, RewardSchedule, TokenLock, TokenLockInfo, UnclaimedReward, CreatorClaimableRewardState,
    },
};

use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw_storage_plus::Bound;
use dexter::asset::Asset;
use dexter::multi_staking::{
    MAX_ALLOWED_LP_TOKENS, MAX_USER_LP_TOKEN_LOCKS, MIN_REWARD_SCHEDULE_PROPOSAL_START_DELAY, ProposedRewardSchedule,
    ProposedRewardSchedulesResponse, ReviewProposedRewardSchedule, RewardScheduleResponse
};

use crate::{
    error::ContractError,
    state::{
        ASSET_STAKER_INFO, CONFIG, OWNERSHIP_PROPOSAL, REWARD_SCHEDULE_PROPOSALS, LP_TOKEN_ASSET_REWARD_SCHEDULE,
        USER_LP_TOKEN_LOCKS, USER_BONDED_LP_TOKENS, LP_GLOBAL_STATE, ASSET_LP_REWARD_STATE, REWARD_SCHEDULES, next_reward_schedule_id, CREATOR_CLAIMABLE_REWARD,
    },
};
use crate::state::next_reward_schedule_proposal_id;

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
            owner: deps.api.addr_validate(msg.owner.as_str())?,
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
        ExecuteMsg::ProposeRewardSchedule {
            lp_token,
            title,
            description,
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
            let proposer = info.sender.clone();

            propose_reward_schedule(
                deps,
                env,
                info,
                lp_token,
                title,
                description,
                proposer,
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: sent_asset.denom,
                    },
                    amount: sent_asset.amount,
                },
                start_block_time,
                end_block_time,
            )
        }
        ExecuteMsg::ReviewRewardScheduleProposals { reviews } => review_reward_schedule_proposals(deps, env, info, reviews),
        ExecuteMsg::DropRewardScheduleProposal { proposal_id } => drop_reward_schedule_proposal(deps, env, info, proposal_id),
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
    compute_creator_claimable_reward(deps.storage, env, &reward_schedule, &mut creator_claimable_reward_state)?;

    // Verify that the reward schedule has unclaimed reward
    if creator_claimable_reward_state.amount.is_zero() {
        return Err(ContractError::NoUnallocatedReward);
    }

    // Update the reward schedule to be claimed
    creator_claimable_reward_state.claimed = true;
    CREATOR_CLAIMABLE_REWARD.save(deps.storage, reward_schedule_id, &creator_claimable_reward_state)?;

    // Send the unclaimed reward to the reward schedule creator
    let msg = build_transfer_token_to_user_msg(reward_schedule.asset, reward_schedule.creator, creator_claimable_reward_state.amount)?;

    let event = Event::new("claim_unallocated_reward")
        .add_attribute("reward_schedule_id", reward_schedule_id.to_string())
        .add_attribute("amount", creator_claimable_reward_state.amount.to_string());

    Ok(Response::new().add_event(event).add_message(msg))
}

fn compute_creator_claimable_reward(
    store: &dyn Storage,
    env: Env,
    reward_schedule: &RewardSchedule,
    creator_claimable_reward_state: &mut CreatorClaimableRewardState,
) -> ContractResult<()> {
    let lp_global_state = LP_GLOBAL_STATE.may_load(store, &reward_schedule.staking_lp_token)?.unwrap_or_default();
    let asset_state = ASSET_LP_REWARD_STATE
        .may_load(store, (&reward_schedule.asset.to_string(), &reward_schedule.staking_lp_token))?
        .unwrap_or(AssetRewardState {
            reward_index: Decimal::zero(),
            last_distributed: 0,
        });
    let current_block_time = env.block.time.seconds();

    if lp_global_state.total_bond_amount.is_zero() && asset_state.last_distributed < reward_schedule.end_block_time {
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
        let distribution_amount_per_second: Decimal = Decimal::from_ratio(reward_schedule.amount, time);
        let distributed_amount = distribution_amount_per_second * Uint128::from(passed_time as u128);

        creator_claimable_reward_state.amount = creator_claimable_reward_state.amount.checked_add(distributed_amount)?;
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

    // verify that lp token is not already allowed
    if config.allowed_lp_tokens.contains(&lp_token) {
        return Err(ContractError::LpTokenAlreadyAllowed);
    }

    config.allowed_lp_tokens.push(lp_token.clone());
    CONFIG.save(deps.storage, &config)?;

    let response = Response::new()
        .add_event(Event::new("dexter-multistaking::allow_lp_token")
            .add_attribute("lp_token", lp_token.to_string())
        );
    Ok(response)
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

    let response = Response::new()
        .add_event(Event::new("dexter-multistaking::remove_lp_token")
            .add_attribute("lp_token", lp_token.to_string())
        );

    Ok(response)
}

pub fn propose_reward_schedule(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    lp_token: Addr,
    title: String,
    description: Option<String>,
    proposer: Addr,
    asset: Asset,
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
    if start_block_time <= env.block.time.seconds() + MIN_REWARD_SCHEDULE_PROPOSAL_START_DELAY {
        return Err(ContractError::ProposedStartBlockTimeMustBeReviewable);
    }

    let proposal_id: u64 = next_reward_schedule_proposal_id(deps.storage)?;

    REWARD_SCHEDULE_PROPOSALS.save(
        deps.storage,
        proposal_id.clone(),
        &ProposedRewardSchedule {
            lp_token: lp_token.clone(),
            proposer: proposer.clone(),
            title: title.clone(),
            description,
            asset: asset.clone(),
            start_block_time,
            end_block_time,
            rejected: false, // => not yet reviewed
        })?;

    let event = Event::new("dexter-multistaking::propose_reward_schedule")
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("lp_token", lp_token.to_string())
        .add_attribute("proposer", proposer.to_string())
        .add_attribute("title", title)
        .add_attribute("asset", serde_json_wasm::to_string(&asset).unwrap())
        .add_attribute("start_block_time", start_block_time.to_string())
        .add_attribute("end_block_time", end_block_time.to_string());

    let response = Response::new()
        .add_event(event);
    Ok(response)
}

pub fn review_reward_schedule_proposals(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    reviews: Vec<ReviewProposedRewardSchedule>,
) -> ContractResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    if config.owner != info.sender {
        return Err(ContractError::Unauthorized);
    }

    // ensure that reviews are for unique proposal_ids, otherwise we might end up creating duplicate reward schedules
    let mut reviewed_ids: HashSet<u64> = HashSet::with_capacity(reviews.len());
    for review in reviews.iter() {
        if !reviewed_ids.insert(review.proposal_id.clone()) {
            return Err(ContractError::DuplicateReview{ proposal_id: review.proposal_id.clone() });
        }
    }

    let mut accepted_reward_proposals: Vec<(u64, u64)> = vec![];
    let mut rejected_reward_proposals: Vec<u64> = vec![];

    // act on all the reviews
    for review in reviews.into_iter() {
        let mut proposal: ProposedRewardSchedule = REWARD_SCHEDULE_PROPOSALS
            .load(deps.storage,review.proposal_id)
            .map_err(|_| ContractError::ProposalNotFound { proposal_id: review.proposal_id.clone() })?;

        // skip the proposal if rejected already. No need to error, just ignore it.
        if proposal.rejected {
            rejected_reward_proposals.push(review.proposal_id);
            continue
        }

        // if approved and proposal is still valid, then need to save the reward schedule
        if review.approve && proposal.start_block_time > env.block.time.seconds() {
            // still need to check as an LP token might have been removed after the reward schedule was proposed
            check_if_lp_token_allowed(&config, &proposal.lp_token)?;

            let mut lp_global_state = LP_GLOBAL_STATE
                .may_load(deps.storage, &proposal.lp_token)?
                .unwrap_or_default();

            if !lp_global_state.active_reward_assets.contains(&proposal.asset.info) {
                lp_global_state.active_reward_assets.push(proposal.asset.info.clone());
            }

            LP_GLOBAL_STATE.save(deps.storage, &proposal.lp_token, &lp_global_state)?;

            let reward_schedule_id = next_reward_schedule_id(deps.storage)?;

            accepted_reward_proposals.push((review.proposal_id, reward_schedule_id));
            let reward_schedule = RewardSchedule {
                title: proposal.title,
                creator: proposal.proposer,
                asset: proposal.asset.info.clone(),
                amount: proposal.asset.amount,
                staking_lp_token: proposal.lp_token.clone(),
                start_block_time: proposal.start_block_time,
                end_block_time: proposal.end_block_time,
            };

            REWARD_SCHEDULES.save(deps.storage, reward_schedule_id, &reward_schedule)?;

            let mut reward_schedules_ids = LP_TOKEN_ASSET_REWARD_SCHEDULE
                .may_load(deps.storage, (&proposal.lp_token, &proposal.asset.info.to_string()))?
                .unwrap_or_default();

            reward_schedules_ids.push(reward_schedule_id);
            LP_TOKEN_ASSET_REWARD_SCHEDULE.save(
                deps.storage,
                (&proposal.lp_token, &proposal.asset.info.to_string()),
                &reward_schedules_ids,
            )?;

            // remove the approved proposal from the state
            REWARD_SCHEDULE_PROPOSALS.remove(deps.storage, review.proposal_id);
        }
        // otherwise, mark the proposal rejected
        else {
            proposal.rejected = true;
            rejected_reward_proposals.push(review.proposal_id);
            REWARD_SCHEDULE_PROPOSALS.save(deps.storage, review.proposal_id, &proposal)?;
        }
    }

    let event = Event::new("dexter-multistaking::review_reward_schedule_proposals")
        .add_attribute("accepted_proposals", serde_json_wasm::to_string(&accepted_reward_proposals).unwrap())
        .add_attribute("rejected_proposals", serde_json_wasm::to_string(&rejected_reward_proposals).unwrap());

    let response = Response::new().add_event(event);
    Ok(response)
}



pub fn drop_reward_schedule_proposal(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    proposal_id: u64,
) -> ContractResult<Response> {
    let proposal = REWARD_SCHEDULE_PROPOSALS.load(deps.storage, proposal_id)?;

    // only the proposer can drop the proposal
    if proposal.proposer != info.sender {
        return Err(ContractError::Unauthorized{});
    }

    let msg = build_transfer_token_to_user_msg(proposal.asset.info.clone(), proposal.proposer, proposal.asset.amount)?;

    REWARD_SCHEDULE_PROPOSALS.remove(deps.storage, proposal_id);

    let event = Event::new("dexter-multistaking::drop_reward_schedule_proposal")
        .add_attribute("proposal_id", proposal_id.to_string());

    Ok(Response::default().add_event(event).add_message(msg))
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::Bond {} => {
            let token_address = info.sender;
            let cw20_sender = deps.api.addr_validate(&cw20_msg.sender)?;
            bond(deps, env, cw20_sender, token_address, cw20_msg.amount)
        },
        Cw20HookMsg::BondForBeneficiary { beneficiary } => {
            let token_address = info.sender;
            let beneficiary = deps.api.addr_validate(beneficiary.as_str())?;
            bond(deps, env, beneficiary, token_address, cw20_msg.amount)
        }
        Cw20HookMsg::ProposeRewardSchedule {
            lp_token,
            title,
            description,
            start_block_time,
            end_block_time,
        } => {
            let token_addr = info.sender.clone();
            let proposer = deps.api.addr_validate(&cw20_msg.sender)?;

            propose_reward_schedule(
                deps,
                env,
                info,
                lp_token,
                title,
                description,
                proposer,
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: token_addr,
                    },
                    amount: cw20_msg.amount,
                },
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
    reward_schedules: Vec<(u64, RewardSchedule)>,
    // Current creator claimable rewards for the above reward schedule ids
    creator_claimable_reward: &mut HashMap<u64, CreatorClaimableRewardState>
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
            let current_creator_claimable_reward = creator_claimable_reward.get(id).cloned().unwrap();
            // don't update already claimed creator claimable rewards
            if !current_creator_claimable_reward.claimed {
                let amount = current_creator_claimable_reward.amount;
                let new_amount = amount.checked_add(distributed_amount).unwrap();
                creator_claimable_reward.insert(*id, CreatorClaimableRewardState {
                    claimed: false,
                    amount: new_amount,
                    last_update: current_block_time
                });
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

    if amount.is_zero() {
        return Err(ContractError::ZeroAmount);
    }

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

    let user_updated_bond_amount = current_bond_amount.checked_add(amount)?;

    // Increase user bond amount
    USER_BONDED_LP_TOKENS.save(
        deps.storage,
        (&lp_token, &user),
        &user_updated_bond_amount,
    )?;

    let event = Event::new("dexter-multistaking::bond")
        .add_attribute("lp_token", lp_token)
        .add_attribute("user", user)
        .add_attribute("amount", amount)
        .add_attribute("total_bond_amount", lp_global_state.total_bond_amount)
        .add_attribute("user_updated_bond_amount", user_updated_bond_amount);

    response = response.add_event(event);
    Ok(response)
}

/// Unbond LP tokens
pub fn unbond(
    mut deps: DepsMut,
    env: Env,
    sender: Addr,
    lp_token: Addr,
    amount: Option<Uint128>,
) -> ContractResult<Response> {
    // We don't have to check for LP token allowed here, because there's a scenario that we allowed bonding
    // for an asset earlier and then we remove the LP token from the list of allowed LP tokens. In this case
    // we still want to allow unbonding.
    let mut response = Response::new();

    let current_bond_amount = USER_BONDED_LP_TOKENS
        .may_load(deps.storage, (&lp_token, &sender))?
        .unwrap_or_default();

    // if user didn't explicitly mention any amount, unbond everything.
    let amount= amount.unwrap_or(current_bond_amount);
    if amount.is_zero() {
        return Err(ContractError::ZeroAmount);
    }

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
        &(current_bond_amount.checked_sub(amount).map_err(|_| ContractError::CantUnbondMoreThanBonded {
            amount_to_unbond: amount,
            current_bond_amount,
        })?),
    )?;

    // Start unlocking clock for the user's LP Tokens
    let mut unlocks = USER_LP_TOKEN_LOCKS
        .may_load(deps.storage, (&lp_token, &sender))?
        .unwrap_or_default();

    if unlocks.len() == MAX_USER_LP_TOKEN_LOCKS {
        return Err(ContractError::CantAllowAnyMoreLpTokenUnbonds);
    }

    let config = CONFIG.load(deps.storage)?;

    let unlock_time = env.block.time.seconds() + config.unlock_period;
    unlocks.push(TokenLock {
        unlock_time,
        amount,
    });

    USER_LP_TOKEN_LOCKS.save(deps.storage, (&lp_token, &sender), &unlocks)?;

    let event = Event::new("dexter-multistaking::unbond")
        .add_attribute("lp_token", lp_token)
        .add_attribute("user", sender)
        .add_attribute("amount", amount)
        .add_attribute("total_bond_amount", lp_global_state.total_bond_amount)
        .add_attribute("user_updated_bond_amount", current_bond_amount.checked_sub(amount)?)
        .add_attribute("unlock_time", unlock_time.to_string());

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
    operation_post_update: Option<fn(&Addr, &Addr, &mut AssetRewardState, &mut AssetStakerInfo, &mut Response) -> ContractResult<()>>,
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
        .may_load(
            deps.storage,
            (&lp_token, &asset.to_string()),
        )?
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

    compute_reward(current_block_time, total_bond_amount, &mut asset_state, reward_schedules, &mut current_creator_claimable_rewards);
    compute_staker_reward(current_bond_amount, &mut asset_state, &mut asset_staker_info)?;

    if let Some(operation) = operation_post_update {
        operation(user, lp_token, &mut asset_state, &mut asset_staker_info, response)?;
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

    for (id, reward) in current_creator_claimable_rewards {
        CREATOR_CLAIMABLE_REWARD.save(deps.storage, id, &reward)?;
    }

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

    let event = Event::new("dexter-multistaking::unlock")
        .add_attribute("lp_token", lp_token)
        .add_attribute("user", sender)
        .add_attribute("amount", total_unlocked_amount);

    response = response.add_event(event);
    Ok(response)
}

fn withdraw_pending_reward(
    user: &Addr,
    lp_token: &Addr,
    _asset_reward_state: &mut AssetRewardState,
    asset_staker_info: &mut AssetStakerInfo,
    response: &mut Response,
) -> ContractResult<()> {
    let pending_reward = asset_staker_info.pending_reward;
    
    let event = Event::new("dexter-multistaking::withdraw_reward")
        .add_attribute("user", user)
        .add_attribute("lp_token", lp_token)
        .add_attribute("asset", asset_staker_info.asset.to_string())
        .add_attribute("amount", pending_reward);

    if pending_reward > Uint128::zero() {
        let res = response.clone().add_message(
            build_transfer_token_to_user_msg(
                asset_staker_info.asset.clone(),
                user.clone(),
                pending_reward,
            )?,
        ).add_event(event);
        *response = res;
    }

    asset_staker_info.pending_reward = Uint128::zero();

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

    // At each withdraw, we withdraw all earned rewards by the user.
    // If we keep a track of the reward at the subgraph level, then that much data can really suffice.
    let event = Event::new("dexter-multistaking::withdraw")
        .add_attribute("lp_token", lp_token.clone())
        .add_attribute("user", sender);

    response = response.add_event(event);
    Ok(response)
}

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

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
                        last_distributed: 0,
                    });

                
                let reward_schedule_ids = LP_TOKEN_ASSET_REWARD_SCHEDULE
                .may_load(
                    deps.storage,
                    (&lp_token, &asset.to_string()),
                )?
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

                compute_reward(block_time, lp_global_state.total_bond_amount, &mut asset_state, reward_schedules, &mut current_creator_claimable_rewards);
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
        QueryMsg::ProposedRewardSchedules { start_after, limit } => {
            let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
            let start = start_after.map(Bound::exclusive);
            let proposals: Vec<ProposedRewardSchedulesResponse> = REWARD_SCHEDULE_PROPOSALS
                .range(deps.storage, start, None, Order::Ascending)
                .take(limit)
                .map(|p| {
                    p.map(|(proposal_id, proposal)| {
                        ProposedRewardSchedulesResponse {
                            proposal_id,
                            proposal,
                        }
                    })
                })
                .collect::<StdResult<_>>()?;

            to_binary(&proposals).map_err(ContractError::from)
        }
        QueryMsg::ProposedRewardSchedule { proposal_id } => {
            let reward_schedule = REWARD_SCHEDULE_PROPOSALS.load(deps.storage, proposal_id)?;
            to_binary(&reward_schedule).map_err(ContractError::from)
        }
        QueryMsg::RewardSchedules { lp_token, asset } => {
            let reward_schedule_ids= LP_TOKEN_ASSET_REWARD_SCHEDULE
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
        QueryMsg::CreatorClaimableReward {
            reward_schedule_id
        } => {
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
    }
}