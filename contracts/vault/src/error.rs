use cosmwasm_std::{OverflowError, StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("CodeId cannot be 0")]
    InvalidCodeId {},

    #[error("InvalidSubMsgId")]
    InvalidSubMsgId {},

    #[error("Invalid lp token name")]
    InvalidLpTokenName {},

    #[error("Invalid lp token name")]
    InvalidLpTokenSymbol {},

    #[error("LP Token address not found")]
    LpTokenNotFound {},

    #[error("Swap in / out amount cannot be 0")]
    SwapAmountZero {},

    #[error("Number of LP tokens to burn when withdrawing liquidity cannot be 0")]
    BurnAmountZero {},

    #[error("MaxSpendError - offer amount {offer_amount} is more than manimum allowed spent amount {max_spend}")]
    MaxSpendError {
        max_spend: Uint128,
        offer_amount: Uint128,
    },

    #[error("MinReceiveError - return amount {ask_amount} is less than minimum requested amount {min_receive}")]
    MinReceiveError {
        min_receive: Uint128,
        ask_amount: Uint128,
    },

    #[error("Pool Type already exists")]
    PoolTypeAlreadyExists {},

    #[error("Insufficient number of {denom} tokens sent. Tokens sent = {sent}. Tokens needed = {needed}")]
    InsufficientNativeTokensSent {
        denom: String,
        sent: Uint128,
        needed: Uint128,
    },

    #[error("Cannot burn more LP tokens than what's been sent by the users")]
    InsufficientLpTokensToExit {},

    #[error("Invalid number of assets")]
    InvalidNumberOfAssets {},

    #[error("Pool logic not satisfied. Reason : {error}")]
    PoolQueryFailed { error: String },

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

    #[error("Duplicate of Pool Configs")]
    PoolTypeConfigDuplicate {},

    #[error("Invalid FeeInfo params")]
    InvalidFeeInfo {},

    #[error("Config for pool type not found")]
    PoolTypeConfigNotFound {},

    #[error("Pool is currently disabled. No new pool instances can be created")]
    PoolConfigDisabled {},

    #[error("Repeated assets in asset infos")]
    RepeatedAssets {},

    #[error("Address already whitelisted")]
    AddressAlreadyWhitelisted,

    #[error("Address is not whitelisted currently")]
    AddressNotWhitelisted,

    #[error("Instantiation of this pool type is disabled")]
    PoolTypeInstantiationDisabled,

    #[error("Cannot add admin to whitelist. Admin is always whitelisted by default")]
    CannotAddOwnerToWhitelist,

    #[error("Pool creation fee must be null or greater than 0")]
    InvalidPoolCreationFee,
}

impl From<OverflowError> for ContractError {
    fn from(o: OverflowError) -> Self {
        StdError::from(o).into()
    }
}
