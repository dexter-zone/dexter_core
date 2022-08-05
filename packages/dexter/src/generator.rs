use crate::asset::{Asset, AssetInfo};
use crate::vault::PoolType;
use crate::DecimalCheckedOps;
use cosmwasm_std::{Addr, Decimal, StdError, StdResult, Uint128, Uint64};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;


// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x       InstantiateMsg, ExecuteMsg QueryMsg        x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------


/// This structure describes the parameters used for creating a contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// Address that can change contract settings
    pub owner: String,
    /// Address of vault contract
    pub vault: String,
    /// Address of guardian : who has permission to blacklist tokens
    pub guardian: Option<String>,
    /// DEX token contract address
    pub dex_token: Option<String>,
    /// Amount of DEX distributed per block among all pairs
    pub tokens_per_block: Uint128,
    /// Start block for distributing DEX
    pub start_block: Uint64,
    /// The DEX vesting contract that drips DEX rewards
    pub vesting_contract: String,
    /// Number of seconds to wait before a user can withdraw his LP tokens once they are in unbonding phase
    pub unbonding_period: u64,    
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Update the address of the DEX vesting contract
    /// ## Executor
    /// Only the owner can execute it.
    UpdateConfig {
        /// The new vesting contract address
        vesting_contract: Option<String>,
        /// The new generator guardian
        guardian: Option<String>,
        /// The amount of generators
        checkpoint_generator_limit: Option<u32>,
        /// Number of seconds to wait before a user can withdraw his LP tokens after unbonding
        unbonding_period: Option<u64>,
    },
    /// Setup generators with their respective allocation points.
    /// ## Executor
    /// Only the owner  can execute this.
    SetupPools {
        /// The list of pools with allocation point.
        pools: Vec<(String, Uint128)>,
    },
    /// Update the given pool's DEX allocation slice
    /// ## Executor
    /// Only the owner  can execute this.
    UpdatePool {
        /// The address of the LP token contract address whose allocation we change
        lp_token: String,
        /// This flag determines whether the pool gets 3rd party token rewards
        has_asset_rewards: bool,
    },
    /// Update rewards and return it to user.
    ClaimRewards {
        /// the LP token contract address
        lp_tokens: Vec<String>,
    },
    /// Unbond LP tokens from the Generator
    Withdraw {
        /// The address of the LP token to withdraw
        lp_token: String,
        /// The amount to withdraw
        amount: Uint128,
    },
    /// Unbond LP tokens from the Generator without withdrawing outstanding rewards. 
    EmergencyWithdraw {
        /// The address of the LP token to withdraw
        lp_token: String,
    },
    /// Unlock LP tokens from the Generator
    Unlock {
        /// The address of the LP token to withdraw
        lp_token: String,
    },
    /// Allowed reward proxy contracts that can interact with the Generator
    SetAllowedRewardProxies {
        /// The full list of allowed proxy contracts
        proxies: Vec<String>,
    },
    /// Sends orphan proxy rewards (which were left behind after emergency withdrawals) to another address
    SendOrphanProxyReward {
        /// The transfer recipient
        recipient: String,
        /// The address of the LP token contract for which we send orphaned rewards
        lp_token: String,
    },
    /// Receives a message of type [`Cw20ReceiveMsg`]
    Receive(Cw20ReceiveMsg),
    /// Set a new amount of DEX to distribute per block
    /// ## Executor
    /// Only the owner can execute this.
    SetTokensPerBlock {
        /// The new amount of DEX to distro per block
        amount: Uint128,
    },
    /// Creates a request to change contract ownership
    /// ## Executor
    /// Only the current owner can execute this.
    ProposeNewOwner {
        /// The newly proposed owner
        owner: String,
        /// The validity period of the proposal to change the contract owner
        expires_in: u64,
    },
    /// Removes a request to change contract ownership
    /// ## Executor
    /// Only the current owner can execute this
    DropOwnershipProposal {},
    /// Claims contract ownership
    /// ## Executor
    /// Only the newly proposed owner can execute this
    ClaimOwnership {},
    /// Add or remove a proxy contract that can interact with the Generator
    UpdateAllowedProxies {
        /// Allowed proxy contract
        add: Option<Vec<String>>,
        /// Proxy contracts to remove
        remove: Option<Vec<String>>,
    },
    /// Sets a new proxy contract for a specific generator
    /// Sets a proxy for the pool
    /// ## Executor
    /// Only the current owner  can execute this
    MoveToProxy {
        lp_token: String,
        proxy: String,
    },
    /// Sets the allocation point to zero for the specified pool
    DeactivatePool {
        lp_token: String,
    },
}

/// This structure describes custom hooks for the CW20.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// Deposit performs a token deposit on behalf of the message sender.
    Deposit {},
    /// DepositFor performs a token deposit on behalf of another address that's not the message sender.
    DepositFor(Addr),
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Config returns the main contract parameters
    Config {},
    /// Returns the length of the array that contains all the active pool generators
    ActivePoolLength {},
    /// PoolLength returns the length of the array that contains all the instantiated pool generators
    PoolLength {},
    /// Deposit returns the LP token amount deposited in a specific generator
    Deposit { lp_token: String, user: String },
    /// PendingToken returns the amount of rewards that can be claimed by an account that deposited a specific LP token in a generator
    PendingToken { lp_token: String, user: String },
    /// RewardInfo returns reward information for a specified LP token
    RewardInfo { lp_token: String },
    /// OrphanProxyRewards returns orphaned reward information for the specified LP token
    OrphanProxyRewards { lp_token: String },
    /// PoolInfo returns information about a pool associated with the specified LP token alongside
    /// the total pending amount of DEX and proxy rewards claimable by generator stakers (for that LP token)
    PoolInfo { lp_token: String },
    /// SimulateFutureReward returns the amount of DEX that will be distributed until a future block and for a specific generator
    SimulateFutureReward { lp_token: String, future_block: u64 },
}


/// This structure describes a migration message.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct MigrateMsg {
    /// The Factory address
    pub vault: Option<String>,
    /// The blocked list of tokens
    pub blocked_list_tokens: Option<Vec<AssetInfo>>,
    /// The guardian address
    pub guardian: Option<String>,
    /// Whitelist code id
    pub whitelist_code_id: Option<u64>,
    /// The voting escrow contract
    pub voting_escrow: Option<String>,
    /// The limit of generators
    pub generator_limit: Option<u32>,
}

// ----------------x----------------x----------------x----------------x-------------x----------------
// ----------------     Type Definitions : PoolInfo, UserInfo, UnbondingInfo        x----------------
// ----------------x----------------x----------------x----------------x-------------x----------------

/// This structure describes the main information of pool
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfo {
    /// Accumulated amount of reward per share unit. Used for reward calculations
    pub last_reward_block: Uint64,
    pub accumulated_rewards_per_share: Decimal,
    /// the reward proxy contract
    pub reward_proxy: Option<Addr>,
    /// Accumulated reward indexes per reward proxy. Vector of pools (reward_proxy, index).
    pub accumulated_proxy_rewards_per_share: RestrictedVector<Decimal>,
    /// for calculation of new proxy rewards
    pub proxy_reward_balance_before_update: Uint128,
    /// the orphan proxy rewards which are left by emergency withdrawals. Vector of pools (reward_proxy, index).
    pub orphan_proxy_rewards: RestrictedVector<Uint128>,
    /// The pool has assets giving additional rewards
    pub has_asset_rewards: bool,
}

/// This structure stores the outstanding amount of token rewards that a user accrued.
/// Currently the contract works with UserInfoV2 structure, but this structure is kept for
/// compatibility with the old version.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct UserInfo {
    /// The amount of LP tokens staked
    pub amount: Uint128,
    /// The amount of DEX rewards a user already received or is not eligible for; used for proper reward calculation
    pub reward_debt: Uint128,
    /// Proxy reward amount a user already received per reward proxy; used for proper reward calculation
    /// Vector of pools (reward_proxy, reward debited).
    pub reward_debt_proxy: RestrictedVector<Uint128>,
    /// Vector containing unbonding information for each unbonding period.
    pub unbonding_periods: Vec<UnbondingInfo>,
}


/// This structure stores the outstanding amount of token rewards that a user accrued.
/// Currently the contract works with UserInfoV2 structure, but this structure is kept for
/// compatibility with the old version.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct UnbondingInfo {
    /// The amount of LP tokens being unbonded
    pub amount: Uint128,
    /// Timestamp at which the unbonding period will end adn the tokens become claimable by the user
    pub unlock_timstamp: u64,
}


// ----------------x----------------x--------------x--------------
// ----------------     Response Definitions       x--------------
// ----------------x----------------x--------------x--------------


/// This structure holds the response returned when querying the total length of the array that keeps track of instantiated generators
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolLengthResponse {
    pub length: usize,
}

/// This structure holds the response returned when querying the amount of pending rewards that can be withdrawn from a 3rd party
/// rewards contract
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PendingTokenResponse {
    /// The amount of pending DEX
    pub pending: Uint128,
    /// The amount of pending 3rd party reward tokens
    pub pending_on_proxy: Option<Vec<Asset>>,
}

/// This structure holds the response returned when querying the contract for general parameters
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    /// Address that's allowed to change contract parameters
    pub owner: Addr,
    /// the Factory address
    pub vault: Addr,
    /// DEX token contract address
    pub dex_token: Option<Addr>,
    /// Total amount of DEX distributed per block
    pub tokens_per_block: Uint128,
    /// Sum of total allocation points across all active generators
    pub total_alloc_point: Uint128,
    /// Start block for DEX incentives
    pub start_block: Uint64,
    /// List of 3rd party reward proxies allowed to interact with the Generator contract
    pub allowed_reward_proxies: Vec<Addr>,
    /// The DEX vesting contract address
    pub vesting_contract: Addr,
    /// The list of active pools with allocation points
    pub active_pools: Vec<(Addr, Uint128)>,
    /// The guardian address
    pub guardian: Option<Addr>,
    /// The amount of generators
    pub checkpoint_generator_limit: Option<u32>,
}


/// This structure holds the response returned when querying for a pool's information
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfoResponse {
    /// The slice of DEX that this pool's generator gets per block
    pub alloc_point: Uint128,
    /// Amount of DEX tokens being distributed per block to this LP pool
    pub dex_tokens_per_block: Uint128,
    /// The last block when token emissions were snapshotted (distributed)
    pub last_reward_block: u64,
    /// Current block number. Useful for computing APRs off-chain
    pub current_block: u64,
    /// Total amount of DEX rewards already accumulated per LP token staked
    pub global_reward_index: Decimal,
    /// Pending amount of total DEX rewards which are claimable by stakers right now
    pub pending_astro_rewards: Uint128,
    /// The address of the 3rd party reward proxy contract
    pub reward_proxy: Option<Addr>,
    /// Pending amount of total proxy rewards which are claimable by stakers right now
    pub pending_proxy_rewards: Option<Uint128>,
    /// Total amount of 3rd party token rewards already accumulated per LP token staked per proxy
    pub accumulated_proxy_rewards_per_share: Vec<(Addr, Decimal)>,
    /// Reward balance for the dual rewards proxy before updating accrued rewards
    pub proxy_reward_balance_before_update: Uint128,
    /// The amount of orphan proxy rewards which are left behind by emergency withdrawals and not yet transferred out
    pub orphan_proxy_rewards: Vec<(Addr, Uint128)>,
    /// Total amount of lp tokens staked in the pool's generator
    pub lp_supply: Uint128,
}


/// This structure holds the response returned when querying for the token addresses used to reward a specific generator
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfoResponse {
    /// The address of the base reward token
    pub base_reward_token: Option<Addr>,
    /// The address of the 3rd party reward token
    pub proxy_reward_token: Option<Addr>,
}


/// This structure holds the parameters used to return information about a staked in
/// a specific generator.
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct StakerResponse {
    // The staker's address
    pub account: String,
    // The amount that the staker currently has in the generator
    pub amount: Uint128,
}

// ----------------x----------------x--------------x--------------
// ----------------     RestrictedVector       x--------------
// ----------------x----------------x--------------x--------------

/// Vec wrapper for internal use.
/// Some business logic relies on an order of this vector, thus it is forbidden to sort it
/// or remove elements. New values can be added using .update() ONLY.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct RestrictedVector<T>(Vec<(Addr, T)>);

pub trait Increaseable
where
    Self: Sized,
{
    fn increase(self, new: Self) -> StdResult<Self>;
}

impl<T> RestrictedVector<T>
where
    T: Copy + Increaseable,
{
    pub fn new(first_proxy: Addr, first_reward_index: T) -> Self {
        Self(vec![(first_proxy, first_reward_index)])
    }

    pub fn get_last(&self, proxy: &Addr) -> StdResult<T> {
        self.0
            .last()
            .filter(|(p, _)| p.as_str() == proxy.as_str())
            .map(|(_, v)| v)
            .cloned()
            .ok_or_else(|| StdError::generic_err(format!("Proxy {} not found", proxy)))
    }

    pub fn update(&mut self, key: &Addr, value: T) -> StdResult<()> {
        let proxy_ref = self
            .0
            .iter_mut()
            .find(|(proxy_addr, _)| proxy_addr.as_str() == key.as_str());
        match proxy_ref {
            Some((_, index)) => *index = index.increase(value)?,
            _ => self.0.push((key.clone(), value)),
        }

        Ok(())
    }

    pub fn inner_ref(&self) -> &Vec<(Addr, T)> {
        &self.0
    }
}

impl Increaseable for Decimal {
    fn increase(self, new: Decimal) -> StdResult<Decimal> {
        self.checked_add(new).map_err(Into::into)
    }
}

impl Increaseable for Uint128 {
    fn increase(self, new: Uint128) -> StdResult<Uint128> {
        self.checked_add(new).map_err(Into::into)
    }
}

impl<T> From<Vec<(Addr, T)>> for RestrictedVector<T> {
    fn from(v: Vec<(Addr, T)>) -> Self {
        Self(v)
    }
}