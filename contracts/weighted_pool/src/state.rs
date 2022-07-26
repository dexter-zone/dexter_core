use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::Item;
use cosmwasm_std::{Addr, Uint128};
use dexter::pool::Config;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// ## Description
/// Stores config at the given key
pub const CONFIG: Item<Config> = Item::new("config");


/// Stores custom Twap at the given key which can be different between different dexter pools
pub const TWAPINFO: Item<Twap> = Item::new("twap");

/// Stores custom config at the given key which can be different between different dexter pools
pub const MATHCONFIG: Item<MathConfig> = Item::new("math_config");

/// Stores map of AssetInfo (as String) -> precision
const PRECISIONS: Map<String, Decimal> = Map::new("precisions");


/// Stores map of AssetInfo (as String) -> Normalized Weight
const WEIGHTS: Map<String, Decimal> = Map::new("weights");


/// ## Description
/// This structure describes the main math config of pool.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MathConfig {
    // This Vector contains the current weights for each asset in the pool
    pub weights: Vec<Uint128>,
    /// The greatest precision of assets in the pool
    pub greatest_precision: u8,       
}


/// ## Description
/// This structure which stores the TWAP calcs related info for the pool
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Twap {
    /// The vector contains cumulative prices for each pair of assets in the pool
    pub cumulative_prices: Vec<(AssetInfo, AssetInfo, Uint128)>,
    pub block_time_last: u64,
}


pub fn get_normalized_weight(identifer: String) {
    let weight: Decimal = CONFIG.load(deps.storage, &identifer)?;
    Ok(weight)
}



/// Stores map of AssetInfo (as String) -> precision
const PRECISIONS: Map<String, u8> = Map::new("precisions");

/// ## Description
/// Store all token precisions and return the greatest one.
pub(crate) fn store_precisions(deps: DepsMut, asset_infos: &[AssetInfo]) -> StdResult<u8> {
    let mut max = 0u8;

    for asset_info in asset_infos {
        let precision = asset_info.query_token_precision(&deps.querier)?;
        max = max.max(precision);
        PRECISIONS.save(deps.storage, asset_info.to_string(), &precision)?;
    }

    Ok(max)
}

/// ## Description
/// Loads precision of the given asset info.
pub(crate) fn get_precision(storage: &dyn Storage, asset_info: &AssetInfo) -> StdResult<u8> {
    PRECISIONS.load(storage, asset_info.to_string())
}