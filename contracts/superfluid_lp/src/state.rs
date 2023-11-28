use cosmwasm_std::{Uint128, Addr};
use cw_storage_plus::{Item, Map};
use dexter::{router::Config, asset::{AssetInfo, Asset}};

pub const LOCKED_TOKENS: Map<(&Addr, &String), Uint128> = Map::new("locked_tokens");
