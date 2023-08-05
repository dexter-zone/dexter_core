use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

/// ## Description
/// This enum describes maker contract errors!
#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("DEX Token contract already set")]
    DexTokenAlreadySet {},

    #[error("Staking contract already set")]
    StakingAddrAlreadySet {},

    #[error("Insufficient funds to execute this transaction")]
    InsufficientBalance,

    #[error("Insufficient funds sent for pool creation")]
    InsuffiencentFundsSent
}

impl From<OverflowError> for ContractError {
    fn from(o: OverflowError) -> Self {
        StdError::from(o).into()
    }
}
