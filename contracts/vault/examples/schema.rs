use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema_with_title, remove_schemas, schema_for};

use dexter::asset::{Asset, AssetInfo};
use dexter::vault::{
    Config, ConfigResponse, Cw20HookMsg, ExecuteMsg, FeeInfo, InstantiateMsg, MigrateMsg,
    PoolConfigResponse, PoolInfo, PoolInfoResponse, PoolType, QueryMsg, SingleSwapRequest,
};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema_with_title(&schema_for!(InstantiateMsg), &out_dir, "InstantiateMsg");
    export_schema_with_title(&schema_for!(ExecuteMsg), &out_dir, "ExecuteMsg");
    export_schema_with_title(&schema_for!(QueryMsg), &out_dir, "QueryMsg");

    export_schema_with_title(&schema_for!(Config), &out_dir, "Config");
    export_schema_with_title(&schema_for!(Asset), &out_dir, "Asset");
    export_schema_with_title(&schema_for!(AssetInfo), &out_dir, "AssetInfo");
    export_schema_with_title(&schema_for!(PoolInfo), &out_dir, "PoolInfo");
    export_schema_with_title(&schema_for!(SingleSwapRequest), &out_dir, "SingleSwapRequest");
    export_schema_with_title(&schema_for!(FeeInfo), &out_dir, "FeeInfo");
    export_schema_with_title(&schema_for!(PoolType), &out_dir, "PoolType");

    export_schema_with_title(&schema_for!(ConfigResponse), &out_dir, "ConfigResponse");
    export_schema_with_title(&schema_for!(PoolConfigResponse), &out_dir, "PoolConfigResponse");
    export_schema_with_title(&schema_for!(PoolInfoResponse), &out_dir, "PoolInfoResponse");

    export_schema_with_title(&schema_for!(Cw20HookMsg), &out_dir, "Cw20HookMsg");
    export_schema_with_title(&schema_for!(MigrateMsg), &out_dir, "MigrateMsg");
}
