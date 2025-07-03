use cosmwasm_std::{coins, Addr, Uint128};
use cw_multi_test::Executor;
use dexter::asset::{Asset, AssetInfo};
use dexter::vault::{DefunctPoolInfo, ExecuteMsg, QueryMsg};

pub mod utils;

#[test]
fn test_defunct_check_with_active_pool() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), vec![
        cosmwasm_std::Coin {
            denom: "denom1".to_string(),
            amount: Uint128::from(100_000_000_000u128),
        },
        cosmwasm_std::Coin {
            denom: "denom2".to_string(),
            amount: Uint128::from(100_000_000_000u128),
        },
        cosmwasm_std::Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100_000_000_000u128),
        },
    ]);
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize the token contracts first
    let (token1, token2, token3) = utils::initialize_3_tokens(&mut app, &owner);

    // Mint tokens and set allowances
    utils::mint_some_tokens(
        &mut app,
        owner.clone(),
        token1.clone(),
        Uint128::from(10000000_000000u128),
        owner.to_string(),
    );
    utils::mint_some_tokens(
        &mut app,
        owner.clone(),
        token2.clone(),
        Uint128::from(10000000_000000u128),
        owner.to_string(),
    );
    utils::mint_some_tokens(
        &mut app,
        owner.clone(),
        token3.clone(),
        Uint128::from(10000000_000000u128),
        owner.to_string(),
    );

    utils::increase_token_allowance(
        &mut app,
        owner.clone(),
        token1.clone(),
        vault_instance.to_string(),
        Uint128::from(10000000_000000u128),
    );
    utils::increase_token_allowance(
        &mut app,
        owner.clone(),
        token2.clone(),
        vault_instance.to_string(),
        Uint128::from(10000000_000000u128),
    );
    utils::increase_token_allowance(
        &mut app,
        owner.clone(),
        token3.clone(),
        vault_instance.to_string(),
        Uint128::from(10000000_000000u128),
    );

    let (_, _lp_token_instance, pool_id) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1.clone(),
        token2.clone(),
        token3.clone(),
        "denom1".to_string(),
        "denom2".to_string(),
    );

    // Try to join an active (non-defunct) pool - should succeed
    // The weighted pool has 5 assets in this order: denom1, denom2, token2, token1, token3
    let join_msg = ExecuteMsg::JoinPool {
        pool_id,
        recipient: None,
        assets: Some(vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "denom1".to_string(),
                },
                amount: Uint128::from(1000u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "denom2".to_string(),
                },
                amount: Uint128::from(1000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token2.clone(),
                },
                amount: Uint128::from(1000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token1.clone(),
                },
                amount: Uint128::from(1000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token3.clone(),
                },
                amount: Uint128::from(1000u128),
            },
        ]),
        min_lp_to_receive: None,
        auto_stake: None,
    };

    // This should NOT fail because pool is active
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &join_msg, &[
        cosmwasm_std::Coin {
            denom: "denom1".to_string(),
            amount: Uint128::from(1000u128),
        },
        cosmwasm_std::Coin {
            denom: "denom2".to_string(),
            amount: Uint128::from(1000u128),
        },
    ]);
    assert!(result.is_ok(), "Join pool should succeed on active pool");
}

#[test]
fn test_defunct_check_with_defunct_pool() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize the token contracts first
    let (token1, token2, token3) = utils::initialize_3_tokens(&mut app, &owner);

    let (_, _lp_token_instance, pool_id) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1,
        token2,
        token3,
        "denom1".to_string(),
        "denom2".to_string(),
    );

    // First, make the pool defunct
    let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
    assert!(result.is_ok(), "Defunct pool should succeed");

    // Now try to join the defunct pool - should fail
    let join_msg = ExecuteMsg::JoinPool {
        pool_id,
        recipient: None,
        assets: Some(vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "denom1".to_string(),
                },
                amount: Uint128::from(1000u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "denom2".to_string(),
                },
                amount: Uint128::from(1000u128),
            },
        ]),
        min_lp_to_receive: None,
        auto_stake: None,
    };

    // This SHOULD fail because pool is defunct
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &join_msg, &coins(2000u128, "uusd"));
    assert!(result.is_err(), "Join pool should fail on defunct pool");
    
    // Verify it's the correct error
    let error_msg = result.unwrap_err().root_cause().to_string();
    assert!(error_msg.contains("Pool is already defunct") || error_msg.contains("PoolAlreadyDefunct") || error_msg.contains("pool already defunct"));
}

#[test]
fn test_execute_defunct_pool_successful() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), vec![
        cosmwasm_std::Coin {
            denom: "denom1".to_string(),
            amount: Uint128::from(100_000_000_000u128),
        },
        cosmwasm_std::Coin {
            denom: "denom2".to_string(),
            amount: Uint128::from(100_000_000_000u128),
        },
        cosmwasm_std::Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100_000_000_000u128),
        },
    ]);
    let vault_instance = utils::instantiate_contract(&mut app, &owner);
    let multistaking_instance = utils::initialize_multistaking_contract(&mut app, &owner);

    // Add multistaking contract to vault config
    let update_msg = ExecuteMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: None,
        pool_creation_fee: None,
        auto_stake_impl: Some(dexter::vault::AutoStakeImpl::Multistaking {
            contract_addr: multistaking_instance.clone(),
        }),
        paused: None,
    };
    app.execute_contract(owner.clone(), vault_instance.clone(), &update_msg, &[]).unwrap();

    // add validation assets
    let update_msg = ExecuteMsg::UpdateRewardScheduleValidationAssets {
        assets: vec![
            AssetInfo::NativeToken { denom: "uxprt".to_string() },
        ],
    };
    app.execute_contract(owner.clone(), vault_instance.clone(), &update_msg, &[]).unwrap();

    // Initialize the token contracts first
    let (token1, token2, token3) = utils::initialize_3_tokens(&mut app, &owner);

    // Mint tokens and set allowances
    utils::mint_some_tokens(
        &mut app,
        owner.clone(),
        token1.clone(),
        Uint128::from(10000000_000000u128),
        owner.to_string(),
    );
    utils::mint_some_tokens(
        &mut app,
        owner.clone(),
        token2.clone(),
        Uint128::from(10000000_000000u128),
        owner.to_string(),
    );
    utils::mint_some_tokens(
        &mut app,
        owner.clone(),
        token3.clone(),
        Uint128::from(10000000_000000u128),
        owner.to_string(),
    );

    utils::increase_token_allowance(
        &mut app,
        owner.clone(),
        token1.clone(),
        vault_instance.to_string(),
        Uint128::from(10000000_000000u128),
    );
    utils::increase_token_allowance(
        &mut app,
        owner.clone(),
        token2.clone(),
        vault_instance.to_string(),
        Uint128::from(10000000_000000u128),
    );
    utils::increase_token_allowance(
        &mut app,
        owner.clone(),
        token3.clone(),
        vault_instance.to_string(),
        Uint128::from(10000000_000000u128),
    );

    let (_pool_addr, lp_token_instance, pool_id) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1.clone(),
        token2.clone(),
        token3.clone(),
        "denom1".to_string(),
        "denom2".to_string(),
    );

    // Allow LP token in multistaking contract
    let allow_lp_msg = dexter::multi_staking::ExecuteMsg::AllowLpToken {
        lp_token: lp_token_instance.clone(),
    };
    app.execute_contract(
        owner.clone(),
        multistaking_instance.clone(),
        &allow_lp_msg,
        &[],
    )
    .unwrap();

    // User joins pool and gets LP tokens, then auto-stakes them
    let user = Addr::unchecked("user");
    app.send_tokens(owner.clone(), user.clone(), &coins(10000, "denom1")).unwrap();
    app.send_tokens(owner.clone(), user.clone(), &coins(10000, "denom2")).unwrap();

    // Mint tokens to the user
    utils::mint_some_tokens(&mut app, owner.clone(), token1.clone(), Uint128::from(100u128), user.to_string());
    utils::mint_some_tokens(&mut app, owner.clone(), token2.clone(), Uint128::from(100u128), user.to_string());
    utils::mint_some_tokens(&mut app, owner.clone(), token3.clone(), Uint128::from(100u128), user.to_string());

    // Grant allowance to the vault
    utils::increase_token_allowance(&mut app, user.clone(), token1.clone(), vault_instance.to_string(), Uint128::from(100u128));
    utils::increase_token_allowance(&mut app, user.clone(), token2.clone(), vault_instance.to_string(), Uint128::from(100u128));
    utils::increase_token_allowance(&mut app, user.clone(), token3.clone(), vault_instance.to_string(), Uint128::from(100u128));
    
    let join_msg = ExecuteMsg::JoinPool {
        pool_id,
        recipient: Some(user.to_string()),
        assets: Some(vec![
            Asset { info: AssetInfo::NativeToken { denom: "denom1".to_string() }, amount: Uint128::from(100u128) },
            Asset { info: AssetInfo::NativeToken { denom: "denom2".to_string() }, amount: Uint128::from(100u128) },
            Asset { info: AssetInfo::Token { contract_addr: token1.clone() }, amount: Uint128::from(100u128) },
            Asset { info: AssetInfo::Token { contract_addr: token2.clone() }, amount: Uint128::from(100u128) },
            Asset { info: AssetInfo::Token { contract_addr: token3.clone() }, amount: Uint128::from(100u128) },
        ]),
        min_lp_to_receive: None,
        auto_stake: Some(true),
    };
    
    app.execute_contract(user.clone(), vault_instance.clone(), &join_msg, &[
        cosmwasm_std::Coin {
            denom: "denom1".to_string(),
            amount: Uint128::from(100u128),
        },
        cosmwasm_std::Coin {
            denom: "denom2".to_string(),
            amount: Uint128::from(100u128),
        },
    ]).unwrap();

    // Verify user's LP tokens are bonded in multistaking
    let bonded_balance: Uint128 = app.wrap().query_wasm_smart(
        multistaking_instance.clone(),
        &dexter::multi_staking::QueryMsg::BondedLpTokens {
            lp_token: lp_token_instance.clone(),
            user: user.clone(),
        }
    ).unwrap();
    assert!(!bonded_balance.is_zero(), "User should have bonded LP tokens");

    // Execute defunct pool
    let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
    
    println!("result: {:?}", result);
    assert!(result.is_ok(), "Defunct pool should succeed");

    // Verify pool is in defunct state
    let query_msg = QueryMsg::GetDefunctPoolInfo { pool_id };
    let defunct_info: Option<DefunctPoolInfo> = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &query_msg)
        .unwrap();
    
    assert!(defunct_info.is_some(), "Pool should be in defunct state");
    
    let defunct_info = defunct_info.unwrap();
    assert_eq!(defunct_info.pool_id, pool_id);
    assert_eq!(defunct_info.lp_token_addr, lp_token_instance);
    assert!(!defunct_info.total_lp_supply_at_defunct.is_zero(), "Should have captured LP supply");
    assert!(!defunct_info.total_assets_at_defunct.is_empty(), "Should have captured assets");
}

#[test]
fn test_execute_defunct_pool_unauthorized() {
    let owner = Addr::unchecked("owner");
    let unauthorized = Addr::unchecked("hacker");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize the token contracts first
    let (token1, token2, token3) = utils::initialize_3_tokens(&mut app, &owner);

    let (_, _, pool_id) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1,
        token2,
        token3,
        "denom1".to_string(),
        "denom2".to_string(),
    );

    // Try to defunct pool with unauthorized user
    let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
    let result = app.execute_contract(unauthorized, vault_instance.clone(), &defunct_msg, &[]);
    
    assert!(result.is_err(), "Defunct pool should fail for unauthorized user");
    
    let error_msg = result.unwrap_err().root_cause().to_string();
    assert!(error_msg.contains("Unauthorized"));
}

#[test]
fn test_execute_defunct_pool_nonexistent() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Try to defunct a non-existent pool
    let nonexistent_pool_id = Uint128::from(999u128);
    let defunct_msg = ExecuteMsg::DefunctPool { pool_id: nonexistent_pool_id };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
    
    assert!(result.is_err(), "Defunct pool should fail for non-existent pool");
    
    let error_msg = result.unwrap_err().root_cause().to_string();
    assert!(error_msg.contains("Invalid PoolId") || error_msg.contains("InvalidPoolId") || error_msg.contains("pool not found"));
}

#[test]
fn test_execute_defunct_pool_already_defunct() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize the token contracts first
    let (token1, token2, token3) = utils::initialize_3_tokens(&mut app, &owner);

    let (_, _, pool_id) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1,
        token2,
        token3,
        "denom1".to_string(),
        "denom2".to_string(),
    );

    // Make pool defunct first time
    let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
    assert!(result.is_ok(), "First defunct should succeed");

    // Try to make it defunct again - should fail
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
    assert!(result.is_err(), "Second defunct should fail");
    
    let error_msg = result.unwrap_err().root_cause().to_string();
    assert!(error_msg.contains("Pool is already defunct") || error_msg.contains("PoolAlreadyDefunct") || error_msg.contains("pool already defunct"));
}

#[test]
fn test_operations_on_defunct_pool_join() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize the token contracts first
    let (token1, token2, token3) = utils::initialize_3_tokens(&mut app, &owner);

    let (_, _, pool_id) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1,
        token2,
        token3,
        "denom1".to_string(),
        "denom2".to_string(),
    );

    // Make pool defunct
    let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
    assert!(result.is_ok(), "Defunct pool should succeed");

    // Try to join defunct pool - should fail
    let join_msg = ExecuteMsg::JoinPool {
        pool_id,
        recipient: None,
        assets: Some(vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "denom1".to_string(),
                },
                amount: Uint128::from(1000u128),
            },
        ]),
        min_lp_to_receive: None,
        auto_stake: None,
    };

    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &join_msg, &coins(1000u128, "uusd"));
    assert!(result.is_err(), "Join should fail on defunct pool");
    
    let error_msg = result.unwrap_err().root_cause().to_string();
    assert!(error_msg.contains("Pool is already defunct") || error_msg.contains("PoolAlreadyDefunct") || error_msg.contains("pool already defunct"));
}

#[test]
fn test_operations_on_defunct_pool_swap() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), vec![
        cosmwasm_std::Coin {
            denom: "denom1".to_string(),
            amount: Uint128::from(100_000_000_000u128),
        },
        cosmwasm_std::Coin {
            denom: "denom2".to_string(),
            amount: Uint128::from(100_000_000_000u128),
        },
        cosmwasm_std::Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100_000_000_000u128),
        },
    ]);
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize the token contracts first
    let (token1, token2, token3) = utils::initialize_3_tokens(&mut app, &owner);

    let (_, _, pool_id) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1,
        token2,
        token3,
        "denom1".to_string(),
        "denom2".to_string(),
    );

    // Make pool defunct
    let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
    assert!(result.is_ok(), "Defunct pool should succeed");

    // Try to swap in defunct pool - should fail
    let swap_msg = ExecuteMsg::Swap {
        swap_request: dexter::vault::SingleSwapRequest {
            pool_id,
            swap_type: dexter::vault::SwapType::GiveIn {},
            asset_in: AssetInfo::NativeToken {
                denom: "denom1".to_string(),
            },
            asset_out: AssetInfo::NativeToken {
                denom: "denom2".to_string(),
            },
            amount: Uint128::from(100u128),
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };

    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &swap_msg, &coins(100u128, "denom1"));
    assert!(result.is_err(), "Swap should fail on defunct pool");
    
    let error_msg = result.unwrap_err().root_cause().to_string();
    assert!(error_msg.contains("Pool is already defunct") || error_msg.contains("PoolAlreadyDefunct") || error_msg.contains("pool already defunct"));
}

#[test]
fn test_query_defunct_pool_info_existing() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize the token contracts first
    let (token1, token2, token3) = utils::initialize_3_tokens(&mut app, &owner);

    let (_, lp_token_instance, pool_id) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1,
        token2,
        token3,
        "denom1".to_string(),
        "denom2".to_string(),
    );

    // Make pool defunct
    let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
    assert!(result.is_ok(), "Defunct pool should succeed");

    // Query defunct pool info
    let query_msg = QueryMsg::GetDefunctPoolInfo { pool_id };
    let defunct_info: Option<DefunctPoolInfo> = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &query_msg)
        .unwrap();
    
    assert!(defunct_info.is_some(), "Should return defunct pool info");
    
    let defunct_info = defunct_info.unwrap();
    assert_eq!(defunct_info.pool_id, pool_id);
    assert_eq!(defunct_info.lp_token_addr, lp_token_instance);
}

#[test]
fn test_query_defunct_pool_info_nonexistent() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Query defunct pool info for non-existent pool
    let query_msg = QueryMsg::GetDefunctPoolInfo { 
        pool_id: Uint128::from(999u128) 
    };
    let defunct_info: Option<DefunctPoolInfo> = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &query_msg)
        .unwrap();
    
    assert!(defunct_info.is_none(), "Should return None for non-existent defunct pool");
}

#[test]
fn test_query_is_user_refunded_false() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize the token contracts first
    let (token1, token2, token3) = utils::initialize_3_tokens(&mut app, &owner);

    let (_, _, pool_id) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1,
        token2,
        token3,
        "denom1".to_string(),
        "denom2".to_string(),
    );

    // Query user refund status (should be false by default)
    let query_msg = QueryMsg::IsUserRefunded { 
        pool_id,
        user: owner.to_string(),
    };
    let is_refunded: bool = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &query_msg)
        .unwrap();
    
    assert!(!is_refunded, "User should not be refunded initially");
}

#[test]
fn test_process_refund_batch_successful() {
    let owner = Addr::unchecked("owner");
    let user1 = Addr::unchecked("user1");
    let user2 = Addr::unchecked("user2");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize the token contracts first
    let (token1, token2, token3) = utils::initialize_3_tokens(&mut app, &owner);

    let (_, _, pool_id) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1,
        token2,
        token3,
        "denom1".to_string(),
        "denom2".to_string(),
    );

    // Make pool defunct first
    let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
    assert!(result.is_ok(), "Defunct pool should succeed");

    // Process refund batch
    let refund_msg = ExecuteMsg::ProcessRefundBatch {
        pool_id,
        user_addresses: vec![user1.to_string(), user2.to_string()],
    };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &refund_msg, &[]);
    assert!(result.is_ok(), "Process refund batch should succeed");
}

#[test]
fn test_process_refund_batch_unauthorized() {
    let owner = Addr::unchecked("owner");
    let unauthorized = Addr::unchecked("hacker");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize the token contracts first
    let (token1, token2, token3) = utils::initialize_3_tokens(&mut app, &owner);

    let (_, _, pool_id) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1,
        token2,
        token3,
        "denom1".to_string(),
        "denom2".to_string(),
    );

    // Make pool defunct first
    let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
    assert!(result.is_ok(), "Defunct pool should succeed");

    // Try to process refund batch with unauthorized user
    let refund_msg = ExecuteMsg::ProcessRefundBatch {
        pool_id,
        user_addresses: vec!["user1".to_string()],
    };
    let result = app.execute_contract(unauthorized, vault_instance.clone(), &refund_msg, &[]);
    assert!(result.is_err(), "Process refund batch should fail for unauthorized user");
    
    let error_msg = result.unwrap_err().root_cause().to_string();
    assert!(error_msg.contains("Unauthorized"));
}

#[test]
fn test_process_refund_batch_non_defunct_pool() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize the token contracts first
    let (token1, token2, token3) = utils::initialize_3_tokens(&mut app, &owner);

    let (_, _, pool_id) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1,
        token2,
        token3,
        "denom1".to_string(),
        "denom2".to_string(),
    );

    // Try to process refund batch on active (non-defunct) pool
    let refund_msg = ExecuteMsg::ProcessRefundBatch {
        pool_id,
        user_addresses: vec!["user1".to_string()],
    };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &refund_msg, &[]);
    assert!(result.is_err(), "Process refund batch should fail for non-defunct pool");
    
    let error_msg = result.unwrap_err().root_cause().to_string();
    assert!(error_msg.contains("Pool is not defunct") || error_msg.contains("PoolNotDefunct") || error_msg.contains("pool not defunct"));
}

#[test]
fn test_defunct_pool_succeeds_without_multistaking() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), vec![
        cosmwasm_std::Coin {
            denom: "uxprt".to_string(),
            amount: Uint128::from(100_000_000_000u128),
        },
        cosmwasm_std::Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100_000_000_000u128),
        },
    ]);
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize the token contracts first
    let (token1, token2, token3) = utils::initialize_3_tokens(&mut app, &owner);

    let (_, _, pool_id) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1,
        token2,
        token3,
        "denom1".to_string(),
        "denom2".to_string(),
    );

    // Mock a situation where there might be active reward schedules
    // Note: This test will pass because our validation only checks common assets
    // and the test environment doesn't have multistaking enabled by default
    // In a real environment with multistaking and active reward schedules,
    // this would fail with PoolHasActiveRewardSchedules error

    let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
    
        // This should succeed because there are no active reward schedules in our test environment
    assert!(result.is_ok(), "Defunct pool should succeed when no active reward schedules exist");
}

#[test]
fn test_defunct_pool_with_active_reward_schedules_fails() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(
        owner.clone(),
        vec![
            cosmwasm_std::Coin {
                denom: "uxprt".to_string(),
                amount: Uint128::from(100_000_000_000u128),
            },
            cosmwasm_std::Coin {
                denom: "denom1".to_string(),
                amount: Uint128::from(100_000_000_000u128),
            },
            cosmwasm_std::Coin {
                denom: "denom2".to_string(),
                amount: Uint128::from(100_000_000_000u128),
            },
        ],
    );

    // Instantiate vault and multistaking contracts
    let vault_instance = utils::instantiate_contract(&mut app, &owner);
    let multistaking_instance = utils::initialize_multistaking_contract(&mut app, &owner);

    // Update vault config to use the multistaking contract for auto-staking
    let update_msg = ExecuteMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: None,
        pool_creation_fee: None,
        auto_stake_impl: Some(dexter::vault::AutoStakeImpl::Multistaking {
            contract_addr: multistaking_instance.clone(),
        }),
        paused: None,
    };
    app.execute_contract(owner.clone(), vault_instance.clone(), &update_msg, &[])
        .unwrap();

    // add validation assets
    let update_msg = ExecuteMsg::UpdateRewardScheduleValidationAssets {
        assets: vec![AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        }],
    };
    app.execute_contract(owner.clone(), vault_instance.clone(), &update_msg, &[])
        .unwrap();

    let (token1, token2, token3) = utils::initialize_3_tokens(&mut app, &owner);

    let (_, lp_token, pool_id) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1.clone(),
        token2.clone(),
        token3.clone(),
        "denom1".to_string(),
        "denom2".to_string(),
    );

    // Allow LP token in multistaking contract
    let allow_lp_msg = dexter::multi_staking::ExecuteMsg::AllowLpToken {
        lp_token: lp_token.clone(),
    };
    app.execute_contract(
        owner.clone(),
        multistaking_instance.clone(),
        &allow_lp_msg,
        &[],
    )
    .unwrap();

    // Create an active reward schedule. We create it in the future and then
    // advance the block time to make it active.
    let current_time = app.block_info().time.seconds();
    let create_schedule_msg = dexter::multi_staking::ExecuteMsg::CreateRewardSchedule {
        lp_token: lp_token.clone(),
        title: "Test Reward Schedule".to_string(),
        actual_creator: None,
        start_block_time: current_time + 1,
        end_block_time: current_time + 1000,
    };
    app.execute_contract(
        owner.clone(),
        multistaking_instance.clone(),
        &create_schedule_msg,
        &coins(1000, "uxprt"),
    )
    .unwrap();

    // Make the reward schedule active
    app.update_block(|block| {
        block.time = block.time.plus_seconds(1);
    });

    // Attempt to defunct the pool
    let defunct_msg = ExecuteMsg::DefunctPool {
        pool_id,
    };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);

    // Assert failure
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .root_cause()
        .to_string()
        .contains("Pool has active reward schedules"));
}

#[test]
fn test_defunct_pool_with_bonded_lp_tokens_refund() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(
        owner.clone(),
        vec![
            cosmwasm_std::Coin {
                denom: "denom1".to_string(),
                amount: Uint128::from(100_000_000_000u128),
            },
            cosmwasm_std::Coin {
                denom: "denom2".to_string(),
                amount: Uint128::from(100_000_000_000u128),
            },
            cosmwasm_std::Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(100_000_000_000u128),
            },
        ],
    );

    // Instantiate vault and multistaking contracts
    let vault_instance = utils::instantiate_contract(&mut app, &owner);
    let multistaking_instance = utils::initialize_multistaking_contract(&mut app, &owner);

    // Update vault config to use the multistaking contract for auto-staking
    let update_msg = ExecuteMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: None,
        pool_creation_fee: None,
        auto_stake_impl: Some(dexter::vault::AutoStakeImpl::Multistaking {
            contract_addr: multistaking_instance.clone(),
        }),
        paused: None,
    };
    app.execute_contract(owner.clone(), vault_instance.clone(), &update_msg, &[])
        .unwrap();

    // add validation assets
    let update_msg = ExecuteMsg::UpdateRewardScheduleValidationAssets {
        assets: vec![
            AssetInfo::NativeToken { denom: "uusd".to_string() },
        ],
    };
    app.execute_contract(owner.clone(), vault_instance.clone(), &update_msg, &[]).unwrap();

    let (token1, token2, token3) = utils::initialize_3_tokens(&mut app, &owner);

    let (_, lp_token, pool_id) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1.clone(),
        token2.clone(),
        token3.clone(),
        "denom1".to_string(),
        "denom2".to_string(),
    );

    // Allow LP token in multistaking contract
    let allow_lp_msg = dexter::multi_staking::ExecuteMsg::AllowLpToken {
        lp_token: lp_token.clone(),
    };
    app.execute_contract(
        owner.clone(),
        multistaking_instance.clone(),
        &allow_lp_msg,
        &[],
    )
    .unwrap();

    // User joins pool and gets LP tokens, then auto-stakes them
    let user = Addr::unchecked("user");
    app.send_tokens(owner.clone(), user.clone(), &coins(10000, "denom1")).unwrap();
    app.send_tokens(owner.clone(), user.clone(), &coins(10000, "denom2")).unwrap();

    // Mint tokens to the user
    utils::mint_some_tokens(&mut app, owner.clone(), token1.clone(), Uint128::from(100u128), user.to_string());
    utils::mint_some_tokens(&mut app, owner.clone(), token2.clone(), Uint128::from(100u128), user.to_string());
    utils::mint_some_tokens(&mut app, owner.clone(), token3.clone(), Uint128::from(100u128), user.to_string());

    // Grant allowance to the vault
    utils::increase_token_allowance(&mut app, user.clone(), token1.clone(), vault_instance.to_string(), Uint128::from(100u128));
    utils::increase_token_allowance(&mut app, user.clone(), token2.clone(), vault_instance.to_string(), Uint128::from(100u128));
    utils::increase_token_allowance(&mut app, user.clone(), token3.clone(), vault_instance.to_string(), Uint128::from(100u128));

    let join_msg = ExecuteMsg::JoinPool {
        pool_id,
        recipient: Some(user.to_string()),
        assets: Some(vec![
            Asset { info: AssetInfo::NativeToken { denom: "denom1".to_string() }, amount: Uint128::from(100u128) },
            Asset { info: AssetInfo::NativeToken { denom: "denom2".to_string() }, amount: Uint128::from(100u128) },
            Asset { info: AssetInfo::Token { contract_addr: token1.clone() }, amount: Uint128::from(100u128) },
            Asset { info: AssetInfo::Token { contract_addr: token2.clone() }, amount: Uint128::from(100u128) },
            Asset { info: AssetInfo::Token { contract_addr: token3.clone() }, amount: Uint128::from(100u128) },
        ]),
        min_lp_to_receive: None,
        auto_stake: Some(true),
    };

    app.execute_contract(user.clone(), vault_instance.clone(), &join_msg, &[
        cosmwasm_std::Coin {
            denom: "denom1".to_string(),
            amount: Uint128::from(100u128),
        },
        cosmwasm_std::Coin {
            denom: "denom2".to_string(),
            amount: Uint128::from(100u128),
        },
    ]).unwrap();

    // Verify user's LP tokens are bonded in multistaking
    let bonded_balance: Uint128 = app.wrap().query_wasm_smart(
        multistaking_instance.clone(),
        &dexter::multi_staking::QueryMsg::BondedLpTokens {
            lp_token: lp_token.clone(),
            user: user.clone(),
        }
    ).unwrap();
    assert!(!bonded_balance.is_zero(), "User should have bonded LP tokens");

    // Make the pool defunct
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &ExecuteMsg::DefunctPool { pool_id }, &[]);
    println!("result: {:?}", result);
    assert!(result.is_ok(), "Defunct pool should succeed");

    // Admin processes refund for the user
    let process_refund_msg = ExecuteMsg::ProcessRefundBatch {
        pool_id,
        user_addresses: vec![user.to_string()],
    };
    app.execute_contract(owner.clone(), vault_instance.clone(), &process_refund_msg, &[]).unwrap();

    // Verify user is marked as refunded
    let is_refunded: bool = app.wrap().query_wasm_smart(vault_instance.clone(), &QueryMsg::IsUserRefunded { pool_id, user: user.to_string() }).unwrap();
    assert!(is_refunded, "User should be marked as refunded");
}

