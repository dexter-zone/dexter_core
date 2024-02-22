use cosmwasm_std::{ConversionOverflowError, OverflowError, StdError, Uint128};
use cw_utils::PaymentError;
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

    #[error("Asset is not currently allowed")]
    AssetNotInAllowedList,

    #[error("Asset is already allowed to be locked")]
    AssetAlreadyAllowedToBeLocked,

    #[error("Payment error: {0}")]
    PaymentError(PaymentError),

    #[error("Duplicate denom")]
    DuplicateDenom,

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
