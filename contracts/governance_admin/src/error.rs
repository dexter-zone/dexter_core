use cosmwasm_std::{OverflowError, StdError, Uint128};
use dexter::governance_admin::GovAdminProposalRequestType;
use thiserror::Error;

/// ## Description
/// This enum describes maker contract errors!
#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Bug(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Invalid native asset precision list provided. It should only and exactly contain all native assets of the pool")]
    InvalidNativeAssetPrecisionList,

    #[error("DEX Token contract already set")]
    DexTokenAlreadySet,

    #[error("Staking contract already set")]
    StakingAddrAlreadySet,

    #[error("Invalid reward schedule request status")]
    InvalidRewardScheduleRequestStatus,

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

    #[error("Proposal content mismatch. Expected: {expected} Actual: {actual}")]
    ProposalContentMismatch { expected: String, actual: String },

    #[error("Bootstrapping amount must exactly include all the assets in the pool")]
    BootstrappingAmountMismatchAssets,

    #[error("Bootstrapping amount must be greater than zero")]
    BootstrappingAmountMustBeGreaterThanZero,

    #[error("Invalid reward schedule start block time")]
    InvalidRewardScheduleStartBlockTime,

    #[error("End block time must be after start block time")]
    InvalidRewardScheduleEndBlockTime,

    #[error("Must provide at least one reward schedule")]
    EmptyRewardSchedule,

    #[error("Voting period is null in governance params")]
    VotingPeriodNull,

    #[error("Latest proposal not found which querying the governance module")]
    LatestProposalNotFound,

    #[error("LP Token is expected in the reward schedule creation request but it is None")]
    LpTokenNull,

    #[error("LP Token not allowed for reward schedule creation yet")]
    LpTokenNotAllowed,

    #[error("Cannot decode proposal status from {status} to a valid proposal status enum variant")]
    CannotDecodeProposalStatus { status: i32 },

    #[error("Governance params are null")]
    GovParamsNull,

    #[error("No proposals found for the given query")]
    NoProposalsFound,

    #[error("Proposal id not set for request: {request_type:?}")]
    ProposalIdNotSet {
        request_type: GovAdminProposalRequestType,
    },

    #[error("Proposal id not found in the gov module {proposal_id}")]
    ProposalIdNotFound { proposal_id: u64 },

    #[error("Auto stake implementation is expected to be multi-staking")]
    InvalidAutoStakeImpl,

    #[error("Proposal status must be either REJECTED or FAILED or PASSED to be refundable")]
    InvalidProposalStatusForRefund,

    #[error("Funds already claimed for this request at block height: {refund_block_height}")]
    FundsAlreadyClaimed { refund_block_height: u64 },
}

impl From<OverflowError> for ContractError {
    fn from(o: OverflowError) -> Self {
        StdError::from(o).into()
    }
}
