use cosmwasm_std::{Decimal, Decimal256, StdResult, Uint128};
use std::str::FromStr;

use dexter::{
    approx_pow::calculate_pow,
    asset::DecimalAsset,
    helper::{adjust_precision, decimal2decimal256},
};

use crate::state::WeightedAsset;

// // https://github.com/officialnico/balancerv2cad/blob/main/src/balancerv2cad/WeightedMath.py
// pub fn calculate_invariant(
//     normalized_weights: Vec<Decimal>,
//     balances: Vec<DecimalAsset>,
// ) -> StdResult<Decimal256> {
//     println!("\n--- calculate_invariant() FN CALLED ---");
//     //  /**********************************************************************************************
//     // invariant               _____                                                             //
//     // wi = weight index i      | |      wi                                                      //
//     // bi = balance index i     | |  bi ^   = i                                                  //
//     // i = invariant                                                                             //
//     // **********************************************************************************************/
//     let mut invariant: Decimal256 = Decimal256::one();
//     for (wi, bi) in normalized_weights.into_iter().zip(balances.into_iter()) {
//         println!("asset: {}, balance: {}, weight:{}", bi.info.as_string(), bi.amount, wi);
//         let pow_calc = calculate_pow( Decimal::from_str(&bi.amount.to_string())?,  wi,  None,)?;
//         println!("pow_calc: {}", pow_calc);
//         invariant = invariant * decimal2decimal256(pow_calc)?;
//         println!("invariant: {}", invariant);
//     }
//     Ok(invariant)
// }

// Referenced from Balancer Weighted pool implementation by  Osmosis here - https://github.com/osmosis-labs/osmosis/blob/47a2366c5eeee474de9e1cb4777fab0ccfbb9592/x/gamm/pool-models/balancer/amm.go#L94
// solveConstantFunctionInvariant solves the constant function of an AMM
// that determines the relationship between the differences of two sides
// of assets inside the pool.
// --------------------------
// For fixed balanceXBefore, balanceXAfter, weightX, balanceY, weightY,
// we could deduce the balanceYDelta, calculated by:
// balanceYDelta = balanceY * (1 - (balanceXBefore/balanceXAfter)^(weightX/weightY))
// balanceYDelta is positive when the balance liquidity decreases.
// balanceYDelta is negative when the balance liquidity increases.
pub fn solve_constant_function_invariant(
    token_balance_fixed_before: Decimal,
    token_balance_fixed_after: Decimal,
    token_weight_fixed: Decimal,
    token_balance_unknown_before: Decimal,
    token_weight_unknown: Decimal,
) -> StdResult<Decimal> {
    println!("\n-solve_constant_function_invariant FN CALLED-");
    // weight_ratio = (weightX/weightY)
    let weight_ratio = token_weight_fixed / token_weight_unknown;
    println!("weight_ratio = (weightX/weightY): {}", weight_ratio);

    // y = balanceXBefore/balanceXAfter
    let y = token_balance_fixed_before / token_balance_fixed_after;
    println!("y = balanceXBefore/balanceXAfter : {}", y);

    // amount_y = balanceY * (1 - (y ^ weight_ratio))
    let y_to_weight_ratio = calculate_pow(y, weight_ratio, None)?;
    println!("y_to_weight_ratio: {}", y_to_weight_ratio);

    // Decimal is an unsigned so always return abs value
    let paranthetical = if y_to_weight_ratio <= Decimal::one() {
        Decimal::one() - y_to_weight_ratio
    } else {
        y_to_weight_ratio - Decimal::one()
    };
    println!("paranthetical: {}", paranthetical);

    let amount_y = token_balance_unknown_before * paranthetical;
    println!("amount_y = (balanceY * (1 - (y ^ weight_ratio))): {}", amount_y);

    return Ok(amount_y);
}

// /// ## Description
// /// Calculates the ask amount (the amount of tokens swapped to).
// /// ## Params
// /// * **curBalTokenIn** is an object of type [`u128`]. This is the amount of offer tokens currently in a pool.
// /// * **weightIn** Normailized weight of the offer tokens.
// /// * **curBalTokenOut** is an object of type [`u128`]. This is the amount of ask tokens currently in a pool.
// /// * **weightOut** Normailized weight of the ask tokens.
// /// * **offer_amount** is an object of type [`u128`]. This is the amount of offer tokens that will be swapped.
// pub fn calc_ask_amount(
//     curBalTokenIn: u128,
//     weightIn: u128,
//     curBalTokenOut: u128,
//     weightOut: u128,
//     offer_amount: u128
// ) -> Option<u128, u128> {
//         /**********************************************************************************************
//         // outGivenIn                                                                                //
//         // aO = amountOut                                                                            //
//         // bO = balanceOut                                                                           //
//         // bI = balanceIn              /      /            bI             \    (wI / wO) \           //
//         // aI = amountIn    aO = bO * |  1 - | --------------------------  | ^            |          //
//         // wI = weightIn               \      \       ( bI + aI )         /              /           //
//         // wO = weightOut                                                                            //
//         **********************************************************************************************/
//     // let base = curBalTokenIn.checked_div( curBalTokenIn.checked_add( offer_amount )? )?;
//     // let power = weightIn.checked_div( weightOut )?;
//     // let sub = Decimal::from(base as u64).checked_powf(power)?.to_u128()?;
//     // let return_amount = curBalTokenOut.checked_mul( 1 - sub )?;
//     // let spread_amount = offer_asset.amount.saturating_sub(return_amount);
//     // let commission = 0 as u128;

//     Some(return_amount, spread_amount, commission)
// }

// /// ## Description
// /// Calculates the amount to be swapped (the offer amount).
// /// ## Params
// /// * **curBalTokenIn** is an object of type [`u128`]. This is the amount of offer tokens currently in a pool.
// /// * **weightIn** Normailized weight of the offer tokens.
// /// * **curBalTokenOut** is an object of type [`u128`]. This is the amount of ask tokens currently in a pool.
// /// * **weightOut** Normailized weight of the ask tokens.
// /// * **ask_amount** is an object of type [`u128`]. This is the amount of ask tokens that will be swapped.
// pub fn calc_offer_amount(
//     curBalTokenIn: u128,
//     weightIn: u128,
//     curBalTokenOut: u128,
//     weightOut: u128,
//     ask_amount: u128
// ) -> Option<u128> {
//         /**********************************************************************************************
//         // inGivenOut                                                                                //
//         // aO = amountOut                                                                            //
//         // bO = balanceOut                                                                           //
//         // bI = balanceIn              /  /            bO             \    (wO / wI)      \          //
//         // aI = amountIn    aI = bI * |  | --------------------------  | ^            - 1  |         //
//         // wI = weightIn               \  \       ( bO - aO )         /                   /          //
//         // wO = weightOut                                                                            //
//         **********************************************************************************************/
//         // let base = curBalTokenOut.checked_div( curBalTokenOut.checked_sub( ask_amount )? )?;
//         // let power = weightOut.checked_div( weightIn )?;
//         // let sub = Decimal::from(base as u64).checked_powf(power)?.to_u128()?;
//         // let return_amount = curBalTokenIn.checked_mul( sub - 1  )?;
//         // let spread_amount = offer_asset.amount.saturating_sub(return_amount);
//         // let commission = 0 as u128;

//         Some(return_amount, spread_amount, commission)
// }

// def calc_tokens_out_given_exact_lp_burnt(assets:Vec<Asset>, normalized_weights:Vec<u128>,  lp_total_supply: Uint128, lp_burned : Uint128) {
//     // swap_fee: Decimal) -> Option<u128> {
//     //     let mut tokens_out : Vec<Asset> = vec![];

//     //     for asset in assets.iter() {
//     //         let mut fraction = lp_total_supply.checked_sub(lp_minted),checked_div(lp_total_supply)?;
//     //         fraction   = Uint128::one().checked_sub(lp_total_supply)?;
//     //         let tokens_out.push(Asset {
//     //             amount: asset.amount.checked_mul(fraction)?,
//     //             info: asset.info.clone()
//     //         });
//     //     }
//     //    Some(tokens_out)

//     }



pub fn calc_minted_shares_given_single_asset_in(
    token_amount_in: Uint128,
    in_precision: u32,
    asset_weight_and_balance: &WeightedAsset,
    total_shares: Uint128,
    swap_fee: Decimal,
) -> StdResult<Uint128> {

    // deduct swapfee on the in asset.
    // We don't charge swap fee on the token amount that we imagine as unswapped (the normalized weight).
    // So, effective_swapfee = swapfee * (1 - normalized_token_weight)
    let fee_ratio = fee_ratio(asset_weight_and_balance.weight, swap_fee);
    let token_amount_in_after_fee = token_amount_in * fee_ratio;
    println!("token_amount_in_after_fee: {:?} , fee_ratio : {:?}", token_amount_in_after_fee, fee_ratio);

    let in_decimal = Decimal::from_atomics(token_amount_in_after_fee, in_precision).unwrap();
    let balance_decimal = Decimal::from_atomics(asset_weight_and_balance.asset.amount, in_precision).unwrap();
    println!("in_decimal: {:?} , balance_decimal : {:?}", in_decimal, balance_decimal);

    // To figure out the number of shares we add, first notice that we can treat
    // the number of shares as linearly related to the `k` value function. This is due to the normalization.
    // e.g, if x^.5 y^.5 = k, then we `n` x the liquidity to `(nx)^.5 (ny)^.5 = nk = k'`
    // ---------
    // We generalize this linear relation to do the liquidity add for the not-all-asset case.
    // Suppose we increase the supply of x by x', so we want to solve for `k'/k`.
    // This is `(x + x')^{weight} * old_terms / (x^{weight} * old_terms) = (x + x')^{weight} / (x^{weight})`
    // The number of new shares we need to make is then `old_shares * ((k'/k) - 1)`
    let pool_amount_out = solve_constant_function_invariant(
        balance_decimal + in_decimal,
        balance_decimal,
        asset_weight_and_balance.weight,
        Decimal::from_atomics(total_shares, 6).unwrap(),
        Decimal::one(),
    )?;
    let pool_amount_out_adj = adjust_precision(
        pool_amount_out.atomics(),
        pool_amount_out.decimal_places() as u8,
        6.into(),
    );
    println!("pool_amount_out: {:?} , pool_amount_out_adj : {:?}", pool_amount_out, pool_amount_out_adj);

    return pool_amount_out_adj;
}

// feeRatio returns the fee ratio that is defined as follows:
// 1 - ((1 - normalizedTokenWeightOut) * swapFee)
fn fee_ratio(normalized_weight: Decimal, swap_fee: Decimal) -> Decimal {
    return Decimal::one() - ((Decimal::one() - normalized_weight) * swap_fee);
}

/// ## Description
/// Calculates the weight of an asset as % of the total weight share. Returns a decimal.
///
/// ## Params
/// * **weight** is the weight of the asset.
/// * **total_weight** is the total weight of all assets.
pub fn get_normalized_weight(weight: Uint128, total_weight: Uint128) -> Decimal {
    Decimal::from_ratio(weight, total_weight)
}
