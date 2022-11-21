use std::str::FromStr;

use cosmwasm_std::{Decimal, StdError, StdResult, Uint128};

const DECIMAL_FRACTIONAL: Uint128 = Uint128::new(1_000_000_000_000_000_000u128);

/// Returns mod subtraction and boolean indicating if the result is negative
fn sub_sign(a: Decimal, b: Decimal) -> (Decimal, bool) {
    if a >= b {
        return (a - b, false);
    } else {
        return (b - a, true);
    }
}

// Pow computes base^(exp)
// However since the exponent is not an integer, we must do an approximation algorithm.
// This implementation is inspired from Osmosis - https://github.com/osmosis-labs/osmosis/blob/1e80a2a220911cbd776f68e8fa5655870a5f5d98/osmomath/math.go#L53
pub fn calculate_pow(
    base: Decimal,
    exp: Decimal,
    precision: Option<Decimal>,
) -> StdResult<Decimal> {
    println!("calculate_pow");
    println!("base: {:?}", base);
    println!("exp: {:?}", exp);
    println!("precision: {:?}", precision);

    let precision = precision.unwrap_or_else(|| Decimal::from_str("0.00000001").unwrap());
    if exp.is_zero() || base.is_zero() {
        return Ok(base);
    };

    // we can adjust the algorithm in this setting.
    if base > Decimal::from_str("2").unwrap() {
        return Err(StdError::generic_err(
            "calculate_pow : base must be less than 2",
        ));
    }

    // We will use an approximation algorithm to compute the power.
    // Since computing an integer power is easy, we split up the exponent into
    // an integer component and a fractional component.
    let integer = exp.atomics() / DECIMAL_FRACTIONAL;
    println!("integer: {:?}", integer);

    let fractional = Decimal::from_atomics(exp.atomics() % DECIMAL_FRACTIONAL, 18)
        .map_err(|e| StdError::generic_err(e.to_string()))?;
    println!("fractional: {:?}", fractional);

    let integer_pow = base.checked_pow(integer.u128() as u32)?;
    println!("integer_pow: {:?}", integer_pow);

    if fractional.is_zero() {
        return Ok(integer_pow);
    }

    // Contract: 0 < base <= 2
    // 0 <= exp < 1.
    let fractional_pow = pow_approx(base, Decimal::from_str(&fractional.to_string())?, precision)?;
    println!("integer_pow: {:?}", integer_pow);
    println!("fractional_pow: {:?}", fractional_pow);

    let result = integer_pow * fractional_pow;
    println!("result: {:?}", result);

    Ok(result)
}

// Contract: 0 < base <= 2
// 0 <= exp < 1.
pub fn pow_approx(base: Decimal, exp: Decimal, precision: Decimal) -> StdResult<Decimal> {
    // We compute this via taking the maclaurin series of (1 + x)^a
    // where x = base - 1.
    // The maclaurin series of (1 + x)^a = sum_{k=0}^{infty} binom(a, k) x^k
    // Binom(a, k) takes the natural continuation on the first parameter, namely that
    // Binom(a, k) = N/D, where D = k!, and N = a(a-1)(a-2)...(a-k+1)
    // Next we show that the absolute value of each term is less than the last term.
    // Note that the change in term n's value vs term n + 1 is a multiplicative factor of
    // v_n = x(a - n) / (n+1)
    // So if |v_n| < 1, we know that each term has a lesser impact on the result than the last.
    // For our bounds on |x| < 1, |a| < 1,
    // it suffices to see for what n is |v_n| < 1,
    // in the worst parameterization of x = 1, a = -1.
    // v_n = |(-1 + epsilon - n) / (n+1)|
    // So |v_n| is always less than 1, as n ranges over the integers.
    //
    // Note that term_n of the expansion is 1 * prod_{i=0}^{n-1} v_i
    // The error if we stop the expansion at term_n is:
    // error_n = sum_{k=n+1}^{infty} term_k
    // At this point we further restrict a >= 0, so 0 <= a < 1.
    // Now we take the _INCORRECT_ assumption that if term_n < p, then
    // error_n < p.
    // This assumption is obviously wrong.
    // However our usages of this function don't use the full domain.
    // With a > 0, |x| << 1, and p sufficiently low, perhaps this actually is true.

    // :-_-: If theres a bug, balancer and osmosis are also wrong here :-_-:

    // println!("pow_approx");
    // println!("base: {:?}", base);
    // println!("exp: {:?}", exp);
    // println!("precision: {:?}", precision);

    let base = base.clone();
    let (x, xneg) = sub_sign(base, Decimal::one());
    // println!("x: {:?}", x);
    // println!("xneg: {:?}", xneg);

    let mut term = Decimal::one();
    // println!("term: {:?}", term);

    let mut sum = term;
    // println!("sum: {:?}", sum);

    let mut negative = false;
    // println!("negative: {:?}", negative);

    let a = exp.clone();
    // println!("a: {:?}", a);
    let mut big_k = Decimal::zero();
    // println!("big_k: {:?}", big_k);

    let mut i = 1u128;
    while term >= precision {
        // println!("i: {:?}", i);
        // At each iteration, we need two values, i and i-1.
        // To avoid expensive big.Int allocation, we reuse bigK variable.
        let (c, cneg) = sub_sign(a, big_k);
        // println!("c: {:?}", c);
        // println!("cneg: {:?}", cneg);

        // On this line, bigK == i.
        big_k = Decimal::from_atomics(Uint128::from(i), 0)
            .map_err(|e| StdError::generic_err(e.to_string()))?;
        // println!("big_k: {:?}", big_k);

        term = term
            .checked_mul(c)?
            .checked_mul(x)?
            .checked_div(big_k)
            .map_err(|e| StdError::generic_err(e.to_string()))?;
        // println!("term: {:?}", term);

        if term.is_zero() {
            break;
        }
        if xneg {
            negative = !negative
        }

        if cneg {
            negative = !negative
        }

        if negative {
            sum = sum.checked_sub(term)?;
        } else {
            sum = sum.checked_add(term)?;
        }
        // println!("sum: {:?}", sum);

        i += 1;
    }
    return Ok(sum);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn check_approx_pow() {
        // 1.45^1.5
        let mut res = calculate_pow(
            Decimal::from_str("1.45").unwrap(),
            Decimal::from_str("7.5").unwrap(),
            Some(Decimal::from_str("0.00000001").unwrap()),
        );
        // assert_eq!(
        //     &res.as_ref().unwrap().clone().to_string()[0..10],
        //     "1.74603121"
        // );

        // 1.05^3.5
        // res = calculate_pow(
        //     Decimal::from_str("1.05").unwrap(),
        //     Decimal::from_str("3.5").unwrap(),
        //     Some(Decimal::from_str("0.00000001").unwrap()),
        // );
        // assert_eq!(
        //     &res.as_ref().unwrap().clone().to_string()[0..11],
        //     "1.186212638"
        // );

        // // 0.91^1.85
        // res = calculate_pow(
        //     Decimal::from_str("0.91").unwrap(),
        //     Decimal::from_str("1.85").unwrap(),
        //     Some(Decimal::from_str("0.00000001").unwrap()),
        // );
        // assert_eq!(
        //     &res.as_ref().unwrap().clone().to_string()[0..11],
        //     "0.839898055"
        // );
    }
}
