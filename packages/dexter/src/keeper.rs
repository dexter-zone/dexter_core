use crate::asset::{Asset, AssetInfo};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    Instantiate, Execute Msgs and Queries      x----------------x--
// ----------------x----------------x----------------x----------------x----------------x----------------

/// This struct describes the Msg used to instantiate in this contract.
#[cw_serde]
pub struct InstantiateMsg {
    /// The vault contract address
    pub vault_contract: String,
}

/// This struct describes the functions that can be executed in this contract.
#[cw_serde]
pub enum ExecuteMsg {
    /// Updates general settings
    UpdateConfig {
        /// The DEX token contract address
        dex_token_contract: Option<String>,
        /// The DEX token staking contract address
        staking_contract: Option<String>,
    },
}

/// This struct describes the query functions available in the contract.
#[cw_serde]
pub enum QueryMsg {
    /// Returns information about the Keeper configs that contains in the [`ConfigResponse`]
    Config {},
    /// Returns the balance for each asset in the specified input parameters
    Balances { assets: Vec<AssetInfo> },
}

/// This struct describes a migration message.
/// We currently take no arguments for migrations.
#[cw_serde]
pub struct MigrateMsg {}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    Response Types      x----------------x----------------x--------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// A custom struct that holds contract parameters and is used to retrieve them.
#[cw_serde]
pub struct ConfigResponse {
    /// The DEX token contract address
    pub dex_token_contract: Option<Addr>,
    /// The vault contract address
    pub vault_contract: Addr,
    /// The DEX token staking contract address
    pub staking_contract: Option<Addr>,
}

/// A custom struct used to return multiple asset balances.
#[cw_serde]
pub struct BalancesResponse {
    pub balances: Vec<Asset>,
}
