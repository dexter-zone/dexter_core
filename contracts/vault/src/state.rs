use cosmwasm_std::Uint128;
use cw_storage_plus::{Item, Map};
use dexter::helper::OwnershipProposal;
use dexter::vault::{Config, PoolInfo, PoolTypeConfig, TmpPoolInfo, DefunctPoolInfo};

// Stores Vault contract's core Configuration parameters in a [`Config`] struct
pub const CONFIG: Item<Config> = Item::new("config");

// Stores configuration data associated with each [`PoolType`] supported by the Vault in a [`PoolConfig`] struct
pub const REGISTRY: Map<String, PoolTypeConfig> = Map::new("pool_configs");

// Stores current state of each Pool instance identified by its ID supported by the Vault in a [`PoolInfo`] struc
pub const ACTIVE_POOLS: Map<&[u8], PoolInfo> = Map::new("pool_info");

// Stores mapping of LP token address to the Pool Id
pub const LP_TOKEN_TO_POOL_ID: Map<&[u8], Uint128> = Map::new("lp_token_to_pool");

// Ownership Proposal currently active in the Vault in a [`OwnershipProposal`] struct
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");

// Temporarily stores the PoolInfo of the Pool which is currently being created in a [`PoolInfo`] struc
pub const TMP_POOL_INFO: Item<TmpPoolInfo> = Item::new("tmp_pool_info");

// Stores information about defunct pools
pub const DEFUNCT_POOLS: Map<Uint128, DefunctPoolInfo> = Map::new("defunct_pools");

// Tracks which users have been refunded from defunct pools (pool_id, user_addr) -> bool
pub const REFUNDED_USERS: Map<(Uint128, &str), bool> = Map::new("refunded_users");
