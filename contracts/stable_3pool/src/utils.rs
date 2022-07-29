use std::cmp::Ordering;

use cosmwasm_std::{
    to_binary, wasm_execute, Addr, Api, CosmosMsg, Decimal, Deps, Env, QuerierWrapper, StdResult,
    Storage, Uint128, Uint64, Decimal256 
};
use cw20::Cw20ExecuteMsg;
use itertools::Itertools;

use dexter::asset::{Asset, AssetInfo, Decimal256Ext, DecimalAsset};
use dexter::vault::TWAP_PRECISION;
use crate::state::MathConfig;

use dexter::DecimalCheckedOps;

use crate::error::ContractError;
use crate::math::calc_y;
use crate::state::{get_precision, Twap};

use dexter::helper::{select_pools, adjust_precision};
use dexter::pool::Config;



/// ## Description
/// Returns the result of a swap in form of a [`SwapResult`] object. In case of error, returns [`ContractError`].
/// 
/// ## Params
/// * **config** is an object of type [`Config`].
/// * **offer_asset** is an object of type [`Asset`]. This is the asset that is being offered.
/// * **offer_pool** is an object of type [`DecimalAsset`]. This is the pool of offered asset.
/// * **ask_pool** is an object of type [`DecimalAsset`]. This is the asked asset.
/// * **pools** is an array of [`DecimalAsset`] type items. These are the assets available in the pool.
pub(crate) fn compute_swap(
    storage: &dyn Storage,
    env: &Env,
    math_config: &MathConfig,
    offer_asset: &DecimalAsset,
    offer_pool: &DecimalAsset,
    ask_pool: &DecimalAsset,
    pools: &[DecimalAsset],
) -> StdResult<(Uint128, Uint128)> {
    // get ask asset precisison
    let token_precision = get_precision(storage, &ask_pool.info)?;

    let new_ask_pool = calc_y(
        &offer_asset.info,
        &ask_pool.info,
        offer_pool.amount + offer_asset.amount,
        pools,
        compute_current_amp(math_config, env)?,
        token_precision,
    )?;

    let return_amount = ask_pool.amount.to_uint128_with_precision(token_precision)? - new_ask_pool;
    let offer_asset_amount = offer_asset
        .amount
        .to_uint128_with_precision(token_precision)?;

    // We consider swap rate 1:1 in stable swap thus any difference is considered as spread.
    let spread_amount = offer_asset_amount.saturating_sub(return_amount);

    Ok((return_amount, spread_amount))
}


/// ## Description
/// Returns an amount of offer assets for a specified amount of ask assets.
/// ## Params
/// * **offer_pool** is an object of type [`Uint128`]. This is the total amount of offer assets in the pool.
/// * **offer_precision** is an object of type [`u8`]. This is the token precision used for the offer amount.
/// * **ask_pool** is an object of type [`Uint128`]. This is the total amount of ask assets in the pool.
/// * **ask_precision** is an object of type [`u8`]. This is the token precision used for the ask amount.
/// * **ask_amount** is an object of type [`Uint128`]. This is the amount of ask assets to swap to.
/// * **commission_rate** is an object of type [`Decimal`]. This is the total amount of fees charged for the swap.
pub(crate) fn compute_offer_amount(
    storage: &dyn Storage,
    env: &Env,
    math_config: &MathConfig,
    ask_asset: &DecimalAsset,
    offer_pool: &DecimalAsset,
    ask_pool: &DecimalAsset,
    pools: &[DecimalAsset],
    commission_rate: Decimal,
    greatest_precision: u8,
) -> StdResult<(Uint128, Uint128, Uint128)> {


    let offer_precision = get_precision(storage, &offer_pool.info)?;
    let ask_precision = get_precision(storage, &ask_asset.info)?;

    let before_commission = Decimal256::with_precision(
        (Decimal::one() - commission_rate)
            .inv()
            .unwrap_or_else(Decimal::one)
            .checked_mul_uint128(ask_asset.amount)?,
        ask_precision as u32,
    )?;

    let offer_amount = calc_y(
        &ask_pool.info,
        &offer_pool.info,
        ask_pool.amount - before_commission,
        &pools,
        compute_current_amp(&math_config, &env)?,
        greatest_precision,
    )?;

    let offer_amount = offer_amount.checked_sub(
        offer_pool
            .amount
            .to_uint128_with_precision(greatest_precision)?,
    )?;

    let offer_amount = adjust_precision(offer_amount, greatest_precision, offer_precision)?;    

    // We assume the assets should stay in a 1:1 ratio, so the true exchange rate is 1. Any exchange rate < 1 could be considered the spread
    let spread_amount = offer_amount
    .saturating_sub(before_commission.to_uint128_with_precision(ask_precision)?);

    let commission_amount = commission_rate.checked_mul_uint128(before_commission.to_uint128_with_precision(ask_precision)?)?;
    Ok((offer_amount, spread_amount, commission_amount))
}




// --------x--------x--------x--------x--------x--------x--------
// --------x--------x AMP COMPUTE Functions   x--------x---------
// --------x--------x--------x--------x--------x--------x--------


/// ## Description
/// Compute the current pool amplification coefficient (AMP).
/// 
/// ## Params
/// * **math_config** is an object of type [`MathConfig`].
fn compute_current_amp(math_config: &MathConfig, env: &Env) -> StdResult<u64> {
    // Get block time
    let block_time = env.block.time.seconds();

    // If we are in the period of AMP change, we calculate the latest new AMP
    if block_time < math_config.next_amp_time {
        // initial and final AMPs
        let init_amp = Uint128::from(math_config.init_amp);
        let next_amp = Uint128::from(math_config.next_amp);

        // time elapsed since AMP change init and the total time range of AMP change
        let elapsed_time =
            Uint128::from(block_time).checked_sub(Uint128::from(math_config.init_amp_time))?;
        let time_range =
            Uint128::from(math_config.next_amp_time).checked_sub(Uint128::from(math_config.init_amp_time))?;

        // Calculate AMP based on if AMP is being increased or decreased
        if math_config.next_amp > math_config.init_amp {
            let amp_range = next_amp - init_amp;
            let res = init_amp + (amp_range * elapsed_time).checked_div(time_range)?;
            Ok(res.u128() as u64)
        } else {
            let amp_range = init_amp - next_amp;
            let res = init_amp - (amp_range * elapsed_time).checked_div(time_range)?;
            Ok(res.u128() as u64)
        }
    } else {
        Ok(math_config.next_amp)
    }
}



// /// ## Description
// /// Helper function to check if the given asset infos are valid.
// pub(crate) fn check_asset_infos(
//     api: &dyn Api,
//     asset_infos: &[AssetInfo],
// ) -> Result<(), ContractError> {
//     if !asset_infos.iter().all_unique() {
//         return Err(ContractError::DoublingAssets {});
//     }

//     asset_infos
//         .iter()
//         .try_for_each(|asset_info| asset_info.check(api))
//         .map_err(Into::into)
// }

// /// ## Description
// /// Helper function to check that the assets in a given array are valid.
// pub(crate) fn check_assets(api: &dyn Api, assets: &[Asset]) -> Result<(), ContractError> {
//     let asset_infos = assets.iter().map(|asset| asset.info.clone()).collect_vec();
//     check_asset_infos(api, &asset_infos)
// }

// /// ## Description
// /// Checks that cw20 token is part of the pool. Returns [`Ok(())`] in case of success,
// /// otherwise [`ContractError`].
// /// ## Params
// /// * **config** is an object of type [`Config`].
// ///
// /// * **cw20_sender** is cw20 token address which is being checked.
// pub(crate) fn check_cw20_in_pool(config: &Config, cw20_sender: &Addr) -> Result<(), ContractError> {
//     for asset_info in &config.assets.info {
//         match asset_info {
//             AssetInfo::Token { contract_addr } if contract_addr == cw20_sender => return Ok(()),
//             _ => {}
//         }
//     }

//     Err(ContractError::Unauthorized {})
// }



/// ## Description
/// Accumulate token prices for the assets in the pool.
/// 
/// ## Params
/// * **config** is an object of type [`Config`].
/// * **pools** is an array of [`DecimalAsset`] type items. These are the assets available in the pool.
pub fn accumulate_prices(
    deps: Deps,
    env: Env,
    config: &mut Config,
    twap: &mut Twap,
    pools: &[DecimalAsset],
) -> Result<(), ContractError> {
    // Calculate time elapsed since last price update.
    let block_time = env.block.time.seconds();
    if block_time <= config.block_time_last {
        return Ok(());
    }
    let time_elapsed = Uint128::from(block_time - config.block_time_last);

    // Iterate over all asset pairs in the pool and accumulate prices.
    let immut_config = config.clone();
    for (from, to, value) in twap.cumulative_prices.iter_mut() {
        let offer_asset = DecimalAsset {
            info: from.clone(),
            amount: Decimal256::one(),
        };

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
    // Update last block time.
    config.block_time_last = block_time;
    Ok(())
}