use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::Item;
use dexter::pool::Config;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// ## Description
/// Stores config at the given key
pub const CONFIG: Item<Config> = Item::new("config");


/// Stores custom Twap at the given key which can be different between different dexter pools
pub const TWAPINFO: Item<Twap> = Item::new("twap");


/// Stores custom config at the given key which can be different between different dexter pools
pub const MATHCONFIG: Item<MathConfig> = Item::new("math-config");

/// ## Description
/// This structure describes the main math config of pool.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MathConfig {
    // This is the current amplification used in the pool
    pub init_amp: u64,
    // This is the start time when amplification starts to scale up or down
    pub init_amp_time: u64,
    // This is the target amplification to reach at `next_amp_time`
    pub next_amp: u64,
    // This is the timestamp when the current pool amplification should be `next_amp`
    pub next_amp_time: u64,
}


/// ## Description
/// This structure which stores the TWAP calcs related info for the pool
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Twap {
    pub price0_cumulative_last: Uint128,
    pub price1_cumulative_last: Uint128,
    pub block_time_last: u64,
}
