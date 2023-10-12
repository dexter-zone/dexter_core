use cosmwasm_std::{OverflowError, StdError, Uint128};
use thiserror::Error;

/// ## Description
/// This enum describes maker contract errors!
#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Invalid native asset precision list provided. It should only and exactly contain all native assets of the pool")]
    InvalidNativeAssetPrecisionList,

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("DEX Token contract already set")]
    DexTokenAlreadySet {},

    #[error("Staking contract already set")]
    StakingAddrAlreadySet {},

    #[error("Insufficient funds to execute this transaction")]
    InsufficientBalance,

    #[error("Insufficient funds sent for pool creation for {denom} - Amount Sent: {amount_sent} - Needed Amount: {needed_amount}")]
    InsufficientFundsSent {
        denom: String,
        amount_sent: Uint128,
        needed_amount: Uint128,
    },

    #[error("Insufficient spend limit for token {token_addr} - Current approval: {current_approval} - Needed Approval: {needed_approval_for_spend}")]
    InsufficientSpendLimit {
        token_addr: String,
        current_approval: Uint128,
        needed_approval_for_spend: Uint128,
    },

    #[error("Bootstrapping amount must include all the assets in the pool")]
    BootstrappingAmountMissingAssets {},

    #[error("Bootstrapping amount must be greater than zero")]
    BootstrappingAmountMustBeGreaterThanZero {},

    #[error("Invalid reward schedule start block time")]
    InvalidRewardScheduleStartBlockTime {},

    #[error("End block time must be after start block time")]
    InvalidRewardScheduleEndBlockTime {},
}

impl From<OverflowError> for ContractError {
    fn from(o: OverflowError) -> Self {
        StdError::from(o).into()
    }
}
