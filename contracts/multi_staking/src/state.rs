use cosmwasm_std::{Addr, StdResult, Storage, Uint128};
use cw_storage_plus::{Map, Item};
use dexter::{multi_staking::{AssetRewardState, Config, AssetStakerInfo, RewardSchedule, TokenLock, LpGlobalState}, helper::OwnershipProposal};
use dexter::multi_staking::ProposedRewardSchedule;

// Global config of the contract
pub const CONFIG: Item<Config> = Item::new("config");

/// Ownership proposal in case of ownership transfer is initiated
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");

/// The count of how many reward schedules have been proposed till now
pub const REWARD_SCHEDULE_PROPOSAL_COUNT: Item<u64> = Item::new("reward_schedule_proposal_count");

/// Map of Proposal ID to the proposed reward schedule for that LP token.
/// Only when these proposals are approved, they will be moved to REWARD_SCHEDULES.
pub const REWARD_SCHEDULE_PROPOSALS: Map<u64, ProposedRewardSchedule> = Map::new("reward_schedule_proposals");

/// Map between (LP Token, User, Asset identifier) to AssetStakerInfo
/// This is used to store the staker's information for each asset and LP token pair
/// for keeping a track of the rewards that the user has earned.
/// The rewards are calculated based on the difference between the current asset's reward index and the
/// user's last reward index.
pub const ASSET_STAKER_INFO: Map<(&Addr, &Addr, &str), AssetStakerInfo> = Map::new("asset_staker_info");

/// Store of all reward schedules for a (LP token, Asset) pair.
pub const REWARD_SCHEDULES: Map<(&Addr, &str) , Vec<RewardSchedule>> = Map::new("rewards");

/// This is used to keep track of the LP tokens that are currently locked for the user
/// after they have unbonded their tokens.
pub const USER_LP_TOKEN_LOCKS: Map<(&Addr, &Addr), Vec<TokenLock>> = Map::new("user_lp_token_unlocks");

/// This is used to keep track of the LP tokens that are currently bonded by the user.
pub const USER_BONDED_LP_TOKENS: Map<(&Addr, &Addr), Uint128> = Map::new("user_bonded_lp_tokens");

/// This is used to keep track of global state against reward for a particular asset against 
/// lockup of a particular LP token.
/// We maintain a global reward index for each asset and LP token pair.
/// This is used to calculate the rewards that the user has earned.
/// The rewards are calculated based on the difference between the current asset's reward index and the user's last reward index.
pub const ASSET_LP_REWARD_STATE: Map<(&str, &Addr), AssetRewardState> = Map::new("asset_lp_reward_state");

/// This is used to keep track of the global state of the reward for a particular LP token.
/// It tracks total bonded amount of the LP token across all users and also the assets that are currently
/// being rewarded for the LP token.
pub const LP_GLOBAL_STATE: Map<&Addr, LpGlobalState> = Map::new("lp_global_state");

pub fn next_reward_schedule_proposal_id(store: &mut dyn Storage) -> StdResult<u64> {
    let id: u64 = REWARD_SCHEDULE_PROPOSAL_COUNT.may_load(store)?.unwrap_or_default() + 1;
    REWARD_SCHEDULE_PROPOSAL_COUNT.save(store, &id)?;
    Ok(id)
}