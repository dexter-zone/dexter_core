use std::str::FromStr;

use cosmwasm_std::{Decimal, Decimal256, Deps, Env, StdError, StdResult, Storage, Uint128};
use dexter::asset::{Asset, DecimalAsset};
use dexter::helper::{adjust_precision, decimal2decimal256, select_pools};
use dexter::pool::{Config, ResponseType};

use crate::error::ContractError;
use crate::math::{calc_minted_shares_given_single_asset_in, solve_constant_function_invariant};
use crate::state::{get_precision, get_weight, MathConfig, Twap, WeightedAsset};
use dexter::vault::FEE_PRECISION;

// --------x--------x--------x--------x--------x--------x--------x--------x---------
// --------x--------x SWAP :: Offer and Ask amount computations  x--------x---------
// --------x--------x--------x--------x--------x--------x--------x--------x---------

/// ## Description
///  Returns the result of a swap, if erros then returns [`ContractError`].
///
/// ## Params
/// * **config** is an object of type [`Config`].
/// * **offer_asset** is an object of type [`Asset`]. This is the asset that is being offered.
/// * **offer_pool** is an object of type [`DecimalAsset`]. This is the pool of offered asset.
/// * **ask_pool** is an object of type [`DecimalAsset`]. This is the asked asset.
/// * **pools** is an array of [`DecimalAsset`] type items. These are the assets available in the pool.
pub(crate) fn compute_swap(
    storage: &dyn Storage,
    _env: &Env,
    offer_asset: &DecimalAsset,
    offer_pool: &DecimalAsset,
    offer_weight: Decimal,
    ask_pool: &DecimalAsset,
    ask_weight: Decimal,
) -> StdResult<(Uint128, Uint128)> {
    // get ask asset precisison
    let token_precision = get_precision(storage, &ask_pool.info)?;

    let pool_post_swap_in_balance = offer_pool.amount + offer_asset.amount;

    //         /**********************************************************************************************
    //         // outGivenIn                                                                                //
    //         // aO = amountOut                                                                            //
    //         // bO = balanceOut                                                                           //
    //         // bI = balanceIn              /      /            bI             \    (wI / wO) \           //
    //         // aI = amountIn    aO = bO * |  1 - | --------------------------  | ^            |          //
    //         // wI = weightIn               \      \       ( bI + aI )         /              /           //
    //         // wO = weightOut                                                                            //
    //         **********************************************************************************************/
    // deduct swapfee on the tokensIn
    // delta balanceOut is positive(tokens inside the pool decreases)
    let return_amount = solve_constant_function_invariant(
        Decimal::from_str(&offer_pool.amount.to_string())?,
        Decimal::from_str(&pool_post_swap_in_balance.to_string())?,
        offer_weight,
        Decimal::from_str(&ask_pool.amount.to_string())?,
        ask_weight,
    )?;

    // adjust return amount to correct precision
    let return_amount = adjust_precision(
        return_amount.atomics(),
        return_amount.decimal_places() as u8,
        token_precision,
    )?;

    // difference in return amount compared to "ideal" swap.
    Ok((return_amount, Uint128::zero()))
}

/// ## Description
///  Returns the result of a swap, if erros then returns [`ContractError`].
///
/// ## Params
/// * **config** is an object of type [`Config`].
/// * **offer_asset** is an object of type [`Asset`]. This is the asset that is being offered.
/// * **offer_pool** is an object of type [`DecimalAsset`]. This is the pool of offered asset.
/// * **ask_pool** is an object of type [`DecimalAsset`]. This is the asked asset.
/// * **pools** is an array of [`DecimalAsset`] type items. These are the assets available in the pool.
pub(crate) fn compute_offer_amount(
    storage: &dyn Storage,
    _env: &Env,
    ask_asset: &DecimalAsset,
    ask_pool: &DecimalAsset,
    ask_weight: Decimal,
    offer_pool: &DecimalAsset,
    offer_weight: Decimal,
    commission_rate: u16,
) -> StdResult<(Uint128, Uint128, Uint128)> {
    // get ask asset precisison
    let token_precision = get_precision(storage, &offer_pool.info)?;

    let one_minus_commission = Decimal256::one()
        - decimal2decimal256(Decimal::from_ratio(commission_rate, FEE_PRECISION))?;
    let inv_one_minus_commission = Decimal256::one() / one_minus_commission;

    let ask_asset_amount = Decimal::from_str(&ask_asset.amount.clone().to_string())?;
    let before_commission_deduction =
        ask_asset_amount * Decimal::from_str(&inv_one_minus_commission.clone().to_string())?;

    // Ask pool balance after swap
    let pool_post_swap_out_balance =
        Decimal::from_str(&ask_pool.amount.to_string())? - before_commission_deduction;

    //         /**********************************************************************************************
    //         // inGivenOut                                                                                //
    //         // aO = amountOut                                                                            //
    //         // bO = balanceOut                                                                           //
    //         // bI = balanceIn              /  /            bO             \    (wO / wI)      \          //
    //         // aI = amountIn    aI = bI * |  | --------------------------  | ^            - 1  |         //
    //         // wI = weightIn               \  \       ( bO - aO )         /                   /          //
    //         // wO = weightOut                                                                            //
    //         **********************************************************************************************/
    // deduct swapfee on the tokensIn
    // delta balanceOut is positive(tokens inside the pool decreases)
    let real_offer = solve_constant_function_invariant(
        Decimal::from_str(&ask_pool.amount.to_string())?,
        pool_post_swap_out_balance,
        ask_weight,
        Decimal::from_str(&offer_pool.amount.to_string())?,
        offer_weight,
    )?;
    // adjust return amount to correct precision
    let real_offer = adjust_precision(
        real_offer.atomics(),
        real_offer.decimal_places() as u8,
        token_precision,
    )?;

    let before_commission_deduction_ = adjust_precision(
        before_commission_deduction.atomics(),
        before_commission_deduction.decimal_places() as u8,
        token_precision,
    )?;

    Ok((real_offer, Uint128::zero(), before_commission_deduction_))
}

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
    _math_config: MathConfig,
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
    for (from, to, value) in twap.cumulative_prices.iter_mut() {
        let offer_asset = DecimalAsset {
            info: from.clone(),
            amount: Decimal256::one(),
        };

        let from_weight = get_weight(deps.storage, from)?;
        let to_weight = get_weight(deps.storage, to)?;

        // retrive the offer and ask asset pool's latest balances
        let (offer_pool, ask_pool) = select_pools(Some(from), Some(to), pools).unwrap();

        // Compute the current price of ask asset in base asset
        let (return_amount, _) = compute_swap(
            deps.storage,
            &env,
            &offer_asset,
            &offer_pool,
            from_weight,
            &ask_pool,
            to_weight,
        )?;

        // accumulate the price
        *value = value.wrapping_add(time_elapsed.checked_mul(return_amount)?);
    }

    // Update last block time.
    config.block_time_last = block_time;
    Ok(())
}

/// --------- x --------- x --------- x --------- x --------- x --------- x --------- x --------- x --------- x ---------
/// MaximalExactRatioJoin calculates the maximal amount of tokens that can be joined whilst maintaining pool asset's ratio
/// returning the number of shares that'd be and how many coins would be left over.
///
///	e.g) suppose we have a pool of 10 foo tokens and 10 bar tokens, with the total amount of 100 shares.
///		 if `tokensIn` provided is 1 foo token and 2 bar tokens, `MaximalExactRatioJoin`
///		 would be returning (10 shares, 1 bar token, nil)
///
/// This can be used when `tokensIn` are not guaranteed the same ratio as assets in the pool.
/// Calculation for this is done in the following steps.
///  1. iterate through all the tokens provided as an argument, calculate how much ratio it accounts for the asset in the pool
///  2. get the minimal share ratio that would work as the benchmark for all tokens.
///  3. calculate the number of shares that could be joined (total share * min share ratio), return the remaining coins
pub fn maximal_exact_ratio_join(
    act_assets_in: Vec<Asset>,
    pool_assets_weighted: &Vec<WeightedAsset>,
    total_share: Uint128,
) -> StdResult<(Uint128, Vec<Asset>, ResponseType)> {
    let mut min_share = Decimal::one();
    let mut max_share = Decimal::zero();
    let mut asset_shares = vec![];

    for (asset_in, weighted_pool_in) in act_assets_in
        .clone()
        .into_iter()
        .zip(pool_assets_weighted.into_iter())
    {
        if !weighted_pool_in.asset.info.equal(&asset_in.info) {
            return Err(StdError::generic_err("Assets not sorted in order"));
        }
        // denom will never be 0 as long as total_share > 0
        let share_ratio = Decimal::from_ratio(asset_in.amount, weighted_pool_in.asset.amount);
        min_share = min_share.min(share_ratio);
        max_share = max_share.max(share_ratio);
        asset_shares.push(share_ratio);
    }

    let new_shares = min_share * total_share;
    let mut rem_assets = vec![];

    if min_share.ne(&max_share) {
        // assets aren't balanced and we have to calculate remCoins
        let mut i = 0;
        for (asset_in, weighted_pool_in) in act_assets_in
            .clone()
            .into_iter()
            .zip(pool_assets_weighted.into_iter())
        {
            // if coinShareRatios[i] == minShareRatio, no remainder
            if asset_shares[i].eq(&min_share) {
                i += 1;
                continue;
            }
            i += 1;

            // account for unused amounts
            let used_amount = min_share * weighted_pool_in.asset.amount;
            let new_amount = asset_in.amount - used_amount;

            // if coinShareRatios[i] == minShareRatio, no remainder
            if !new_amount.is_zero() {
                rem_assets.push(Asset {
                    info: asset_in.info.clone(),
                    amount: new_amount,
                });
            }
        }
    }

    Ok((new_shares, rem_assets, ResponseType::Success {}))
}

/// Calculate the amount of LP tokens that should be minted for single asset deposit.
/// Returns the amount of LP tokens to be minted
pub fn calc_single_asset_join(
    deps: Deps,
    asset_in: &Asset,
    total_fee_bps: u16,
    pool_asset_weighted: &WeightedAsset,
    total_shares: Uint128,
) -> StdResult<(Uint128, Uint128)> {
    let in_precision = get_precision(deps.storage, &asset_in.info)?;

    // Asset weights already normalized
    calc_minted_shares_given_single_asset_in(
        asset_in.amount,
        in_precision.into(),
        pool_asset_weighted,
        total_shares,
        Decimal::from_ratio(total_fee_bps, FEE_PRECISION),
    )
}

// --------x--------x--------x--------x--------x--------x---
// --------x--------x Helper Functions   x--------x---------
// --------x--------x--------x--------x--------x--------x---

/// ## Description
/// Converts [`Vec<Asset>`] to [`Vec<DecimalAsset>`].
pub fn transform_to_decimal_asset(deps: Deps, assets: &Vec<Asset>) -> Vec<DecimalAsset> {
    let decimal_assets = assets
        .iter()
        .map(|asset| {
            let precision = get_precision(deps.storage, &asset.info)?;
            asset.to_decimal_asset(precision)
        })
        .collect::<StdResult<Vec<DecimalAsset>>>()
        .unwrap();
    decimal_assets
}

// Update pool liquidity balances after joins
pub fn update_pool_state_for_joins(
    tokens_joined: &[Asset],
    pool_assets_weighted: &mut Vec<WeightedAsset>,
) {
    for asset in tokens_joined {
        for pool_asset in pool_assets_weighted.iter_mut() {
            if asset.info.equal(&pool_asset.asset.info) {
                pool_asset.asset.amount += asset.amount;
            }
        }
    }
}
