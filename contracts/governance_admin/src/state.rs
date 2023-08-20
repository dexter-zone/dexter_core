use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Uint128, Binary, Storage, StdResult};
use cw_storage_plus::{Item, Map};
use dexter::asset::Asset;
use dexter::governance_admin::PoolCreationRequest;
use dexter::vault::{FeeInfo, NativeAssetPrecisionInfo};

// ## Description
// Stores the contract configuration at the given key
// pub const CONFIG: Item<Config> = Item::new("config");

// Ownership Proposal currently active in the Vault in a [`OwnershipProposal`] struc
// pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");

#[cw_serde]
pub struct CreatePoolTempData {
    pub assumed_pool_id: Uint128,
    pub vault_addr: String,
    pub bootstrapping_amount_payer: String,
    pub pool_type: String,
    pub fee_info: Option<FeeInfo>,
    pub native_asset_precisions: Vec<NativeAssetPrecisionInfo>,
    pub assets: Vec<Asset>,
    pub init_params: Option<Binary>
}


/// map of pool creation request id to pool creation request
pub const POOL_CREATION_REQUESTS: Map<u64, PoolCreationRequest> = Map::new("pool_creation_requests");

/// map of pool creation request id to proposal id
pub const POOL_CREATION_REQUEST_PROPOSAL_ID: Map<u64, u64> = Map::new("pool_creation_request_proposal_id");

/// count of pool creation requests to generate unique ids
pub const POOL_CREATION_REQUESTS_COUNT: Item<u64> = Item::new("pool_creation_requests_count");

pub fn next_pool_creation_request_id(store: &mut dyn Storage) -> StdResult<u64> {
    let id: u64 = POOL_CREATION_REQUESTS_COUNT
        .may_load(store)?
        .unwrap_or_default()
        + 1;
    POOL_CREATION_REQUESTS_COUNT.save(store, &id)?;
    Ok(id)
}