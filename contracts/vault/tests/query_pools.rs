use cosmwasm_std::{coins, Addr, Uint128};
use cw_multi_test::Executor;
use dexter::vault::{ExecuteMsg, PoolInfoResponse, QueryMsg};

pub mod utils;

#[test]
fn test_query_pools_basic() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initially, should return empty list
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert!(pools.is_empty(), "Should have no pools initially");

    // Initialize tokens
    let (token1, _token2, _token3) = utils::initialize_3_tokens(&mut app, &owner);

    // Create first pool (pool_id = 1)
    let (_pool_addr_1, _lp_token_1, pool_id_1) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1.clone(),
        _token2.clone(),
        _token3.clone(),
        "denom1".to_string(),
        "denom2".to_string(),
    );

    // Query pools - should return 1 pool
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(pools.len(), 1, "Should have 1 pool");
    assert_eq!(pools[0].pool_id, pool_id_1, "Pool ID should match");

    // Create second pool (pool_id = 2)
    let (_pool_addr_2, _lp_token_2, pool_id_2) = utils::initialize_stable_5_pool_2_asset(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1.clone(),
        "denom3".to_string(),
    );

    // Query pools - should return 2 pools
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(pools.len(), 2, "Should have 2 pools");
    assert_eq!(pools[0].pool_id, pool_id_1, "First pool ID should match");
    assert_eq!(pools[1].pool_id, pool_id_2, "Second pool ID should match");
}

#[test]
fn test_query_pools_with_pagination() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize tokens
    let (token1, _token2, _token3) = utils::initialize_3_tokens(&mut app, &owner);

    // Create 3 pools
    let (_pool_addr_1, _lp_token_1, pool_id_1) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1.clone(),
        _token2.clone(),
        _token3.clone(),
        "denom1".to_string(),
        "denom2".to_string(),
    );

    let (_pool_addr_2, _lp_token_2, pool_id_2) = utils::initialize_stable_5_pool_2_asset(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1.clone(),
        "denom3".to_string(),
    );

    let (_pool_addr_3, _lp_token_3, pool_id_3) = utils::initialize_stable_5_pool_2_asset(
        &mut app,
        &owner,
        vault_instance.clone(),
        _token2.clone(),
        "denom4".to_string(),
    );

    // Test limit = 2
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: None,
                limit: Some(2),
            },
        )
        .unwrap();
    assert_eq!(pools.len(), 2, "Should return 2 pools with limit=2");
    assert_eq!(pools[0].pool_id, pool_id_1);
    assert_eq!(pools[1].pool_id, pool_id_2);

    // Test pagination with start_after
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: Some(pool_id_1),
                limit: Some(2),
            },
        )
        .unwrap();
    assert_eq!(pools.len(), 2, "Should return 2 pools after pool_id_1");
    assert_eq!(pools[0].pool_id, pool_id_2);
    assert_eq!(pools[1].pool_id, pool_id_3);

    // Test start_after with remaining pools
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: Some(pool_id_2),
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(pools.len(), 1, "Should return 1 pool after pool_id_2");
    assert_eq!(pools[0].pool_id, pool_id_3);
}

#[test]
fn test_query_pools_with_defunct_pools() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize tokens
    let (token1, _token2, _token3) = utils::initialize_3_tokens(&mut app, &owner);

    // Create 4 pools
    let (_pool_addr_1, _lp_token_1, pool_id_1) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1.clone(),
        _token2.clone(),
        _token3.clone(),
        "denom1".to_string(),
        "denom2".to_string(),
    );

    let (_pool_addr_2, _lp_token_2, pool_id_2) = utils::initialize_stable_5_pool_2_asset(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1.clone(),
        "denom3".to_string(),
    );

    let (_pool_addr_3, _lp_token_3, pool_id_3) = utils::initialize_stable_5_pool_2_asset(
        &mut app,
        &owner,
        vault_instance.clone(),
        _token2.clone(),
        "denom4".to_string(),
    );

    let (_pool_addr_4, _lp_token_4, pool_id_4) = utils::initialize_stable_5_pool_2_asset(
        &mut app,
        &owner,
        vault_instance.clone(),
        _token3.clone(),
        "denom5".to_string(),
    );

    // Verify all 4 pools are active
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(pools.len(), 4, "Should have 4 active pools");

    // Make pool_id_2 defunct
    let defunct_msg = ExecuteMsg::DefunctPool { pool_id: pool_id_2 };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
    assert!(result.is_ok(), "Should successfully make pool defunct");

    // Query pools again - should return 3 active pools (skipping defunct pool_id_2)
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        pools.len(),
        3,
        "Should have 3 active pools after making one defunct"
    );

    // Verify the returned pools are the correct ones (not including defunct pool_id_2)
    let returned_pool_ids: Vec<Uint128> = pools.iter().map(|p| p.pool_id).collect();
    assert!(
        returned_pool_ids.contains(&pool_id_1),
        "Should include pool_id_1"
    );
    assert!(
        !returned_pool_ids.contains(&pool_id_2),
        "Should NOT include defunct pool_id_2"
    );
    assert!(
        returned_pool_ids.contains(&pool_id_3),
        "Should include pool_id_3"
    );
    assert!(
        returned_pool_ids.contains(&pool_id_4),
        "Should include pool_id_4"
    );

    // Make pool_id_1 defunct as well
    let defunct_msg = ExecuteMsg::DefunctPool { pool_id: pool_id_1 };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
    assert!(
        result.is_ok(),
        "Should successfully make second pool defunct"
    );

    // Query pools - should return 2 active pools
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        pools.len(),
        2,
        "Should have 2 active pools after making two defunct"
    );

    let returned_pool_ids: Vec<Uint128> = pools.iter().map(|p| p.pool_id).collect();
    assert!(
        !returned_pool_ids.contains(&pool_id_1),
        "Should NOT include defunct pool_id_1"
    );
    assert!(
        !returned_pool_ids.contains(&pool_id_2),
        "Should NOT include defunct pool_id_2"
    );
    assert!(
        returned_pool_ids.contains(&pool_id_3),
        "Should include pool_id_3"
    );
    assert!(
        returned_pool_ids.contains(&pool_id_4),
        "Should include pool_id_4"
    );
}

#[test]
fn test_query_pools_with_defunct_pools_and_pagination() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize tokens
    let (token1, _token2, _token3) = utils::initialize_3_tokens(&mut app, &owner);

    // Create 5 pools
    let mut pool_ids = Vec::new();
    for i in 0..5 {
        let denom = format!("denom{}", i + 1);
        let (_pool_addr, _lp_token, pool_id) = utils::initialize_stable_5_pool_2_asset(
            &mut app,
            &owner,
            vault_instance.clone(),
            token1.clone(),
            denom,
        );
        pool_ids.push(pool_id);
    }

    // Make pools 2 and 4 defunct (pool_ids[1] and pool_ids[3])
    for &pool_id in &[pool_ids[1], pool_ids[3]] {
        let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
        let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
        assert!(result.is_ok(), "Should successfully make pool defunct");
    }

    // Test pagination with limit=2 - should return first 2 active pools
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: None,
                limit: Some(2),
            },
        )
        .unwrap();
    assert_eq!(pools.len(), 2, "Should return 2 active pools with limit=2");
    assert_eq!(
        pools[0].pool_id, pool_ids[0],
        "First pool should be pool_ids[0]"
    );
    assert_eq!(
        pools[1].pool_id, pool_ids[2],
        "Second pool should be pool_ids[2] (skipping defunct pool_ids[1])"
    );

    // Test pagination starting after first pool - should return remaining active pools
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: Some(pool_ids[0]),
                limit: Some(2),
            },
        )
        .unwrap();
    assert_eq!(
        pools.len(),
        2,
        "Should return 2 active pools after pool_ids[0]"
    );
    assert_eq!(
        pools[0].pool_id, pool_ids[2],
        "First pool should be pool_ids[2] (skipping defunct pool_ids[1])"
    );
    assert_eq!(
        pools[1].pool_id, pool_ids[4],
        "Second pool should be pool_ids[4] (skipping defunct pool_ids[3])"
    );

    // Test that we get all remaining active pools
    let all_pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(all_pools.len(), 3, "Should have 3 active pools total");

    let active_pool_ids: Vec<Uint128> = all_pools.iter().map(|p| p.pool_id).collect();
    assert_eq!(
        active_pool_ids,
        vec![pool_ids[0], pool_ids[2], pool_ids[4]],
        "Should return only active pools in order"
    );
}

#[test]
fn test_query_pools_all_pools_defunct() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize tokens
    let (token1, _token2, _token3) = utils::initialize_3_tokens(&mut app, &owner);

    // Create 2 pools
    let (_pool_addr_1, _lp_token_1, pool_id_1) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1.clone(),
        _token2.clone(),
        _token3.clone(),
        "denom1".to_string(),
        "denom2".to_string(),
    );

    let (_pool_addr_2, _lp_token_2, pool_id_2) = utils::initialize_stable_5_pool_2_asset(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1.clone(),
        "denom3".to_string(),
    );

    // Make both pools defunct
    for &pool_id in &[pool_id_1, pool_id_2] {
        let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
        let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
        assert!(result.is_ok(), "Should successfully make pool defunct");
    }

    // Query pools - should return empty list
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert!(
        pools.is_empty(),
        "Should return empty list when all pools are defunct"
    );

    // Test with pagination parameters - should still return empty
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: Some(Uint128::zero()),
                limit: Some(10),
            },
        )
        .unwrap();
    assert!(
        pools.is_empty(),
        "Should return empty list with pagination when all pools are defunct"
    );
}

#[test]
fn test_query_pools_edge_cases() {
    let owner = Addr::unchecked("owner");
    let mut app = utils::mock_app(owner.clone(), coins(100_000_000_000u128, "uusd"));
    let vault_instance = utils::instantiate_contract(&mut app, &owner);

    // Initialize tokens
    let (token1, _token2, _token3) = utils::initialize_3_tokens(&mut app, &owner);

    // Create 3 pools
    let (_pool_addr_1, _lp_token_1, pool_id_1) = utils::initialize_weighted_pool(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1.clone(),
        _token2.clone(),
        _token3.clone(),
        "denom1".to_string(),
        "denom2".to_string(),
    );

    let (_pool_addr_2, _lp_token_2, pool_id_2) = utils::initialize_stable_5_pool_2_asset(
        &mut app,
        &owner,
        vault_instance.clone(),
        token1.clone(),
        "denom3".to_string(),
    );

    let (_pool_addr_3, _lp_token_3, pool_id_3) = utils::initialize_stable_5_pool_2_asset(
        &mut app,
        &owner,
        vault_instance.clone(),
        _token2.clone(),
        "denom4".to_string(),
    );

    // Test limit = 0 (should return empty)
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: None,
                limit: Some(0),
            },
        )
        .unwrap();
    assert!(pools.is_empty(), "Should return empty list with limit=0");

    // Test start_after beyond existing pools
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: Some(Uint128::from(999u128)),
                limit: None,
            },
        )
        .unwrap();
    assert!(
        pools.is_empty(),
        "Should return empty list when start_after is beyond all pools"
    );

    // Test start_after equal to last pool
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: Some(pool_id_3),
                limit: None,
            },
        )
        .unwrap();
    assert!(
        pools.is_empty(),
        "Should return empty list when start_after equals last pool"
    );

    // Make middle pool defunct and test edge case
    let defunct_msg = ExecuteMsg::DefunctPool { pool_id: pool_id_2 };
    let result = app.execute_contract(owner.clone(), vault_instance.clone(), &defunct_msg, &[]);
    assert!(
        result.is_ok(),
        "Should successfully make middle pool defunct"
    );

    // Test limit=1 starting from first pool
    let pools: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pools {
                start_after: Some(pool_id_1),
                limit: Some(1),
            },
        )
        .unwrap();
    assert_eq!(pools.len(), 1, "Should return 1 pool");
    assert_eq!(
        pools[0].pool_id, pool_id_3,
        "Should skip defunct pool and return pool_id_3"
    );
}
