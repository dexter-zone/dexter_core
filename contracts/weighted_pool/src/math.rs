use std::{convert::TryFrom, str::FromStr};
use cosmwasm_std::{Addr, DepsMut,Uint256, Storage, StdResult, Decimal, Uint128, Decimal256};

use dexter::{U256, approx_pow::pow_approx, asset::DecimalAsset};

// https://github.com/officialnico/balancerv2cad/blob/main/src/balancerv2cad/WeightedMath.py

pub fn calculate_invariant(normalized_weights: Vec<Decimal>, balances: Vec<DecimalAsset>) -> StdResult<Uint256> {
    //  /**********************************************************************************************
    // invariant               _____                                                             //
    // wi = weight index i      | |      wi                                                      //
    // bi = balance index i     | |  bi ^   = i                                                  //
    // i = invariant                                                                             //
    // **********************************************************************************************/
    let mut invariant: Uint256 = Uint256::from(1u128);
    for (wi, bi) in normalized_weights.into_iter().zip(balances.into_iter()) {
        invariant = invariant * Decimal256::from_str(&pow_approx(Decimal::from_str(&bi.amount.to_string())?, wi, None)?.to_string())?;
    }
    Ok(invariant)
}


// Referenced from Balancer Weighted pool implementation by  Osmosis here - https://github.com/osmosis-labs/osmosis/blob/47a2366c5eeee474de9e1cb4777fab0ccfbb9592/x/gamm/pool-models/balancer/amm.go#L94
// solveConstantFunctionInvariant solves the constant function of an AMM
// that determines the relationship between the differences of two sides
// of assets inside the pool.
// For fixed balanceXBefore, balanceXAfter, weightX, balanceY, weightY,
// we could deduce the balanceYDelta, calculated by:
// balanceYDelta = balanceY * (1 - (balanceXBefore/balanceXAfter)^(weightX/weightY))
// balanceYDelta is positive when the balance liquidity decreases.
// balanceYDelta is negative when the balance liquidity increases.
pub fn solve_constant_function_invariant(
	token_balance_fixed_before: Uint128,
	token_balance_fixed_after: Uint128,
	token_weight_fixed: Decimal,
	token_balance_unknown_before: Uint128,
	token_weight_unknown: Decimal,
) -> StdResult<Uint128> {

    // weight_ratio = (weightX/weightY)
	let weight_ratio = token_weight_fixed / token_weight_unknown;

	// y = balanceXBefore/balanceXAfter
	let y = Decimal::from_ratio(token_balance_fixed_before , token_balance_fixed_after);
	// amount_y = balanceY * (1 - (y ^ weight_ratio))
	let y_to_weight_ratio = pow_approx(y,weight_ratio, None)?;
	let paranthetical = Decimal::one() - y_to_weight_ratio;
	let amount_y= token_balance_unknown_before * paranthetical;

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


// def calc_tokens_in_given_exact_lp_minted(assets:Vec<Asset>, normalized_weights:Vec<u128>) {
//     // lp_total_supply: Uint128,lp_minted : Uint128, swap_fee: Decimal) -> Option<Vec<Asset>> { 

//     //     let mut tokens_in : Vec<Asset> = vec![];

//     //     for asset in assets.iter() {
//     //         let fraction = lp_total_supply.checked_add(lp_minted),checked_div(lp_total_supply).checked_sub(Uint128::one())?;            
//     //         let tokens_in.push(Asset {
//     //             amount: asset.amount.checked_mul(fraction)?,
//     //             info: asset.info.clone()
//     //         });
//         // }
//        Some(tokens_in)
//     }



/// ## Description
/// Calculates the weight of an asset as % of the total weight share. Returns a decimal.
/// 
/// ## Params
/// * **weight** is the weight of the asset.
/// * **total_weight** is the total weight of all assets.
pub fn get_normalized_weight(weight: u128, total_weight: u128) -> Decimal {
    Decimal::from_ratio(weight, total_weight)
    }
