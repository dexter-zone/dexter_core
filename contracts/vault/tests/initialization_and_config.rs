pub mod utils;

use cosmwasm_std::{attr, coin, Addr, Coin, Uint128};
use cw20::MinterResponse;
use cw_multi_test::Executor;
use dexter::asset::AssetInfo;
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{FeeResponse, QueryMsg as PoolQueryMsg};
use dexter::vault::{
    ConfigResponse, ExecuteMsg, FeeInfo, InstantiateMsg, PauseInfo, PauseInfoUpdateType,
    PoolConfigResponse, PoolInfoResponse, PoolType, PoolTypeConfig, QueryMsg,
};

use crate::utils::{
    initialize_3_tokens, initialize_xyk_pool, instantiate_contract, mock_app, store_stable5_pool_code,
    store_stable_pool_code, store_token_code, store_vault_code, store_weighted_pool_code, store_xyk_pool_code,
};

#[test]
fn proper_initialization() {
    let owner = Addr::unchecked("owner");
    let mut app = mock_app(Addr::unchecked(owner.clone()), vec![]);
    let vault_code_id = store_vault_code(&mut app);
    let xyk_pool_code_id = store_xyk_pool_code(&mut app);
    let stable_pool_code_id = store_stable_pool_code(&mut app);
    let weighted_pool_code_id = store_weighted_pool_code(&mut app);
    let stable5_pool_code_id = store_stable5_pool_code(&mut app);
    let token_code_id = store_token_code(&mut app);

    let pool_configs = vec![
        PoolTypeConfig {
            code_id: xyk_pool_code_id,
            pool_type: PoolType::Xyk {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: Some(Addr::unchecked(&"xyk_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
            paused: PauseInfo::default(),
        },
        PoolTypeConfig {
            code_id: stable_pool_code_id,
            pool_type: PoolType::Stable2Pool {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: Some(Addr::unchecked(&"stable_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
            paused: PauseInfo::default(),
        },
        PoolTypeConfig {
            code_id: stable5_pool_code_id,
            pool_type: PoolType::Stable5Pool {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: Some(Addr::unchecked(&"stable5_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
            paused: PauseInfo::default(),
        },
        PoolTypeConfig {
            code_id: weighted_pool_code_id,
            pool_type: PoolType::Weighted {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: Some(Addr::unchecked(&"weighted_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
            paused: PauseInfo::default(),
        },
    ];

    //// -----x----- Success :: Initialize Vault Contract -----x----- ////

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: Some(token_code_id),
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        auto_stake_impl: None,
        multistaking_address: None,
        generator_address: None,
        pool_creation_fee: None,
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
    assert_eq!(token_code_id, config_res.lp_token_code_id.unwrap());
    assert_eq!(
        Some(Addr::unchecked("fee_collector".to_string())),
        config_res.fee_collector
    );
    assert_eq!(None, config_res.generator_address);
    assert_eq!(PauseInfo {
        deposit: false,
        swap: false,
    }, config_res.paused);

    // Check XYK Pool Config
    // ---------------------
    let xyk_pool_config_res: PoolConfigResponse = app
        .wrap()
        .query_wasm_smart(
            &vault_instance,
            &QueryMsg::QueryRegistry {
                pool_type: PoolType::Xyk {},
            },
        )
        .unwrap();

    assert!(xyk_pool_config_res.is_some());
    let xyk_pool_config_res = xyk_pool_config_res.unwrap();
    assert_eq!(xyk_pool_code_id, xyk_pool_config_res.code_id);
    assert_eq!(PoolType::Xyk {}, xyk_pool_config_res.pool_type);
    assert_eq!(
        pool_configs[0].allow_instantiation,
        xyk_pool_config_res.allow_instantiation
    );
    assert_eq!(
        pool_configs[0].is_generator_disabled,
        xyk_pool_config_res.is_generator_disabled
    );

    // Check Stabl Pool Config
    // ---------------------
    let stablepool_config_res: PoolConfigResponse = app
        .wrap()
        .query_wasm_smart(
            &vault_instance,
            &QueryMsg::QueryRegistry {
                pool_type: PoolType::Stable2Pool {},
            },
        )
        .unwrap();

    assert!(stablepool_config_res.is_some());
    let stablepool_config_res = stablepool_config_res.unwrap();
    assert_eq!(stable_pool_code_id, stablepool_config_res.code_id);
    assert_eq!(PoolType::Stable2Pool {}, stablepool_config_res.pool_type);
    assert_eq!(
        pool_configs[1].default_fee_info,
        stablepool_config_res.default_fee_info
    );
    assert_eq!(
        pool_configs[1].allow_instantiation,
        stablepool_config_res.allow_instantiation
    );
    assert_eq!(
        pool_configs[1].is_generator_disabled,
        stablepool_config_res.is_generator_disabled
    );

    // Check Stabl-5-Pool Config
    // ---------------------
    let stable5pool_config_res: PoolConfigResponse = app
        .wrap()
        .query_wasm_smart(
            &vault_instance,
            &QueryMsg::QueryRegistry {
                pool_type: PoolType::Stable5Pool {},
            },
        )
        .unwrap();

    assert!(stable5pool_config_res.is_some());
    let stable5pool_config_res = stable5pool_config_res.unwrap();
    assert_eq!(stable5_pool_code_id, stable5pool_config_res.code_id);
    assert_eq!(PoolType::Stable5Pool {}, stable5pool_config_res.pool_type);
    assert_eq!(
        pool_configs[2].default_fee_info,
        stable5pool_config_res.default_fee_info
    );
    assert_eq!(
        pool_configs[2].allow_instantiation,
        stable5pool_config_res.allow_instantiation
    );
    assert_eq!(
        pool_configs[2].is_generator_disabled,
        stable5pool_config_res.is_generator_disabled
    );

    // Check Weighted Config
    // ---------------------
    let weightedpool_config_res: PoolConfigResponse = app
        .wrap()
        .query_wasm_smart(
            &vault_instance,
            &QueryMsg::QueryRegistry {
                pool_type: PoolType::Weighted {},
            },
        )
        .unwrap();

    assert!(weightedpool_config_res.is_some());
    let weightedpool_config_res = weightedpool_config_res.unwrap();
    assert_eq!(weighted_pool_code_id, weightedpool_config_res.code_id);
    assert_eq!(PoolType::Weighted {}, weightedpool_config_res.pool_type);
    assert_eq!(
        pool_configs[3].default_fee_info,
        weightedpool_config_res.default_fee_info
    );
    assert_eq!(
        pool_configs[3].allow_instantiation,
        weightedpool_config_res.allow_instantiation
    );
    assert_eq!(
        pool_configs[3].is_generator_disabled,
        weightedpool_config_res.is_generator_disabled
    );

    //// -----x----- Error :: PoolConfigDuplicate Error -----x----- ////

    let pool_configs = vec![
        PoolTypeConfig {
            code_id: xyk_pool_code_id,
            pool_type: PoolType::Xyk {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: None,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
            paused: PauseInfo::default(),
        },
        PoolTypeConfig {
            code_id: xyk_pool_code_id,
            pool_type: PoolType::Xyk {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: None,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
            paused: PauseInfo::default(),
        },
    ];

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: Some(token_code_id),
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        auto_stake_impl: None,
        multistaking_address: None,
        generator_address: None,
        pool_creation_fee: None,
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

    let pool_configs = vec![PoolTypeConfig {
        code_id: xyk_pool_code_id,
        pool_type: PoolType::Xyk {},
        default_fee_info: FeeInfo {
            total_fee_bps: 30000u16,
            protocol_fee_percent: 49u16,
            dev_fee_percent: 15u16,
            developer_addr: None,
        },
        allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
        is_generator_disabled: false,
        paused: PauseInfo::default(),
    }];

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: Some(token_code_id),
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        auto_stake_impl: None,
        multistaking_address: None,
        generator_address: None,
        pool_creation_fee: None,
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
fn test_add_to_registery() {
    let owner = Addr::unchecked("owner");
    let mut app = mock_app(Addr::unchecked(owner.clone()), vec![]);
    let vault_code_id = store_vault_code(&mut app);
    let xyk_pool_code_id = store_xyk_pool_code(&mut app);
    let token_code_id = store_token_code(&mut app);

    let pool_configs = vec![PoolTypeConfig {
        code_id: xyk_pool_code_id,
        pool_type: PoolType::Xyk {},
        default_fee_info: FeeInfo {
            total_fee_bps: 300u16,
            protocol_fee_percent: 49u16,
            dev_fee_percent: 15u16,
            developer_addr: None,
        },
        allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
        is_generator_disabled: false,
        paused: PauseInfo::default(),
    }];

    //// -----x----- Success :: Initialize Vault Contract -----x----- ////

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: Some(token_code_id),
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        auto_stake_impl: None,
        generator_address: None,
        multistaking_address: None,
        pool_creation_fee: None,
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

    let msg = QueryMsg::QueryRegistry {
        pool_type: PoolType::Xyk {},
    };
    let registery_res: PoolConfigResponse =
        app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();

    assert!(registery_res.is_some());
    let pool_config_res = registery_res.unwrap();

    assert_eq!(xyk_pool_code_id, pool_config_res.code_id);
    assert_eq!(PoolType::Xyk {}, pool_config_res.pool_type);
    assert_eq!(
        pool_configs[0].default_fee_info,
        pool_config_res.default_fee_info
    );
    assert_eq!(
        pool_configs[0].allow_instantiation,
        pool_config_res.allow_instantiation
    );
    assert_eq!(
        pool_configs[0].is_generator_disabled,
        pool_config_res.is_generator_disabled
    );

    //// -----x----- Error :: Only Owner can add new PoolType to registery || Pool Type already exists -----x----- ////

    let msg = ExecuteMsg::AddToRegistry {
        new_pool_type_config: PoolTypeConfig {
            code_id: xyk_pool_code_id,
            pool_type: PoolType::Xyk {},
            default_fee_info: FeeInfo {
                total_fee_bps: 10_0u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: None,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
            paused: PauseInfo::default(),
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

    let msg = ExecuteMsg::AddToRegistry {
        new_pool_type_config: PoolTypeConfig {
            code_id: xyk_pool_code_id,
            pool_type: PoolType::Stable2Pool {},
            default_fee_info: FeeInfo {
                total_fee_bps: 10_001u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: None,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
            paused: PauseInfo::default(),
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
    let msg = ExecuteMsg::AddToRegistry {
        new_pool_type_config: PoolTypeConfig {
            code_id: stable_pool_code_id,
            pool_type: PoolType::Stable2Pool {},
            default_fee_info: FeeInfo {
                total_fee_bps: 1_001u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: None,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
            paused: PauseInfo::default(),
        },
    };

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    let msg = QueryMsg::QueryRegistry {
        pool_type: PoolType::Stable2Pool {},
    };
    let registery_res: PoolConfigResponse =
        app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();

    assert!(registery_res.is_some());
    let pool_config_res = registery_res.unwrap();
    assert_eq!(stable_pool_code_id, pool_config_res.code_id);
    assert_eq!(PoolType::Stable2Pool {}, pool_config_res.pool_type);
    assert_eq!(
        FeeInfo {
            total_fee_bps: 1_001u16,
            protocol_fee_percent: 49u16,
            dev_fee_percent: 15u16,
            developer_addr: None,
        },
        pool_config_res.default_fee_info
    );
    assert_eq!(
        dexter::vault::AllowPoolInstantiation::Everyone,
        pool_config_res.allow_instantiation
    );
    assert_eq!(false, pool_config_res.is_generator_disabled);
}

#[test]
fn update_config() {
    let owner = String::from("owner");
    let mut app = mock_app(Addr::unchecked(owner.clone()), vec![]);
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
    assert_eq!(PauseInfo::default(), after_init_config_res.paused);

    //// -----x----- Success :: update config -----x----- ////

    let pause_info = PauseInfo{
        swap: true,
        deposit: false,
    };

    let msg = ExecuteMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: Some("fee_address".to_string()),
        generator_address: Some("generator_address".to_string()),
        auto_stake_impl: None,
        multistaking_address: None,
        pool_creation_fee: None,
        paused: Some(pause_info.clone()),
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
    assert_eq!(pause_info, config_res.paused);
}

#[test]
fn test_pool_config_update() {
    let owner = String::from("owner");
    let mut app = mock_app(
        Addr::unchecked(owner.clone()),
        vec![Coin {
            denom: "uxprt".to_string(),
            amount: Uint128::from(1_000_000_000u64),
        }],
    );

    let owner_addr = Addr::unchecked(owner.clone());
    let user_addr = Addr::unchecked("user".to_string());

    // Send some funds from owner to user
    app.send_tokens(
        owner_addr.clone(),
        user_addr.clone(),
        &[coin(200_000_000u128, "uxprt")],
    )
    .unwrap();

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
        fee_info: None,
    };

    let res = app
        .execute_contract(
            Addr::unchecked(owner.clone()),
            vault_instance.clone(),
            &msg,
            &[],
        )
        .unwrap();

    assert_eq!(res.events[1].attributes[2], attr("pool_type", "xyk"));

    let pool_id: u64 = 1;

    // get pool address from vault contract
    let query_msg = QueryMsg::GetPoolById {
        pool_id: Uint128::from(pool_id),
    };
    let res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &query_msg)
        .unwrap();
    let pool_address = res.pool_addr;

    let pause_info = PauseInfo {
        swap: true,
        deposit: true,
    };

    // update config for this pool now
    let msg = ExecuteMsg::UpdatePoolConfig {
        pool_id: Uint128::from(pool_id),
        fee_info: Some(FeeInfo {
            total_fee_bps: 400u16,
            protocol_fee_percent: 40u16,
            dev_fee_percent: 0u16,
            developer_addr: None,
        }),
        paused: Some(pause_info.clone()),
    };

    let res = app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    );

    assert!(res.is_ok());

    // Fetch pool info
    let msg = QueryMsg::GetPoolById {
        pool_id: Uint128::from(pool_id),
    };
    let res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &msg)
        .unwrap();

    assert_eq!(res.fee_info.total_fee_bps, 400u16);
    assert_eq!(res.fee_info.protocol_fee_percent, 40u16);
    assert_eq!(res.fee_info.dev_fee_percent, 0u16);
    assert_eq!(res.paused, pause_info);

    // Fetch fee from the pool contract too to see if total fee is updated
    let msg = PoolQueryMsg::FeeParams {};
    let res: FeeResponse = app.wrap().query_wasm_smart(pool_address, &msg).unwrap();

    assert_eq!(res.total_fee_bps, 400u16);
}

#[test]
fn test_update_pause_info() {
    let owner = String::from("owner");
    let denom0 = "token0".to_string();

    let mut app = mock_app(
        Addr::unchecked(owner.clone()),
        vec![Coin {
            denom: "uxprt".to_string(),
            amount: Uint128::from(1_000_000_000u64),
        }],
    );

    let owner_addr = Addr::unchecked(owner.clone());
    let user_addr = Addr::unchecked("user".to_string());

    // Send some funds from owner to user
    app.send_tokens(
        owner_addr.clone(),
        user_addr.clone(),
        &[coin(300_000_000u128, "uxprt")],
    )
        .unwrap();

    let vault_instance = instantiate_contract(&mut app, &owner_addr);
    let (token_instance1, _, _) = initialize_3_tokens(&mut app, &owner_addr);

    // Create XYK pool
    let (_, _, xyk_pool_id) = initialize_xyk_pool(
        &mut app,
        &owner_addr,
        vault_instance.clone(),
        token_instance1.clone(),
        denom0.clone(),
    );

    // Add user to whitelist
    app.execute_contract(
        owner_addr.clone(),
        vault_instance.clone(),
        &ExecuteMsg::AddAddressToWhitelist {
            address: user_addr.to_string(),
        },
        &[],
    )
        .unwrap();

    // --------------- begin actual testing -----------------

    // assert the pause status via queries before updating

    // pool type config
    let res: PoolConfigResponse = app.wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::QueryRegistry {pool_type: PoolType::Xyk {}}
        ).unwrap();
    assert_eq!(res.unwrap().paused, PauseInfo::default());

    // pool id config
    let res: PoolInfoResponse = app.wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::GetPoolById {pool_id: xyk_pool_id}
        ).unwrap();
    assert_eq!(res.paused, PauseInfo::default());

    // update the pause info for type
    let expected_pause_info = PauseInfo { deposit: true, swap: false };
    app.execute_contract(
        user_addr.clone(),
        vault_instance.clone(),
        &ExecuteMsg::UpdatePauseInfo {
            update_type: PauseInfoUpdateType::PoolType(PoolType::Xyk {}),
            pause_info: expected_pause_info.clone(),
        },
        &[],
    ).unwrap();

    // assert the pause status via queries after updating only for pool type

    // pool type config
    let res: PoolConfigResponse = app.wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::QueryRegistry {pool_type: PoolType::Xyk {}}
        ).unwrap();
    assert_eq!(res.unwrap().paused, expected_pause_info.clone());

    // pool id config
    let res: PoolInfoResponse = app.wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::GetPoolById {pool_id: xyk_pool_id}
        ).unwrap();
    assert_eq!(res.paused, PauseInfo::default());

    // update the pause info for id
    app.execute_contract(
        user_addr.clone(),
        vault_instance.clone(),
        &ExecuteMsg::UpdatePauseInfo {
            update_type: PauseInfoUpdateType::PoolId(xyk_pool_id),
            pause_info: expected_pause_info.clone(),
        },
        &[],
    ).unwrap();

    // assert the pause status for pool id as well
    let res: PoolInfoResponse = app.wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::GetPoolById {pool_id: xyk_pool_id}
        ).unwrap();
    assert_eq!(res.paused, expected_pause_info);

    // trying to update the pause info from a non-whitelisted address should fail
    let res = app.execute_contract(
        Addr::unchecked("non-whitelisted-addr"),
        vault_instance.clone(),
        &ExecuteMsg::UpdatePauseInfo {
            update_type: PauseInfoUpdateType::PoolId(xyk_pool_id),
            pause_info: expected_pause_info.clone(),
        },
        &[],
    );
    assert_eq!(res.is_err(), true);
    assert_eq!(res.unwrap_err().root_cause().to_string(), "Unauthorized");
}
