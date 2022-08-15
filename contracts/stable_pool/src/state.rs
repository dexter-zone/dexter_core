use cosmwasm_std::Uint128;
use cw_storage_plus::Item;
use dexter::pool::Config;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// ## Description
/// Stores config at the given key
pub const CONFIG: Item<Config> = Item::new("config");

/// Stores Twap prices for the tokens supported by the pool
pub const TWAPINFO: Item<Twap> = Item::new("twap");

/// Stores custom config at the given key which can be different between different dexter pools
pub const MATHCONFIG: Item<MathConfig> = Item::new("math-config");

/// ## Description
/// This struct which stores the TWAP calcs related info for the pool
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Twap {
    pub price0_cumulative_last: Uint128,
    pub price1_cumulative_last: Uint128,
    pub block_time_last: u64,
}

/// ## Description
/// This struct describes the main math configuration of pool.
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

/// This struct holds stableswap pool parameters.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct StablePoolParams {
    /// The current stableswap pool amplification
    pub amp: u64,
}

/// This enum stores the options available to start and stop changing a stableswap pool's amplification.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StablePoolUpdateParams {
    StartChangingAmp { next_amp: u64, next_amp_time: u64 },
    StopChangingAmp {},
}
