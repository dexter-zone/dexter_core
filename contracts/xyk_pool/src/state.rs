use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint128;
use cw_storage_plus::Item;
use dexter::pool::Config;

/// ## Description
/// Stores config at the given key
pub const CONFIG: Item<Config> = Item::new("config");

/// Stores Twap prices for the tokens supported by the pool
pub const TWAPINFO: Item<Twap> = Item::new("twap");

/// ## Description
/// This structure which stores the TWAP calcs related info for the pool
#[cw_serde]
pub struct Twap {
    pub price0_cumulative_last: Uint128,
    pub price1_cumulative_last: Uint128,
    pub block_time_last: u64,
}
