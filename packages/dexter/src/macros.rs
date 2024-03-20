#[macro_export]
macro_rules! uint128_with_precision {
    ($value:expr, $precision:expr) => {
        cosmwasm_std::Uint128::from($value)
            .checked_mul(cosmwasm_std::Uint128::from(10u64).pow($precision as u32))
            .unwrap()
    };
}