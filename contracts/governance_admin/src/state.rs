use cosmwasm_schema::cw_serde;
use cosmwasm_std::{StdResult, Storage, Uint128, Addr};
use cw_storage_plus::{Item, Map};
use dexter::asset::Asset;
use dexter::governance_admin::{PoolCreationRequest, RewardScheduleCreationRequestsState};

// ## Description
// Stores the contract configuration at the given key
// pub const CONFIG: Item<Config> = Item::new("config");

// Ownership Proposal currently active in the Vault in a [`OwnershipProposal`] struc
// pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");

#[cw_serde]
pub enum PoolCreationRequestStatus {
    PendingProposalCreation,
    ProposalCreated {
        proposal_id: u64,
    },
    PoolCreated {
        proposal_id: u64,
        pool_id: Uint128,
    },
    RequestFailedAndRefunded {
        proposal_id: u64,
        refund_block_height: u64,
    }
} 

impl PoolCreationRequestStatus {

    pub fn proposal_id(&self) -> Option<u64> {
        match self {
            PoolCreationRequestStatus::ProposalCreated { proposal_id } => Some(*proposal_id),
            PoolCreationRequestStatus::PoolCreated { proposal_id, .. } => Some(*proposal_id),
            PoolCreationRequestStatus::RequestFailedAndRefunded { proposal_id, .. } => Some(*proposal_id),
            _ => None,
        }
    }
}

#[cw_serde]
pub struct PoolCreateRequesContextData {
    pub status: PoolCreationRequestStatus,
    pub request_sender: Addr,
    pub total_funds_acquired_from_user: Vec<Asset>,
    pub pool_creation_request: PoolCreationRequest,
}

/// map of pool creation request id to pool creation request
pub const POOL_CREATION_REQUEST_DATA: Map<u64, PoolCreateRequesContextData> =
    Map::new("pool_creation_requests_context_data");

pub const REWARD_SCHEDULE_REQUESTS: Map<u64, RewardScheduleCreationRequestsState> =
    Map::new("reward_schedule_requests");

// /// map of pool creation request id to proposal id
// pub const POOL_CREATION_REQUEST_PROPOSAL_ID: Map<u64, u64> =
//     Map::new("pool_creation_request_proposal_id");

/// map of reward schedule request id to proposal id
pub const REWARD_SCHEDULE_REQUEST_PROPOSAL_ID: Map<u64, u64> =
    Map::new("reward_schedule_request_proposal_id");

/// count of pool creation requests to generate unique ids
pub const POOL_CREATION_REQUESTS_COUNT: Item<u64> = Item::new("pool_creation_requests_count");

/// count of reward schedule requests to generate unique ids
pub const REWARD_SCHEDULE_REQUESTS_COUNT: Item<u64> = Item::new("reward_schedule_requests_count");

pub fn next_pool_creation_request_id(store: &mut dyn Storage) -> StdResult<u64> {
    let id: u64 = POOL_CREATION_REQUESTS_COUNT
        .may_load(store)?
        .unwrap_or_default()
        + 1;
    POOL_CREATION_REQUESTS_COUNT.save(store, &id)?;
    Ok(id)
}

pub fn next_reward_schedule_request_id(store: &mut dyn Storage) -> StdResult<u64> {
    let id: u64 = REWARD_SCHEDULE_REQUESTS_COUNT
        .may_load(store)?
        .unwrap_or_default()
        + 1;
    REWARD_SCHEDULE_REQUESTS_COUNT.save(store, &id)?;
    Ok(id)
}
