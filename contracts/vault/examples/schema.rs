use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};

use dexter::asset::{Asset, AssetInfo};
use dexter::vault::{
    Config, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg,
    PoolConfigResponse, PoolInfo, PoolInfoResponse, PoolType, QueryMsg, SingleSwapRequest, FeeInfo,
};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);

    export_schema(&schema_for!(Config), &out_dir);
    export_schema(&schema_for!(Asset), &out_dir);
    export_schema(&schema_for!(AssetInfo), &out_dir);
    export_schema(&schema_for!(PoolInfo), &out_dir);
    export_schema(&schema_for!(SingleSwapRequest), &out_dir);
    export_schema(&schema_for!(FeeInfo), &out_dir);
    export_schema(&schema_for!(PoolType), &out_dir);

    
    export_schema(&schema_for!(ConfigResponse), &out_dir);
    export_schema(&schema_for!(PoolConfigResponse), &out_dir);
    export_schema(&schema_for!(PoolInfoResponse), &out_dir);

    export_schema(&schema_for!(Cw20HookMsg), &out_dir);
    export_schema(&schema_for!(MigrateMsg), &out_dir);



}
