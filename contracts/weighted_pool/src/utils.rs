use std::cmp::Ordering;

use cosmwasm_std::{
    to_binary, wasm_execute, Addr, Api, CosmosMsg, Decimal, Decimal256, Deps, Env, QuerierWrapper,
    StdResult, Storage, Uint128, Uint64,
};
use dexter::asset::{Asset, AssetInfo, DecimalAsset};
use dexter::helper::{adjust_precision, select_pools};
use dexter::pool::{Config, ResponseType};
use dexter::DecimalCheckedOps;

use crate::error::ContractError;
use crate::math::calc_minted_shares_given_single_asset_in;
use crate::state::{get_precision, MathConfig, Twap, WeightedAsset};

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
    env: &Env,
    offer_asset: &DecimalAsset,
    offer_pool: &DecimalAsset,
    offer_weight: Decimal,
    ask_pool: &DecimalAsset,
    ask_weight: Decimal,
) -> StdResult<(Uint128, Uint128)> {
    // get ask asset precisison
    let token_precision = get_precision(&storage, &ask_pool.info)?;

    let pool_post_swap_in_balance = offer_pool.amount.checked_add(offer_asset.amount)?;

    // deduct swapfee on the tokensIn
    // delta balanceOut is positive(tokens inside the pool decreases)
    let return_amount = solveConstantFunctionInvariant(
        offer_pool.amount,
        pool_post_swap_in_balance,
        offer_weight,
        ask_pool.amount,
        ask_weight,
    )?;
    // TO-DO : Implement the spread calculation.
    let spread_amount = Uint128::zero();
    Ok((return_amount, spread_amount))
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
    env: &Env,
    offer_pool: &DecimalAsset,
    offer_weight: Decimal,
    ask_asset: &DecimalAsset,
    ask_pool: &DecimalAsset,
    ask_weight: Decimal,
) -> StdResult<(Uint128, Uint128)> {
    // get ask asset precisison
    let token_precision = get_precision(&storage, &ask_pool.info)?;

    let pool_post_swap_out_balance = ask_pool.amount.checked_sub(ask_asset.amount)?;

    // deduct swapfee on the tokensIn
    // delta balanceOut is positive(tokens inside the pool decreases)
    let in_amount = solveConstantFunctionInvariant(
        ask_pool.amount,
        pool_post_swap_out_balance,
        ask_weight,
        offer_pool.amount,
        offer_weight,
    )?;
    // TO-DO : Implement the spread calculation.
    let spread_amount = Uint128::zero();
    Ok((in_amount, spread_amount))
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
    math_config: MathConfig,
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
        // retrive the offer and ask asset pool's latest balances
        let (offer_pool, ask_pool) = select_pools(Some(from), Some(to), pools).unwrap();
        // Compute the current price of ask asset in base asset
        let (return_amount, _) = compute_swap(
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

/// Calculate the max price-matching asset basket and the left-over assets along with the amount of LP tokens that should be minted.
pub fn maximal_exact_ratio_join(
    act_assets_in: Vec<Asset>,
    pool_assets_weighted: Vec<WeightedAsset>,
    total_share: Uint128,
) -> StdResult<(Uint128, Vec<Asset>, ResponseType)> {
    // Max price-matching asset basket is defined by the smallest share of some asset X.
    let mut min_share = Decimal::one();
    let mut max_share = Decimal::zero();
    let mut asset_shares = vec![];
    for asset in &act_assets_in {
        for weighted_asset in &pool_assets_weighted {
            // Would have been better with HashMap type.
            if weighted_asset.asset.info.equal(&asset.info) {
                // denom will never be 0 as long as total_share > 0
                let share_ratio = Decimal::from_ratio(asset.amount, weighted_asset.asset.amount);
                min_share = min_share.min(share_ratio);
                max_share = max_share.max(share_ratio);
                asset_shares.push(share_ratio);
            }
        }
    }
    let new_shares = min_share * total_share;

    let mut rem_assets = vec![];

    if min_share.ne(&max_share) {
        // assets aren't balanced
        for (i, asset) in act_assets_in.iter().enumerate() {
            if asset_shares[i].eq(&min_share) {
                continue;
            }
            // account for unused amounts
            let used_amount = act_assets_in[i].amount - min_share * act_assets_in[i].amount;
            let new_amount = act_assets_in[i].amount - used_amount;
            if new_amount.is_zero() {
                continue;
            }
            rem_assets.push(Asset {
                info: act_assets_in[i].info,
                amount: new_amount,
            });
        }
    }
    Ok((new_shares, rem_assets, ResponseType::Success {}))
}

pub fn calc_single_asset_join(
    asset_in: Asset,
    total_fee_bps: Decimal,
    pool_asset_weighted: &WeightedAsset,
    total_shares: Uint128,
) -> StdResult<Uint128> {
    // Asset weights already normalized
    calc_minted_shares_given_single_asset_in(
        asset_in.amount,
        pool_asset_weighted,
        total_shares,
        total_fee_bps,
    )
}
