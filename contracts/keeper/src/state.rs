use cosmwasm_std::{Addr};
use cw_storage_plus::{Item};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// ## Description
/// Stores the contract configuration at the given key
pub const CONFIG: Item<Config> = Item::new("config");


/// ## Description
/// This structure stores the main paramters for the Keeper contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// The factory contract address
    pub vault_contract: Addr,
    /// The DEX token address
    pub dex_token_contract: Option<Addr>,
    /// The DEX Token staking contract address
    pub staking_contract: Option<Addr>,
}


