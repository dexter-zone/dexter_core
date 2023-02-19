use std::collections::HashMap;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{DepsMut, StdResult, Storage, Uint128, Addr, Decimal256};
use cw_storage_plus::{Item, Map};
use dexter::asset::AssetInfo;
use dexter::pool::Config;

/// ## Description
/// Stores config at the given key
pub const CONFIG: Item<Config> = Item::new("config");
/// Stores extra config for stableswap at the given key
pub const STABLESWAP_CONFIG: Item<StableSwapConfig> = Item::new("stableswap_config");

///  Stores Twap prices for the tokens supported by the pool
pub const TWAPINFO: Item<Twap> = Item::new("twap");

/// Stores custom config at the given key which can be different between different dexter pools
pub const MATHCONFIG: Item<MathConfig> = Item::new("math_config");
/// ## Description
/// This struct describes the main math config of pool.
#[cw_serde]
pub struct MathConfig {
    // This is the current amplification used in the pool
    pub init_amp: u64,
    // This is the start time when amplification starts to scale up or down
    pub init_amp_time: u64,
    // This is the target amplification to reach at `next_amp_time`
    pub next_amp: u64,
    // This is the timestamp when the current pool amplification should be `next_amp`
    pub next_amp_time: u64,
    /// The greatest precision of assets in the pool
    pub greatest_precision: u8,
}

#[cw_serde]
pub struct StableSwapConfig {
    pub supports_scaling_factors_update: bool,
    pub scaling_factors: Vec<AssetScalingFactor>,
    // This address is allowed to update scaling factors. This address is required if support_scaling_factors_update is true.
    pub scaling_factor_manager: Option<Addr>,
}

impl StableSwapConfig {
    pub fn scaling_factors(&self) -> HashMap<AssetInfo, Decimal256> {
        let mut scaling_factors = HashMap::new();
        for scaling_factor in &self.scaling_factors {
            scaling_factors.insert(scaling_factor.asset_info.clone(), scaling_factor.scaling_factor);
        }
        scaling_factors
    }

    pub fn get_scaling_factor_for(&self, asset_info: &AssetInfo) -> Option<Decimal256> {
        for scaling_factor in &self.scaling_factors {
            if scaling_factor.asset_info == *asset_info {
                return Some(scaling_factor.scaling_factor);
            }
        }
        None
    }
}

/// ## Description
/// This struct which stores the TWAP calcs related info for the pool
#[cw_serde]
pub struct Twap {
    /// The vector contains cumulative prices for each pair of assets in the pool
    pub cumulative_prices: Vec<(AssetInfo, AssetInfo, Uint128)>,
    /// The latest timestamp when TWAP prices of asset pairs were last updated.
    /// Although it seems same as the param inside CONFIG, but it is different. As the TWAP price
    /// accumulation not always succeeds, so this might be different than the one in config.
    /// So, better to keep it here.
    pub block_time_last: u64,
}

/// This struct holds stableswap pool parameters.
#[cw_serde]
pub struct StablePoolParams {
    /// The current stableswap pool amplification
    pub amp: u64,
    /// Support scaling factors update
    pub supports_scaling_factors_update: bool,
    /// Scaling factors
    pub scaling_factors: Vec<AssetScalingFactor>,
    /// Scaling factor manager
    pub scaling_factor_manager: Option<Addr>,
}

#[cw_serde]
pub struct AssetScalingFactor {
    pub asset_info: AssetInfo,
    pub scaling_factor: Decimal256,
}

/// This enum stores the options available to start and stop changing a stableswap pool's amplification.
#[cw_serde]
pub enum StablePoolUpdateParams {
    StartChangingAmp { next_amp: u64, next_amp_time: u64 },
    StopChangingAmp {},
    UpdateScalingFactorManager { manager: Addr },
    UpdateScalingFactor { asset: AssetInfo, scaling_factor: Decimal256 },
}

// ----------------x----------------x----------------x----------------
// ----------------x      PRESISION : Store and getter fns     x------
// ----------------x----------------x----------------x----------------

/// Stores map of AssetInfo (as String) -> precision
pub const PRECISIONS: Map<String, u8> = Map::new("precisions");

/// ## Description
/// Store all token precisions and return the greatest one.
pub(crate) fn store_precisions(deps: DepsMut, native_asset_precision: &Vec<(String, u8)>, asset_infos: &[AssetInfo]) -> StdResult<u8> {
    let mut max = 0u8;

    for asset_info in asset_infos {
        let precision = asset_info.decimals(native_asset_precision, &deps.querier)?;
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
