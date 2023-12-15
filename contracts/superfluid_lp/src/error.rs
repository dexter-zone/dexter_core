use cosmwasm_std::{ConversionOverflowError, OverflowError, StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),
    
    #[error("Insuffient balance")]
    InsufficientBalance { denom: String, available_balance: Uint128, required_balance: Uint128 },

    #[error("Unauthorized to perform this action")]
    Unauthorized,

    #[error("Unsupported asset type")]
    UnsupportedAssetType,

    #[error("Invalid amount sent")]
    InvalidAmount,

    #[error("Not implemented")]
    NotImplemented,

    #[error("Only whitelisted assets can be locked")]
    AssetNotAllowedToBeLocked,

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
