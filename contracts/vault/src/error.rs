use cosmwasm_std::{OverflowError, StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("LP Token address not found")]
    LpTokenNotFound {},

    #[error("Insufficient number of {denom} tokens sent. Tokens sent = {sent}. Tokens needed = {needed}")]
    InsufficientNativeTokensSent { denom: String, sent: Uint128, needed: Uint128 },

    #[error("Cannot burn more LP tokens than what's been sent by the users")]
    InsufficientLpTokensToExit {},

    #[error("Invalid number of assets")]
    InvalidNumberOfAssets {},

    #[error("Pool's math boundations not satisfied")]
    PoolQueryFailed {},

    #[error("Mismatched assets")]
    MismatchedAssets {},

    #[error("Invalid sequence of assets")]
    InvalidSequenceOfAssets {},

    #[error("Time limit for trade exceeded")]
    DeadlineExpired {},

    #[error("Amount cannot be 0")]
    InvalidAmount {},

    #[error("Cannot swap same tokens")]
    SameTokenError {},

    #[error("Insufficient number of native tokens sent to the Vault")]
    InsufficientTokensSent {},

    #[error("Swap limit exceeded")]
    SwapLimit {},

    #[error("Duplicate of pair configs")]
    PoolConfigDuplicate {},

    #[error("Fee bps in pair config must be smaller than or equal to 10,000")]
    InvalidFeeInfo {},

    #[error("Pool config not found")]
    PoolConfigNotFound {},

    #[error("Pool is currently disabled. No new pool instances can be created")]
    PoolConfigDisabled {},

    #[error("Doubling assets in asset infos")]
    RepeatedAssets {},

}

impl From<OverflowError> for ContractError {
    fn from(o: OverflowError) -> Self {
        StdError::from(o).into()
    }
}
