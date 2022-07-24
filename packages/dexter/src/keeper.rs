use crate::asset::{Asset, AssetInfo};
use cosmwasm_std::{Addr, Decimal, Uint128, Uint64};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// This structure stores general parameters for the contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// The factory contract address
    pub vault_contract: String,
    /// The percentage of fees that goes to community contract
    pub community_contract: Option<String>,
    /// The percentage of fees that goes to community contract
    pub community_percent: Uint64,
}

/// This structure describes the functions that can be executed in this contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Updates general settings
    UpdateConfig {
        /// The DEX token contract address
        dex_token_contract: Option<String>,
        /// The DEX token staking contract address
        staking_contract: Option<String>,
        /// The percentage of fees that go to community_contract
        community_percent: Option<Uint64>,
    },
    /// Distributes collected fees to Community Contract
    DistributeFees { assets: Vec<AssetInfo> },
}

/// This structure describes the query functions available in the contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Returns information about the Keeper configs that contains in the [`ConfigResponse`]
    Config {},
    /// Returns the balance for each asset in the specified input parameters
    Balances { assets: Vec<AssetInfo> },
    /// Returns the fee collection and distribution info for each asset in the specified input parameters
    Fees { assets: Vec<AssetInfo> },
}

/// A custom struct that holds contract parameters and is used to retrieve them.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    /// The DEX token contract address
    pub dex_token_contract: Option<Addr>,
    /// The factory contract address
    pub vault_contract: Addr,
    /// The DEX token staking contract address
    pub staking_contract: Option<Addr>,
    /// The community contract address
    pub community_contract: Option<Addr>,
    /// The percentage of fees that go to community_contract
    pub community_percent: Uint64,
}

/// A custom struct used to return multiple asset balances.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BalancesResponse {
    pub balances: Vec<Asset>,
}

/// ## Description
/// This structure stores the main paramters for the Keeper contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct FeeResponse {
    /// Total number of tokens collected as fees collected till now
    pub total_collected: Uint128,
    /// Total number of tokens distributed to staking contract
    pub distributed_to_staking: Uint128,
    /// Total number of tokens distributed to community contract
    pub distributed_to_community: Uint128,
    // Total number of tokens which have been recently collected as Fees and are yet to be accounted for
    pub newly_collected: Uint128,
}

/// A custom struct used to return multiple asset balances.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct FeeInfoResponse {
    pub fees: Vec<FeeResponse>,
}

/// This structure describes a migration message.
/// We currently take no arguments for migrations.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
