use cw_storage_plus::{Item, Map};
use dexter::helper::OwnershipProposal;
use dexter::vault::{Config, PoolConfig, PoolInfo};

// Stores Vault contract's core COnfiguration data.
pub const CONFIG: Item<Config> = Item::new("config");

// Stores config data for each PoolTypes supported by the Vault
pub const POOL_CONFIGS: Map<String, PoolConfig> = Map::new("pool_configs");

// Stores current state of each Pool instance supported by the Vault 
pub const POOLS: Map<&[u8], PoolInfo> = Map::new("pool_info");

// Ownership Proposal currently active in the Vault.
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");

// Temporarily stores the PoolInfo of the Pool which is currently being created.
pub const TMP_POOL_INFO: Item<PoolInfo> = Item::new("tmp_pool_info");

/// settings for pagination : The maximum limit for reading pools
const MAX_LIMIT: u32 = 15;

/// settings for pagination : The default limit for reading pools
const DEFAULT_LIMIT: u32 = 10;

