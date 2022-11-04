use cosmwasm_schema::write_api;
use dexter::lp_token::InstantiateMsg;
use cw20_base::msg::{QueryMsg, ExecuteMsg};

fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        execute: ExecuteMsg,
        query: QueryMsg,
    }
}
