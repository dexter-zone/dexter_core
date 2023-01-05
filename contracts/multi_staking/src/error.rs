use cosmwasm_std::{StdError, Uint128, OverflowError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

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

    #[error("Impossible contract state: {error}")]
    ImpossibleContractState {
        error: String,
    },

    #[error("No reward state found for the asset since the reward is not distributed for it yet")]
    NoRewardState,
    
    #[error("No reward state found for the asset for the user since the reward is not distributed to the user yet")]
    NoUserRewardState,

    #[error("Invalid amount. Amount cannot be zero")]
    ZeroAmount,
    
}

impl From<OverflowError> for ContractError {
    fn from(o: OverflowError) -> Self {
        StdError::from(o).into()
    }
}