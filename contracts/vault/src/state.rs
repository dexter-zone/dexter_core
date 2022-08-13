use cw_storage_plus::{Item, Map};
use dexter::helper::OwnershipProposal;
use dexter::vault::{Config, PoolConfig, PoolInfo};

// Stores Vault contract's core Configuration parameters in a [`Config`] struct   
pub const CONFIG: Item<Config> = Item::new("config");

// Stores configuration data associated with each [`PoolType`] supported by the Vault in a [`PoolConfig`] struct
pub const REGISTERY: Map<String, PoolConfig> = Map::new("pool_configs");

// Stores current state of each Pool instance identified by its ID supported by the Vault in a [`PoolInfo`] struc
pub const ACTIVE_POOLS: Map<&[u8], PoolInfo> = Map::new("pool_info");

// Ownership Proposal currently active in the Vault in a [`OwnershipProposal`] struc
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");

// Temporarily stores the PoolInfo of the Pool which is currently being created in a [`PoolInfo`] struc 
pub const TMP_POOL_INFO: Item<PoolInfo> = Item::new("tmp_pool_info");
