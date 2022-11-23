use cosmwasm_std::{Coin, Uint128, Addr, Timestamp};
use cw_multi_test::Executor;
use dexter::{multi_staking::{ExecuteMsg, UnclaimedReward, QueryMsg}, asset::AssetInfo};

use crate::utils::{mock_app, setup, create_reward_schedule, bond_lp_tokens, unbond_lp_tokens, query_token_locks, unlock_lp_tokens, withdraw_unclaimed_rewards, mint_lp_tokens_to_addr, query_unclaimed_rewards, assert_user_lp_token_balance};
mod utils;

#[test]
fn test_staking() {
    let admin = String::from("admin");
    let user = String::from("user");

    let coins = vec![
        Coin::new(1000_000_000, "uxprt"),
        Coin::new(1000_000_000, "uatom"),
    ];

    let admin_addr = Addr::unchecked(admin.clone());
    let user_addr = Addr::unchecked(user.clone());

    let mut app = mock_app(admin_addr.clone(), coins);

    let (multi_staking_instance, lp_token_addr) = setup(&mut app, admin_addr.clone());

    create_reward_schedule(&mut app, 
        &admin_addr, 
        &multi_staking_instance, 
        &lp_token_addr, 
        AssetInfo::NativeToken { denom: "uxprt".to_string() },
        Uint128::from( 100_000_000 as u64),
        1000_001_000,
        1000_002_000
    );

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_000);
        b.height = b.height + 100;
    });

    // Mint some LP tokens
    mint_lp_tokens_to_addr(&mut app, &admin_addr, &lp_token_addr, &user_addr, Uint128::from(100_000_000 as u64));
    // Check user LP Balance
    assert_user_lp_token_balance(&mut app, &user_addr, &lp_token_addr, Uint128::from(100_000_000 as u64));

    bond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_addr, Uint128::from(100_000_000 as u64));

    // Validate that user balance is reduced after bonding
    assert_user_lp_token_balance(&mut app, &user_addr, &lp_token_addr, Uint128::from(0 as u64));

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_500);
        b.height = b.height + 100;
    });

    // Unbond half of the amoutt at 50% of the reward schedule
    unbond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_addr, Uint128::from(50_000_000 as u64));

    // Validate that user balance is still zero after bonding till unlock happens
    assert_user_lp_token_balance(&mut app, &user_addr, &lp_token_addr, Uint128::from(0 as u64));

    let token_lock_info = query_token_locks(&mut app, &multi_staking_instance, &lp_token_addr, &user_addr, None);
    let token_locks = token_lock_info.locks;
    
    assert_eq!(token_lock_info.unlocked_amount, Uint128::from(0 as u64));
    assert_eq!(token_locks.len(), 1);
    assert_eq!(token_locks[0].amount, Uint128::from(50_000_000 as u64));
    assert_eq!(token_locks[0].unlock_time, 1_000_002_500);

    // try to unlock some tokens, but it should not alter any balance as unlock time is not reached
    unlock_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_addr);

    // Validate that user balance is still zero after bonding till unlock happens
    assert_user_lp_token_balance(&mut app, &user_addr, &lp_token_addr, Uint128::from(0 as u64));


    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_002_001);
        b.height = b.height + 100;
    });

    unbond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_addr, Uint128::from(50_000_000 as u64));

    // validate new unlock that must have been issued after second unbonding
    let token_lock_info = query_token_locks(&mut app, &multi_staking_instance, &lp_token_addr, &user_addr, None);
    let token_locks = token_lock_info.locks;
    assert_eq!(token_locks.len(), 2);
    assert_eq!(token_locks[0].amount, Uint128::from(50_000_000 as u64));
    assert_eq!(token_locks[0].unlock_time, 1_000_002_500);
    assert_eq!(token_locks[1].amount, Uint128::from(50_000_000 as u64));
    assert_eq!(token_locks[1].unlock_time, 1_000_003_001);

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_002_501);
        b.height = b.height + 100;
    });

    // Unlock first set of tokens
    unlock_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_addr);

    // Validate that user LP balance is updated post unlock
    assert_user_lp_token_balance(&mut app, &user_addr, &lp_token_addr, Uint128::from(50_000_000 as u64));

    // validate unlocks are updated after first unlock
    let token_lock_info = query_token_locks(&mut app, &multi_staking_instance, &lp_token_addr, &user_addr, None);
    let token_locks = token_lock_info.locks;
    assert_eq!(token_locks.len(), 1);
    assert_eq!(token_locks[0].amount, Uint128::from(50_000_000 as u64));
    assert_eq!(token_locks[0].unlock_time, 1_000_003_001);

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_003_002);
        b.height = b.height + 100;
    });

    let token_lock_info = query_token_locks(&mut app, &multi_staking_instance, &lp_token_addr, &user_addr, None);
    let token_locks = token_lock_info.locks;
    assert_eq!(token_locks.len(), 0);
    assert_eq!(token_lock_info.unlocked_amount, Uint128::from(50_000_000 as u64));

    // Unlock second set of tokens
    unlock_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_addr);

    // Validate that user LP balance is updated post unlock
    assert_user_lp_token_balance(&mut app, &user_addr, &lp_token_addr, Uint128::from(100_000_000 as u64));

    // validate unlocks are updated after second unlock
    let token_lock_info = query_token_locks(&mut app, &multi_staking_instance, &lp_token_addr, &user_addr, None);
    let token_locks = token_lock_info.locks;
    assert_eq!(token_locks.len(), 0);
    
    let query_msg = QueryMsg::UnclaimedRewards { lp_token: lp_token_addr.clone(), user: user_addr.clone(), block_time: None };
    let response: Vec<UnclaimedReward> = app.wrap().query_wasm_smart(multi_staking_instance.clone(), &query_msg).unwrap();

    assert_eq!(response.len(), 1);
    let unclaimed_reward = response.get(0).unwrap();
    assert_eq!(unclaimed_reward.amount, Uint128::from(100_000_000 as u64));
    assert_eq!(unclaimed_reward.asset, dexter::asset::AssetInfo::NativeToken { denom: "uxprt".to_string() });


    // Withdraw the rewards
    withdraw_unclaimed_rewards(&mut app, &multi_staking_instance, &lp_token_addr, &user_addr);

    // query bank module for user balance
    let balances =  app.wrap().query_all_balances(user_addr).unwrap();
    let balance_uxprt = balances.iter().find(|b| b.denom == "uxprt").unwrap();
    assert_eq!(balance_uxprt.amount, Uint128::from(100_000_000 as u64));
}

#[test]
fn test_multi_asset_multi_reward_schedules() {
    let admin = String::from("admin");
    let user_1 = String::from("user_1");

    let coins = vec![
        Coin::new(1000_000_000, "uxprt"),
        Coin::new(1000_000_000, "uatom"),
    ];

    let admin_addr = Addr::unchecked(admin.clone());
    let user_1_addr = Addr::unchecked(user_1.clone());

    let mut app = mock_app(admin_addr.clone(), coins);

    let (multi_staking_instance, lp_token_addr) = setup(&mut app, admin_addr.clone());

    create_reward_schedule(&mut app, 
        &admin_addr, 
        &multi_staking_instance, 
        &lp_token_addr, 
        AssetInfo::NativeToken { denom: "uxprt".to_string() },
        Uint128::from(100_000_000 as u64),
        1000_001_000,
        1000_002_000
    );

    create_reward_schedule(&mut app, 
        &admin_addr, 
        &multi_staking_instance, 
        &lp_token_addr, 
        AssetInfo::NativeToken { denom: "uxprt".to_string() },
        Uint128::from(150_000_000 as u64),
        1000_001_500,
        1000_002_000
    );

    create_reward_schedule(&mut app, 
        &admin_addr, 
        &multi_staking_instance, 
        &lp_token_addr, 
        AssetInfo::NativeToken { denom: "uatom".to_string() },
        Uint128::from(200_000_000 as u64),
        1000_001_200,
        1000_002_000
    );

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_000);
        b.height = b.height + 100;
    });

    // Mint some LP tokens
    mint_lp_tokens_to_addr(&mut app, &admin_addr, &lp_token_addr, &user_1_addr, Uint128::from(200_000 as u64));

    bond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr, Uint128::from(100_000 as u64));

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_500);
        b.height = b.height + 100;
    });

    // Unbond half of the amoutt at 50% of the reward schedule
    unbond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr, Uint128::from(50_000 as u64));

     
    let unclaimed_rewards_user_1 = query_unclaimed_rewards(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr);    
    assert_eq!(unclaimed_rewards_user_1.len(), 2);

    for unclaimed_reward in unclaimed_rewards_user_1 {
        match unclaimed_reward.asset {
            AssetInfo::NativeToken { denom } => {
                if denom == "uxprt" {
                    assert_eq!(unclaimed_reward.amount, Uint128::from(50_000_000 as u64));
                } else if denom == "uatom" {
                    assert_eq!(unclaimed_reward.amount, Uint128::from(75_000_000 as u64));
                } else {
                    panic!("Unexpected denom");
                }
            },
            _ => panic!("Unexpected asset type"),
        }
    }

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_002_001);
        b.height = b.height + 100;
    });

    unbond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr, Uint128::from(50_000 as u64));

    let unclaimed_rewards_user_1 = query_unclaimed_rewards(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr);    
    assert_eq!(unclaimed_rewards_user_1.len(), 2);

    // validate unclaimed rewards
    for unclaimed_reward in unclaimed_rewards_user_1 {
        match unclaimed_reward.asset {
            AssetInfo::NativeToken { denom } => {
                if denom == "uxprt" {
                    assert_eq!(unclaimed_reward.amount, Uint128::from(250_000_000 as u64));
                } else if denom == "uatom" {
                    assert_eq!(unclaimed_reward.amount, Uint128::from(200_000_000 as u64));
                } else {
                    panic!("Unexpected denom");
                }
            },
            _ => panic!("Unexpected asset type"),
        }
    }

    // withdraw the rewards
    withdraw_unclaimed_rewards(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr);
    // validate the withdrawn amount
    let balances =  app.wrap().query_all_balances(user_1_addr.clone()).unwrap();
    let uxprt_balance = balances.iter().find(|b| b.denom == "uxprt").unwrap();
    let uatom_balance = balances.iter().find(|b| b.denom == "uatom").unwrap();

    assert_eq!(uxprt_balance.amount, Uint128::from(250_000_000 as u64));
    assert_eq!(uatom_balance.amount, Uint128::from(200_000_000 as u64));

}

#[test]
fn test_multi_user_multi_reward_schedule() {
    let admin = String::from("admin");
    let user_1 = String::from("user_1");
    let user_2 = String::from("user_2");

    let coins = vec![
        Coin::new(1000_000_000, "uxprt"),
        Coin::new(1000_000_000, "uatom"),
    ];

    let admin_addr = Addr::unchecked(admin.clone());
    let user_1_addr = Addr::unchecked(user_1.clone());
    let user_2_addr = Addr::unchecked(user_2.clone());

    let mut app = mock_app(admin_addr.clone(), coins);

    let (multi_staking_instance, lp_token_addr) = setup(&mut app, admin_addr.clone());

    create_reward_schedule(&mut app, 
        &admin_addr, 
        &multi_staking_instance, 
        &lp_token_addr, 
        AssetInfo::NativeToken { denom: "uxprt".to_string() },
        Uint128::from(100_000_000 as u64),
        1000_001_000,
        1000_002_000
    );

    create_reward_schedule(&mut app, 
        &admin_addr, 
        &multi_staking_instance, 
        &lp_token_addr, 
        AssetInfo::NativeToken { denom: "uxprt".to_string() },
        Uint128::from(100_000_000 as u64),
        1000_001_500,
        1000_002_000
    );

    create_reward_schedule(&mut app, 
        &admin_addr, 
        &multi_staking_instance, 
        &lp_token_addr, 
        AssetInfo::NativeToken { denom: "uatom".to_string() },
        Uint128::from(200_000_000 as u64),
        1000_001_200,
        1000_002_000
    );

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_000);
        b.height = b.height + 100;
    });

    // Mint some LP tokens
    mint_lp_tokens_to_addr(&mut app, &admin_addr, &lp_token_addr, &user_1_addr, Uint128::from(200_000 as u64));
    mint_lp_tokens_to_addr(&mut app, &admin_addr, &lp_token_addr, &user_2_addr, Uint128::from(1_000_000 as u64));

    bond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr, Uint128::from(100_000 as u64));

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_200);
        b.height = b.height + 100;
    });

    bond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_2_addr, Uint128::from(1_000_000 as u64));

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_500);
        b.height = b.height + 100;
    });

    // Unbond half of the amoutt at 50% of the reward schedule
    unbond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr, Uint128::from(50_000 as u64));

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_002_001);
        b.height = b.height + 100;
    });

    unbond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr, Uint128::from(50_000 as u64));
    unbond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_2_addr, Uint128::from(1_000_000 as u64));

    let unclaimed_rewards_user_1 = query_unclaimed_rewards(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr);
    let unclaimed_rewards_user_2 = query_unclaimed_rewards(&mut app, &multi_staking_instance, &lp_token_addr, &user_2_addr);
    
    // validator unclaimed rewards 
    assert_eq!(unclaimed_rewards_user_1.len(), 2);
    assert_eq!(unclaimed_rewards_user_2.len(), 2);

    for unclaimed_reward in unclaimed_rewards_user_1 {
        if let AssetInfo::NativeToken { denom } = unclaimed_reward.asset {
            match denom.as_str() {
                "uxprt" => assert_eq!(unclaimed_reward.amount, Uint128::from(29_870_129 as u64)),
                "uatom" => assert_eq!(unclaimed_reward.amount, Uint128::from(12_770_562 as u64)),
                _ => panic!("Unexpected denom"),
            }
        } else {
            panic!("Unexpected asset type")
        }
    }

    for unclaimed_reward in unclaimed_rewards_user_2 {
        if let AssetInfo::NativeToken { denom } = unclaimed_reward.asset {
            match denom.as_str() {
                "uxprt" => assert_eq!(unclaimed_reward.amount, Uint128::from(170_129_870 as u64)),
                "uatom" => assert_eq!(unclaimed_reward.amount, Uint128::from(187_229_437 as u64)),
                _ => panic!("Unexpected denom"),
            }
        } else {
            panic!("Unexpected asset type")
        }
    }

    // withdraw rewards
    withdraw_unclaimed_rewards(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr);
    
    let user_1_balance = app.wrap().query_all_balances(user_1_addr.clone()).unwrap();
    
    let user1_uxprt_balance = user_1_balance.iter().find(|b| b.denom == "uxprt").unwrap();
    let user1_uatom_balance = user_1_balance.iter().find(|b| b.denom == "uatom").unwrap();
    
    assert_eq!(user1_uxprt_balance.amount, Uint128::from(29_870_129 as u64));
    assert_eq!(user1_uatom_balance.amount, Uint128::from(12_770_562 as u64));
    
    withdraw_unclaimed_rewards(&mut app, &multi_staking_instance, &lp_token_addr, &user_2_addr);
    
    let user_2_balance = app.wrap().query_all_balances(user_2_addr.clone()).unwrap();
    
    let user2_uxprt_balance = user_2_balance.iter().find(|b| b.denom == "uxprt").unwrap();
    let user2_uatom_balance = user_2_balance.iter().find(|b| b.denom == "uatom").unwrap();

    assert_eq!(user2_uxprt_balance.amount, Uint128::from(170_129_870 as u64));
    assert_eq!(user2_uatom_balance.amount, Uint128::from(187_229_437 as u64));

}