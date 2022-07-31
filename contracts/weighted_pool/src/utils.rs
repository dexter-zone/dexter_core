

use std::cmp::Ordering;

use cosmwasm_std::{
    to_binary, wasm_execute, Addr, Api, CosmosMsg, Decimal, Deps, Env, QuerierWrapper, StdResult,
    Storage, Uint128, Uint64,
};
use cw20::Cw20ExecuteMsg;
use itertools::Itertools;

use dexter::asset::{Asset, AssetInfo, AssetInfoExt};
use dexter::pool::TWAP_PRECISION;
use dexter::DecimalCheckedOps;

use crate::error::ContractError;
use crate::math::calc_y;
use crate::state::{get_precision, Config};



// --------x--------x--------x--------x--------x--------x--------
// --------x--------x TWAP Helper Functions   x--------x---------
// --------x--------x--------x--------x--------x--------x--------



/// ## Description
/// Accumulate token prices for the asset pairs in the pool.
/// 
/// ## Params
/// ## Params
/// * **config** is an object of type [`Config`].
/// * **math_config** is an object of type [`MathConfig`]
/// * **twap** is of [`Twap`] type. It consists of cumulative_prices of type [`Vec<(AssetInfo, AssetInfo, Uint128)>`] and block_time_last of type [`u64`] which is the latest timestamp when TWAP prices of asset pairs were last updated.
/// * **pools** is an array of [`DecimalAsset`] type items. These are the assets available in the pool.
pub fn accumulate_prices(
    deps: Deps,
    env: Env,
    config: &mut Config,
    math_config: MathConfig,
    twap: &mut Twap,
    pools: &[DecimalAsset]
) -> Result<(), ContractError> {
    // Calculate time elapsed since last price update.
    let block_time = env.block.time.seconds();
    if block_time <= config.block_time_last {
        return Ok(());
    }
    let time_elapsed = Uint128::from(block_time - config.block_time_last);

    // Iterate over all asset pairs in the pool and accumulate prices.
    for (from, to, value) in twap.cumulative_prices.iter_mut() {
        let offer_asset = DecimalAsset {
            info: from.clone(),
            amount: Decimal256::one(),
        };
        // retrive the offer and ask asset pool's latest balances
        let (offer_pool, ask_pool) = select_pools(Some(from), Some(to), pools).unwrap();
        // Compute the current price of ask asset in base asset 
        let (return_amount, _)= compute_swap(
            deps.storage,
            &env,
            &math_config,
            &offer_asset,
            &offer_pool,
            &ask_pool,
            pools,
        )?;
        // accumulate the price
        *value = value.wrapping_add(time_elapsed.checked_mul(return_amount)?);
    }

    // Update last block time.
    config.block_time_last = block_time;
    Ok(())
}
