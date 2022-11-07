use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Uint128, Uint64};

use cw20::Cw20ReceiveMsg;

use crate::asset::AssetInfo;

/// This structure stores the core parameters for the Generator contract.
#[cw_serde]
pub struct Config {
    /// Address allowed to change contract parameters
    pub owner: Addr,
    /// The Vault address
    pub vault: Addr,
    /// The DEX token address
    pub dex_token: Option<Addr>,
    /// Total amount of DEX TOKEN rewards per block
    pub tokens_per_block: Uint128,
    /// Total allocation points. Must be the sum of all allocation points in all active generators
    pub total_alloc_point: Uint128,
    /// The block number when the DEX TOKEN distribution starts
    pub start_block: Uint64,
    /// The list of allowed proxy reward contracts
    pub allowed_reward_proxies: Vec<Addr>,
    /// The vesting contract from which rewards are distributed
    pub vesting_contract: Option<Addr>,
    /// The list of active pools (LP Token Addresses) with allocation points
    pub active_pools: Vec<(Addr, Uint128)>,
    /// Number of seconds to wait before a user can withdraw his LP tokens once they are in unbonding phase
    pub unbonding_period: u64,
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x       InstantiateMsg, ExecuteMsg QueryMsg        x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// This structure describes the parameters used for creating a contract.
#[cw_serde]
pub struct InstantiateMsg {
    /// Address that can change contract settings
    pub owner: String,
    /// Address of vault contract
    pub vault: String,
    /// DEX token contract address
    pub dex_token: Option<String>,
    /// Amount of DEX distributed per block among all pairs
    pub tokens_per_block: Uint128,
    /// Start block for distributing DEX
    pub start_block: Uint64,
    /// Number of seconds to wait before a user can withdraw his LP tokens once they are in unbonding phase
    pub unbonding_period: u64,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Failitates updating some of the configuration param of the Dexter Generator Contract
    /// ## Executor -  Only the owner can execute it.
    UpdateConfig {
        // The DEX Token address
        dex_token: Option<String>,
        /// The DEX Vesting contract address
        vesting_contract: Option<String>,
        /// Number of seconds to wait before a user can withdraw his LP tokens after unbonding. Doesn't update
        /// period for existing unbonding positions
        unbonding_period: Option<u64>,
    },
    /// Set a new amount of DEX tokens to distribute per block
    /// ## Executor - Only the owner can execute this.
    SetTokensPerBlock {
        /// The new amount of DEX to distro per block
        amount: Uint128,
    },
    /// Setup generators with their respective allocation points.
    /// ## Executor - Only the owner can execute this.
    SetupPools {
        /// The list of pools with allocation point.
        pools: Vec<(String, Uint128)>,
    },
    /// Allowed reward proxy contracts that can interact with the Generator
    /// ## Executor - Only the owner can execute this.
    SetAllowedRewardProxies {
        /// The full list of allowed proxy contracts
        proxies: Vec<String>,
    },
    /// Setup proxies (should be whitelisted) for a generator.
    /// ## Executor - Only the owner can execute this.
    SetupProxyForPool {
        /// The list of pools with allocation point.
        lp_token: String,
        proxy_addr: String,
    },
    /// Sends orphan proxy rewards (which were left behind after emergency withdrawals) to another address
    /// ## Executor - Only the owner can execute this.
    SendOrphanProxyReward {
        /// The transfer recipient
        recipient: String,
        /// The address of the LP token contract for which we send orphaned rewards
        lp_token: String,
    },
    /// Add or remove a proxy contract that can interact with the Generator
    /// ## Executor - Only the owner can execute this.
    UpdateAllowedProxies {
        /// Allowed proxy contract
        add: Option<Vec<String>>,
        /// Proxy contracts to remove
        remove: Option<Vec<String>>,
    },
    /// Sets the allocation point to zero for the specified pool
    /// ## Executor -  Only the current owner  can execute this
    DeactivatePool { lp_token: String },
    /// Update rewards and transfer them to user.
    /// ## Executor - Open for users
    ClaimRewards {
        /// the LP token contract address
        lp_tokens: Vec<String>,
    },
    /// Unstake LP tokens from the Generator. LP tokens need to be unbonded for a period of time before they can be withdrawn.
    /// ## Executor - Open for users
    Unstake {
        /// The address of the LP token to withdraw
        lp_token: String,
        /// The amount to withdraw
        amount: Uint128,
    },
    ///  Unstake LP tokens from the Generator without withdrawing outstanding rewards.  LP tokens need to be unbonded for a period of time before they can be withdrawn.
    /// ## Executor - Open for users
    EmergencyUnstake {
        /// The address of the LP token to withdraw
        lp_token: String,
    },
    /// Unlock and withdraw LP tokens from the Generator
    /// ## Executor - Open for users
    Unlock {
        /// The address of the LP token to withdraw
        lp_token: String,
    },
    /// Receives a message of type [`Cw20ReceiveMsg`]
    Receive(Cw20ReceiveMsg),

    /// Creates a request to change contract ownership
    /// ## Executor -  Only the current owner can execute this.
    ProposeNewOwner {
        /// The newly proposed owner
        owner: String,
        /// The validity period of the proposal to change the contract owner
        expires_in: u64,
    },
    /// Removes a request to change contract ownership
    /// ## Executor -  Only the current owner can execute this
    DropOwnershipProposal {},
    /// Claims contract ownership
    /// ## Executor - Only the newly proposed owner can execute this
    ClaimOwnership {},
}

/// This structure describes custom hooks for the CW20.
#[cw_serde]
pub enum Cw20HookMsg {
    /// Deposit performs a token deposit on behalf of the message sender.
    Deposit {},
    DepositFor {
        /// The address to which the tokens will be deposited
        beneficiary: Addr,
    },
}

#[cw_serde]
pub enum ExecuteOnReply {
    /// Stake LP tokens in the Generator to receive token emissions
    Deposit {
        /// The LP token to stake
        lp_token: Addr,
        /// The account that receives ownership of the staked tokens
        account: Addr,
        /// The amount of tokens to deposit
        amount: Uint128,
    },
    /// Updates reward and returns it to user.
    ClaimRewards {
        /// The list of LP tokens contract
        lp_tokens: Vec<Addr>,
        /// The rewards recipient
        account: Addr,
    },
    /// Unstake LP tokens from the Generator
    Unstake {
        /// The LP tokens to withdraw
        lp_token: Addr,
        /// The account that receives the withdrawn LP tokens
        account: Addr,
        /// The amount of tokens to withdraw
        amount: Uint128,
    },
    /// Sets a new amount of DEX TOKEN to distribute per block between all active generators
    SetTokensPerBlock {
        /// The new amount of DEX TOKEN to distribute per block
        amount: Uint128,
    },
    /// Sets a proxy contract for an existing generator pool
    AddProxy { lp_token: Addr, reward_proxy: Addr },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Config returns the main contract parameters
    #[returns(ConfigResponse)]
    Config {},
    /// Returns the length of the array that contains all the active pool generators
    #[returns(PoolLengthResponse)]
    ActivePoolLength {},
    /// PoolLength returns the length of the array that contains all the instantiated pool generators
    #[returns(PoolLengthResponse)]
    PoolLength {},
    /// Deposit returns the LP token amount deposited in a specific generator
    #[returns(Uint128)]
    Deposit { lp_token: String, user: String },
    /// PendingToken returns the amount of rewards that can be claimed by an account that deposited a specific LP token in a generator
    #[returns(PendingTokenResponse)]
    PendingToken { lp_token: String, user: String },
    /// RewardInfo returns reward information for a specified LP token
    #[returns(RewardInfoResponse)]
    RewardInfo { lp_token: String },
    /// OrphanProxyRewards returns orphaned reward information for the specified LP token
    #[returns((AssetInfo, Uint128))]
    OrphanProxyRewards { lp_token: String },
    /// PoolInfo returns information about a pool associated with the specified LP token alongside
    /// the total pending amount of DEX and proxy rewards claimable by generator stakers (for that LP token)
    #[returns(PoolInfoResponse)]
    PoolInfo { lp_token: String },
    #[returns(UserInfoResponse)]
    UserInfo { lp_token: String, user: String },
    /// SimulateFutureReward returns the amount of DEX that will be distributed until a future block and for a specific generator
    #[returns(Uint128)]
    SimulateFutureReward { lp_token: String, future_block: u64 },
}

/// This structure describes a migration message.
#[cw_serde]
pub struct MigrateMsg {}

// ----------------x----------------x----------------x----------------x-------------x----------------
// ----------------     Type Definitions : PoolInfo, UserInfo, UnbondingInfo        x----------------
// ----------------x----------------x----------------x----------------x-------------x----------------

pub type UserInfoResponse = UserInfo;

/// This structure describes the main information of pool
#[cw_serde]
pub struct PoolInfo {
    /// Accumulated amount of reward per share unit. Used for reward calculations
    pub last_reward_block: Uint64,
    pub accumulated_rewards_per_share: Decimal,
    /// the reward proxy contract
    pub reward_proxy: Option<Addr>,
    /// Accumulated reward indexes per reward proxy. Vector of pools (reward_proxy, index).
    pub accumulated_proxy_rewards_per_share: Decimal,
    /// for calculation of new proxy rewards
    pub proxy_reward_balance_before_update: Uint128,
    /// the orphan proxy rewards which are left by emergency withdrawals. Vector of pools (reward_proxy, index).
    pub orphan_proxy_rewards: Uint128,
}

/// This structure stores the outstanding amount of token rewards that a user accrued.
/// Currently the contract works with UserInfoV2 structure, but this structure is kept for
/// compatibility with the old version.
#[cw_serde]
#[derive(Default)]
pub struct UserInfo {
    /// The amount of LP tokens staked
    pub amount: Uint128,
    /// The amount of DEX rewards a user already received or is not eligible for; used for proper reward calculation
    pub reward_debt: Uint128,
    /// Proxy reward amount a user already received per reward proxy; used for proper reward calculation
    pub reward_debt_proxy: Uint128,
    /// Vector containing unbonding information for each unbonding period.
    pub unbonding_periods: Vec<UnbondingInfo>,
}

/// This structure stores the outstanding amount of token rewards that a user accrued.
/// Currently the contract works with UserInfoV2 structure, but this structure is kept for
/// compatibility with the old version.
#[cw_serde]
pub struct UnbondingInfo {
    /// The amount of LP tokens being unbonded
    pub amount: Uint128,
    /// Timestamp at which the unbonding period will end adn the tokens become claimable by the user
    pub unlock_timestamp: u64,
}

// ----------------x----------------x--------------x--------------
// ----------------     Response Definitions       x--------------
// ----------------x----------------x--------------x--------------

/// This structure holds the response returned when querying the total length of the array that keeps track of instantiated generators
#[cw_serde]
pub struct PoolLengthResponse {
    pub length: usize,
}

/// This structure holds the response returned when querying the amount of pending rewards that can be withdrawn from a 3rd party
/// rewards contract
#[cw_serde]
pub struct PendingTokenResponse {
    /// The amount of pending DEX
    pub pending: Uint128,
    /// a pending token on proxy
    pub pending_on_proxy: Option<Uint128>,
}

/// This structure holds the response returned when querying the contract for general parameters
#[cw_serde]
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
    pub vesting_contract: Option<Addr>,
    /// The list of active pools with allocation points
    pub active_pools: Vec<(Addr, Uint128)>,
    /// Unbonding period
    pub unbonding_period: u64,
}

/// This structure holds the response returned when querying for a pool's information
#[cw_serde]
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
    pub pending_dex_rewards: Uint128,
    /// The address of the 3rd party reward proxy contract
    pub reward_proxy: Option<Addr>,
    /// Pending amount of total proxy rewards which are claimable by stakers right now
    pub pending_proxy_rewards: Option<Uint128>,
    /// Total amount of 3rd party token rewards already accumulated per LP token staked per proxy
    pub accumulated_proxy_rewards_per_share: Decimal,
    /// Reward balance for the dual rewards proxy before updating accrued rewards
    pub proxy_reward_balance_before_update: Uint128,
    /// The amount of orphan proxy rewards which are left behind by emergency withdrawals and not yet transferred out
    pub orphan_proxy_rewards: Uint128,
    /// Total amount of lp tokens staked in the pool's generator
    pub lp_supply: Uint128,
}

/// This structure holds the response returned when querying for the token addresses used to reward a specific generator
#[cw_serde]
pub struct RewardInfoResponse {
    /// The address of the base reward token
    pub base_reward_token: Option<Addr>,
    /// The address of the 3rd party reward token
    pub proxy_reward_token: Option<Addr>,
}
