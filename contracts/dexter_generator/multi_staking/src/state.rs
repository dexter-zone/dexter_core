use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Map, Item};
use dexter::{asset::AssetInfo, multi_staking::{AssetRewardState, Config, AssetStakerInfo, RewardSchedule, TokenLock}};



pub const CONFIG: Item<Config> = Item::new("config");
// Map between (LP Token, User, Asset identifier) to AssetStakerInfo
pub const ASSET_STAKER_INFO: Map<(&Addr, &Addr, &str), AssetStakerInfo> = Map::new("asset_staker_info");

pub const LP_ACTIVE_REWARD_ASSETS: Map<&Addr, Vec<AssetInfo>> = Map::new("lp_active_reward_assets");
pub const REWARD_SCHEDULES: Map<(&Addr, &str) , Vec<RewardSchedule>> = Map::new("rewards");
pub const REWARD_STATES: Map<&str, AssetRewardState> = Map::new("reward_states");

// Map between (LP Token, User) to (Unlock time, Amount)
pub const USER_LP_TOKEN_LOCKS: Map<(&Addr, &Addr), Vec<TokenLock>> = Map::new("user_lp_token_unlocks");