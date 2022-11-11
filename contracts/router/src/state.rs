use cw_storage_plus::Item;
use dexter::router::Config;

pub const CONFIG: Item<Config> = Item::new("config");
