use cosmwasm_std::{Decimal, OverflowError, StdError};
use thiserror::Error;

/// ## Description
/// This enum describes pair contract errors!
#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid exit fee. Exit fee cannot be more than 1%")]
    InvalidExitFee {},

    #[error("Weight cannot be 0")]
    ZeroWeight {},

    #[error("LP token already set")]
    LpTokenAlreadySet {},

    #[error("Number of assets and weights provided do not match")]
    NumberOfAssetsAndWeightsMismatch {},

    #[error("{asset} weight list and asset list mismatch")]
    WeightedAssetAndAssetMismatch { asset: String },

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

    #[error("GeneratorAddress is not set in factory. Cannot autostake")]
    AutoStakeError {},
}

impl From<OverflowError> for ContractError {
    fn from(o: OverflowError) -> Self {
        StdError::from(o).into()
    }
}
