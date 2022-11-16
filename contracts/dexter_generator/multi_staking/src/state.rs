use std::ops::Add;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Uint128, Addr, CanonicalAddr, Decimal};
use cw_storage_plus::{Map, Item};
use dexter::{asset::AssetInfo, multi_staking::{State, AssetRewardState}};


#[cw_serde]
pub struct RewardSchedule {
    pub asset: AssetInfo,
    pub amount: Uint128,
    pub staking_lp_token: Addr,
    pub start_block_time: u64,
    pub end_block_time: u64,
}


#[cw_serde]
pub struct Config {
    // TODO: check if we need this
    pub allowed_lp_tokens: Vec<Addr>
}

#[cw_serde]
pub struct AssetStakerInfo {
    pub asset: AssetInfo,
    pub reward_index: Decimal,
    pub bond_amount: Uint128,
    pub pending_reward: Uint128,
}


pub const CONFIG: Item<Config> = Item::new("config");

// pub const LP_ACTIVE_REWARD_ASSETS: Map<&Addr, Vec<AssetInfo>> = Map::new("lp_active_reward_assets");
// Map between (LP Token, User, Asset identifier) to AssetStakerInfo
pub const ASSET_STAKER_INFO: Map<(&Addr, &Addr, &str), AssetStakerInfo> = Map::new("asset_staker_info");

pub const LP_ACTIVE_REWARD_ASSETS: Map<&Addr, Vec<AssetInfo>> = Map::new("lp_active_reward_assets");
pub const REWARD_SCHEDULES: Map<(&Addr, &str) , Vec<RewardSchedule>> = Map::new("rewards");
pub const REWARD_STATES: Map<&str, AssetRewardState> = Map::new("reward_states");