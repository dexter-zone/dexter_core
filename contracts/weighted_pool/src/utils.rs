

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
use crate::state::{get_precision, Config};


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
    ask_weight: Decimal
) -> StdResult<(Uint128, Uint128)> {
    // get ask asset precisison
    let token_precision = get_precision(storage, &ask_pool.info)?;

    let pool_post_swap_in_balance = offer_pool.amount.checked_add( offer_asset.amount )?;

	// deduct swapfee on the tokensIn
	// delta balanceOut is positive(tokens inside the pool decreases)
   let return_amount = solveConstantFunctionInvariant(   offer_pool.amount,
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
    ask_weight: Decimal
) -> StdResult<(Uint128, Uint128)> {
    // get ask asset precisison
    let token_precision = get_precision(storage, &ask_pool.info)?;

    let pool_post_swap_out_balance = ask_pool.amount.checked_sub( ask_asset.amount )?;

	// deduct swapfee on the tokensIn
	// delta balanceOut is positive(tokens inside the pool decreases)
   let in_amount = solveConstantFunctionInvariant(   ask_pool.amount,
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
