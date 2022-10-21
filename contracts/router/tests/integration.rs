use cosmwasm_std::{attr, Addr, Uint128};
use cw20::MinterResponse;
use cw_multi_test::{App, BasicApp, ContractWrapper, Executor};
use dexter::asset::{Asset, AssetInfo};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::vault::{
    ConfigResponse, ExecuteMsg, FeeInfo, InstantiateMsg, PoolConfig, PoolConfigResponse, PoolInfo,
    PoolType, QueryMsg,
};

fn mock_app() -> App {
    BasicApp::default()
}

fn store_vault_code(app: &mut App) -> u64 {
    let factory_contract = Box::new(
        ContractWrapper::new_with_empty(
            dexter_vault::contract::execute,
            dexter_vault::contract::instantiate,
            dexter_vault::contract::query,
        )
        .with_reply_empty(dexter_vault::contract::reply),
    );
    app.store_code(factory_contract)
}

fn store_xyk_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(ContractWrapper::new_with_empty(
        xyk_pool::contract::execute,
        xyk_pool::contract::instantiate,
        xyk_pool::contract::query,
    ));
    app.store_code(pool_contract)
}

fn store_token_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        lp_token::contract::execute,
        lp_token::contract::instantiate,
        lp_token::contract::query,
    ));
    app.store_code(token_contract)
}

fn instantiate_contract(app: &mut App, owner: &Addr) -> Addr {
    let xyk_pool_code_id = store_xyk_pool_code(app);
    let vault_code_id = store_vault_code(app);
    let token_code_id = store_token_code(app);

    let pool_configs = vec![PoolConfig {
        code_id: xyk_pool_code_id,
        pool_type: PoolType::Xyk {},
        fee_info: FeeInfo {
            total_fee_bps: 300u16,
            protocol_fee_percent: 49u16,
            dev_fee_percent: 15u16,
            developer_addr: None,
        },
        is_disabled: false,
        is_generator_disabled: false,
    }];

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: token_code_id,
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        generator_address: None,
    };

    let vault_instance = app
        .instantiate_contract(
            vault_code_id,
            owner.to_owned(),
            &vault_init_msg,
            &[],
            "vault",
            None,
        )
        .unwrap();

    return vault_instance;
}

#[test]
fn proper_initialization() {
    let mut app = mock_app();
    let owner = Addr::unchecked("owner");

    let vault_code_id = store_vault_code(&mut app);
    let xyk_pool_code_id = store_xyk_pool_code(&mut app);
    let token_code_id = store_token_code(&mut app);

    let pool_configs = vec![PoolConfig {
        code_id: xyk_pool_code_id,
        pool_type: PoolType::Xyk {},
        fee_info: FeeInfo {
            total_fee_bps: 300u16,
            protocol_fee_percent: 49u16,
            dev_fee_percent: 15u16,
            developer_addr: None,
        },
        is_disabled: false,
        is_generator_disabled: false,
    }];

    //// -----x----- Success :: Initialize Vault Contract -----x----- ////

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: token_code_id,
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        generator_address: None,
    };

    let vault_instance = app
        .instantiate_contract(
            vault_code_id,
            Addr::unchecked(owner.clone()),
            &vault_init_msg,
            &[],
            "vault",
            None,
        )
        .unwrap();

    let msg = QueryMsg::Config {};
    let config_res: ConfigResponse = app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();

    assert_eq!(owner, config_res.owner);
    assert_eq!(token_code_id, config_res.lp_token_code_id);
    assert_eq!(
        Some(Addr::unchecked("fee_collector".to_string())),
        config_res.fee_collector
    );
    assert_eq!(None, config_res.generator_address);

    let msg = QueryMsg::QueryRegistry {
        pool_type: PoolType::Xyk {},
    };
    let registery_res: PoolConfigResponse =
        app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();
    assert_eq!(xyk_pool_code_id, registery_res.code_id);
    assert_eq!(PoolType::Xyk {}, registery_res.pool_type);
    assert_eq!(pool_configs[0].fee_info, registery_res.fee_info);
    assert_eq!(pool_configs[0].is_disabled, registery_res.is_disabled);
    assert_eq!(
        pool_configs[0].is_generator_disabled,
        registery_res.is_generator_disabled
    );

    //// -----x----- Error :: PoolConfigDuplicate Error -----x----- ////

    let pool_configs = vec![
        PoolConfig {
            code_id: xyk_pool_code_id,
            pool_type: PoolType::Xyk {},
            fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: None,
            },
            is_disabled: false,
            is_generator_disabled: false,
        },
        PoolConfig {
            code_id: xyk_pool_code_id,
            pool_type: PoolType::Xyk {},
            fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: None,
            },
            is_disabled: false,
            is_generator_disabled: false,
        },
    ];

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: token_code_id,
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        generator_address: None,
    };

    let res = app
        .instantiate_contract(
            vault_code_id,
            Addr::unchecked(owner.clone()),
            &vault_init_msg,
            &[],
            "vault",
            None,
        )
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "Duplicate of Pool Configs");

    //// -----x----- Error :: InvalidFeeInfo Error -----x----- ////

    let pool_configs = vec![PoolConfig {
        code_id: xyk_pool_code_id,
        pool_type: PoolType::Xyk {},
        fee_info: FeeInfo {
            total_fee_bps: 30000u16,
            protocol_fee_percent: 49u16,
            dev_fee_percent: 15u16,
            developer_addr: None,
        },
        is_disabled: false,
        is_generator_disabled: false,
    }];

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: token_code_id,
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        generator_address: None,
    };

    let res = app
        .instantiate_contract(
            vault_code_id,
            Addr::unchecked(owner.clone()),
            &vault_init_msg,
            &[],
            "vault",
            None,
        )
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "Invalid FeeInfo params");
}
