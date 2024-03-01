use cosmwasm_std::{Decimal256, StdError, StdResult, Uint128, Uint256, Uint64};
use dexter::asset::{AssetInfo, Decimal256Ext, DecimalAsset};
use dexter::error::ContractError;
use dexter::pool::{FeeStructs, SpotPrice};
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
/// D invariant calculation in non-overflowing integer operations
/// iteratively

/// A * sum(x_i) * n**n + D = A * D * n**n + D**(n+1) / (n**n * prod(x_i))

/// Converging solution:
/// D[j+1] = (A * n**n * sum(x_i) - D[j]**(n+1) / (n**n prod(x_i))) / (A * n**n - 1)///
///
/// ## Params
/// * **amp** is an object of type [`Uint64`].
/// * **pools** is a vector with values of type [`Decimal256`].
/// * **greatest_precision** object of type [`u8`].
pub(crate) fn compute_d(amp: Uint64, pools: &[Decimal256]) -> StdResult<Decimal256> {
    if pools.iter().any(|pool| pool.is_zero()) {
        return Ok(Decimal256::zero());
    }

    // Sum of all the pools liquidity,  Eq - xp: [1242000000, 1542000000, 1456000000] = 4240000000
    let sum_x = pools.iter().fold(Decimal256::zero(), |acc, x| acc + (*x));

    let n_coins = pools.len() as u8;

    // ann = amp * n                Eq - 100 * 3 = 300
    let ann = Decimal256::from_integer(amp.checked_mul(n_coins.into())?.u64() / AMP_PRECISION);
    let n_coins = Uint64::from(n_coins);

    // Initial D = sum_x, which is the sum of all the pools liquidity
    let mut d = sum_x;

    // ann_sum_x = ann * sum_x
    let ann_sum_x = ann * sum_x;

    // while abs(D - Dprev) > 1:
    for _ in 0..ITERATIONS {
        // Start loop: D_P = D_P * D / (_x * N_COINS)
        let d_p = pools
            .iter()
            .try_fold::<_, _, StdResult<_>>(d, |acc, pool| {
                let denominator = pool.atomics().checked_mul(n_coins.into())?;
                let print_calc_ = acc.checked_multiply_ratio(d, Decimal256::new(denominator));
                print_calc_
            })?;

        let d_prev = d;

        d = (ann_sum_x + d_p * Decimal256::from_integer(n_coins.u64())) * d
            / ((ann - Decimal256::one()) * d
                + (Decimal256::from_integer(n_coins.u64()) + Decimal256::one()) * d_p);

        if d >= d_prev {
            if d - d_prev
                <= Decimal256::with_precision(Uint64::from(1u8), Decimal256::DECIMAL_PLACES)?
            {
                return Ok(d);
            }
        } else if d_prev - d
            <= Decimal256::with_precision(Uint64::from(1u8), Decimal256::DECIMAL_PLACES)?
        {
            return Ok(d);
        }
    }

    Ok(d)
}

/// ## Description
/// Computes the new balance of a `to` pool if one makes `from` pool = `new_amount`.
///
/// Done by solving quadratic equation iteratively.
/// `x_1**2 + x_1 * (sum' - (A*n**n - 1) * D / (A * n**n)) = D ** (n + 1) / (n ** (2 * n) * prod' * A)`
/// `x_1**2 + b*x_1 = c`
///
/// `x_1 = (x_1**2 + c) / (2*x_1 + b)`
pub(crate) fn calc_y(
    from_asset: &DecimalAsset,
    to: &AssetInfo,
    new_amount: Decimal256,
    pools: &[DecimalAsset],
    amp: u64,
    output_precision: u8,
) -> StdResult<Uint128> {
    let amp = Uint64::from(amp);

    if from_asset.info.equal(to) {
        return Err(StdError::generic_err(
            ContractError::SameAssets {}.to_string(),
        ));
    }

    let n_coins = Uint64::from(pools.len() as u8);
    let ann = Uint256::from(amp.checked_mul(n_coins)?.u64() / AMP_PRECISION);
    let mut sum = Decimal256::zero();
    let pool_values = pools.iter().map(|asset| asset.amount).collect_vec();

    // d is computed with the largest precision possible i.e Decimal256::DECIMAL_PLACES i.e 18
    let d = compute_d(amp, &pool_values)?.to_uint256_with_precision(Decimal256::DECIMAL_PLACES)?;

    let mut c = d;

    for pool in pools {
        let pool_amount: Decimal256 = if pool.info.eq(&from_asset.info) {
            new_amount
        } else if !pool.info.eq(to) {
            pool.amount
        } else {
            continue;
        };
        c = c
            .checked_multiply_ratio(
                d,
                pool_amount.to_uint256_with_precision(Decimal256::DECIMAL_PLACES)?
                    * Uint256::from(n_coins),
            )
            .map_err(|_| StdError::generic_err("CheckedMultiplyRatioError"))?;
        sum += pool_amount;
    }

    let c = c * d / (ann * Uint256::from(n_coins));
    let sum = sum.to_uint256_with_precision(Decimal256::DECIMAL_PLACES)?;

    let b = sum + d / ann;

    let mut y = d;

    let d = y;

    for _ in 0..ITERATIONS {
        let y_prev = y;
        y = (y * y + c) / (y + y + b - d);

        if y >= y_prev {
            if y - y_prev <= Uint256::from(1u8) {
                // We need to scale the value from the MAX_PRECISION to the precision of the asset
                // We do this by dividing the value by the ratio of the two precisions
                let decimal_difference = Decimal256::DECIMAL_PLACES - output_precision as u32; // this is safe because ask_asset_precision is always <= 18
                let precision_ratio = Uint256::from(10u8).pow(decimal_difference as u32);
                let y = y.checked_div(precision_ratio)?;

                return Ok(y.try_into()?);
            }
        } else if y_prev - y <= Uint256::from(1u8) {
            // We need to scale the value from the MAX_PRECISION to the precision of the asset
            // We do this by dividing the value by the ratio of the two precisions
            let decimal_difference = Decimal256::DECIMAL_PLACES - output_precision as u32; // this is safe because ask_asset_precision is always <= 18
            let precision_ratio = Uint256::from(10u8).pow(decimal_difference as u32);
            let y = y.checked_div(precision_ratio)?;
            return Ok(y.try_into()?);
        }
    }

    // Should definitely converge in 32 iterations.
    Err(StdError::generic_err("y is not converging"))
}

pub(crate) fn calc_spot_price(
    from: &AssetInfo,
    to: &AssetInfo,
    from_asset_scaling_factor: &Decimal256,
    to_asset_scaling_factor: &Decimal256,
    pools: &[DecimalAsset],
    fee: FeeStructs,
    amp: u64,
) -> StdResult<SpotPrice> {
    // first we figure out the amount of the from asset in the pool
    let from_asset = pools.iter().find(|asset| asset.info.eq(from)).unwrap();
    let to_asset = pools.iter().find(|asset| asset.info.eq(to)).unwrap();

    // if from asset amount is zero, then return 0 as the spot price
    // this is becasuse ideally both will be zero if the pool is empty, but we'll just check for from_asset
    if from_asset.amount.is_zero() {
        return Ok(SpotPrice {
            from: from.clone(),
            to: to.clone(),
            price: Decimal256::zero(),
            price_including_fee: Decimal256::zero(),
        });
    }

    // now, since it's really hard to find the price derivative of the stableswap invariant, we'll just use the
    // approximation as the price of a very very small trade. This is the same as the spot price.
    // We will define a very small trade as 0.001% of the pool liquidity
    // For most 6 decimal precision assets, even 1 unit = 1e6, so 0.001% = 1e6 / 1e5 = 1e = 10 units which is still 
    // a decent small trade size for having a good approximation without running into decimal precision issues.
    // Also, since we always use 18 decimal precision in our calculations, we actually have decent precision in our
    // calculations for having a very accurate spot price.
    let from_asset_small_trade_amount = from_asset.amount.checked_mul(Decimal256::from_ratio(
        Uint256::from(1u128),
        Uint256::from(100000u128),
    ))?;
    let fee_on_trade = from_asset_small_trade_amount.checked_mul(Decimal256::from_ratio(
        fee.total_fee_bps,
        Uint256::from(10000u128),
    ))?;

    let from_asset_small_trade_amount_after_fee =
        from_asset_small_trade_amount.checked_sub(fee_on_trade)?;

    let new_from_asset = from_asset
        .amount
        .checked_add(from_asset_small_trade_amount)?;
    let new_from_asset_after_fee_deduction = from_asset
        .amount
        .checked_add(from_asset_small_trade_amount_after_fee)?;

    let y = calc_y(
        from_asset,
        to,
        new_from_asset,
        pools,
        amp,
        Decimal256::DECIMAL_PLACES as u8,
    )?;

    let y_including_fee = calc_y(
        from_asset,
        to,
        new_from_asset_after_fee_deduction,
        pools,
        amp,
        Decimal256::DECIMAL_PLACES as u8,
    )?;

    let y_price_decimal = Decimal256::with_precision(y, Decimal256::DECIMAL_PLACES)?;
    let y_price_including_fee_decimal =
        Decimal256::with_precision(y_including_fee, Decimal256::DECIMAL_PLACES)?;

    let y_diff = to_asset.amount.checked_sub(y_price_decimal)?;
    let y_diff_including_fee = to_asset.amount.checked_sub(y_price_including_fee_decimal)?;

    let y_diff_unscaled = y_diff.without_scaling_factor(*to_asset_scaling_factor)?;

    let y_diff_including_fee_scaled = y_diff_including_fee.without_scaling_factor(*to_asset_scaling_factor)?;

    let from_asset_small_trade_amount_unscaled =
        from_asset_small_trade_amount.without_scaling_factor(*from_asset_scaling_factor)?;

    let spot_price = y_diff_unscaled
        .checked_div(from_asset_small_trade_amount_unscaled)
        .unwrap();

    let spot_price_with_fee = y_diff_including_fee_scaled
        .checked_div(from_asset_small_trade_amount_unscaled)
        .unwrap();

    let spot_price = SpotPrice {
        from: from.clone(),
        to: to.clone(),
        price: spot_price,
        price_including_fee: spot_price_with_fee,
    };

    Ok(spot_price)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use dexter::{asset::{native_asset, Asset}, uint128_with_precision};
    use sim::StableSwapModel;

    fn decimal_asset_pools_with_precision(pools: Vec<Asset>, precision: u8) -> Vec<DecimalAsset> {
        pools
            .iter()
            .map(|pool| pool.to_decimal_asset(precision).unwrap())
            .collect::<Vec<DecimalAsset>>()
    }

    #[test]
    fn test_spot_price() {

        let amp = 100u64;
        let amp_final = amp * AMP_PRECISION;

        let fee = FeeStructs {
            total_fee_bps: 30,
        };

        let pools = vec![
            native_asset("test1".to_string(), uint128_with_precision!(100_000u128, 6)),
            native_asset("test2".to_string(), uint128_with_precision!(100_000u128, 6)),
            native_asset("test3".to_string(), uint128_with_precision!(100_000u128, 6)),
        ];

        let pools_decimal = decimal_asset_pools_with_precision(pools.clone(), 6);
        let spot_price = calc_spot_price(
            &pools[0].info,
            &pools[1].info, 
            &Decimal256::from_str("1").unwrap(),
            &Decimal256::from_str("1").unwrap(),
            &pools_decimal, 
            fee.clone(), 
            amp_final
        );
        assert_eq!(spot_price.is_ok(), true);
        assert_eq!(spot_price.unwrap().price, Decimal256::from_str("0.999999900990108804").unwrap());

        // test opposite direction
        let spot_price = calc_spot_price(
            &pools[0].info,
            &pools[1].info, 
            &Decimal256::from_str("1").unwrap(),
            &Decimal256::from_str("1").unwrap(),
            &pools_decimal, 
            fee.clone(), 
            amp_final
        );
        assert_eq!(spot_price.is_ok(), true);
        assert_eq!(spot_price.unwrap().price, Decimal256::from_str("0.999999900990108804").unwrap());


        let pools = vec![
            native_asset("test1".to_string(), uint128_with_precision!(1000u128, 6)),
            native_asset("test2".to_string(), uint128_with_precision!(1000u128, 6)),
            native_asset("test3".to_string(), uint128_with_precision!(1000u128, 6)),
        ];

  
        let pools_decimal = decimal_asset_pools_with_precision(pools.clone(), 6);
        let spot_price = calc_spot_price(
            &pools[0].info,
            &pools[1].info, 
            &Decimal256::from_str("1").unwrap(),
            &Decimal256::from_str("1").unwrap(),
            &pools_decimal, 
            fee.clone(), 
            amp_final
        );
        assert_eq!(spot_price.is_ok(), true);
        assert_eq!(spot_price.unwrap().price, Decimal256::from_str("0.9999999009901089").unwrap());
        
        let pools = vec![
            native_asset("test1".to_string(), uint128_with_precision!(1u128, 6)),
            native_asset("test2".to_string(), uint128_with_precision!(1u128, 6)),
            native_asset("test3".to_string(), uint128_with_precision!(1u128, 6)),
        ];

        let pools_decimal = decimal_asset_pools_with_precision(pools.clone(), 6);
        let spot_price = calc_spot_price(
            &pools[0].info,
            &pools[1].info, 
            &Decimal256::from_str("1").unwrap(),
            &Decimal256::from_str("1").unwrap(),
            &pools_decimal, 
            fee.clone(), 
            amp_final
        );
        assert_eq!(spot_price.is_ok(), true);
        assert_eq!(spot_price.unwrap().price, Decimal256::from_str("0.9999999009902").unwrap());
        
        let pools = vec![
            native_asset("test1".to_string(), uint128_with_precision!(1u128, 6)),
            // 1/5th the liquidity of test1 for test2. Let's see the effect of this on the spot price.
            native_asset("test2".to_string(), Uint128::from(200000u128)),
        ];
            
        let pools_decimal = decimal_asset_pools_with_precision(pools.clone(), 6);
        let spot_price = calc_spot_price(
            &pools[0].info,
            &pools[1].info, 
            &Decimal256::from_str("1").unwrap(),
            &Decimal256::from_str("1").unwrap(),
            &pools_decimal, 
            fee.clone(), 
            amp_final
        );
        assert_eq!(spot_price.is_ok(), true);
        assert_eq!(spot_price.unwrap().price, Decimal256::from_str("0.9594664670086").unwrap());

        // test opposite direction
        let spot_price = calc_spot_price(
            &pools[1].info,
            &pools[0].info, 
            &Decimal256::from_str("1").unwrap(),
            &Decimal256::from_str("1").unwrap(),
            &pools_decimal, 
            fee.clone(), 
            amp_final
        );
        assert_eq!(spot_price.is_ok(), true);
        assert_eq!(spot_price.unwrap().price, Decimal256::from_str("1.0422433526755").unwrap());

        let pools = vec![
            native_asset("test1".to_string(), uint128_with_precision!(1u128, 6)),
            // 1/10th the liquidity of test1 for test2. Let's see the effect of this on the spot price.
            native_asset("test2".to_string(), Uint128::from(100000u128)),
        ];
        
        let pools_decimal = decimal_asset_pools_with_precision(pools.clone(), 6);
        let spot_price = calc_spot_price(
            &pools[0].info,
            &pools[1].info, 
            &Decimal256::from_str("1").unwrap(),
            &Decimal256::from_str("1").unwrap(),
            &pools_decimal, 
            fee.clone(), 
            amp_final
        );
        assert_eq!(spot_price.is_ok(), true);
        assert_eq!(spot_price.unwrap().price, Decimal256::from_str("0.8748087775978").unwrap());

        // test opposite direction
        let spot_price = calc_spot_price(
            &pools[1].info,
            &pools[0].info, 
            &Decimal256::from_str("1").unwrap(),
            &Decimal256::from_str("1").unwrap(),
            &pools_decimal, 
            fee.clone(), 
            amp_final
        );
        assert_eq!(spot_price.is_ok(), true);
        let spot_price = spot_price.unwrap();
        
        assert_eq!(spot_price.clone().price, Decimal256::from_str("1.143093026548").unwrap());
        assert_eq!(spot_price.price_including_fee, Decimal256::from_str("1.139663751743").unwrap());


        // let's try even smaller pools
        let pools = vec![
            native_asset("test1".to_string(), Uint128::from(10u128)),
            native_asset("test2".to_string(), Uint128::from(10u128)),
        ];


        let pools_decimal = decimal_asset_pools_with_precision(pools.clone(), 6);
        let spot_price = calc_spot_price(
            &pools[0].info,
            &pools[1].info, 
            &Decimal256::from_str("1").unwrap(),
            &Decimal256::from_str("1").unwrap(),
            &pools_decimal, 
            fee.clone(), 
            amp_final
        );
        assert_eq!(spot_price.is_ok(), true);
        assert_eq!(spot_price.unwrap().price, Decimal256::from_str("0.99999991").unwrap());


        // let's try even smaller pools
        let pools = vec![
            native_asset("test1".to_string(), uint128_with_precision!(1u64, 12)),
            native_asset("test2".to_string(), uint128_with_precision!(1u64, 12)),
        ];

        let pools_decimal = decimal_asset_pools_with_precision(pools.clone(), 18);

        let spot_price = calc_spot_price(
            &pools[0].info,
            &pools[1].info, 
            &Decimal256::from_str("1").unwrap(),
            &Decimal256::from_str("1").unwrap(),
            &pools_decimal, 
            fee.clone(), 
            amp_final
        );
        assert_eq!(spot_price.is_ok(), true);
        // simulation breaks at this value. 
        // with integer invariant calculation, we must be able to go below this value.
        assert_eq!(spot_price.unwrap().price, Decimal256::from_str("1").unwrap());

    }

    #[test]
    fn test_compute_d() {
        // we multiply amp with AMP_PRECISION to avoid floating point arithmetic errors, so amp will always be >= AMP_PRECISION
        // -----------x-----------x-----------x-----------x-----------
        // -----------x-----------x   Test-1  x-----------x-----------
        // -----------x-----------x-----------x-----------x-----------

        let amp = Uint64::from(100u64);
        let pool1 = Uint128::from(100_000u128);
        let pool2 = Uint128::from(100_000u128);
        let pool3 = Uint128::from(100_000u128);
        let model = StableSwapModel::new(
            amp.u64().into(),
            vec![pool1.u128(), pool2.u128(), pool3.u128()],
            3,
        );

        let sim_d = model.sim_d();
        let d = compute_d(
            amp,
            &vec![
                Decimal256::from_integer(pool1.u128()),
                Decimal256::from_integer(pool2.u128()),
                Decimal256::from_integer(pool3.u128()),
            ],
        )
        .unwrap();
        assert_eq!(Uint256::from(sim_d), d.to_uint256());

        // -----------x-----------x-----------x-----------x-----------
        // -----------x-----------x   Test-2  x-----------x-----------
        // -----------x-----------x-----------x-----------x-----------

        // sum_x: "4240000000"
        // ann (amp * n_coins) : "3"
        // sum_x = d: "4240000000"
        // ann_sum_x = ann * sum_x: "12720000000"
        // -------------x-------------x-------------x-------------
        // Start Loop: while abs(D - Dprev) > 1
        // -- Start For Loop: D_P = D_P * D / (_x * N_COINS)
        // d_p: 4240000000
        // pool_liq: 1242000000 n_coins: 3
        // denominator (pool_liq * n_coins) : "3726000000000000000000000000"
        // new d_p: Ok(Decimal256(Uint256(4824906065485775626800000000)))
        // -----
        // d_p: 4824906065.4857756268
        // pool_liq: 1542000000 n_coins: 3
        // denominator (pool_liq * n_coins) : "4626000000000000000000000000"
        // new d_p: Ok(Decimal256(Uint256(4422309061318566502912266567)))
        // -----
        // d_p: 4422309061.318566502912266567
        // pool_liq: 1456000000 n_coins: 3
        // denominator (pool_liq * n_coins) : "4368000000000000000000000000"
        // new d_p: Ok(Decimal256(Uint256(4292717586994212901464610765)))
        // ------
        // new d_p (calculated via D_P = D_P * D / (_x * N_COINS) ): 4292717586.994212901464610765
        // d_prev: 4240000000
        // d = (Ann * S + D_P * self.n) * D // ((Ann - 1) * D + (self.n + 1) * D_P) : 4231285965.512156881371063356
        // -- Start For Loop: D_P = D_P * D / (_x * N_COINS)
        // -----
        // d_p: 4231285965.512156881371063356
        // pool_liq: 1242000000 n_coins: 3
        // denominator (pool_liq * n_coins) : "3726000000000000000000000000"
        // new d_p: Ok(Decimal256(Uint256(4805094181948509301844637442)))
        // -----
        // d_p: 4805094181.948509301844637442
        // pool_liq: 1542000000 n_coins: 3
        // denominator (pool_liq * n_coins) : "4626000000000000000000000000"
        // new d_p: Ok(Decimal256(Uint256(4395098913757640684595323001)))
        // -----
        // d_p: 4395098913.757640684595323001
        // pool_liq: 1456000000 n_coins: 3
        // denominator (pool_liq * n_coins) : "4368000000000000000000000000"
        // new d_p: Ok(Decimal256(Uint256(4257536710352662679544225681)))
        // ------
        // new d_p (calculated via D_P = D_P * D / (_x * N_COINS) ): 4257536710.352662679544225681
        // d_prev: 4231285965.512156881371063356
        // d = (Ann * S + D_P * self.n) * D // ((Ann - 1) * D + (self.n + 1) * D_P) : 4231267933.197213235689277215
        // -- Start For Loop: D_P = D_P * D / (_x * N_COINS)
        // -----
        // d_p: 4231267933.197213235689277215
        // pool_liq: 1242000000 n_coins: 3
        // denominator (pool_liq * n_coins) : "3726000000000000000000000000"
        // new d_p: Ok(Decimal256(Uint256(4805053226651373206008817831)))
        // -----
        // d_p: 4805053226.651373206008817831
        // pool_liq: 1542000000 n_coins: 3
        // denominator (pool_liq * n_coins) : "4626000000000000000000000000"
        // new d_p: Ok(Decimal256(Uint256(4395042722705524534375018302)))
        // -----
        // d_p: 4395042722.705524534375018302
        // pool_liq: 1456000000 n_coins: 3
        // denominator (pool_liq * n_coins) : "4368000000000000000000000000"
        // new d_p: Ok(Decimal256(Uint256(4257464134069518670739830360)))
        // ------
        // new d_p (calculated via D_P = D_P * D / (_x * N_COINS) ): 4257464134.06951867073983036
        // d_prev: 4231267933.197213235689277215
        // d = (Ann * S + D_P * self.n) * D // ((Ann - 1) * D + (self.n + 1) * D_P) : 4231267933.120206881338543707
        // -- Start For Loop: D_P = D_P * D / (_x * N_COINS)
        // -----
        // d_p: 4231267933.120206881338543707
        // pool_liq: 1242000000 n_coins: 3
        // denominator (pool_liq * n_coins) : "3726000000000000000000000000"
        // new d_p: Ok(Decimal256(Uint256(4805053226476475448918363438)))
        // -----
        // d_p: 4805053226.476475448918363438
        // pool_liq: 1542000000 n_coins: 3
        // denominator (pool_liq * n_coins) : "4626000000000000000000000000"
        // new d_p: Ok(Decimal256(Uint256(4395042722465563683718690834)))
        // -----
        // d_p: 4395042722.465563683718690834
        // pool_liq: 1456000000 n_coins: 3
        // denominator (pool_liq * n_coins) : "4368000000000000000000000000"
        // new d_p: Ok(Decimal256(Uint256(4257464133759586236627241436)))
        // ------
        // new d_p (calculated via D_P = D_P * D / (_x * N_COINS) ): 4257464133.759586236627241436
        // d_prev: 4231267933.120206881338543707
        // d = (Ann * S + D_P * self.n) * D // ((Ann - 1) * D + (self.n + 1) * D_P) : 4231267933.120206881454012313
        // d: 4231267933.120206881454012313

        let amp = Uint64::from(100u64);
        let pool1 = Uint128::from(1242_000000u128);
        let pool2 = Uint128::from(1542_000000u128);
        let pool3 = Uint128::from(1456_000000u128);
        let d = compute_d(
            amp,
            &vec![
                Decimal256::from_integer(pool1.u128()),
                Decimal256::from_integer(pool2.u128()),
                Decimal256::from_integer(pool3.u128()),
            ],
        )
        .unwrap();

        assert_eq!(Uint256::from(4231267933u128), d.to_uint256());
    }

    #[test]
    fn test_compute_y() {
        pub const NATIVE_TOKEN_PRECISION: u8 = 6;
        let amp = 100u64;

        // -----------x-----------x-----------x-----------x-----------
        // -----------x-----------x   Test-1  x-----------x-----------
        // -----------x-----------x-----------x-----------x-----------

        let pool1 = Uint128::from(100_000_000000u128);
        let pool2 = Uint128::from(100_000_000000u128);
        let pool3 = Uint128::from(100_000_000000u128);
        let pools = vec![
            native_asset("test1".to_string(), pool1),
            native_asset("test2".to_string(), pool2),
            native_asset("test3".to_string(), pool3),
        ];

        let offer_amount = Uint128::from(100_000000u128);
        let y = calc_y(
            &pools[0].to_decimal_asset(NATIVE_TOKEN_PRECISION).unwrap(),
            &pools[1].info,
            Decimal256::with_precision(pools[0].amount + offer_amount, NATIVE_TOKEN_PRECISION)
                .unwrap(),
            &pools
                .iter()
                .map(|pool| pool.to_decimal_asset(NATIVE_TOKEN_PRECISION).unwrap())
                .collect::<Vec<DecimalAsset>>(),
            amp * AMP_PRECISION,
            NATIVE_TOKEN_PRECISION,
        )
        .unwrap()
        .u128();

        let model = StableSwapModel::new(
            amp as u128,
            vec![pool1.u128(), pool2.u128(), pool3.u128()],
            3,
        );
        let sim_y = model.sim_y(0, 1, pool1.u128() + offer_amount.u128());

        assert_eq!(sim_y, y);

        // -----------x-----------x-----------x-----------x-----------
        // -----------x-----------x   Test-2  x-----------x-----------
        // -----------x-----------x-----------x-----------x-----------

        // pool_values: [Decimal256(Uint256(1546325000000000000000000)), Decimal256(Uint256(1728525000000000000000000)), Decimal256(Uint256(1335325000000000000000000))]
        // compute_d() Function
        // d: 4609919816251
        // c: 4609919816251
        // Start Loop: c = c * d / (pool_amount * n_coins)
        // -- pool_amount: 1546998
        // c: 4579053692433
        // sum: 1546998
        // -- pool_amount: 1335325
        // c: 5269396428191
        // sum: 2882323
        // --------------------------------
        // c: 26990550015555478262591
        // sum: 2882323000000
        // b (sum + d / ann): 2897689399387
        // y: 4609919816251
        // d: 4609919816251
        // Start Loop: y = (y*y + c) / (2*y + b - d)
        // Returned y: 1727851292418
        // let pool1 = Uint128::from(1546325_000000u128);
        // let pool2 = Uint128::from(1728525_000000u128);
        // let pool3 = Uint128::from(1335325_000000u128);
        // let pools = vec![
        //     native_asset("test1".to_string(), pool1),
        //     native_asset("test2".to_string(), pool2),
        //     native_asset("test3".to_string(), pool3),
        // ];

        // The comments above aren't exactly for the tests below. But, better to keep them for easy remembering purposes.

        let offer_amount = Uint128::from(673_000000u128);
        let y = calc_y(
            &pools[0].to_decimal_asset(NATIVE_TOKEN_PRECISION).unwrap(),
            &pools[1].info,
            Decimal256::with_precision(pools[0].amount + offer_amount, NATIVE_TOKEN_PRECISION)
                .unwrap(),
            &pools
                .iter()
                .map(|pool| pool.to_decimal_asset(NATIVE_TOKEN_PRECISION).unwrap())
                .collect::<Vec<DecimalAsset>>(),
            amp * AMP_PRECISION,
            NATIVE_TOKEN_PRECISION,
        )
        .unwrap()
        .u128();

        let model = StableSwapModel::new(
            amp as u128,
            vec![pool1.u128(), pool2.u128(), pool3.u128()],
            3,
        );
        // pool1 --> pool2
        let sim_y = model.sim_y(0, 1, pool1.u128() + offer_amount.u128());
        assert_eq!(sim_y, y);

        // -----------x-----------x-----------x-----------x-----------
        // -----------x-----------x   Test-3  x-----------x-----------
        // -----------x-----------x-----------x-----------x-----------

        // n_coins: 3
        // pool_values: [Decimal256(Uint256(1546325000000000000000000)), Decimal256(Uint256(1728525000000000000000000)), Decimal256(Uint256(1335325000000000000000000))]
        // compute_d() Function
        // d: 4609919816251
        // c: 4609919816251
        // Start Loop: c = c * d / (pool_amount * n_coins)
        // -- pool_amount: 1734269
        // c: 4084595241042
        // sum: 1734269
        // -- pool_amount: 1335325
        // c: 4700392923831
        // sum: 3069594
        // ------------------------
        // c: 24076038315260560176641
        // sum: 3069594000000
        // b (sum + d / ann): 3084960399387
        // y: 4609919816251
        // d: 4609919816251
        // Start Loop: y = (y*y + c) / (2*y + b - d)
        // Returned y: 1540587248612
        let offer_amount = Uint128::from(5744_000000u128);
        let y = calc_y(
            &pools[1].to_decimal_asset(NATIVE_TOKEN_PRECISION).unwrap(),
            &pools[0].info,
            Decimal256::with_precision(pools[1].amount + offer_amount, NATIVE_TOKEN_PRECISION)
                .unwrap(),
            &pools
                .iter()
                .map(|pool| pool.to_decimal_asset(NATIVE_TOKEN_PRECISION).unwrap())
                .collect::<Vec<DecimalAsset>>(),
            amp * AMP_PRECISION,
            NATIVE_TOKEN_PRECISION,
        )
        .unwrap()
        .u128();

        let model = StableSwapModel::new(
            amp as u128,
            vec![pool1.u128(), pool2.u128(), pool3.u128()],
            3,
        );
        // pool2 -->pool1
        let sim_y = model.sim_y(1, 0, pool2.u128() + offer_amount.u128());
        assert_eq!(sim_y, y);
    }
}
