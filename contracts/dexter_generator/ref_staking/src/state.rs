use cosmwasm_schema::cw_serde;

use cosmwasm_std::{
    to_binary, Addr, CanonicalAddr, Decimal, QuerierWrapper, QueryRequest, StdResult, Uint128,
    WasmQuery,
};
use cw20::{Cw20QueryMsg, MinterResponse};
use cw_storage_plus::{Item, Map};

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
pub const USERS: Map<&Addr, StakerInfo> = Map::new("user");

#[cw_serde]
pub struct Config {
    pub anchor_token: CanonicalAddr,
    pub staking_token: CanonicalAddr,
    pub distribution_schedule: Vec<(u64, u64, Uint128)>,
}

// pub fn store_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
//     KEY_CONFIG.save(storage, config)?
// }

// pub fn read_config(storage: &dyn Storage) -> StdResult<Config> {
//     singleton_read(storage, KEY_CONFIG).load()
// }

#[cw_serde]
pub struct State {
    pub last_distributed: u64,
    pub total_bond_amount: Uint128,
    pub global_reward_index: Decimal,
}

// pub fn store_state(storage: &mut dyn Storage, state: &State) -> StdResult<()> {
//     singleton(storage, KEY_STATE).save(state)
// }

// pub fn read_state(storage: &dyn Storage) -> StdResult<State> {
//     singleton_read(storage, KEY_STATE).load()
// }

#[cw_serde]
pub struct StakerInfo {
    pub reward_index: Decimal,
    pub bond_amount: Uint128,
    pub pending_reward: Uint128,
}

impl Default for StakerInfo {
    fn default() -> Self {
        StakerInfo {
            reward_index: Decimal::zero(),
            bond_amount: Uint128::zero(),
            pending_reward: Uint128::zero(),
        }
    }
}

// /// returns return staker_info of the given owner
// pub fn store_staker_info(
//     storage: &mut dyn Storage,
//     owner: &CanonicalAddr,
//     staker_info: &StakerInfo,
// ) -> StdResult<()> {
//     Bucket::new(storage, PREFIX_REWARD).save(owner.as_slice(), staker_info)
// }

// /// remove staker_info of the given owner
// pub fn remove_staker_info(storage: &mut dyn Storage, owner: &CanonicalAddr) {
//     Bucket::<StakerInfo>::new(storage, PREFIX_REWARD).remove(owner.as_slice())
// }

// /// returns rewards owned by this owner
// /// (read-only version for queries)
// pub fn read_staker_info(storage: &dyn Storage, owner: &CanonicalAddr) -> StdResult<StakerInfo> {
//     match ReadonlyBucket::new(storage, PREFIX_REWARD).may_load(owner.as_slice())? {
//         Some(staker_info) => Ok(staker_info),
//         None => Ok(StakerInfo {
//             reward_index: Decimal::zero(),
//             bond_amount: Uint128::zero(),
//             pending_reward: Uint128::zero(),
//         }),
//     }
// }

/// Query asset price igonoring price age
pub fn query_anc_minter(querier: &QuerierWrapper, anchor_token: Addr) -> StdResult<String> {
    let res: MinterResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: anchor_token.to_string(),
        msg: to_binary(&Cw20QueryMsg::Minter {})?,
    }))?;

    Ok(res.minter)
}
