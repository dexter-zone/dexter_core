use cosmwasm_std::{ConversionOverflowError, OverflowError, StdError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Error: {0}", msg)]
    InvalidMultihopSwapRequest { msg: String },

    #[error("Invalid contract version for upgrade {upgrade_version}. Expected: {expected}, Actual: {actual}")]
    InvalidContractVersionForUpgrade {
        upgrade_version: String,
        expected: String,
        actual: String,
    },

    #[error("Invalid contract name for migration. Expected: {expected}, Actual: {actual}")]
    InvalidContractNameForMigration { expected: String, actual: String },
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
