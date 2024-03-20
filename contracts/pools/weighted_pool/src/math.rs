use crate::approx_pow::calculate_pow;
use crate::state::WeightedAsset;
use cosmwasm_std::{Decimal, Decimal256, StdError, StdResult, Uint128};
use dexter::{asset::DecimalAsset, helper::{adjust_precision, decimal_to_decimal256}};

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
    // weight_ratio = (weightX/weightY)
    let weight_ratio = token_weight_fixed
        .checked_div(token_weight_unknown)
        .map_err(|e| StdError::generic_err(e.to_string()))?;

    // y = balanceXBefore/balanceXAfter
    let y = token_balance_fixed_before
        .checked_div(token_balance_fixed_after)
        .map_err(|e| StdError::generic_err(e.to_string()))?;

    // amount_y = balanceY * (1 - (y ^ weight_ratio))
    let y_to_weight_ratio = calculate_pow(y, weight_ratio, None)?;
    // Decimal is an unsigned so always return abs value
    let paranthetical = if y_to_weight_ratio <= Decimal::one() {
        Decimal::one().checked_sub(y_to_weight_ratio)?
    } else {
        y_to_weight_ratio.checked_sub(Decimal::one())?
    };

    let amount_y = token_balance_unknown_before.checked_mul(paranthetical)?;
    Ok(amount_y)
}

pub fn calc_spot_price(
    offer_asset_pool: &DecimalAsset,
    ask_asset_pool: &DecimalAsset,
    offer_asset_weight_weight: Decimal,
    ask_asset_weight: Decimal,
) -> StdResult<Decimal256> {
    let offer_asset_weight_decimal_256 = decimal_to_decimal256(offer_asset_weight_weight)?;
    let ask_asset_weight_decimal_256 = decimal_to_decimal256(ask_asset_weight)?;

    let numerator = ask_asset_pool.amount.checked_div(ask_asset_weight_decimal_256)
        .map_err(|e| StdError::generic_err(e.to_string()))?;

    let denominator = offer_asset_pool.amount.checked_div(offer_asset_weight_decimal_256)
        .map_err(|e| StdError::generic_err(e.to_string()))?;

    if denominator.is_zero() {
        return Ok(Decimal256::zero());
    }

    let spot_price = numerator.checked_div(denominator).unwrap();
    Ok(spot_price)
}

/// ## Description - Inspired from Osmosis implementaton here - https://github.com/osmosis-labs/osmosis/blob/main/x/gamm/pool-models/balancer/amm.go#L116
/// Calculates the amount of LP shares to be minted for Single asset joins.
pub fn calc_minted_shares_given_single_asset_in(
    token_amount_in: Uint128,
    in_precision: u32,
    asset_weight_and_balance: &WeightedAsset,
    total_shares: Uint128,
    swap_fee_rate: Decimal,
) -> StdResult<(Uint128, Uint128)> {
    // deduct swapfee on the in asset.
    // We don't charge swap fee on the token amount that we imagine as unswapped (the normalized weight).
    // So, effective_swapfee = swapfee * (1 - normalized_token_weight)
    let fee_ratio = fee_ratio(asset_weight_and_balance.weight, swap_fee_rate);
    let token_amount_in_after_fee = token_amount_in * fee_ratio;
    let fee_charged = token_amount_in.checked_sub(token_amount_in_after_fee)?;

    let in_decimal = Decimal::from_atomics(token_amount_in_after_fee, in_precision).unwrap();
    let balance_decimal =
        Decimal::from_atomics(asset_weight_and_balance.asset.amount, in_precision).unwrap();

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
        Decimal::from_atomics(total_shares, Decimal::DECIMAL_PLACES).unwrap(),
        Decimal::one(),
    )?;
    let pool_amount_out_adj = adjust_precision(
        pool_amount_out.atomics(),
        pool_amount_out.decimal_places() as u8,
        Decimal::DECIMAL_PLACES as u8,
    )?;

    return Ok((pool_amount_out_adj, fee_charged));
}

// feeRatio returns the fee ratio that is defined as follows:
// 1 - ((1 - normalizedTokenWeightOut) * swapFee)
fn fee_ratio(normalized_weight: Decimal, swap_fee: Decimal) -> Decimal {
    return Decimal::one() - ((Decimal::one() - normalized_weight) * swap_fee);
}

/// ## Description
/// Calculates the weight of an asset as % of the total weight share. Returns a decimal.
/// ## Params
/// * **weight** is the weight of the asset.
/// * **total_weight** is the total weight of all assets.
pub fn get_normalized_weight(weight: Uint128, total_weight: Uint128) -> Decimal {
    Decimal::from_ratio(weight, total_weight)
}


#[cfg(test)]
mod test {
    use std::str::FromStr;

    use cosmwasm_std::{Decimal, Decimal256};
    use dexter::{asset::{AssetInfo, DecimalAsset}, uint128_with_precision};

    use super::calc_spot_price;

    #[test]
    fn test_spot_price() {

        let offer_asset_pool = DecimalAsset {
            amount: Decimal256::from_atomics(uint128_with_precision!(1000u128, 6), 6).unwrap(),
            info: AssetInfo::native_token("test1".to_string()),
        };
        let ask_asset_pool = DecimalAsset {
            amount: Decimal256::from_atomics(uint128_with_precision!(1000u128, 6), 6).unwrap(),
            info: AssetInfo::native_token("test2".to_string()),
        };

        let offer_asset_weight = Decimal::from_str("0.5").unwrap();
        let ask_asset_weight = Decimal::from_str("0.5").unwrap();

        let spot_price = calc_spot_price(
            &offer_asset_pool,
            &ask_asset_pool,
            offer_asset_weight,
            ask_asset_weight,
        ).unwrap();

        assert_eq!(spot_price, Decimal256::from_str("1").unwrap());

        // 1 asset liquidity is 50% of the other asset liquidity
        let offer_asset_pool = DecimalAsset {
            amount: Decimal256::from_atomics(uint128_with_precision!(1000u128, 6), 6).unwrap(),
            info: AssetInfo::native_token("test1".to_string()),
        };

        let ask_asset_pool = DecimalAsset {
            amount: Decimal256::from_atomics(uint128_with_precision!(500u128, 6), 6).unwrap(),
            info: AssetInfo::native_token("test2".to_string()),
        };

        let offer_asset_weight = Decimal::from_str("0.5").unwrap();
        let ask_asset_weight = Decimal::from_str("0.5").unwrap();

        let spot_price = calc_spot_price(
            &offer_asset_pool,
            &ask_asset_pool,
            offer_asset_weight,
            ask_asset_weight,
        ).unwrap();

        assert_eq!(spot_price, Decimal256::from_str("0.5").unwrap());

        // same liuquidity but different weights
        let offer_asset_pool = DecimalAsset {
            amount: Decimal256::from_atomics(uint128_with_precision!(1000u128, 6), 6).unwrap(),
            info: AssetInfo::native_token("test1".to_string()),
        };

        let ask_asset_pool = DecimalAsset {
            amount: Decimal256::from_atomics(uint128_with_precision!(1000u128, 6), 6).unwrap(),
            info: AssetInfo::native_token("test2".to_string()),
        };

        let offer_asset_weight = Decimal::from_str("0.1").unwrap();
        let ask_asset_weight = Decimal::from_str("0.9").unwrap();

        let spot_price = calc_spot_price(
            &offer_asset_pool,
            &ask_asset_pool,
            offer_asset_weight,
            ask_asset_weight,
        ).unwrap();

        assert_eq!(spot_price, Decimal256::from_str("0.111111111111111111").unwrap());

        // different liquidity and different weights
        let offer_asset_pool = DecimalAsset {
            amount: Decimal256::from_atomics(uint128_with_precision!(1000u128, 6), 6).unwrap(),
            info: AssetInfo::native_token("test1".to_string()),
        };

        let ask_asset_pool = DecimalAsset {
            amount: Decimal256::from_atomics(uint128_with_precision!(500u128, 6), 6).unwrap(),
            info: AssetInfo::native_token("test2".to_string()),
        };

        let offer_asset_weight = Decimal::from_str("0.1").unwrap();
        let ask_asset_weight = Decimal::from_str("0.9").unwrap();

        let spot_price = calc_spot_price(
            &offer_asset_pool,
            &ask_asset_pool,
            offer_asset_weight,
            ask_asset_weight,
        ).unwrap();

        assert_eq!(spot_price, Decimal256::from_str("0.055555555555555555").unwrap());

        // 0 liquidity
        let offer_asset_pool = DecimalAsset {
            amount: Decimal256::from_atomics(uint128_with_precision!(0u128, 6), 6).unwrap(),
            info: AssetInfo::native_token("test1".to_string()),
        };

        let ask_asset_pool = DecimalAsset {
            amount: Decimal256::from_atomics(uint128_with_precision!(500u128, 6), 6).unwrap(),
            info: AssetInfo::native_token("test2".to_string()),
        };

        let offer_asset_weight = Decimal::from_str("0.1").unwrap();
        let ask_asset_weight = Decimal::from_str("0.9").unwrap();

        let spot_price = calc_spot_price(
            &offer_asset_pool,
            &ask_asset_pool,
            offer_asset_weight,
            ask_asset_weight,
        ).unwrap();

        assert_eq!(spot_price, Decimal256::from_str("0").unwrap());
    }
}