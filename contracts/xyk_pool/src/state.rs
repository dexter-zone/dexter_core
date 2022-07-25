use cosmwasm_std::Uint128;
use cw_storage_plus::Item;
use dexter::pool::Config;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// ## Description
/// Stores config at the given key
pub const CONFIG: Item<Config> = Item::new("config");

/// Stores custom Twap at the given key which can be different between different dexter pools
pub const TWAPINFO: Item<Twap> = Item::new("twap");

/// ## Description
/// This structure which stores the TWAP calcs related info for the pool
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Twap {
    pub price0_cumulative_last: Uint128,
    pub price1_cumulative_last: Uint128,
    pub block_time_last: u64,
}
