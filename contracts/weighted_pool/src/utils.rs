

use std::cmp::Ordering;

use cosmwasm_std::{
    to_binary, wasm_execute, Addr, Api, CosmosMsg, Decimal, Deps, Env, QuerierWrapper, StdResult,
    Storage, Uint128, Uint64,
};
use cw20::Cw20ExecuteMsg;
use itertools::Itertools;

use dexter::asset::{Asset, AssetInfo, AssetInfoExt};
use dexter::pool::TWAP_PRECISION;
use dexter::querier::{query_fee_info, query_vault_config};
use dexter::DecimalCheckedOps;

use crate::error::ContractError;
use crate::math::calc_y;
use crate::state::{get_precision, Config};






/// ## Description
/// Accumulate token prices for the assets in the pool.
/// ## Params
/// * **deps** is an object of type [`Deps`].
/// * **env** is an object of type [`Env`].
/// * **config** is an object of type [`Config`].
/// * **pools** is an array of [`Asset`] type items. These are the assets available in the pool.
pub fn accumulate_prices(
    deps: Deps,
    env: Env,
    config: &mut Config,
    pools: &[Asset],
) -> Result<(), ContractError> {
    let block_time = env.block.time.seconds();
    if block_time <= config.block_time_last {
        return Ok(());
    }

    let greater_precision = config.greatest_precision.max(TWAP_PRECISION);

    let time_elapsed = Uint128::from(block_time - config.block_time_last);

    let immut_config = config.clone();
    for (from, to, value) in config.cumulative_prices.iter_mut() {
        let offer_asset = from.with_balance(adjust_precision(
            Uint128::from(1u8),
            0u8,
            greater_precision,
        )?);

        let (offer_pool, ask_pool) = select_pools(Some(from), Some(to), pools)?;
        let SwapResult { return_amount, .. } = compute_swap(
            deps.storage,
            &env,
            &immut_config,
            &offer_asset,
            &offer_pool,
            &ask_pool,
            pools,
        )?;

        *value = value.wrapping_add(time_elapsed.checked_mul(return_amount)?);
    }

    config.block_time_last = block_time;

    Ok(())
}
