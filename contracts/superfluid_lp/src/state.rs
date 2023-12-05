use cosmwasm_std::Addr;
use cw_storage_plus::{Map, Item};
use dexter::{superfluid_lp::{LockInfo, Config}, helper::OwnershipProposal};


/// Stores the locked token information for a user
/// After the unlock time has passed, that lock is free and can be used.
/// Locked and unlocked tokens, both may be used to join pool.
/// Unlocked tokens can be withdrawn by the user.
/// Locked tokens can be used to join pool but cannot be withdrawn by the user directly.
/// When user specified an amount of locked tokens to spend, to join pool, then that
/// amount is checked across all locks, and the newest lock is used first to make most unlocked tokens available to the user.
/// 
/// If a lock amount is used partially, then the remaining amount is still locked and the lock is updated with the remaining amount.
pub const LOCKED_TOKENS: Map<(&Addr, &String), Vec<LockInfo>> = Map::new("locked_tokens");


pub const CONFIG: Item<Config> = Item::new("config");

// Ownership Proposal currently active in the Vault in a [`OwnershipProposal`] struct
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");