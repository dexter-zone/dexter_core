use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw20::Cw20ReceiveMsg;

use crate::asset::AssetInfo;

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: Addr,
    pub unlock_period: u64,
}

#[cw_serde]
pub struct MigrateMsg {}

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
    pub asset: AssetInfo,
    pub amount: Uint128,
    pub staking_lp_token: Addr,
    pub start_block_time: u64,
    pub end_block_time: u64,
}

#[cw_serde]
pub struct Config {
    /// owner has privilege to add/remove allowed lp tokens for reward
    pub owner: Addr,
    /// LP Token addresses for which reward schedules can be added
    pub allowed_lp_tokens: Vec<Addr>,
    /// Unlocking period in seconds
    /// This is the minimum time that must pass before a user can withdraw their staked tokens and rewards
    /// after they have called the unbond function
    pub unlock_period: u64,
}

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
#[derive(QueryResponses)]
pub enum QueryMsg {
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
    #[returns(Uint128)]
    /// Returns the total staked amount for a given LP token
    BondedLpTokens {
        lp_token: Addr,
        user: Addr,
    },
    /// Returns the LP tokens which are whitelisted for rewards
    #[returns(Vec<Addr>)]
    AllowedLPTokensForReward {},
    /// Returns the current owner of the contract
    #[returns(Addr)]
    Owner {},
    /// Returns the reward schedule for a given LP token and a reward asset
    #[returns(Vec<RewardSchedule>)]
    RewardSchedules { lp_token: Addr, asset: AssetInfo },
    /// Returns the current reward state for a given LP token and a reward asset
    #[returns(AssetRewardState)]
    RewardState { lp_token: Addr, asset: AssetInfo },
    /// Returns the staking information for a given user based on the last 
    /// interaction with the contract
    #[returns(AssetStakerInfo)]
    StakerInfo { lp_token: Addr, asset: AssetInfo, user: Addr },
}

#[cw_serde]
pub enum Cw20HookMsg {
    /// This hook message is called from LP token contract when user wants to bond it.
    /// This is a single message flow vs. two message allowance flow. 
    Bond {},
    /// This hook message is called when a different address is bonding on user's behalf.
    BondForBeneficiary { beneficiary: Addr },
    /// This hook message is sent from a CW20 asset contract to create a reward schedule for some LP.
    /// The LP Token contract must be in the allowed_lp_tokens list.
    AddRewardSchedule {
        lp_token: Addr,
        start_block_time: u64,
        end_block_time: u64,
    },
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Adds a new reward schedule for rewarding LP token holders a specific asset.
    /// Asset is distributed linearly over the duration of the reward schedule.
    /// This entry point is strictly meant for creating reward schedules with native tokens.
    /// For creating reward schedules with CW20 tokens, CW20 transfer with AddRewardSchedule HookMsg is used.
    AddRewardSchedule {
        lp_token: Addr,
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
    /// Allows the contract to receive CW20 tokens.
    /// The contract can receive CW20 tokens from LP tokens for staking and 
    /// CW20 assets to be used as rewards.
    Receive(Cw20ReceiveMsg),
    /// Allows to bond LP tokens to the contract.
    /// Bonded tokens are elligible to receive rewards.
    Bond {
        lp_token: Addr,
        amount: Uint128,
    },
    /// Allows to unbond LP tokens from the contract.
    /// After unbonding, the tokens are still locked for a locking period.
    /// During this period, the tokens are still elligible to receive rewards.\
    /// After the locking period, the tokens can be withdrawn.
    Unbond {
        lp_token: Addr,
        amount: Uint128,
    },
    /// Unlocks the tokens which are locked for a locking period.
    /// After unlocking, the tokens are withdrawn from the contract and sent to the user.
    Unlock {
        lp_token: Addr,
    },
    /// Allows to withdraw unbonded rewards for a specific LP token.
    /// The rewards are sent to the user's address.
    Withdraw {
        lp_token: Addr,
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
