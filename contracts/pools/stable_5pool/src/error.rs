use crate::math::{MAX_AMP, MAX_AMP_CHANGE, MIN_AMP_CHANGING_TIME};
use cosmwasm_std::{
    CheckedMultiplyRatioError, ConversionOverflowError, Decimal, OverflowError, StdError, CheckedFromRatioError,
};
use thiserror::Error;

/// ## Description
/// This enum describes pair contract errors!
#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    CheckedMultiplyRatioError(#[from] CheckedMultiplyRatioError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Repeated assets in asset infos")]
    RepeatedAssets {},

    #[error("LP token already set")]
    LpTokenAlreadySet {},

    #[error(
        "Invalid number of assets. This pool type supports at least 2 and at most 5 assets within a stable pool"
    )]
    InvalidNumberOfAssets {},

    #[error("Prices update for twap failed")]
    PricesUpdateFailed {},

    #[error("Operation non supported")]
    NonSupported {},

    #[error("Event of zero transfer")]
    InvalidZeroAmount {},

    #[error("Operation exceeds max spread limit. Current spread = {spread_amount}")]
    MaxSpreadAssertion { spread_amount: Decimal },

    #[error("Provided spread amount exceeds allowed limit")]
    AllowedSpreadAssertion {},

    #[error("Operation exceeds max splippage tolerance")]
    MaxSlippageAssertion {},

    #[error("Doubling assets in asset infos")]
    DoublingAssets {},

    #[error("Asset mismatch between the requested and stored in contract")]
    AssetMismatch {},

    #[error("Pair type mismatch. Check factory pair configs")]
    PoolTypeMismatch {},

    #[error(
        "Amp coefficient must be greater than 0 and less than or equal to {}",
        MAX_AMP
    )]
    IncorrectAmp {},

    #[error(
        "The difference between the old and new amp value must not exceed {} times",
        MAX_AMP_CHANGE
    )]
    MaxAmpChangeAssertion {},

    #[error(
        "Amp coefficient cannot be changed more often than once per {} seconds",
        MIN_AMP_CHANGING_TIME
    )]
    MinAmpChangingTimeAssertion {},

    #[error("The asset {0} does not belong to the pair")]
    InvalidAsset(String),

    #[error("The greatest token precision must be less than or equal to 18")]
    InvalidGreatestPrecision,

    #[error("This pool doesn't support scaling factors or manager update")]
    ScalingFactorUpdateNotSupported,

    #[error("Unsupport asset")]
    UnsupportedAsset,

    #[error("Invalid scaling factor. Scaling factor should be postive non-zero value")]
    InvalidScalingFactor,

    #[error("Invalid scaling factor asset infos. Scaling factor asset infos should be a subset of the pool asset infos")]
    InvalidScalingFactorAssetInfo,

    #[error("Scaling factor manager must be specified")]
    ScalingFactorManagerNotSpecified,

    #[error("Scaling factor manager shouldn't be specified if scaling factor is not updatable")]
    ScalingFactorManagerSpecified,
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

impl From<CheckedFromRatioError> for ContractError {
    fn from(o: CheckedFromRatioError) -> Self {
        StdError::generic_err(o.to_string()).into()
    }
}
