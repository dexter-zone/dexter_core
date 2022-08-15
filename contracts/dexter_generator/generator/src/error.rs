use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

/// This enum describes generator contract errors!
#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Dex token already set")]
    DexTokenAlreadySet {},

    #[error("Vesting contract already set")]
    VestingContractAlreadySet {},

    #[error("Insufficient balance in contract to process claim")]
    BalanceTooSmall {},

    #[error("Pool with the LP token already exists!")]
    TokenPoolAlreadyExists {},

    #[error("Reward proxy not allowed!")]
    RewardProxyNotAllowed {},

    #[error("Pool doesn't have additional rewards!")]
    PoolDoesNotHaveAdditionalRewards {},

    #[error("Insufficient amount of orphan rewards!")]
    ZeroOrphanRewards {},

    #[error("Contract can't be migrated!")]
    MigrationError {},

    #[error("The pool already has a reward proxy contract!")]
    PoolAlreadyHasRewardProxyContract {},

    #[error("Generator is disabled!")]
    GeneratorIsDisabled {},

    #[error("Duplicate of pool")]
    PoolDuplicate {},

    #[error("Pair is not registered in factory!")]
    PairNotRegistered {},

    #[error("ASTRO or Terra native assets (UST, LUNA etc) cannot be blocked!")]
    AssetCannotBeBlocked {},

    #[error("Maximum generator limit exceeded!")]
    GeneratorsLimitExceeded {},
}

impl From<OverflowError> for ContractError {
    fn from(o: OverflowError) -> Self {
        StdError::from(o).into()
    }
}