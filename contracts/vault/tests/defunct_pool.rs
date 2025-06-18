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

    // Join the pool to create some LP tokens
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
    assert!(result.is_ok(), "Should be able to join pool before defuncting");

    // Execute defunct pool
    let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
    
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