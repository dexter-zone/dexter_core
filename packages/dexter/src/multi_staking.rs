use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw20::Cw20ReceiveMsg;
use thiserror::Error;

use crate::asset::AssetInfo;

/// Maximum number of LP tokens that are allowed by the multi-staking contract at a point in time.
/// This limit exists to prevent out-of-gas issues during allow and remove LP token operations.
pub const MAX_ALLOWED_LP_TOKENS: usize = 100_000;

/// Maximum number of LP token locks a user is allowed to have at a point in time.
/// This limit exists to prevent out-of-gas issues during LP token unlock.
pub const MAX_USER_LP_TOKEN_LOCKS: usize = 100_000;

/// Max allowed fee for instant LP unbonding i.e. 10%
pub const MAX_INSTANT_UNBOND_FEE_BP: u64 = 1000;


#[cw_serde]
pub struct InstantiateMsg {
    pub owner: Addr,
    pub keeper_addr: Addr,
    pub unbond_config: UnbondConfig,
}

#[cw_serde]
pub enum MigrateMsg {

    /// Removes the reward schedule proposal start delay config param
    /// Instant unbonding fee and keeper address are added
    V3_1FromV1 {
        keeper_addr: Addr,
        instant_unbond_fee_bp: u64,
        instant_unbond_min_fee_bp: u64,
        fee_tier_interval: u64
    },
    // Removes the reward schedule proposal start delay config param.
    // This migration is supported from version v2.0, v2.1 and v2.2
    V3_1FromV2 {
        keeper_addr: Addr
    },
    V3_1FromV2_2 {},
    V3_1FromV3 {},
}

#[cw_serde]
pub struct AssetRewardState {
    pub reward_index: Decimal,
    pub last_distributed: u64,
}

#[cw_serde]
pub struct State {
    pub next_factory_id: u64,
}

#[cw_serde]
pub struct UnclaimedReward {
    pub asset: AssetInfo,
    pub amount: Uint128,
}

#[cw_serde]
pub struct RewardSchedule {
    pub title: String,
    pub creator: Addr,
    pub asset: AssetInfo,
    pub amount: Uint128,
    pub staking_lp_token: Addr,
    pub start_block_time: u64,
    pub end_block_time: u64,
}

#[cw_serde]
pub struct CreatorClaimableRewardState {
    pub claimed: bool,
    pub amount: Uint128,
    pub last_update: u64,
}

impl Default for CreatorClaimableRewardState {
    fn default() -> Self {
        CreatorClaimableRewardState {
            claimed: false,
            amount: Uint128::zero(),
            last_update: 0,
        }
    }
}


#[cw_serde]
pub struct UnlockFeeTier {
    pub seconds_till_unlock_start: u64,
    pub seconds_till_unlock_end: u64,
    pub unlock_fee_bp: u64
}

#[cw_serde]
pub struct Config {
    /// owner has privilege to add/remove allowed lp tokens for reward
    pub owner: Addr,
    /// Keeper address that acts as treasury of the Dexter protocol. All the fees are sent to this address.
    pub keeper: Addr,
    /// LP Token addresses for which reward schedules can be added
    pub allowed_lp_tokens: Vec<Addr>,
    /// Allowed CW20 tokens for rewards. This is to control the abuse from a malicious CW20 token to create
    ///  unnecessary reward schedules
    pub allowed_reward_cw20_tokens: Vec<Addr>,
    /// Default unbond config
    pub unbond_config: UnbondConfig
}

#[cw_serde]
pub enum InstantUnbondConfig {
    Disabled,
    Enabled {
        /// This is the minimum fee charged for instant LP unlock when the unlock time is less than fee interval in future.
        /// Fee in between the unlock duration and fee tier intervals will be linearly interpolated at fee tier interval boundaries.
        min_fee: u64,
        /// Instant LP unbonding fee. This is the percentage of the LP tokens that will be deducted as fee
        /// value between 0 and 1000 (0% to 10%) are allowed
        max_fee: u64,
        /// This is the interval period in seconds on which we will have fee tier boundaries.
        fee_tier_interval: u64,
    }
}

#[cw_serde]
pub struct UnbondConfig {
    /// Unlocking period in seconds
    /// This is the minimum time that must pass before a user can withdraw their staked tokens and rewards
    /// after they have called the unbond function
    pub unlock_period: u64,
    /// Status of instant unbonding
    pub instant_unbond_config: InstantUnbondConfig
}

#[derive(Error, Debug, PartialEq)]
pub enum UnbondConfigValidationError {

    #[error("Min fee smaller than max fee is not allowed")]
    InvalidMinFee { min_fee: u64, max_fee: u64 },

    #[error("Max fee bigger than {MAX_INSTANT_UNBOND_FEE_BP} is not allowed")]
    InvalidMaxFee { max_fee: u64 },

    #[error("Invalid fee tier interval. Fee tier interval must be a non-zero value lesser than the unlock period")]
    InvalidFeeTierInterval { fee_tier_interval: u64 },
}

impl UnbondConfig {
    // validate the unbond config
    pub fn validate(&self) -> Result<(), UnbondConfigValidationError> {
        match self.instant_unbond_config {
            InstantUnbondConfig::Disabled => Ok(()),
            InstantUnbondConfig::Enabled { min_fee, max_fee, fee_tier_interval } => {
                if min_fee > max_fee {
                    Err(UnbondConfigValidationError::InvalidMinFee { 
                        min_fee,
                        max_fee,
                     })
                } else if max_fee > MAX_INSTANT_UNBOND_FEE_BP {
                    Err(UnbondConfigValidationError::InvalidMaxFee {
                        max_fee
                    })
                } else if fee_tier_interval == 0 || fee_tier_interval > self.unlock_period {
                    Err(UnbondConfigValidationError::InvalidFeeTierInterval {
                        fee_tier_interval,
                    })
                } else {
                    Ok(())
                }
            }
        }
    }
}

/// config structure of contract version v2 and v2.1 . Used for migration.
#[cw_serde]
pub struct ConfigV2_1 {
    pub owner: Addr,
    pub keeper: Option<Addr>,
    pub allowed_lp_tokens: Vec<Addr>,
    pub unlock_period: u64,
    pub minimum_reward_schedule_proposal_start_delay: u64,
    pub instant_unbond_fee_bp: u64,
    pub fee_tier_interval: u64,
    pub instant_unbond_min_fee_bp: u64,
}

/// config structure of contract version v2.2 . Used for migration.
#[cw_serde]
pub struct ConfigV2_2 {
    pub owner: Addr,
    pub keeper: Addr,
    pub allowed_lp_tokens: Vec<Addr>,
    pub unlock_period: u64,
    pub minimum_reward_schedule_proposal_start_delay: u64,
    pub instant_unbond_fee_bp: u64,
    pub fee_tier_interval: u64,
    pub instant_unbond_min_fee_bp: u64,
}

/// config structure of contract version v2.2 . Used for migration.
#[cw_serde]
pub struct ConfigV3 {
    pub owner: Addr,
    pub keeper: Addr,
    pub allowed_lp_tokens: Vec<Addr>,
    pub unlock_period: u64,
    pub instant_unbond_fee_bp: u64,
    pub fee_tier_interval: u64,
    pub instant_unbond_min_fee_bp: u64,
}

/// config structure of contract version v1. Used for migration.
#[cw_serde]
pub struct ConfigV1 {
     pub owner: Addr,
     pub allowed_lp_tokens: Vec<Addr>,
     pub unlock_period: u64,
     pub minimum_reward_schedule_proposal_start_delay: u64,
}

#[derive(Eq)]
#[cw_serde]
pub struct TokenLock {
    pub unlock_time: u64,
    pub amount: Uint128,
}

#[cw_serde]
pub struct TokenLockInfo {
    pub locks: Vec<TokenLock>,
    pub unlocked_amount: Uint128
}

#[cw_serde]
pub struct AssetStakerInfo {
    pub asset: AssetInfo,
    pub reward_index: Decimal,
    pub pending_reward: Uint128,
}

#[cw_serde]
#[derive(Default)]
pub struct LpGlobalState {
    pub total_bond_amount: Uint128,
    pub active_reward_assets: Vec<AssetInfo>
}

#[cw_serde]
pub struct RewardScheduleResponse {
    pub id: u64,
    pub reward_schedule: RewardSchedule,
}

#[cw_serde]
pub struct InstantLpUnlockFee {
    pub time_until_lock_expiry: u64,
    pub unlock_fee_bp: u64,
    pub unlock_fee: Uint128,
    pub unlock_amount: Uint128,
}


#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns current config of the contract
    #[returns(Config)]
    Config {},
    /// Returns current unbond config of a given LP token (or global)
    #[returns(UnbondConfig)]
    UnbondConfig {
        lp_token: Option<Addr>
    },
    /// Returns currently unclaimed rewards for a user for a give LP token
    /// If a future block time is provided, it will return the unclaimed rewards till that block time.
    #[returns(Vec<UnclaimedReward>)]
    UnclaimedRewards {
        lp_token: Addr,
        user: Addr,
        block_time: Option<u64>,
    },
    /// Returns current token locks for a user for a given LP token
    /// If a future block time is provided, it will return the token locks and unlocked value at
    /// that block time based on current unlock period
    #[returns(TokenLockInfo)]
    TokenLocks {
        lp_token: Addr,
        user: Addr,
        block_time: Option<u64>
    },
    /// Returns raw state of the token locks for a user for a given LP token
    /// It might include the token locks which are already unlocked and won't give the current ideal view
    /// of the token locks but the actual one as it is stored in the contract
    #[returns(Vec<TokenLock>)]
    RawTokenLocks {
        lp_token: Addr,
        user: Addr,
    },
    #[returns(Uint128)]
    /// Returns the total staked amount for a given LP token
    BondedLpTokens {
        lp_token: Addr,
        user: Addr,
    },
    /// Returns the current unlocking fee percentage (bp) and actual fee for a given token lock
    #[returns(InstantLpUnlockFee)]
    InstantUnlockFee {
        user: Addr,
        lp_token: Addr,
        token_lock: TokenLock
    },
    #[returns(Vec<UnlockFeeTier>)]
    InstantUnlockFeeTiers {
        lp_token: Addr
    },
    #[returns(Vec<UnlockFeeTier>)]
    DefaultInstantUnlockFeeTiers {},
    /// Returns the LP tokens which are whitelisted for rewards
    #[returns(Vec<Addr>)]
    AllowedLPTokensForReward {},
    /// Returns the current owner of the contract
    #[returns(Addr)]
    Owner {},
    /// Returns the reward schedule for a given LP token and a reward asset
    #[returns(Vec<RewardScheduleResponse>)]
    RewardSchedules { lp_token: Addr, asset: AssetInfo },
    /// Returns the current reward state for a given LP token and a reward asset
    #[returns(AssetRewardState)]
    RewardState { lp_token: Addr, asset: AssetInfo },
    /// Returns the staking information for a given user based on the last 
    /// interaction with the contract
    #[returns(AssetStakerInfo)]
    StakerInfo { lp_token: Addr, asset: AssetInfo, user: Addr },
    /// Returns the reward that the creator of a reward schedule can claim since no token was bonded in a part of the reward period
    #[returns(CreatorClaimableRewardState)]
    CreatorClaimableReward { reward_schedule_id: u64 }
}

#[cw_serde]
pub enum Cw20HookMsg {
    /// This hook message is called from LP token contract when to bond tokens
    /// This is a single message flow vs. two message allowance flow.
    /// If beneficiary user is provided, then the tokens are bonded on behalf of the beneficiary user. 
    /// i.e. beneficiary user is the user who has ownership of the LP tokens being bonded
    /// If beneficiary user is not provided, then the tokens are bonded on behalf of the caller.
    Bond { 
        beneficiary_user: Option<Addr> 
    },
    /// This hook message is sent from a CW20 asset contract to propose a reward schedule for some LP.
    /// The LP Token contract must be in the allowed_lp_tokens list.
    CreateRewardSchedule {
        lp_token: Addr,
        actual_creator: Option<Addr>,
        title: String,
        start_block_time: u64,
        end_block_time: u64,
    },
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Allows an admin to update config params
    UpdateConfig {
        keeper_addr: Option<Addr>,
        unbond_config: Option<UnbondConfig>,
    },
    /// Add custom unbdond config for a given LP token
    SetCustomUnbondConfig {
        lp_token: Addr,
        unbond_config: UnbondConfig,
    },

    /// Unset custom unbdond config for a given LP token
    UnsetCustomUnbondConfig {
        lp_token: Addr,
    },
    /// Creates a new reward schedule for rewarding LP token holders a specific asset.
    /// Asset is distributed linearly over the duration of the reward schedule.
    /// This entry point is strictly meant for proposing reward schedules with native tokens.
    /// For proposing reward schedules with CW20 tokens, CW20 transfer with CreateRewardSchedule
    /// HookMsg is used. Only owner can create a reward schedule proposal.
    CreateRewardSchedule {
        lp_token: Addr,
        title: String,
        /// The user on whose behalf the reward schedule might be being created by the owner
        /// This is particularly useful for the Gov admin scenario where a user proposes a reward schedule on the chain governance
        /// and the gov admin actually creates the reward schedule for the user
        actual_creator: Option<Addr>,
        start_block_time: u64,
        end_block_time: u64,
    },
    /// Allows an admin to allow a new LP token to be rewarded
    /// This is needed to prevent spam related to adding new reward schedules for random LP tokens
    AllowLpToken {
        lp_token: Addr,
    },
    ///. Allows an admin to remove an LP token from being rewarded.
    /// Existing reward schedules for the LP token will still be valid.
    RemoveLpToken {
        lp_token: Addr,
    },
    /// Add reward CW20 token to the list of allowed reward tokens
    AllowRewardCw20Token {
        addr: Addr
    },
    /// Remove reward CW20 token from the list of allowed reward tokens
    RemoveRewardCw20Token {
        addr: Addr,
    },
    /// Allows the contract to receive CW20 tokens.
    /// The contract can receive CW20 tokens from LP tokens for staking and 
    /// CW20 assets to be used as rewards.
    Receive(Cw20ReceiveMsg),
    /// Allows to bond LP tokens to the contract.
    /// Bonded tokens are eligible to receive rewards.
    Bond {
        lp_token: Addr,
        amount: Uint128,
    },
    /// Allows to unbond LP tokens from the contract.
    /// After unbonding, the tokens are still locked for a locking period.
    /// During this period, the tokens are not eligible to receive rewards.
    /// After the locking period, the tokens can be withdrawn.
    Unbond {
        lp_token: Addr,
        amount: Option<Uint128>,
    },
    /// Instantly unbonds LP tokens from the contract.
    /// No locking period is applied. The tokens are withdrawn from the contract and sent to the user.
    /// An Instant Unbonding fee is applicable to the amount being unbonded.
    /// The fee is calculated as a percentage of the amount being unbonded and sent to the protocol treasury.
    InstantUnbond {
        lp_token: Addr,
        amount: Uint128,
    },
    /// Unlocks the tokens which are locked for a locking period.
    /// After unlocking, the tokens are withdrawn from the contract and sent to the user.
    Unlock {
        lp_token: Addr,
    },
    /// Instant unlock is a extension of instant unbonding feature which allows to insantly unbond tokens
    /// which are in a locked state post normal unbonding.
    /// This is useful when a user mistakenly unbonded the tokens instead of instant unbonding or if a black swan event
    /// occurs and the user has the LP tokens in a locked state after unbonding.
    InstantUnlock {
        lp_token: Addr,
        /// Altought it is use possible to index or something similar to calculate this, it would lead to problems with
        /// order of transaction execution, thus it is better to pass the full lock explicitly.
        token_locks: Vec<TokenLock>,
    },
    /// Allows to withdraw unbonded rewards for a specific LP token.
    /// The rewards are sent to the user's address.
    Withdraw {
        lp_token: Addr,
    },
    /// Allows a reward schedule creator to claim back amount that was
    /// not allocated to anyone since no token were bonded.
    /// This can only be claimed after reward schedule expiry
    ClaimUnallocatedReward {
        reward_schedule_id: u64,
    },
    /// Allows the owner to transfer ownership to a new address.
    /// Ownership transfer is done in two steps:
    /// 1. The owner proposes a new owner.
    /// 2. The new owner accepts the ownership.
    /// The proposal expires after a certain period of time within which the new owner must accept the ownership.
    ProposeNewOwner {
        owner: Addr,
        expires_in: u64,
    },
    /// Allows the new owner to accept ownership.
    ClaimOwnership {},
    /// Allows the owner to drop the ownership transfer proposal.
    DropOwnershipProposal {}
}
