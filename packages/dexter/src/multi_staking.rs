use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw20::Cw20ReceiveMsg;

use crate::asset::{Asset, AssetInfo};

/// Maximum number of LP tokens that are allowed by the multi-staking contract at a point in time.
/// This limit exists to prevent out-of-gas issues during allow and remove LP token operations.
pub const MAX_ALLOWED_LP_TOKENS: usize = 100_000;

/// Maximum number of LP token locks a user is allowed to have at a point in time.
/// This limit exists to prevent out-of-gas issues during LP token unlock.
pub const MAX_USER_LP_TOKEN_LOCKS: usize = 100_000;


#[cw_serde]
pub struct InstantiateMsg {
    pub owner: Addr,
    pub unlock_period: u64,
    pub minimum_reward_schedule_proposal_start_delay: u64,
    pub keeper_addr: Option<Addr>,
    /// value between 0 and 1000 (0% to 10%) are allowed
    pub instant_unbond_fee_bp: u64,
    pub instant_unbond_min_fee_bp: u64,
}

#[cw_serde]
pub enum MigrateMsg {
    V2 {
        keeper_addr: Option<Addr>,
        instant_unbond_fee_bp: u64,
        instant_unbond_min_fee_bp: u64,
    }
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
/// The proposed reward schedule for a LP token
pub struct ProposedRewardSchedule {
    /// The LP token for which to propose the reward schedule
    pub lp_token: Addr,
    /// The proposer of the reward schedule
    pub proposer: Addr,
    /// The title of the proposal.
    pub title: String,
    /// Any description that the proposer wants to give about the proposal.
    pub description: Option<String>,
    /// The asset proposed as reward.
    /// The asset would go back to the proposer when the proposer drops the proposal.
    pub asset: Asset,
    /// Block time when the reward schedule will become effective.
    /// This must be at least 3 days in future at the time of proposal to give enough time to review.
    /// This also acts as the expiry of the proposal. If time has elapsed after the start_block_time,
    /// then the proposal can't be approved by the admin. After that, it can only be rejected by the
    /// admin, or dropped by the proposer.
    pub start_block_time: u64,
    /// Block time when reward schedule ends.
    pub end_block_time: u64,
    /// True if proposal was rejected, false if proposal hasn't yet been reviewed.
    /// Once rejected, a proposal can't be reviewed again. It can only be dropped by the proposer.
    pub rejected: bool,
}

#[cw_serde]
/// Review of a proposed reward schedule for a LP token
pub struct ReviewProposedRewardSchedule {
    /// ID of the proposal to review
    pub proposal_id: u64,
    /// true if approved, false if rejected
    pub approve: bool,
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
    pub keeper: Option<Addr>,
    /// LP Token addresses for which reward schedules can be added
    pub allowed_lp_tokens: Vec<Addr>,
    /// Unlocking period in seconds
    /// This is the minimum time that must pass before a user can withdraw their staked tokens and rewards
    /// after they have called the unbond function
    pub unlock_period: u64,
    /// Minimum number of seconds after which a proposed reward schedule can start after it is proposed.
    /// This is to give enough time to review the proposal.
    pub minimum_reward_schedule_proposal_start_delay: u64,
    /// Instant LP unbonding fee. This is the percentage of the LP tokens that will be deducted as fee
    /// value between 0 and 1000 (0% to 10%) are allowed
    pub instant_unbond_fee_bp: u64,
    /// This is the minimum fee charged for instant LP unlock when the unlock is ~1 day or less in future.
    /// Fee in between the unlock duration and 1 day will be linearly interpolated at day boundaries.
    pub instant_unbond_min_fee_bp: u64,
}

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
pub struct ProposedRewardSchedulesResponse {
    pub proposal_id: u64,
    pub proposal: ProposedRewardSchedule,
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
    InstantUnlockFeeTiers {},
    /// Returns the LP tokens which are whitelisted for rewards
    #[returns(Vec<Addr>)]
    AllowedLPTokensForReward {},
    /// Returns the current owner of the contract
    #[returns(Addr)]
    Owner {},
    /// Returns the proposed reward schedules matching the given pagination params.
    #[returns(Vec<ProposedRewardSchedulesResponse>)]
    ProposedRewardSchedules { start_after: Option<u64>, limit: Option<u32> },
    /// Returns the proposed reward schedule matching the given proposal_id
    #[returns(ProposedRewardSchedule)]
    ProposedRewardSchedule { proposal_id: u64 },
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
    ProposeRewardSchedule {
        lp_token: Addr,
        title: String,
        description: Option<String>,
        start_block_time: u64,
        end_block_time: u64,
    },
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Allows an admin to update config params
    UpdateConfig {
        minimum_reward_schedule_proposal_start_delay: Option<u64>,
        unlock_period: Option<u64>,
        instant_unbond_fee_bp: Option<u64>,
        instant_unbond_min_fee_bp: Option<u64>,
    },
    /// Proposes a new reward schedule for rewarding LP token holders a specific asset.
    /// Asset is distributed linearly over the duration of the reward schedule.
    /// This entry point is strictly meant for proposing reward schedules with native tokens.
    /// For proposing reward schedules with CW20 tokens, CW20 transfer with ProposeRewardSchedule
    /// HookMsg is used. Anyone can initiate a reward schedule proposal.
    ProposeRewardSchedule {
        lp_token: Addr,
        title: String,
        description: Option<String>,
        start_block_time: u64,
        end_block_time: u64,
    },
    /// Only the multi-staking admin can approve/reject proposed reward schedules.
    ReviewRewardScheduleProposals {
        reviews: Vec<ReviewProposedRewardSchedule>,
    },
    /// Only the proposer can drop the proposal.
    /// A proposal can be dropped either if its not yet been reviewed or has been rejected by admin.
    /// If approved, a proposal can't be dropped.
    DropRewardScheduleProposal {
        proposal_id: u64,
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
    /// A penalty is applied to the amount being unbonded.
    /// The penalty is calculated as a percentage of the amount being unbonded and sent to the contract keeper as a fee.
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
    /// Penalty fee is same as instant unbonding.
    InstantUnlock {
        lp_token: Addr,
        /// Altought it is possible to index or something similar to calculate this, it would lead to problems with
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
