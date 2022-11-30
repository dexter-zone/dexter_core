use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::Item;
use dexter::helper::OwnershipProposal;

/// ## Description
/// Stores the contract configuration at the given key
pub const CONFIG: Item<Config> = Item::new("config");

// Ownership Proposal currently active in the Vault in a [`OwnershipProposal`] struc
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");


/// ## Description
/// This structure stores the main paramters for the Keeper contract.
#[cw_serde]
pub struct Config {
    /// admin address
    pub owner: Addr,
    /// The factory contract address
    pub vault_contract: Addr,
    /// The DEX token address
    pub dex_token_contract: Option<Addr>,
    /// The DEX Token staking contract address
    pub staking_contract: Option<Addr>,
}
