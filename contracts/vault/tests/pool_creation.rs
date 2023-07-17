pub mod utils;

use std::vec;

use cosmwasm_std::{attr, coin, Addr, Coin, Uint128, to_binary, Decimal};
use cw20::MinterResponse;
use cw_multi_test::Executor;
use dexter::asset::{Asset, AssetInfo};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;

use dexter::vault::{AllowPoolInstantiation, ExecuteMsg, PoolInfo, PoolType, QueryMsg, PoolCreationFee};
use stable_pool::state::StablePoolParams;

use crate::utils::{dummy_pool_creation_msg, instantiate_contract, mock_app, store_token_code};

#[test]
fn test_create_pool_instance() {
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
        pool_type: PoolType::StableSwap {},
        asset_infos: asset_infos.to_vec(),
        native_asset_precisions: vec![],
        init_params: Some(to_binary(&StablePoolParams {
            amp: 100u64,
            scaling_factor_manager: None,
            supports_scaling_factors_update: false,
            scaling_factors: vec![],
            max_allowed_spread: Decimal::from_ratio(50u64, 100u64)
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

    assert_eq!(res.events[1].attributes[2], attr("pool_type", "stable-swap"));

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
        Addr::unchecked("contract3".to_string()),
        pool_res.lp_token_addr
    );
    assert_eq!(
        Addr::unchecked("contract4".to_string()),
        pool_res.pool_addr
    );
    assert_eq!(assets, pool_res.assets);
    assert_eq!(PoolType::StableSwap {}, pool_res.pool_type);
}

#[test]
fn test_pool_creation_whitelist() {
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
        &[coin(300_000_000u128, "uxprt")],
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

    // Set a pool creation fee
    let msg = ExecuteMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: Some("fee_collector".to_string()),
        pool_creation_fee: Some(PoolCreationFee::Enabled {
            fee: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uxprt".to_string(),
                },
                amount: Uint128::from(100_000_000u128),
            }
        }),
        auto_stake_impl: None,
        paused: None,
    };

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    // Pool creation allowed to anyone scenario
    let msg = dummy_pool_creation_msg(&asset_infos);
    let res = app.execute_contract(
        user_addr.clone(),
        vault_instance.clone(),
        &msg,
        &[coin(100_000_000u128, "uxprt")],
    );


    assert!(res.is_ok());

    // disable pool creation for everyone
    let msg = ExecuteMsg::UpdatePoolTypeConfig {
        pool_type: PoolType::StableSwap {},
        allow_instantiation: Some(AllowPoolInstantiation::Nobody),
        new_fee_info: None,
        paused: None,
    };

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    // try creating another pool
    let msg = dummy_pool_creation_msg(&asset_infos);
    let res = app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[coin(100_000_000u128, "uxprt")],
    );

    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().root_cause().to_string(),
        "Creation of this pool type is disabled"
    );

    // enable pool creation for only whitelisted addresses
    let msg = ExecuteMsg::UpdatePoolTypeConfig {
        pool_type: PoolType::StableSwap {},
        allow_instantiation: Some(AllowPoolInstantiation::OnlyGovernance),
        new_fee_info: None,
        paused: None,
    };

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    // Pool instantiation from admin
    let msg = dummy_pool_creation_msg(&asset_infos);
    let res = app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[coin(100_000_000u128, "uxprt")],
    );

    // Pool instantiation from admin should work regardless of empty whitelist
    assert!(res.is_ok());

    // Pool instantiation from non-admin non-whitelisted address
    let msg = dummy_pool_creation_msg(&asset_infos);
    let res = app.execute_contract(
        user_addr.clone(),
        vault_instance.clone(),
        &msg,
        &[coin(100_000_000u128, "uxprt")],
    );

    assert!(res.is_err());
    assert_eq!(res.unwrap_err().root_cause().to_string(), "Unauthorized");

    // Add user to whitelist
    let msg = ExecuteMsg::AddAddressToWhitelist {
        address: user_addr.to_string(),
    };
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    // Pool instantiation from non-admin whitelisted address
    let msg = dummy_pool_creation_msg(&asset_infos);
    let res = app.execute_contract(
        user_addr.clone(),
        vault_instance.clone(),
        &msg,
        &[coin(100_000_000u128, "uxprt")],
    );

    assert!(res.is_ok());

    // Remove user from whitelist and test again
    let msg = ExecuteMsg::RemoveAddressFromWhitelist {
        address: user_addr.to_string(),
    };
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    // Pool instantiation from non-admin non-whitelisted address
    let msg = dummy_pool_creation_msg(&asset_infos);
    let res = app.execute_contract(
        user_addr.clone(),
        vault_instance.clone(),
        &msg,
        &[coin(100_000_000u128, "uxprt")],
    );

    assert!(res.is_err());
    assert_eq!(res.unwrap_err().root_cause().to_string(), "Unauthorized");
}

#[test]
fn test_pool_creation_fee() {
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

    // No pool creation fee
    let msg = ExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::StableSwap {},
        asset_infos: asset_infos.to_vec(),
        native_asset_precisions: vec![],
        init_params: Some(to_binary(&StablePoolParams { 
            amp: 100u64,
            scaling_factor_manager: None,
            supports_scaling_factors_update: false,
            scaling_factors: vec![],
            max_allowed_spread: Decimal::from_ratio(50u128, 100u128)
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

    assert_eq!(res.events[1].attributes[2], attr("pool_type", "stable-swap"));

    // Add fee for pool creation
    let msg = ExecuteMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: None,
        pool_creation_fee: Some(PoolCreationFee::Enabled {
            fee: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uxprt".to_string(),
                },
                amount: Uint128::from(100_000_000u128),
            }
        }),
        auto_stake_impl: None,
        paused: None,
    };

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    // try creating another pool without passing fee
    let msg = dummy_pool_creation_msg(&asset_infos);
    let res = app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    );

    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().root_cause().to_string(),
        "Insufficient number of uxprt tokens sent. Tokens sent = 0. Tokens needed = 100000000"
    );

    // try creating another pool with passing fee
    let msg = dummy_pool_creation_msg(&asset_infos);
    let res = app
        .execute_contract(
            Addr::unchecked(owner.clone()),
            vault_instance.clone(),
            &msg,
            &[coin(100_000_000u128, "uxprt")],
        );
    
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().root_cause().to_string(),
        "Fee collector address is not configured"
    );

    // set fee collector
    let msg = ExecuteMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: Some("fee_collector".to_string()),
        pool_creation_fee: None,
        auto_stake_impl: None,
        paused: None,
    };

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    ).unwrap();

    // try creating another pool with passing fee
    let msg = dummy_pool_creation_msg(&asset_infos);
    let res = app
        .execute_contract(
            Addr::unchecked(owner.clone()),
            vault_instance.clone(),
            &msg,
            &[coin(100_000_000u128, "uxprt")],
        ).unwrap();

    assert_eq!(res.events[1].attributes[2], attr("pool_type", "stable-swap"));
    
    // validate that fee collector has received the fee
    let fee_collector = Addr::unchecked("fee_collector".to_string());
    let res = app
        .wrap()
        .query_balance(fee_collector.clone(), "uxprt")
        .unwrap();

    assert_eq!(res.amount, Uint128::from(100_000_000u128));
}
