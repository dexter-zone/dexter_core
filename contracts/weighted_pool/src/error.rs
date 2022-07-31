use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

/// ## Description
/// This enum describes pair contract errors!
#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error(
        "Number of assets and weights provided do not match"
    )]
    NumberOfAssetsAndWeightsMismatch {},
 
    #[error(
        "Invalid number of assets. This pool type supports at least 2 and at most 5 assets within a stable pool"
    )]
    InvalidNumberOfAssets {},
        

    #[error("Operation non supported")]
    NonSupported {},

    #[error("Event of zero transfer")]
    InvalidZeroAmount {},

    #[error("Operation exceeds max spread limit")]
    MaxSpreadAssertion {},

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
