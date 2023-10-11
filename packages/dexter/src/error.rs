use std::collections::HashSet;

// use crate::math::{MAX_AMP, MAX_AMP_CHANGE, MIN_AMP_CHANGING_TIME};
use cosmwasm_std::{
    CheckedMultiplyRatioError, ConversionOverflowError, Decimal, OverflowError, StdError, Uint128,
};
use thiserror::Error;

use crate::asset::AssetInfo;

/// ## Description
/// This enum describes stableswap pair contract errors!
#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Maximum {max_assets} assets can be supported by this pool type")]
    InvalidNumberOfAssets { max_assets: Uint128 },

    #[error("{0}")]
    CheckedMultiplyRatioError(#[from] CheckedMultiplyRatioError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("LP token already set")]
    LpTokenAlreadySet {},

    #[error("Operation non supported")]
    NonSupported {},

    #[error("Doubling assets in asset infos")]
    DoublingAssets {},

    #[error("Event of zero transfer")]
    InvalidZeroAmount {},

    #[error("Provided spread amount exceeds allowed limit")]
    AllowedSpreadAssertion {},

    #[error("Operation exceeds max slippage limit")]
    MaxSlippageAssertion {},

    #[error("Operation exceeds max spread limit. Current spread = {spread_amount}")]
    MaxSpreadAssertion { spread_amount: Decimal },

    #[error("Native token balance mismatch between the argument and the transferred")]
    AssetMismatch {},

    #[error("You need to provide init params")]
    InitParamsNotFound {},

    #[error("Ask or offer asset is missed")]
    VariableAssetMissed {},

    #[error("Source and target assets are the same")]
    SameAssets {},
}

impl From<OverflowError> for ContractError {
    fn from(o: OverflowError) -> Self {
        StdError::from(o).into()
    }
}

impl From<ConversionOverflowError> for ContractError {
    fn from(o: ConversionOverflowError) -> Self {
        StdError::from(o).into()
    }
}
