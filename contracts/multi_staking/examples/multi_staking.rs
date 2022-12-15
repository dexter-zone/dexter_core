use cosmwasm_schema::write_api;
use dexter::multi_staking::{InstantiateMsg, QueryMsg, ExecuteMsg, MigrateMsg};

fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        query: QueryMsg,
        execute: ExecuteMsg,
        migrate: MigrateMsg,
    }
}
