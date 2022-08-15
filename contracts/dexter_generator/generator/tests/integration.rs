use cosmwasm_std::{attr, Addr, Uint128};
use cw20::MinterResponse;
use cw_multi_test::{App, BasicApp, ContractWrapper, Executor};
use dexter::asset::{Asset, AssetInfo};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::{
    vault::{ConfigResponse as VaultConfigResponse, QueryMsg as VaultQueryMsg, PoolConfig},
    generator::{
        ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, PendingTokenResponse,
        PoolInfoResponse, PoolLengthResponse, QueryMsg, RewardInfoResponse, ExecuteOnReply, Config
    },
    generator_proxy::{
        Cw20HookMsg as ProxyCw20HookMsg, ExecuteMsg as ProxyExecuteMsg, QueryMsg as ProxyQueryMsg,
    },
    vesting::ExecuteMsg as VestingExecuteMsg,
};

fn mock_app() -> App {
    BasicApp::default()
}


fn store_token_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        lp_token::contract::execute,
        lp_token::contract::instantiate,
        lp_token::contract::query,
    ));
    app.store_code(token_contract)
}

fn store_vesting_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_vesting::contract::execute,
        dexter_vesting::contract::instantiate,
        dexter_vesting::contract::query,
    ));
    app.store_code(token_contract)
}

