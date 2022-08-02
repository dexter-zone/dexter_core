use dexter::asset::{Asset, DecimalAsset, Decimal256Ext, AssetInfo};
use cosmwasm_std::{StdError,Decimal256, StdResult, Uint128, Uint256, Uint64};
use itertools::Itertools;

/// The maximum number of calculation steps for Newton's method.
const ITERATIONS: u8 = 32;

pub const MAX_AMP: u64 = 1_000_000;
pub const MAX_AMP_CHANGE: u64 = 10;
pub const MIN_AMP_CHANGING_TIME: u64 = 86400;
pub const AMP_PRECISION: u64 = 100;


// ----------------x----------------x----------------
// ----------------x   STABLE-3-Pool Math     x------
// ----------------x----------------x----------------



/// ## Description
/// Computes the stableswap invariant (D).
///
/// * **Equation**
/// A * sum(x_i) * n**n + D = A * D * n**n + D**(n+1) / (n**n * prod(x_i))
///
/// ## Params
/// * **amp** is an object of type [`Uint64`].
/// * **pools** is a vector with values of type [`Decimal256`].
/// * **greatest_precision** object of type [`u8`].
pub(crate) fn compute_d(
    amp: Uint64,
    pools: &[Decimal256],
    greatest_precision: u8,
) -> StdResult<Decimal256> {
    if pools.iter().any(|pool| pool.is_zero()) {
        return Ok(Decimal256::zero());
    }
    let sum_x = pools.iter().fold(Decimal256::zero(), |acc, x| acc + (*x));

    if sum_x.is_zero() {
        Ok(Decimal256::zero())
    } else {
        let n_coins = pools.len() as u8;
        let ann = Decimal256::from_integer(amp.checked_mul(n_coins.into())?.u64() / AMP_PRECISION);
        let n_coins = Uint64::from(n_coins);
        let mut d = sum_x;
        let ann_sum_x = ann * sum_x;
        for _ in 0..ITERATIONS {
            // loop: D_P = D_P * D / (_x * N_COINS)
            let d_p = pools
                .iter()
                .try_fold::<_, _, StdResult<_>>(d, |acc, pool| {
                    let denominator = pool.atomics().checked_mul(n_coins.into())?;
                    acc.checked_multiply_ratio(d, Decimal256::new(denominator))
                })?;
            let d_prev = d;
            d = (ann_sum_x + d_p * Decimal256::from_integer(n_coins.u64())) * d
                / ((ann - Decimal256::one()) * d
                    + (Decimal256::from_integer(n_coins.u64()) + Decimal256::one()) * d_p);
            if d >= d_prev {
                if d - d_prev <= Decimal256::with_precision(Uint64::from(1u8), greatest_precision)?
                {
                    return Ok(d);
                }
            } else if d < d_prev
                && d_prev - d <= Decimal256::with_precision(Uint64::from(1u8), greatest_precision)?
            {
                return Ok(d);
            }
        }

        Ok(d)
    }
}



/// ## Description
/// Computes the new balance of a `to` pool if one makes `from` pool = `new_amount`.
///
/// Done by solving quadratic equation iteratively.
///
/// `x_1**2 + x_1 * (sum' - (A*n**n - 1) * D / (A * n**n)) = D ** (n + 1) / (n ** (2 * n) * prod' * A)`
///
/// `x_1**2 + b*x_1 = c`
///
/// `x_1 = (x_1**2 + c) / (2*x_1 + b)`
pub(crate) fn calc_y(
    from: &AssetInfo,
    to: &AssetInfo,
    new_amount: Decimal256,
    pools: &[DecimalAsset],
    amp_: u64,
    greatest_precision: u8,
) -> StdResult<Uint128> {

    let amp = Uint64::from(amp_);

    if from.equal(to) {
        return Err(StdError::generic_err(
            "The offer asset and ask asset cannot be the same.",
        ));
    }

    let n_coins = Uint64::from(pools.len() as u8);
    let ann = Uint256::from(amp.checked_mul(n_coins)?.u64() / AMP_PRECISION);
    let mut sum = Decimal256::zero();
    let pool_values = pools.iter().map(|asset| asset.amount).collect_vec();
    let d = compute_d(amp, &pool_values, greatest_precision)?
        .to_uint256_with_precision(greatest_precision)?;
    let mut c = d;
    for pool in pools {
        let pool_amount: Decimal256 = if pool.info.eq(from) {
            new_amount
        } else if !pool.info.eq(to) {
            pool.amount
        } else {
            continue;
        };
        sum += pool_amount;
        c = c
            .checked_multiply_ratio(
                d,
                pool_amount.to_uint256_with_precision(greatest_precision)? * Uint256::from(n_coins),
            )
            .map_err(|_| StdError::generic_err("CheckedMultiplyRatioError"))?;
    }
    let c = c * d / (ann * Uint256::from(n_coins));
    let sum = sum.to_uint256_with_precision(greatest_precision)?;
    let b = sum + d / ann;
    let mut y = d;
    let d = y;
    for _ in 0..ITERATIONS {
        let y_prev = y;
        y = (y * y + c) / (y + y + b - d);
        if y >= y_prev {
            if y - y_prev <= Uint256::from(1u8) {
                return Ok(y.try_into()?);
            }
        } else if y < y_prev && y_prev - y <= Uint256::from(1u8) {
            return Ok(y.try_into()?);
        }
    }

    // Should definitely converge in 32 iterations.
    Err(StdError::generic_err("y is not converging"))
}



// #[cfg(test)]
// mod tests {
//     use super::*;
//     use dexter::asset::native_asset;
//     use sim::StableSwapModel;

//     #[test]
//     fn test_compute_d() {
//         let amp = Uint64::from(100u64);
//         let pool1 = Uint128::from(100_000u128);
//         let pool2 = Uint128::from(100_000u128);
//         let pool3 = Uint128::from(100_000u128);
//         let model = StableSwapModel::new(
//             amp.u64().into(),
//             vec![pool1.u128(), pool2.u128(), pool3.u128()],
//             3,
//         );

//         let sim_d = model.sim_d();
//         let d = compute_d(amp, &vec![pool1, pool2, pool3]).unwrap().u128();

//         assert_eq!(sim_d, d);
//     }

//     #[test]
//     fn test_compute_y() {
//         let amp = Uint64::from(100u64);
//         let pool1 = Uint128::from(100_000_000000u128);
//         let pool2 = Uint128::from(100_000_000000u128);
//         let pool3 = Uint128::from(100_000_000000u128);
//         let model = StableSwapModel::new(
//             amp.u64().into(),
//             vec![pool1.u128(), pool2.u128(), pool3.u128()],
//             3,
//         );

//         let pools = vec![
//             native_asset("test1".to_string(), pool1),
//             native_asset("test2".to_string(), pool2),
//             native_asset("test3".to_string(), pool3),
//         ];

//         let offer_amount = Uint128::from(100_000000u128);
//         let sim_y = model.sim_y(0, 1, pool2.u128() + offer_amount.u128());
//         let y = calc_y(
//             &pools[0].info,
//             &pools[1].info,
//             pools[0].amount + offer_amount,
//             &pools,
//             amp * Uint64::from(AMP_PRECISION),
//         )
//         .unwrap()
//         .u128();

//         assert_eq!(sim_y, y);
//     }
// }

