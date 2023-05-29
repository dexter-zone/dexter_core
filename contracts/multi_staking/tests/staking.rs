use cosmwasm_std::{coin, Addr, Coin, Timestamp, Uint128};
use dexter::multi_staking::{RewardSchedule, RewardScheduleResponse};
use dexter::{
    asset::AssetInfo,
    multi_staking::{CreatorClaimableRewardState, QueryMsg, UnclaimedReward},
};

use crate::utils::{
    assert_user_lp_token_balance, bond_lp_tokens, claim_creator_rewards, create_dummy_cw20_token,
    create_reward_schedule, disallow_lp_token, mint_cw20_tokens_to_addr, mint_lp_tokens_to_addr,
    mock_app, query_balance, query_bonded_lp_tokens, query_cw20_balance, query_token_locks,
    query_unclaimed_rewards, setup, store_cw20_contract, unbond_lp_tokens, unlock_lp_tokens,
    withdraw_unclaimed_rewards,
};
pub mod utils;

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

    create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000 as u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_301_100);
        b.height = b.height + 100;
    });

    // Mint some LP tokens
    mint_lp_tokens_to_addr(
        &mut app,
        &admin_addr,
        &lp_token_addr,
        &user_addr,
        Uint128::from(100_000_000 as u64),
    );
    // Check user LP Balance
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr,
        Uint128::from(100_000_000 as u64),
    );

    bond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        Uint128::from(100_000_000 as u64),
    )
    .unwrap();

    // Validate that user balance is reduced after bonding
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr,
        Uint128::from(0 as u64),
    );

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_301_500);
        b.height = b.height + 100;
    });

    // Unbond half of the amoutt at 50% of the reward schedule
    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        Uint128::from(50_000_000 as u64),
    )
    .unwrap();

    // Query creator claimable reward
    let creator_claimable_reward: CreatorClaimableRewardState = app
        .wrap()
        .query_wasm_smart(
            multi_staking_instance.clone(),
            &QueryMsg::CreatorClaimableReward {
                reward_schedule_id: 1,
            },
        )
        .unwrap();

    assert_eq!(
        creator_claimable_reward.amount,
        Uint128::from(10_000_000 as u64)
    );
    assert_eq!(creator_claimable_reward.claimed, false);

    // Validate that user balance is still zero after bonding till unlock happens
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr,
        Uint128::from(0 as u64),
    );

    let token_lock_info = query_token_locks(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        None,
    );
    let token_locks = token_lock_info.locks;

    assert_eq!(token_lock_info.unlocked_amount, Uint128::from(0 as u64));
    assert_eq!(token_locks.len(), 1);
    assert_eq!(token_locks[0].amount, Uint128::from(50_000_000 as u64));
    assert_eq!(token_locks[0].unlock_time, 1_000_302_500);

    // try to unlock some tokens, but it should not alter any balance as unlock time is not reached
    unlock_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
    );

    // Validate that user balance is still zero after bonding till unlock happens
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr,
        Uint128::from(0 as u64),
    );

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_302_001);
        b.height = b.height + 100;
    });

    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        Uint128::from(50_000_000 as u64),
    )
    .unwrap();

    // validate new unlock that must have been issued after second unbonding
    let token_lock_info = query_token_locks(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        None,
    );
    let token_locks = token_lock_info.locks;
    assert_eq!(token_locks.len(), 2);
    assert_eq!(token_locks[0].amount, Uint128::from(50_000_000 as u64));
    assert_eq!(token_locks[0].unlock_time, 1_000_302_500);
    assert_eq!(token_locks[1].amount, Uint128::from(50_000_000 as u64));
    assert_eq!(token_locks[1].unlock_time, 1_000_303_001);

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_302_501);
        b.height = b.height + 100;
    });

    // Unlock first set of tokens
    unlock_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
    );

    // Validate that user LP balance is updated post unlock
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr,
        Uint128::from(50_000_000 as u64),
    );

    // validate unlocks are updated after first unlock
    let token_lock_info = query_token_locks(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        None,
    );
    let token_locks = token_lock_info.locks;
    assert_eq!(token_locks.len(), 1);
    assert_eq!(token_locks[0].amount, Uint128::from(50_000_000 as u64));
    assert_eq!(token_locks[0].unlock_time, 1_000_303_001);

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_303_002);
        b.height = b.height + 100;
    });

    let token_lock_info = query_token_locks(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        None,
    );
    let token_locks = token_lock_info.locks;
    assert_eq!(token_locks.len(), 0);
    assert_eq!(
        token_lock_info.unlocked_amount,
        Uint128::from(50_000_000 as u64)
    );

    // Unlock second set of tokens
    unlock_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
    );

    // Validate that user LP balance is updated post unlock
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr,
        Uint128::from(100_000_000 as u64),
    );

    // validate unlocks are updated after second unlock
    let token_lock_info = query_token_locks(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        None,
    );
    let token_locks = token_lock_info.locks;
    assert_eq!(token_locks.len(), 0);

    let query_msg = QueryMsg::UnclaimedRewards {
        lp_token: lp_token_addr.clone(),
        user: user_addr.clone(),
        block_time: None,
    };
    let response: Vec<UnclaimedReward> = app
        .wrap()
        .query_wasm_smart(multi_staking_instance.clone(), &query_msg)
        .unwrap();

    assert_eq!(response.len(), 1);
    let unclaimed_reward = response.get(0).unwrap();
    assert_eq!(unclaimed_reward.amount, Uint128::from(90_000_000 as u64));
    assert_eq!(
        unclaimed_reward.asset,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string()
        }
    );

    // ensure the reward schedules query works
    let response: Vec<RewardScheduleResponse> = app
        .wrap()
        .query_wasm_smart(
            multi_staking_instance.clone(),
            &QueryMsg::RewardSchedules {
                lp_token: lp_token_addr.clone(),
                asset: AssetInfo::NativeToken {
                    denom: "uxprt".to_string(),
                },
            },
        )
        .unwrap();
    assert_eq!(response.len(), 1);
    assert_eq!(
        response[0],
        RewardScheduleResponse {
            id: 1,
            reward_schedule: RewardSchedule {
                title: lp_token_addr.as_str().to_owned() + "-" + admin_addr.as_str(),
                creator: admin_addr.clone(),
                asset: AssetInfo::NativeToken {
                    denom: "uxprt".to_string(),
                },
                amount: Uint128::from(100_000_000 as u64),
                staking_lp_token: lp_token_addr.clone(),
                start_block_time: 1000_301_000,
                end_block_time: 1000_302_000,
            },
        }
    );

    // Withdraw the rewards
    withdraw_unclaimed_rewards(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
    );

    // query bank module for user balance
    let balances = app.wrap().query_all_balances(user_addr).unwrap();
    let balance_uxprt = balances.iter().find(|b| b.denom == "uxprt").unwrap();
    assert_eq!(balance_uxprt.amount, Uint128::from(90_000_000 as u64));

    // Claim creator rewards
    claim_creator_rewards(&mut app, &multi_staking_instance, 1, &admin_addr).unwrap();

    // Query creator claimable rewards
    let query_msg = QueryMsg::CreatorClaimableReward {
        reward_schedule_id: 1,
    };

    let response: CreatorClaimableRewardState = app
        .wrap()
        .query_wasm_smart(multi_staking_instance.clone(), &query_msg)
        .unwrap();

    assert_eq!(response.amount, Uint128::from(10_000_000 as u64));
    assert_eq!(response.claimed, true);

    // Verify balance of admin addr
    let balances = app.wrap().query_all_balances(admin_addr.clone()).unwrap();
    let balance_uxprt = balances.iter().find(|b| b.denom == "uxprt").unwrap();

    assert_eq!(balance_uxprt.amount, Uint128::from(910_000_000 as u64));

    // claiming creator rewards again should fail
    let response = claim_creator_rewards(&mut app, &multi_staking_instance, 1, &admin_addr);
    assert_eq!(response.is_err(), true);
    assert_eq!(
        response.unwrap_err().root_cause().to_string(),
        "Unallocated reward for this schedule has already been claimed by the creator"
    );

    // create another reward schedule which won't have any user bonding
    create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000 as u64),
        1000_601_000,
        1000_602_000,
    )
    .unwrap();

    // verify the amount before the beginning of the reward schedule
    let query_msg = QueryMsg::CreatorClaimableReward {
        reward_schedule_id: 2,
    };
    let response: CreatorClaimableRewardState = app
        .wrap()
        .query_wasm_smart(multi_staking_instance.clone(), &query_msg)
        .unwrap();

    assert_eq!(response.amount, Uint128::from(0 as u64));
    assert_eq!(response.claimed, false);

    // Verify balance of admin addr
    let balances = app.wrap().query_all_balances(admin_addr.clone()).unwrap();
    let balance_uxprt = balances.iter().find(|b| b.denom == "uxprt").unwrap();
    assert_eq!(balance_uxprt.amount, Uint128::from(810_000_000 as u64));

    // skip the whole reward schedule duration
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_602_001);
        b.height = b.height + 100;
    });

    // verify the amount after the end of the reward schedule
    let response: CreatorClaimableRewardState = app
        .wrap()
        .query_wasm_smart(multi_staking_instance.clone(), &query_msg)
        .unwrap();

    assert_eq!(response.amount, Uint128::from(100_000_000 as u64));
    assert_eq!(response.claimed, false);

    // claim the unused rewards
    claim_creator_rewards(&mut app, &multi_staking_instance, 2, &admin_addr).unwrap();

    // Verify balance of admin addr
    let balances = app.wrap().query_all_balances(admin_addr).unwrap();
    let balance_uxprt = balances.iter().find(|b| b.denom == "uxprt").unwrap();
    assert_eq!(balance_uxprt.amount, Uint128::from(910_000_000 as u64));
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

    create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000 as u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();

    create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(150_000_000 as u64),
        1000_301_500,
        1000_302_000,
    )
    .unwrap();

    create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        AssetInfo::NativeToken {
            denom: "uatom".to_string(),
        },
        Uint128::from(200_000_000 as u64),
        1000_301_200,
        1000_302_000,
    )
    .unwrap();

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_301_000);
        b.height = b.height + 100;
    });

    // Mint some LP tokens
    mint_lp_tokens_to_addr(
        &mut app,
        &admin_addr,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(200_000 as u64),
    );

    bond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(100_000 as u64),
    )
    .unwrap();

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_301_500);
        b.height = b.height + 100;
    });

    // Unbond half of the amoutt at 50% of the reward schedule
    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(50_000 as u64),
    )
    .unwrap();

    let unclaimed_rewards_user_1 = query_unclaimed_rewards(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
    );
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
            }
            _ => panic!("Unexpected asset type"),
        }
    }

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_302_001);
        b.height = b.height + 100;
    });

    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(50_000 as u64),
    )
    .unwrap();

    let unclaimed_rewards_user_1 = query_unclaimed_rewards(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
    );
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
            }
            _ => panic!("Unexpected asset type"),
        }
    }

    // withdraw the rewards
    withdraw_unclaimed_rewards(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
    );
    // validate the withdrawn amount
    let balances = app.wrap().query_all_balances(user_1_addr.clone()).unwrap();
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

    create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000 as u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();

    create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000 as u64),
        1000_301_500,
        1000_302_000,
    )
    .unwrap();

    create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        AssetInfo::NativeToken {
            denom: "uatom".to_string(),
        },
        Uint128::from(200_000_000 as u64),
        1000_301_200,
        1000_302_000,
    )
    .unwrap();

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_301_000);
        b.height = b.height + 100;
    });

    // Mint some LP tokens
    mint_lp_tokens_to_addr(
        &mut app,
        &admin_addr,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(200_000 as u64),
    );
    mint_lp_tokens_to_addr(
        &mut app,
        &admin_addr,
        &lp_token_addr,
        &user_2_addr,
        Uint128::from(1_000_000 as u64),
    );

    bond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(100_000 as u64),
    )
    .unwrap();

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_301_200);
        b.height = b.height + 100;
    });

    bond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_2_addr,
        Uint128::from(1_000_000 as u64),
    )
    .unwrap();

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_301_500);
        b.height = b.height + 100;
    });

    // Unbond half of the amoutt at 50% of the reward schedule
    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(50_000 as u64),
    )
    .unwrap();

    // check if bonded amount decreased
    let user_1_bonded = query_bonded_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
    );
    assert_eq!(user_1_bonded, Uint128::from(50_000 as u64));

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_302_001);
        b.height = b.height + 100;
    });

    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(50_000 as u64),
    )
    .unwrap();

    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_2_addr,
        Uint128::from(1_000_000 as u64),
    )
    .unwrap();

    let unclaimed_rewards_user_1 = query_unclaimed_rewards(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
    );
    let unclaimed_rewards_user_2 = query_unclaimed_rewards(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_2_addr,
    );

    // validator unclaimed rewards
    assert_eq!(unclaimed_rewards_user_1.len(), 2);
    assert_eq!(unclaimed_rewards_user_2.len(), 2);

    for unclaimed_reward in unclaimed_rewards_user_1 {
        if let AssetInfo::NativeToken { denom } = unclaimed_reward.asset {
            match denom.as_str() {
                "uxprt" => assert_eq!(unclaimed_reward.amount, Uint128::from(29_870_129 as u64)),
                "uatom" => assert_eq!(unclaimed_reward.amount, Uint128::from(12_770_561 as u64)),
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
    withdraw_unclaimed_rewards(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
    );

    let user_1_balance = app.wrap().query_all_balances(user_1_addr.clone()).unwrap();

    let user1_uxprt_balance = user_1_balance.iter().find(|b| b.denom == "uxprt").unwrap();
    let user1_uatom_balance = user_1_balance.iter().find(|b| b.denom == "uatom").unwrap();

    assert_eq!(user1_uxprt_balance.amount, Uint128::from(29_870_129 as u64));
    assert_eq!(user1_uatom_balance.amount, Uint128::from(12_770_561 as u64));

    withdraw_unclaimed_rewards(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_2_addr,
    );

    let user_2_balance = app.wrap().query_all_balances(user_2_addr.clone()).unwrap();

    let user2_uxprt_balance = user_2_balance.iter().find(|b| b.denom == "uxprt").unwrap();
    let user2_uatom_balance = user_2_balance.iter().find(|b| b.denom == "uatom").unwrap();

    assert_eq!(
        user2_uxprt_balance.amount,
        Uint128::from(170_129_870 as u64)
    );
    assert_eq!(
        user2_uatom_balance.amount,
        Uint128::from(187_229_437 as u64)
    );
}

/// This test is to check if the rewards are calculated correctly when we add a new reward schedule
/// after a user has already bonded some LP tokens
#[test]
fn test_reward_schedule_creation_after_bonding() {
    let admin = String::from("admin");
    let user_1 = String::from("user_1");
    let user_2 = String::from("user_2");

    let coins = vec![coin(1_000_000_000, "uxprt"), coin(1_000_000_000, "uatom")];

    let admin_addr = Addr::unchecked(admin.clone());
    let user_1_addr = Addr::unchecked(user_1.clone());
    let user_2_addr = Addr::unchecked(user_2.clone());

    let mut app = mock_app(admin_addr.clone(), coins);

    let (multi_staking_instance, lp_token_addr) = setup(&mut app, admin_addr.clone());

    // create a reward schedule
    create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000 as u64),
        1000_301_000,
        1000_602_000,
    )
    .unwrap();

    // mint some LP tokens to user
    mint_lp_tokens_to_addr(
        &mut app,
        &admin_addr,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(1_000_000 as u64),
    );

    // bond some LP tokens
    bond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(100_000 as u64),
    )
    .unwrap();

    // increase time
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_301_200);
        b.height = b.height + 100;
    });

    // create another reward schedule
    create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(500_000_000 as u64),
        1000_601_500,
        1000_602_000,
    )
    .unwrap();

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_601_600);
        b.height = b.height + 100;
    });

    // mint some LP tokens to user 2
    mint_lp_tokens_to_addr(
        &mut app,
        &admin_addr,
        &lp_token_addr,
        &user_2_addr,
        Uint128::from(1_000_000 as u64),
    );

    // bond LP tokens
    bond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_2_addr,
        Uint128::from(100_000 as u64),
    )
    .unwrap();

    // increase time
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_602_001);
        b.height = b.height + 100;
    });

    // unbond all LP tokens
    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(100_000 as u64),
    )
    .unwrap();

    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_2_addr,
        Uint128::from(100_000 as u64),
    )
    .unwrap();

    // calculate rewards
    let unclaimed_rewards_user_1 = query_unclaimed_rewards(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
    );

    for unclaimed_reward in unclaimed_rewards_user_1 {
        if let AssetInfo::NativeToken { denom } = unclaimed_reward.asset {
            match denom.as_str() {
                "uxprt" => assert_eq!(unclaimed_reward.amount, Uint128::from(399_933_554 as u64)),
                "uatom" => assert_eq!(unclaimed_reward.amount, Uint128::from(0 as u64)),
                _ => panic!("Unexpected denom"),
            }
        } else {
            panic!("Unexpected asset type")
        }
    }

    let unclaimed_rewards_user_2 = query_unclaimed_rewards(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_2_addr,
    );

    for unclaimed_reward in unclaimed_rewards_user_2 {
        if let AssetInfo::NativeToken { denom } = unclaimed_reward.asset {
            match denom.as_str() {
                "uxprt" => assert_eq!(unclaimed_reward.amount, Uint128::from(200_066_445 as u64)),
                "uatom" => assert_eq!(unclaimed_reward.amount, Uint128::from(0 as u64)),
                _ => panic!("Unexpected denom"),
            }
        } else {
            panic!("Unexpected asset type")
        }
    }
}

/// This test is to check if CW20 assets are correctly rewarded
#[test]
fn test_create_cw20_reward_schedule() {
    let admin = String::from("admin");
    let user_1 = String::from("user_1");

    let coins = vec![coin(1_000_000_000, "uxprt"), coin(1_000_000_000, "uatom")];

    let admin_addr = Addr::unchecked(admin.clone());
    let user_1_addr = Addr::unchecked(user_1.clone());

    let mut app = mock_app(admin_addr.clone(), coins);

    let (multi_staking_instance, lp_token_addr) = setup(&mut app, admin_addr.clone());

    let cw20_code_id = store_cw20_contract(&mut app);
    let cw20_token_addr = create_dummy_cw20_token(&mut app, &admin_addr, cw20_code_id);

    // mint cw20 tokens to user 1
    mint_cw20_tokens_to_addr(
        &mut app,
        &admin_addr,
        &cw20_token_addr,
        &admin_addr,
        Uint128::from(100_000_000 as u64),
    );

    // create a reward schedule
    let result = create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        AssetInfo::Token {
            contract_addr: cw20_token_addr.clone(),
        },
        Uint128::from(100_000_000 as u64),
        1000_301_000,
        1000_302_000,
    );

    assert!(result.is_ok());

    // mint lp tokens to user 1
    mint_lp_tokens_to_addr(
        &mut app,
        &admin_addr,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(1_000_000 as u64),
    );
    // bond some LP tokens
    bond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(100_000 as u64),
    )
    .unwrap();

    // update time
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_301_500);
        b.height = b.height + 100;
    });

    // unbond all LP tokens
    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(100_000 as u64),
    )
    .unwrap();

    // increase time
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_303_001);
        b.height = b.height + 100;
    });

    // query rewards
    let unclaimed_rewards_user_1 = query_unclaimed_rewards(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
    );

    for unclaimed_reward in unclaimed_rewards_user_1 {
        if let AssetInfo::Token { contract_addr } = unclaimed_reward.asset {
            assert_eq!(contract_addr, cw20_token_addr);
            assert_eq!(unclaimed_reward.amount, Uint128::from(50_000_000 as u64));
        } else {
            panic!("Unexpected asset type")
        }
    }

    // withdraw rewards
    withdraw_unclaimed_rewards(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
    );

    // query rewards
    let unclaimed_rewards_user_1 = query_unclaimed_rewards(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
    );

    assert_eq!(unclaimed_rewards_user_1.len(), 0);

    // validate user cw20 balance
    let user_1_cw20_balance = query_cw20_balance(&mut app, &cw20_token_addr, &user_1_addr);
    assert_eq!(user_1_cw20_balance, Uint128::from(50_000_000 as u64));
}

/// This test checks if the after disallowing an LP token, the operations of
/// unbonding and withdrawing rewards are still possible
#[test]
fn test_lp_methods_after_lp_allowance_removal() {
    // setup
    let admin = String::from("admin");
    let user_1 = String::from("user_1");

    let coins = vec![coin(1_000_000_000, "uxprt"), coin(1_000_000_000, "uatom")];

    let admin_addr = Addr::unchecked(admin.clone());
    let user_1_addr = Addr::unchecked(user_1.clone());

    let mut app = mock_app(admin_addr.clone(), coins);

    let (multi_staking_instance, lp_token_addr) = setup(&mut app, admin_addr.clone());

    // mint lp tokens to user 1
    mint_lp_tokens_to_addr(
        &mut app,
        &admin_addr,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(1_000_000 as u64),
    );

    // bond some LP tokens
    bond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(100_000 as u64),
    )
    .unwrap();

    // increase time
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_500);
        b.height = b.height + 100;
    });

    // add a reward schedule
    create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000 as u64),
        1000_301_500,
        1000_302_000,
    )
    .unwrap();

    // disallow lp token
    disallow_lp_token(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
    );

    // bond again
    let res = bond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(100_000 as u64),
    );

    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().root_cause().to_string(),
        "LP Token is not allowed for staking"
    );

    // increase time
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_302_000);
        b.height = b.height + 100;
    });

    // unbond all LP tokens
    let unbond_response = unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
        Uint128::from(100_000 as u64),
    );

    assert!(unbond_response.is_ok());

    // query rewards
    let unclaimed_rewards_user_1 = query_unclaimed_rewards(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
    );

    assert_eq!(unclaimed_rewards_user_1.len(), 1);
    for unclaimed_reward in unclaimed_rewards_user_1 {
        if let AssetInfo::NativeToken { denom } = unclaimed_reward.asset {
            assert_eq!(denom, "uxprt");
            assert_eq!(unclaimed_reward.amount, Uint128::from(100_000_000 as u64));
        } else {
            panic!("Unexpected asset type")
        }
    }

    // increase time
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_303_001);
        b.height = b.height + 100;
    });

    // withdraw rewards
    withdraw_unclaimed_rewards(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_1_addr,
    );

    // query balance
    let user_1_balance = query_balance(&mut app, &user_1_addr);
    assert_eq!(user_1_balance, vec![coin(100_000_000, "uxprt")]);
}
