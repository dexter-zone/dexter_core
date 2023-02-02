use cw_storage_plus::Item;
use dexter::helper::OwnershipProposal;
use dexter::keeper::Config;

/// ## Description
/// Stores the contract configuration at the given key
pub const CONFIG: Item<Config> = Item::new("config");

// Ownership Proposal currently active in the Vault in a [`OwnershipProposal`] struc
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");
