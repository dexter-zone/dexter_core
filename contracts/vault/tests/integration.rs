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
    let pool_contract = Box::new(
        ContractWrapper::new_with_empty(
            xyk_pool::contract::execute,
            xyk_pool::contract::instantiate,
            xyk_pool::contract::query,
        )
        .with_reply_empty(xyk_pool::contract::reply),
    );
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

    let msg = QueryMsg::QueryRigistery {
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

#[test]
fn update_config() {
    let mut app = mock_app();
    let owner = String::from("owner");
    let vault_instance = instantiate_contract(&mut app, &Addr::unchecked(owner.clone()));

    let msg = QueryMsg::Config {};
    let after_init_config_res: ConfigResponse =
        app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();

    assert_eq!(owner, after_init_config_res.owner);
    assert_eq!(
        Some(Addr::unchecked("fee_collector".to_string())),
        after_init_config_res.fee_collector
    );
    assert_eq!(None, after_init_config_res.generator_address);

    //// -----x----- Success :: update config -----x----- ////

    let msg = ExecuteMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: Some("fee_address".to_string()),
        generator_address: Some("generator_address".to_string()),
    };

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    let msg = QueryMsg::Config {};
    let config_res: ConfigResponse = app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();

    assert_eq!(owner, config_res.owner);
    assert_eq!(
        Some(Addr::unchecked("fee_address".to_string())),
        config_res.fee_collector
    );
    assert_eq!(
        Some(Addr::unchecked("generator_address".to_string())),
        config_res.generator_address
    );
    assert_eq!(
        after_init_config_res.lp_token_code_id,
        config_res.lp_token_code_id
    );
}

#[test]
fn test_add_to_registery() {
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

    let msg = QueryMsg::QueryRigistery {
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

    //// -----x----- Error :: Only Owner can add new PoolType to registery || Pool Type already exists -----x----- ////

    let msg = ExecuteMsg::AddToRegistery {
        new_pool_config: PoolConfig {
            code_id: xyk_pool_code_id,
            pool_type: PoolType::Xyk {},
            fee_info: FeeInfo {
                total_fee_bps: 10_0u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: None,
            },
            is_disabled: false,
            is_generator_disabled: false,
        },
    };

    let err_res = app
        .execute_contract(
            Addr::unchecked("not_owner".to_string().clone()),
            vault_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Unauthorized");

    let err_res = app
        .execute_contract(
            Addr::unchecked(owner.clone()),
            vault_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Pool Type already exists");

    //// -----x----- Error :: Only Owner can add new PoolType to registery || Pool Type already exists -----x----- ////

    let msg = ExecuteMsg::AddToRegistery {
        new_pool_config: PoolConfig {
            code_id: xyk_pool_code_id,
            pool_type: PoolType::Stable2Pool {},
            fee_info: FeeInfo {
                total_fee_bps: 10_001u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: None,
            },
            is_disabled: false,
            is_generator_disabled: false,
        },
    };

    let err_res = app
        .execute_contract(
            Addr::unchecked(owner.clone()),
            vault_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Invalid FeeInfo params");

    //// -----x----- Success :: Add new PoolType to registery  -----x----- ////
    let stable_pool_code_id = 2u64;
    let msg = ExecuteMsg::AddToRegistery {
        new_pool_config: PoolConfig {
            code_id: stable_pool_code_id,
            pool_type: PoolType::Stable2Pool {},
            fee_info: FeeInfo {
                total_fee_bps: 1_001u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: None,
            },
            is_disabled: false,
            is_generator_disabled: false,
        },
    };

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    let msg = QueryMsg::QueryRigistery {
        pool_type: PoolType::Stable2Pool {},
    };
    let registery_res: PoolConfigResponse =
        app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();
    assert_eq!(stable_pool_code_id, registery_res.code_id);
    assert_eq!(PoolType::Stable2Pool {}, registery_res.pool_type);
    assert_eq!(
        FeeInfo {
            total_fee_bps: 1_001u16,
            protocol_fee_percent: 49u16,
            dev_fee_percent: 15u16,
            developer_addr: None,
        },
        registery_res.fee_info
    );
    assert_eq!(false, registery_res.is_disabled);
    assert_eq!(false, registery_res.is_generator_disabled);
}

#[test]
fn test_create_pool_instance() {
    let mut app = mock_app();
    let owner = String::from("owner");
    let token_code_id = store_token_code(&mut app);
    let vault_instance = instantiate_contract(&mut app, &Addr::unchecked(owner.clone()));

    // Create Token X
    let init_msg = TokenInstantiateMsg {
        name: "x_token".to_string(),
        symbol: "X-Tok".to_string(),
        decimals: 18,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
        marketing: None,
    };
    let token_instance0 = app
        .instantiate_contract(
            token_code_id,
            Addr::unchecked(owner.clone()),
            &init_msg,
            &[],
            "x_token",
            None,
        )
        .unwrap();

    // Create Token Y
    let init_msg = TokenInstantiateMsg {
        name: "y_token".to_string(),
        symbol: "Y-Tok".to_string(),
        decimals: 18,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
        marketing: None,
    };
    let token_instance1 = app
        .instantiate_contract(
            token_code_id,
            Addr::unchecked(owner.clone()),
            &init_msg,
            &[],
            "y_token",
            None,
        )
        .unwrap();

    let asset_infos = vec![
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
        AssetInfo::Token {
            contract_addr: token_instance1.clone(),
        },
    ];

    let msg = ExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Xyk {},
        asset_infos: asset_infos.to_vec(),
        init_params: None,
        lp_token_name: None,
        lp_token_symbol: None,
    };

    let res = app
        .execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

    assert_eq!(res.events[1].attributes[1], attr("action", "create_pool"));
    assert_eq!(res.events[1].attributes[2], attr("pool_type", "xyk"));

    let pool_res: PoolInfo = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::GetPoolById {
                pool_id: Uint128::from(1u128),
            },
        )
        .unwrap();

    let assets = vec![
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::zero(),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::zero(),
        },
    ];

    assert_eq!(Uint128::from(1u128), pool_res.pool_id);
    assert_eq!(
        Some(Addr::unchecked("contract3".to_string())),
        pool_res.pool_addr
    );
    assert_eq!(
        Some(Addr::unchecked("contract4".to_string())),
        pool_res.lp_token_addr
    );
    assert_eq!(assets, pool_res.assets);
    assert_eq!(PoolType::Xyk {}, pool_res.pool_type);
    assert_eq!(None, pool_res.developer_addr);
}

#[test]
fn test_update_owner() {
    let mut app = mock_app();
    let owner = String::from("owner");
    let vault_instance = instantiate_contract(&mut app, &Addr::unchecked(owner.clone()));

    let new_owner = String::from("new_owner");

    // New owner
    let msg = ExecuteMsg::ProposeNewOwner {
        owner: new_owner.clone(),
        expires_in: 100, // seconds
    };

    // Unauthed check
    let err = app
        .execute_contract(
            Addr::unchecked("not_owner"),
            vault_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    // Claim before proposal
    let err = app
        .execute_contract(
            Addr::unchecked(new_owner.clone()),
            vault_instance.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Ownership proposal not found"
    );

    // Propose new owner
    app.execute_contract(Addr::unchecked("owner"), vault_instance.clone(), &msg, &[])
        .unwrap();

    // Claim from invalid addr
    let err = app
        .execute_contract(
            Addr::unchecked("invalid_addr"),
            vault_instance.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap_err();
    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    // Drop ownership proposal
    let err = app
        .execute_contract(
            Addr::unchecked(new_owner.clone()),
            vault_instance.clone(),
            &ExecuteMsg::DropOwnershipProposal {},
            &[],
        )
        .unwrap_err();
    // new_owner is not an owner yet
    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &ExecuteMsg::DropOwnershipProposal {},
        &[],
    )
    .unwrap();

    // Try to claim ownership
    let err = app
        .execute_contract(
            Addr::unchecked(new_owner.clone()),
            vault_instance.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Ownership proposal not found"
    );

    // Propose new owner again
    app.execute_contract(Addr::unchecked("owner"), vault_instance.clone(), &msg, &[])
        .unwrap();
    // Claim ownership
    app.execute_contract(
        Addr::unchecked(new_owner.clone()),
        vault_instance.clone(),
        &ExecuteMsg::ClaimOwnership {},
        &[],
    )
    .unwrap();

    // Let's query the contract state
    let msg = QueryMsg::Config {};
    let res: ConfigResponse = app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();

    assert_eq!(res.owner, new_owner)
}
