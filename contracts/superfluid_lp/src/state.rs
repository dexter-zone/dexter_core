use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Map, Item};
use dexter::{superfluid_lp::Config, helper::OwnershipProposal};


// Stores the amount of LST tokens that are locked for the user
pub const LOCK_AMOUNT: Map<(&Addr, &String), Uint128> = Map::new("lock_amount");

pub const CONFIG: Item<Config> = Item::new("config");

// Ownership Proposal currently active in the Vault in a [`OwnershipProposal`] struct
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");