use crate::asset::{Asset, AssetInfo};
use cosmwasm_std::{Addr};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// This structure stores general parameters for the contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// The vault contract address
    pub vault_contract: String,
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
    },
}

/// This structure describes the query functions available in the contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Returns information about the Keeper configs that contains in the [`ConfigResponse`]
    Config {},
    /// Returns the balance for each asset in the specified input parameters
    Balances { assets: Vec<AssetInfo> },
}

/// A custom struct that holds contract parameters and is used to retrieve them.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    /// The DEX token contract address
    pub dex_token_contract: Option<Addr>,
    /// The vault contract address
    pub vault_contract: Addr,
    /// The DEX token staking contract address
    pub staking_contract: Option<Addr>,
}

/// A custom struct used to return multiple asset balances.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BalancesResponse {
    pub balances: Vec<Asset>,
}


/// This structure describes a migration message.
/// We currently take no arguments for migrations.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
