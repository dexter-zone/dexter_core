use std::str::FromStr;

use cosmwasm_std::{Decimal, Uint128, StdResult, StdError};

fn abs_difference_with_sign(a: Decimal, b: Decimal) -> (Decimal, bool) {
	if a >= b {
		return (a-b, false)
	} else {
        return (b-a, true)
	}
}

pub fn pow_approx(base: Decimal, exp: Decimal, precision: Option<Decimal>)-> StdResult<Decimal> {
	let precision = precision.unwrap_or_else(|| Decimal::from_str("0.00000001").unwrap());
	if exp.is_zero() {return Ok(base)};
	let base = base.clone();
	let (x, xneg) = abs_difference_with_sign(base, Decimal::one());
	let mut term = Decimal::one();
	let mut sum = Decimal::one();
	let mut negative = false;

	let a = exp.clone();
	let mut big_k = Decimal::zero();
	// TODO: Document this computation via taylor expansion
	let mut i = 1u128;

	while term >= precision {
		// At each iteration, we need two values, i and i-1.
		// To avoid expensive big.Int allocation, we reuse bigK variable.
		// On this line, bigK == i-1.
		let (c, cneg) = abs_difference_with_sign(a, big_k);

		// On this line, bigK == i.
		big_k = Decimal::from_atomics(Uint128::from(i), 0).map_err(|e|StdError::generic_err(e.to_string()))?;

        // println!("term: {}, c: {}, x: {}, k: {}",&term, &c, &x, &big_k);
		term = (term * c * x) / big_k;

		// a is mutated on absDifferenceWithSign, reset
		
		if term.is_zero() {
            break
		}
		if xneg {
            negative = !negative
		}
        
		if cneg {
            negative = !negative
		}
        
		if negative {
			sum -= term;
		} else {
			sum += term;
		}
        i+=1;
	}
	return Ok(sum)
}

#[cfg(test)]
mod tests {
use super::*;
	#[test]
	fn check_approx_pow() {
		let res = pow_approx(Decimal::from_str("0.8").unwrap(), 
		Decimal::from_str("0.32").unwrap(), 
		Some(Decimal::from_str("0.00000001").unwrap()));
		assert_eq!(&res.unwrap().to_string()[0..10], "0.93108385")
	}
}