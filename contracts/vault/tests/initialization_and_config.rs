pub mod utils;

use cosmwasm_std::{attr, coin, Addr, Coin, Uint128, to_binary};
use cw20::MinterResponse;
use cw_multi_test::Executor;
use dexter::asset::AssetInfo;
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{FeeResponse, QueryMsg as PoolQueryMsg};
use dexter::vault::{
    ConfigResponse, ExecuteMsg, FeeInfo, InstantiateMsg, PauseInfo, PoolTypeConfigResponse, PoolInfoResponse,
    PoolType, PoolTypeConfig, QueryMsg, PoolCreationFee, AutoStakeImpl, PauseInfoUpdateType,
};
use stable5pool::state::StablePoolParams;

use crate::utils::{initialize_3_tokens, initialize_stable_5_pool_2_asset, instantiate_contract, mock_app, store_stable5_pool_code, store_token_code, store_vault_code, store_weighted_pool_code};

#[test]
fn proper_initialization() {
    let owner = Addr::unchecked("owner");
    let mut app = mock_app(Addr::unchecked(owner.clone()), vec![]);
    let vault_code_id = store_vault_code(&mut app);
    let weighted_pool_code_id = store_weighted_pool_code(&mut app);
    let stable5_pool_code_id = store_stable5_pool_code(&mut app);
    let token_code_id = store_token_code(&mut app);

    let pool_configs = vec![
        PoolTypeConfig {
            code_id: stable5_pool_code_id,
            pool_type: PoolType::Stable5Pool {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            paused: PauseInfo::default(),
        },
        PoolTypeConfig {
            code_id: weighted_pool_code_id,
            pool_type: PoolType::Weighted {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            paused: PauseInfo::default(),
        },
    ];

    //// -----x----- Success :: Initialize Vault Contract -----x----- ////

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: Some(token_code_id),
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        auto_stake_impl: dexter::vault::AutoStakeImpl::None,
        pool_creation_fee: PoolCreationFee::Disabled,
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
    assert_eq!(PauseInfo {
        deposit: false,
        swap: false,
        imbalanced_withdraw: false
    }, config_res.paused);

    // Check Stabl-5-Pool Config
    // ---------------------
    let stable5pool_config_res: PoolTypeConfigResponse = app
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
        pool_configs[0].default_fee_info,
        stable5pool_config_res.default_fee_info
    );
    assert_eq!(
        pool_configs[0].allow_instantiation,
        stable5pool_config_res.allow_instantiation
    );

    // Check Weighted Config
    // ---------------------
    let weightedpool_config_res: PoolTypeConfigResponse = app
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
        pool_configs[1].default_fee_info,
        weightedpool_config_res.default_fee_info
    );
    assert_eq!(
        pool_configs[1].allow_instantiation,
        weightedpool_config_res.allow_instantiation
    );

    //// -----x----- Error :: PoolConfigDuplicate Error -----x----- ////

    let pool_configs = vec![
        PoolTypeConfig {
            code_id: stable5_pool_code_id,
            pool_type: PoolType::Stable5Pool {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            paused: PauseInfo::default(),
        },
        PoolTypeConfig {
            code_id: stable5_pool_code_id,
            pool_type: PoolType::Stable5Pool {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            paused: PauseInfo::default(),
        },
    ];

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: Some(token_code_id),
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        auto_stake_impl: AutoStakeImpl::None,
        pool_creation_fee: PoolCreationFee::default(),
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
        code_id: stable5_pool_code_id,
        pool_type: PoolType::Stable5Pool {},
        default_fee_info: FeeInfo {
            total_fee_bps: 30000u16,
            protocol_fee_percent: 49u16,
        },
        allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
        paused: PauseInfo::default(),
    }];

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: Some(token_code_id),
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        auto_stake_impl: dexter::vault::AutoStakeImpl::None,
        pool_creation_fee: PoolCreationFee::Disabled,
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
    let stable5_pool_code_id = store_stable5_pool_code(&mut app);
    let token_code_id = store_token_code(&mut app);

    let pool_configs = vec![PoolTypeConfig {
        code_id: stable5_pool_code_id,
        pool_type: PoolType::Stable5Pool {},
        default_fee_info: FeeInfo {
            total_fee_bps: 300u16,
            protocol_fee_percent: 49u16,
        },
        allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
        paused: PauseInfo::default(),
    }];

    //// -----x----- Success :: Initialize Vault Contract -----x----- ////

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: Some(token_code_id),
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        auto_stake_impl: AutoStakeImpl::None,
        pool_creation_fee: PoolCreationFee::Disabled,
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
        pool_type: PoolType::Stable5Pool {},
    };
    let registery_res: PoolTypeConfigResponse =
        app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();

    assert!(registery_res.is_some());
    let pool_config_res = registery_res.unwrap();

    assert_eq!(stable5_pool_code_id, pool_config_res.code_id);
    assert_eq!(PoolType::Stable5Pool {}, pool_config_res.pool_type);
    assert_eq!(
        pool_configs[0].default_fee_info,
        pool_config_res.default_fee_info
    );
    assert_eq!(
        pool_configs[0].allow_instantiation,
        pool_config_res.allow_instantiation
    );

    //// -----x----- Error :: Only Owner can add new PoolType to registery || Pool Type already exists -----x----- ////

    let msg = ExecuteMsg::AddToRegistry {
        new_pool_type_config: PoolTypeConfig {
            code_id: stable5_pool_code_id,
            pool_type: PoolType::Stable5Pool {},
            default_fee_info: FeeInfo {
                total_fee_bps: 10_0u16,
                protocol_fee_percent: 49u16,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
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
            code_id: stable5_pool_code_id,
            pool_type: PoolType::Weighted {},
            default_fee_info: FeeInfo {
                total_fee_bps: 10_001u16,
                protocol_fee_percent: 49u16,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
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
    let weighted_pool_code_id = 2u64;
    let msg = ExecuteMsg::AddToRegistry {
        new_pool_type_config: PoolTypeConfig {
            code_id: weighted_pool_code_id,
            pool_type: PoolType::Weighted {},
            default_fee_info: FeeInfo {
                total_fee_bps: 1_000u16,
                protocol_fee_percent: 49u16,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
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
        pool_type: PoolType::Weighted {},
    };
    let registery_res: PoolTypeConfigResponse =
        app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();

    assert!(registery_res.is_some());
    let pool_config_res = registery_res.unwrap();
    assert_eq!(weighted_pool_code_id, pool_config_res.code_id);
    assert_eq!(PoolType::Weighted {}, pool_config_res.pool_type);
    assert_eq!(
        FeeInfo {
            total_fee_bps: 1_000u16,
            protocol_fee_percent: 49u16,
        },
        pool_config_res.default_fee_info
    );
    assert_eq!(
        dexter::vault::AllowPoolInstantiation::Everyone,
        pool_config_res.allow_instantiation
    );
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
        None,
        after_init_config_res.fee_collector
    );
    assert_eq!(AutoStakeImpl::None, after_init_config_res.auto_stake_impl);
    assert_eq!(PauseInfo::default(), after_init_config_res.paused);

    //// -----x----- Success :: update config -----x----- ////

    let pause_info = PauseInfo{
        swap: true,
        deposit: false,
        imbalanced_withdraw: false
    };

    let msg = ExecuteMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: Some("fee_address".to_string()),
        auto_stake_impl: Some(AutoStakeImpl::Multistaking { contract_addr: Addr::unchecked("multistaking_address") }),
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
        AutoStakeImpl::Multistaking { contract_addr: Addr::unchecked("multistaking_address") },
        config_res.auto_stake_impl
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
    let vault_instance = instantiate_contract(&mut app, &owner_addr);

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
        pool_type: PoolType::Stable5Pool {},
        asset_infos: asset_infos.to_vec(),
        native_asset_precisions: vec![],
        init_params: Some(to_binary(&StablePoolParams { 
            amp: 100u64,
            supports_scaling_factors_update: false,
            scaling_factor_manager: None,
            scaling_factors: vec![],
        }).unwrap()),
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

    assert_eq!(res.events[1].attributes[2], attr("pool_type", "stable-5-pool"));

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
        imbalanced_withdraw: false
    };

    // update config for this pool now
    let msg = ExecuteMsg::UpdatePoolConfig {
        pool_id: Uint128::from(pool_id),
        fee_info: Some(FeeInfo {
            total_fee_bps: 400u16,
            protocol_fee_percent: 40u16,
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

    // Create stable5 pool
    let (_, _, stable5_pool_id) = initialize_stable_5_pool_2_asset(
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
    let res: PoolTypeConfigResponse = app.wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::QueryRegistry {pool_type: PoolType::Stable5Pool {}}
        ).unwrap();
    assert_eq!(res.unwrap().paused, PauseInfo::default());

    // pool id config
    let res: PoolInfoResponse = app.wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::GetPoolById {pool_id: stable5_pool_id }
        ).unwrap();
    assert_eq!(res.paused, PauseInfo::default());

    // update the pause info for type
    let expected_pause_info = PauseInfo { deposit: true, swap: false, imbalanced_withdraw: false };
    app.execute_contract(
        user_addr.clone(),
        vault_instance.clone(),
        &ExecuteMsg::UpdatePauseInfo {
            update_type: PauseInfoUpdateType::PoolType(PoolType::Stable5Pool {}),
            pause_info: expected_pause_info.clone(),
        },
        &[],
    ).unwrap();

    // assert the pause status via queries after updating only for pool type

    // pool type config
    let res: PoolTypeConfigResponse = app.wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::QueryRegistry {pool_type: PoolType::Stable5Pool {}}
        ).unwrap();
    assert_eq!(res.unwrap().paused, expected_pause_info.clone());

    // pool id config
    let res: PoolInfoResponse = app.wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::GetPoolById {pool_id: stable5_pool_id }
        ).unwrap();
    assert_eq!(res.paused, PauseInfo::default());

    // update the pause info for id
    app.execute_contract(
        user_addr.clone(),
        vault_instance.clone(),
        &ExecuteMsg::UpdatePauseInfo {
            update_type: PauseInfoUpdateType::PoolId(stable5_pool_id),
            pause_info: expected_pause_info.clone(),
        },
        &[],
    ).unwrap();

    // assert the pause status for pool id as well
    let res: PoolInfoResponse = app.wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::GetPoolById {pool_id: stable5_pool_id }
        ).unwrap();
    assert_eq!(res.paused, expected_pause_info);

    // trying to update the pause info from a non-whitelisted address should fail
    let res = app.execute_contract(
        Addr::unchecked("non-whitelisted-addr"),
        vault_instance.clone(),
        &ExecuteMsg::UpdatePauseInfo {
            update_type: PauseInfoUpdateType::PoolId(stable5_pool_id),
            pause_info: expected_pause_info.clone(),
        },
        &[],
    );
    assert_eq!(res.is_err(), true);
    assert_eq!(res.unwrap_err().root_cause().to_string(), "Unauthorized");
}
