use cosmwasm_std::{Addr, Deps, Order};
use cw_storage_plus::{Bound, Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dexter::asset::AssetInfo;
use dexter::helper::OwnershipProposal;
use dexter::vault::{Config, PoolConfig, PoolInfo};

pub const CONFIG: Item<Config> = Item::new("config");
pub const POOL_CONFIGS: Map<String, PoolConfig> = Map::new("pool_configs");
pub const POOLS: Map<&[u8], PoolInfo> = Map::new("pool_info");
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");

pub const TMP_POOL_INFO: Item<PoolInfo> = Item::new("tmp_pool_info");

/// settings for pagination : The maximum limit for reading pools
const MAX_LIMIT: u32 = 30;

/// settings for pagination : The default limit for reading pools
const DEFAULT_LIMIT: u32 = 10;

// /// ## Description - Calculates key of pool from the specified parameters in the `asset_infos` variable.
// /// `asset_infos` it is array with two items the type of [`AssetInfo`].
// pub fn pool_key(asset_infos: &Vec<AssetInfo>) -> Vec<u8> {
//     let mut asset_infos = asset_infos.to_vec();
//     asset_infos.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
//     [asset_infos[0].as_bytes(), asset_infos[1].as_bytes()].concat()
// }

// /// ## Description - Reads pools from the [`PAIRS`] according to the specified parameters in `start_after` and `limit` variables.
// /// Otherwise, it returns the default number of pools.
// /// ## Params
// /// `start_after` is a [`Option`] type. Sets the item to start reading from.
// /// `limit` is a [`Option`] type. Sets the number of items to be read.
// pub fn read_pools(
//     deps: Deps,
//     start_after: Option<Vec<AssetInfo>>,
//     limit: Option<u32>,
// ) -> Vec<Addr> {
//     let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

//     if let Some(data) = calc_range_start(start_after) {
//         PAIRS
//             .range(
//                 deps.storage,
//                 Some(Bound::exclusive(data.as_slice())),
//                 None,
//                 Order::Ascending,
//             )
//             .take(limit)
//             .map(|item| {
//                 let (_, pool_addr) = item.unwrap();
//                 pool_addr
//             })
//             .collect()
//     } else {
//         PAIRS
//             .range(deps.storage, None, None, Order::Ascending)
//             .take(limit)
//             .map(|item| {
//                 let (_, pool_addr) = item.unwrap();
//                 pool_addr
//             })
//             .collect()
//     }
// }

// // this will set the first key after the provided key, by appending a 1 byte
// /// ## Description - Calculates the key of the pool from which to start reading.
// /// ## Params
// /// `start_after` is an [`Option`] type that accepts two [`AssetInfo`] elements.
// fn calc_range_start(start_after: Option<Vec<AssetInfo>>) -> Option<Vec<u8>> {
//     start_after.map(|asset_infos| {
//         let mut asset_infos = asset_infos.to_vec();
//         asset_infos.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
//         let mut v = [asset_infos[0].as_bytes(), asset_infos[1].as_bytes()]
//             .concat()
//             .as_slice()
//             .to_vec();
//         v.push(1);
//         v
//     })
// }
