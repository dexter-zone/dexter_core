use cosmwasm_std::{CheckedMultiplyFractionError, OverflowError, StdError, Uint128};
use dexter::multi_staking::UnbondConfigValidationError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Checked multiply fraction error: {0}")]
    CheckedMultiplyFractionError(CheckedMultiplyFractionError),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Invalid number of assets received. Expected {correct_number}, got {received_number}")]
    InvalidNumberOfAssets {
        correct_number: u8,
        received_number: u8,
    },

    #[error("Invalid asset received. Expected {correct_asset}, got {received_asset}")]
    InvalidAsset {
        correct_asset: String,
        received_asset: String,
    },

    #[error("Less amount received for {asset}. Expected {correct_amount}, got {received_amount}")]
    LessAmountReceived {
        asset: String,
        correct_amount: Uint128,
        received_amount: Uint128,
    },

    #[error("Can't unbond more than bonded. Current bond amount: {current_bond_amount}, Amount to unbond {amount_to_unbond}")]
    CantUnbondMoreThanBonded {
        current_bond_amount: Uint128,
        amount_to_unbond: Uint128,
    },

    #[error("Can't allow any more LP token unbonds, limit reached! First unlock existing unbonds, then initiate new unbond.")]
    CantAllowAnyMoreLpTokenUnbonds,

    #[error("Can't allow any more LP tokens, limit reached!")]
    CantAllowAnyMoreLpTokens,

    #[error("LP Token is already allowed")]
    LpTokenAlreadyAllowed,

    #[error("LP Token is not allowed for staking")]
    LpTokenNotAllowed,

    #[error("Block time cannot be in the past")]
    BlockTimeInPast,

    #[error("Invalid block times. Start block time {start_block_time} is greater than end block time {end_block_time}")]
    InvalidBlockTimes {
        start_block_time: u64,
        end_block_time: u64,
    },

    #[error(
        "Start block time should be later than current block time. Start block time {start_block_time}, Current block time {current_block_time}"
    )]
    InvalidStartBlockTime {
        start_block_time: u64,
        current_block_time: u64,
    },

    #[error("Proposal not found for ID: {proposal_id}")]
    ProposalNotFound { proposal_id: u64 },

    #[error("Duplicate review found for ID: {proposal_id}")]
    DuplicateReview { proposal_id: u64 },

    #[error("Can't query by only proposer! LP token addr must be given")]
    InvalidQuery,

    #[error("Impossible contract state: {error}")]
    ImpossibleContractState { error: String },

    #[error("No reward state found for the asset since the reward is not distributed for it yet")]
    NoRewardState,

    #[error("No reward state found for the asset for the user since the reward is not distributed to the user yet")]
    NoUserRewardState,

    #[error("Invalid amount. Amount cannot be zero")]
    ZeroAmount,

    #[error("Can't perform this operation while reward schedule is active")]
    RewardScheduleIsActive,

    #[error("Unallocated reward for this schedule has already been claimed by the creator")]
    UnallocatedRewardAlreadyClaimed,

    #[error("This reward schedule has no unallocated reward to claim by the creator")]
    NoUnallocatedReward,

    #[error("Token lock doesn't exist")]
    TokenLockNotFound,

    #[error("Invalid contract version for upgrade {upgrade_version}. Expected: {expected}, Actual: {actual}")]
    InvalidContractVersionForUpgrade {
        upgrade_version: String,
        expected: String,
        actual: String,
    },

    #[error("Invalid contract name for upgrade. Expected name: {expected_name} Found: {contract_name}")]
    InvalidContractName { 
        contract_name: String,
        expected_name: String,
    },

    #[error("No locks exist for the user")]
    NoLocks,

    #[error("No valid lock found from supplied input which can be unlocked")]
    NoValidLocks,

    #[error("Instant unbond/unlock is disabled for this LP")]
    InstantUnbondDisabled,

    #[error("Invalid unbond config. Error: {error}")]
    InvalidUnbondConfig { error: UnbondConfigValidationError },

    #[error("CW20 Token is already allowed as a reward asset")]
    Cw20TokenAlreadyAllowed,

    #[error("This CW20 Token is not allowed as a reward asset")]
    Cw20TokenNotAllowed,
}

impl From<OverflowError> for ContractError {
    fn from(o: OverflowError) -> Self {
        StdError::from(o).into()
    }
}

impl From<UnbondConfigValidationError> for ContractError {
    fn from(error: UnbondConfigValidationError) -> Self {
        ContractError::InvalidUnbondConfig { error }
    }
}
