use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Map, Item};
use dexter::{asset::AssetInfo, multi_staking::{AssetRewardState, Config, AssetStakerInfo, RewardSchedule, TokenLock}, helper::OwnershipProposal};

#[cw_serde]
#[derive(Default)]
pub struct LpGlobalState {
    pub total_bond_amount: Uint128,
    pub active_reward_assets: Vec<AssetInfo>
}

pub const CONFIG: Item<Config> = Item::new("config");
// Map between (LP Token, User, Asset identifier) to AssetStakerInfo
pub const ASSET_STAKER_INFO: Map<(&Addr, &Addr, &str), AssetStakerInfo> = Map::new("asset_staker_info");

pub const REWARD_SCHEDULES: Map<(&Addr, &str) , Vec<RewardSchedule>> = Map::new("rewards");

// Map between (LP Token, User) to (Unlock time, Amount)
pub const USER_LP_TOKEN_LOCKS: Map<(&Addr, &Addr), Vec<TokenLock>> = Map::new("user_lp_token_unlocks");
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");

pub const USER_BONDED_LP_TOKENS: Map<(&Addr, &Addr), Uint128> = Map::new("user_bonded_lp_tokens");
pub const ASSET_LP_REWARD_STATE: Map<(&str, &Addr), AssetRewardState> = Map::new("asset_lp_reward_state");
pub const LP_GLOBAL_STATE: Map<&Addr, LpGlobalState> = Map::new("lp_global_state");