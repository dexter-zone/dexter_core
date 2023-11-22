use cosmwasm_std::{StdResult, Storage};
use cw_storage_plus::{Item, Map};
use dexter::governance_admin::{PoolCreateRequestContextData, RewardScheduleCreationRequestsState};

/// map of pool creation request id to pool creation request
pub const POOL_CREATION_REQUEST_DATA: Map<u64, PoolCreateRequestContextData> =
    Map::new("pool_creation_requests_context_data");

pub const REWARD_SCHEDULE_REQUESTS: Map<u64, RewardScheduleCreationRequestsState> =
    Map::new("reward_schedule_requests");

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
