use std::convert::TryFrom;

use dexter::U256;

// https://github.com/officialnico/balancerv2cad/blob/main/src/balancerv2cad/WeightedMath.py

fn calculate_invariant(normalized_weights: Vec<u128>, balances: Vec<Uint128>) -> Uint256 {
    # /**********************************************************************************************
    # // invariant               _____                                                             //
    # // wi = weight index i      | |      wi                                                      //
    # // bi = balance index i     | |  bi ^   = i                                                  //
    # // i = invariant                                                                             //
    # **********************************************************************************************/
    let mut invariant = Uint256::one();
    for (wi, bi) in normalized_weights.iter().zip(balances.iter()) {
        invariant = invariant * (wi.pow(bi.as_u64()));
    }
    invariant
}


/// ## Description
/// Calculates the ask amount (the amount of tokens swapped to).
/// ## Params
/// * **curBalTokenIn** is an object of type [`u128`]. This is the amount of offer tokens currently in a pool.
/// * **weightIn** Normailized weight of the offer tokens.
/// * **curBalTokenOut** is an object of type [`u128`]. This is the amount of ask tokens currently in a pool.
/// * **weightOut** Normailized weight of the ask tokens.
/// * **offer_amount** is an object of type [`u128`]. This is the amount of offer tokens that will be swapped.
pub fn calc_ask_amount(
    curBalTokenIn: u128,
    weightIn: u128,
    curBalTokenOut: u128,
    weightOut: u128,
    offer_amount: u128
) -> Option<u128, u128> {
        /**********************************************************************************************
        // outGivenIn                                                                                //
        // aO = amountOut                                                                            //
        // bO = balanceOut                                                                           //
        // bI = balanceIn              /      /            bI             \    (wI / wO) \           //
        // aI = amountIn    aO = bO * |  1 - | --------------------------  | ^            |          //
        // wI = weightIn               \      \       ( bI + aI )         /              /           //
        // wO = weightOut                                                                            //
        **********************************************************************************************/

    let base = curBalTokenIn.checked_div( curBalTokenIn.checked_add( offer_amount )? )?;
    let power = weightIn.checked_div( weightOut )?;
    let sub = Decimal::from(base as u64).checked_powf(power)?.to_u128()?;
    let return_amount = curBalTokenOut.checked_mul( 1 - sub )?;
    let spread_amount = offer_asset.amount.saturating_sub(return_amount);
    let commission = 0 as u128;

    Some(return_amount, spread_amount, commission)
}

/// ## Description
/// Calculates the amount to be swapped (the offer amount).
/// ## Params
/// * **curBalTokenIn** is an object of type [`u128`]. This is the amount of offer tokens currently in a pool.
/// * **weightIn** Normailized weight of the offer tokens.
/// * **curBalTokenOut** is an object of type [`u128`]. This is the amount of ask tokens currently in a pool.
/// * **weightOut** Normailized weight of the ask tokens.
/// * **ask_amount** is an object of type [`u128`]. This is the amount of ask tokens that will be swapped.
pub fn calc_offer_amount(
    curBalTokenIn: u128,
    weightIn: u128,
    curBalTokenOut: u128,
    weightOut: u128,
    ask_amount: u128
) -> Option<u128> {
        /**********************************************************************************************
        // inGivenOut                                                                                //
        // aO = amountOut                                                                            //
        // bO = balanceOut                                                                           //
        // bI = balanceIn              /  /            bO             \    (wO / wI)      \          //
        // aI = amountIn    aI = bI * |  | --------------------------  | ^            - 1  |         //
        // wI = weightIn               \  \       ( bO - aO )         /                   /          //
        // wO = weightOut                                                                            //
        **********************************************************************************************/   

        let base = curBalTokenOut.checked_div( curBalTokenOut.checked_sub( ask_amount )? )?;
        let power = weightOut.checked_div( weightIn )?;
        let sub = Decimal::from(base as u64).checked_powf(power)?.to_u128()?;
        let return_amount = curBalTokenIn.checked_mul( sub - 1  )?;
        let spread_amount = offer_asset.amount.saturating_sub(return_amount);
        let commission = 0 as u128;

        Some(return_amount, spread_amount, commission)
}


def calc_tokens_out_given_exact_lp_burnt(assets:Vec<Asset>, normalized_weights:Vec<u128>,  lp_total_supply: Uint128, lp_burned : Uint128,
    swap_fee: Decimal) -> Option<u128> { 
        let mut tokens_out : Vec<Asset> = vec![];

        for asset in assets.iter() {
            let mut fraction = lp_total_supply.checked_sub(lp_minted),checked_div(lp_total_supply)?;
            fraction   = Uint128::one().checked_sub(lp_total_supply)?;        
            let tokens_out.push(Asset {
                amount: asset.amount.checked_mul(fraction)?,
                info: asset.info.clone()
            });
        }
       Some(tokens_out)
        
    }


def calc_tokens_in_given_exact_lp_minted(assets:Vec<Asset>, normalized_weights:Vec<u128>,
    lp_total_supply: Uint128,lp_minted : Uint128, swap_fee: Decimal) -> Option<Vec<Asset>> { 

        let mut tokens_in : Vec<Asset> = vec![];

        for asset in assets.iter() {
            let fraction = lp_total_supply.checked_add(lp_minted),checked_div(lp_total_supply).checked_sub(Uint128::one())?;            
            let tokens_in.push(Asset {
                amount: asset.amount.checked_mul(fraction)?,
                info: asset.info.clone()
            });
        }
       Some(tokens_in)
    }
