use cosmwasm_std::{Decimal, Decimal256, Deps, Env, StdResult, Storage, Uint128, Uint256};

use dexter::asset::{Decimal256Ext, DecimalAsset};
use dexter::helper::{adjust_precision, select_pools, decimal2decimal256};
use dexter::vault::FEE_PRECISION;

use crate::error::ContractError;
use crate::math::calc_y;
use crate::state::MathConfig;
use crate::state::{get_precision, Twap};

// --------x--------x--------x--------x--------x--------x--------x--------x---------
// --------x--------x SWAP :: Offer and Ask amount computations  x--------x---------
// --------x--------x--------x--------x--------x--------x--------x--------x---------

/// ## Description
///  Returns the result of a swap, if errors then returns [`ContractError`].
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
    ask_asset_scaling_factor: Decimal256,
) -> StdResult<(Uint128, Uint128)> {
    // get ask asset precision
    let token_precision = get_precision(storage, &ask_pool.info)?;

    let new_ask_pool = calc_y(
        &offer_asset,
        &ask_pool.info,
        offer_pool.amount + offer_asset.amount,
        pools,
        compute_current_amp(math_config, env)?,
        token_precision,
    )?;

    let new_ask_pool_with_scaling_factor = Decimal256::with_precision(new_ask_pool, token_precision as u32)?
        .checked_mul(ask_asset_scaling_factor)?;

    let new_ask_pool = new_ask_pool_with_scaling_factor.to_uint128_with_precision(token_precision)?;
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
    commission_rate: u16,
    greatest_precision: u8,
    offer_asset_scaling_factor: Option<Decimal256>,
) -> StdResult<(Uint128, Uint128, Uint128)> {
    let offer_precision = get_precision(storage, &offer_pool.info)?;
    let ask_precision = get_precision(storage, &ask_asset.info)?;

    let one_minus_commission = Decimal256::one()
        - decimal2decimal256(Decimal::from_ratio(commission_rate, FEE_PRECISION))?;
    let inv_one_minus_commission = Decimal256::one() / one_minus_commission;

    // New offer pool amount here is returned with the greatest precision.
    // To get the corresponding uin1t128 value, we need to make sure we use the greatest precision.
    let new_offer_pool = calc_y(
        &ask_pool,
        &offer_pool.info,
        ask_pool.amount - ask_asset.amount,
        &pools,
        compute_current_amp(&math_config, &env)?,
        greatest_precision,
    )?;

    // offer_amount = new_offer_pool - offer_pool
    let offer_amount_with_scaling_factor = new_offer_pool.checked_sub(
        offer_pool
            .amount
            .to_uint128_with_precision(greatest_precision)?,
    )?;

    let offer_amount_without_scaling_factor = Decimal256::with_precision(offer_amount_with_scaling_factor, greatest_precision as u32)?
        .checked_div(offer_asset_scaling_factor.unwrap_or(Decimal256::one()))
        .unwrap();

    let offer_amount = offer_amount_without_scaling_factor.to_uint128_with_precision(greatest_precision)?;

    let offer_amount_including_fee: Uint128 = (Uint256::from(offer_amount) * inv_one_minus_commission).try_into()?;
    let fee = offer_amount_including_fee - offer_amount;

    let offer_amount_with_precision = adjust_precision(offer_amount_including_fee, greatest_precision, offer_precision)?;
    let fee_with_precision = adjust_precision(fee, greatest_precision, offer_precision)?;

    // We assume the assets should stay in a 1:1 ratio, so the true exchange rate is 1. Any exchange rate < 1 could be considered the spread
    let ask_asset_uint128 = ask_asset.amount.to_uint128_with_precision(ask_precision)?;
    let spread_amount =
        offer_amount_with_scaling_factor.saturating_sub(ask_asset_uint128);

    Ok((offer_amount_with_precision, spread_amount, fee_with_precision))
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
        let time_range = Uint128::from(math_config.next_amp_time)
            .checked_sub(Uint128::from(math_config.init_amp_time))?;

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

// --------x--------x--------x--------x--------x--------x--------
// --------x--------x TWAP Helper Functions   x--------x---------
// --------x--------x--------x--------x--------x--------x--------

/// ## Description
/// Accumulate token prices for the asset pairs in the pool.
///
/// ## Params
/// * **math_config** is an object of type [`MathConfig`]
/// * **twap** is of [`Twap`] type. It consists of cumulative_prices of type [`Vec<(AssetInfo, AssetInfo, Uint128)>`] and block_time_last of type [`u64`] which is the latest timestamp when TWAP prices of asset pairs were last updated.
/// * **pools** is an array of [`DecimalAsset`] type items. These are the assets available in the pool.
pub fn accumulate_prices(
    deps: Deps,
    env: Env,
    math_config: MathConfig,
    twap: &mut Twap,
    pools: &[DecimalAsset],
) -> Result<(), ContractError> {
    // Calculate time elapsed since last price update.
    let block_time = env.block.time.seconds();
    if block_time <= twap.block_time_last {
        return Ok(());
    }
    let time_elapsed = Uint128::from(block_time - twap.block_time_last);

    // Iterate over all asset pairs in the pool and accumulate prices.
    for (from, to, value) in twap.cumulative_prices.iter_mut() {
        let offer_asset = DecimalAsset {
            info: from.clone(),
            amount: Decimal256::one(),
        };

        // TODO: Scaled assets here for prices

        let (offer_pool, ask_pool) = select_pools(from, to, pools).unwrap();
        let (return_amount, _) = compute_swap(
            deps.storage,
            &env,
            &math_config,
            &offer_asset,
            &offer_pool,
            &ask_pool,
            pools,
            Decimal256::one()
        )?;

        // TODO: scale assets here

        *value = value.wrapping_add(time_elapsed.checked_mul(return_amount)?);
    }

    // Update last block time.
    twap.block_time_last = block_time;
    Ok(())
}
