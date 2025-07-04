use cosmwasm_std::{OverflowError, StdError, Uint128};
use thiserror::Error;
use dexter::asset::AssetInfo;

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

    #[error("Invalid lp token symbol")]
    InvalidLpTokenSymbol {},

    #[error("Invalid PoolId")]
    InvalidPoolId {},

    #[error("LP Token address not found")]
    LpTokenNotFound {},

    #[error("Swap in / out amount cannot be 0")]
    SwapAmountZero {},

    #[error("Number of LP tokens to burn when withdrawing liquidity cannot be 0")]
    BurnAmountZero {},

    #[error("MaxSpendError - offer amount {offer_amount} is more than maximum allowed spent amount {max_spend}")]
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

    #[error("ReceivedUnexpectedLpTokens - expected: {expected}, received: {received}")]
    ReceivedUnexpectedLpTokens {
        expected: Uint128,
        received: Uint128,
    },

    #[error("PoolExitTransitionLpToBurnMismatch - expected_to_burn: {expected_to_burn}, actual_burn: {actual_burn}")]
    PoolExitTransitionLpToBurnMismatch {
        expected_to_burn: Uint128,
        actual_burn: Uint128,
    },

    #[error("PoolExitTransitionAssetsOutMismatch - expected_assets_out: {expected_assets_out}, actual_assets_out: {actual_assets_out}")]
    PoolExitTransitionAssetsOutMismatch {
        expected_assets_out: String,
        actual_assets_out: String,
    },

    #[error("MinAssetOutError - return amount {return_amount} is less than minimum requested amount {min_receive} for asset {asset_info}")]
    MinAssetOutError {
        return_amount: Uint128,
        min_receive: Uint128,
        asset_info: AssetInfo,
    },

    #[error("MaxLpToBurnError - burn amount {burn_amount} is more than maximum LP to burn {max_lp_to_burn} allowed by the user")]
    MaxLpToBurnError {
        burn_amount: Uint128,
        max_lp_to_burn: Uint128,
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
    AmountCannotBeZero {},

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

    #[error("Creation of this pool type is disabled")]
    PoolTypeCreationDisabled,

    #[error("Cannot add admin to whitelist. Admin is always whitelisted by default")]
    CannotAddOwnerToWhitelist,

    #[error("Pool creation fee must be non-zero if enabled")]
    InvalidPoolCreationFee,

    #[error("Auto staking is disabled for vault")]
    AutoStakeDisabled,

    #[error("Deposits are paused")]
    PausedDeposit,

    #[error("Swaps are paused")]
    PausedSwap,

    #[error("LP Token ID is not configured")]
    LpTokenCodeIdNotSet,

    #[error("Fee collector address is not configured")]
    FeeCollectorNotSet,

    #[error("Invalid native asset precision list provided. It should only and exactly contain all native assets of the pool")]
    InvalidNativeAssetPrecisionList,

    #[error("Non zero precision value upto 18 is supported")]
    UnsupportedPrecision,

    #[error("Imbalanced exit is paused. Normal exit for a pool is always allowed")]
    ImbalancedExitPaused,

    #[error("Invalid contract version for upgrade {upgrade_version}. Expected: {expected}, Actual: {actual}")]
    InvalidContractVersionForUpgrade {
        upgrade_version: String,
        expected: String,
        actual: String,
    },

    #[error("Invalid contract name for migration. Expected: {expected}, Actual: {actual}")]
    InvalidContractNameForMigration { expected: String, actual: String },

    #[error("Pool is already defunct")]
    PoolAlreadyDefunct,

    #[error("Pool is not defunct")]
    PoolNotDefunct,

    #[error("User has already been refunded from this defunct pool")]
    UserAlreadyRefunded,

    #[error("Cannot process a refund directly to the multistaking contract")]
    CannotRefundToMultistakingContract,

    #[error("Pool has active reward schedules and cannot be made defunct")]
    PoolHasActiveRewardSchedules,

    #[error("Cannot defunct pool with future reward schedules")]
    PoolHasFutureRewardSchedules,

    #[error("LP token balance mismatch. Expected: {expected}, Found: {found}")]
    LpTokenBalanceMismatch { expected: Uint128, found: Uint128 },

    #[error("All operations are disabled for defunct pools")]
    DefunctPoolOperationDisabled,

    #[error("No reward schedule validation assets configured")]
    NoRewardScheduleValidationAssetsConfigured,
}

impl From<OverflowError> for ContractError {
    fn from(o: OverflowError) -> Self {
        StdError::from(o).into()
    }
}
