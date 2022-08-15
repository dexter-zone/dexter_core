use cosmwasm_std::{Addr, Decimal, DepsMut, StdResult, Storage, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dexter::asset::{Asset, AssetInfo};
use dexter::pool::Config;

/// ## Description
/// Stores config at the given key
pub const CONFIG: Item<Config> = Item::new("config");

/// Stores custom Twap at the given key which can be different between different dexter pools
pub const TWAPINFO: Item<Twap> = Item::new("twap");

/// Stores custom config at the given key which can be different between different dexter pools
pub const MATHCONFIG: Item<MathConfig> = Item::new("math_config");

/// Stores map of AssetInfo (as String) -> precision
pub const PRECISIONS: Map<String, u8> = Map::new("precisions");

/// Stores map of AssetInfo (as String) -> precision
pub const WEIGHTS: Map<String, Decimal> = Map::new("weights");

/// ## Description
/// This struct describes the main math config of pool.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MathConfig {
    /// Exit fee in % charged when liquidity is withdrawn from the pool
    pub exit_fee: Option<Decimal>,
    /// The greatest precision of assets in the pool
    pub greatest_precision: u8,
}

/// ## Description
/// This struct which stores the TWAP calcs related info for the pool
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Twap {
    /// The vector contains cumulative prices for each pair of assets in the pool
    pub cumulative_prices: Vec<(AssetInfo, AssetInfo, Uint128)>,
    pub block_time_last: u64,
}

// ----------------x----------------x----------------x----------------
// ----------------x      PRESISION : Store and getter fns     x------
// ----------------x----------------x----------------x----------------

/// ## Description
/// Store all token precisions and return the greatest one.
pub(crate) fn store_precisions(deps: DepsMut, asset_infos: &[AssetInfo]) -> StdResult<u8> {
    let mut max = 0u8;

    for asset_info in asset_infos {
        let precision = asset_info.decimals(&deps.querier)?;
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

// ----------------x----------------x----------------x----------------
// ----------------x      WEIGHTS : Store and getter fns     x------
// ----------------x----------------x----------------x----------------

/// ## Description
/// Store all token weights
pub(crate) fn store_weights(
    deps: DepsMut,
    asset_weights: Vec<(AssetInfo, Decimal)>,
) -> StdResult<()> {
    for (asset_info, weight) in asset_weights.iter() {
        WEIGHTS.save(deps.storage, asset_info.to_string(), weight)?;
    }

    Ok(())
}

/// ## Description
/// Loads precision of the given asset info.
pub(crate) fn get_weight(storage: &dyn Storage, asset_info: &AssetInfo) -> StdResult<Decimal> {
    WEIGHTS.load(storage, asset_info.to_string())
}

/// ## Description - This struct describes a asset (native or CW20) and its normalized weight
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct WeightedAsset {
    /// Information about an asset stored in a [`Asset`] struct
    pub asset: Asset,
    /// The weight of the asset
    pub weight: Decimal,
}
