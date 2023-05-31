use cosmwasm_std::{Addr, Coin, Timestamp, Uint128};
use dexter::asset::AssetInfo;
use utils::update_fee_tier_interval;

use crate::utils::{
    assert_user_bonded_amount, assert_user_lp_token_balance, bond_lp_tokens,
    create_reward_schedule, instant_unbond_lp_tokens, instant_unlock_lp_tokens,
    mint_lp_tokens_to_addr, mock_app, query_instant_lp_unlock_fee, query_instant_unlock_fee_tiers,
    query_raw_token_locks, query_token_locks, setup_generic, unbond_lp_tokens, unlock_lp_tokens,
};
pub mod utils;

#[test]
fn validate_fee_tier_logic() {
    let admin = String::from("admin");
    let keeper = String::from("keeper");

    let coins = vec![
        Coin::new(1000_000_000, "uxprt"),
        Coin::new(1000_000_000, "uatom"),
    ];

    let admin_addr = Addr::unchecked(admin.clone());
    let keeper_addr = Addr::unchecked(keeper.clone());

    let mut app = mock_app(admin_addr.clone(), coins);

    let (multi_staking_instance, _) = setup_generic(
        &mut app,
        admin_addr.clone(),
        Some(keeper_addr.clone()),
        0,
        // 80 minutes less than 7 days. We should still have 7 tiers
        600_000,
        300,
        500,
    );

    // Update fee tier boundary to same time as unlock period
    update_fee_tier_interval(&mut app, &admin_addr, &multi_staking_instance, 600_000);

    // query fee tiers
    let fee_tiers = query_instant_unlock_fee_tiers(&mut app, &multi_staking_instance);

    // validate fee tiers. There should be 1 tier upto the unlock period boundary and max fee
    assert_eq!(fee_tiers.len(), 1);
    assert_eq!(fee_tiers[0].seconds_till_unlock_end, 600_000);
    assert_eq!(fee_tiers[0].seconds_till_unlock_start, 0);
    assert_eq!(fee_tiers[0].unlock_fee_bp, 500u64);

    // update fee tier boundary higher than unlock period to make sure we have 1 tier still
    update_fee_tier_interval(&mut app, &admin_addr, &multi_staking_instance, 600_001);

    // query fee tiers
    let fee_tiers = query_instant_unlock_fee_tiers(&mut app, &multi_staking_instance);

    // validate fee tiers. There should be 1 tier upto the unlock period boundary and max fee
    assert_eq!(fee_tiers.len(), 1);
    assert_eq!(fee_tiers[0].seconds_till_unlock_end, 600_000);
    assert_eq!(fee_tiers[0].seconds_till_unlock_start, 0);

    // update fee tier boundary to 100_000 seconds and validate that we have 6 tiers which are equalled spaced
    update_fee_tier_interval(&mut app, &admin_addr, &multi_staking_instance, 100_000);

    // query fee tiers
    let fee_tiers = query_instant_unlock_fee_tiers(&mut app, &multi_staking_instance);

    // validate fee tiers. There should be 6 tiers upto the unlock period boundary and max fee
    assert_eq!(fee_tiers.len(), 6);
    assert_eq!(fee_tiers[0].seconds_till_unlock_end, 100_000);
    assert_eq!(fee_tiers[0].seconds_till_unlock_start, 0);
    assert_eq!(fee_tiers[0].unlock_fee_bp, 300u64);

    assert_eq!(fee_tiers[1].seconds_till_unlock_end, 200_000);
    assert_eq!(fee_tiers[1].seconds_till_unlock_start, 100_000);
    assert_eq!(fee_tiers[1].unlock_fee_bp, 340u64);

    assert_eq!(fee_tiers[2].seconds_till_unlock_end, 300_000);
    assert_eq!(fee_tiers[2].seconds_till_unlock_start, 200_000);
    assert_eq!(fee_tiers[2].unlock_fee_bp, 380u64);

    assert_eq!(fee_tiers[3].seconds_till_unlock_end, 400_000);
    assert_eq!(fee_tiers[3].seconds_till_unlock_start, 300_000);
    assert_eq!(fee_tiers[3].unlock_fee_bp, 420u64);

    assert_eq!(fee_tiers[4].seconds_till_unlock_end, 500_000);
    assert_eq!(fee_tiers[4].seconds_till_unlock_start, 400_000);
    assert_eq!(fee_tiers[4].unlock_fee_bp, 460u64);

    assert_eq!(fee_tiers[5].seconds_till_unlock_end, 600_000);
    assert_eq!(fee_tiers[5].seconds_till_unlock_start, 500_000);
    assert_eq!(fee_tiers[5].unlock_fee_bp, 500u64);

}

// This test performs the following steps:
// 1. Bonds some LP tokens for the user
// 2. Unbonds some of them normally creating a lock
// 3. Instatntly unbonds some of the tokens
// 4. Unbonds rest of the tokens normally creating a 2nd lock
// 4. Instatntly unlocks the tokens that were locked in step 2 paying the penalty fee
// 5. Validate if one of the lock still exists, the correct one and user balance is updated normally
// 6. Let the lock 2 expire and validate that user balance is updated normally post normal unlock operation.
#[test]
fn test_instant_unbond_and_unlock() {
    let admin = String::from("admin");
    let keeper = String::from("keeper");
    let user = String::from("user");

    let coins = vec![
        Coin::new(1000_000_000, "uxprt"),
        Coin::new(1000_000_000, "uatom"),
    ];

    let admin_addr = Addr::unchecked(admin.clone());
    let user_addr = Addr::unchecked(user.clone());
    let keeper_addr = Addr::unchecked(keeper.clone());

    let mut app = mock_app(admin_addr.clone(), coins);

    let (multi_staking_instance, lp_token_addr) = setup_generic(
        &mut app,
        admin_addr.clone(),
        Some(keeper_addr.clone()),
        0,
        // 80 minutes less than 7 days. We should still have 7 tiers
        600_000,
        300,
        500,
    );

    create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000 as u64),
        1000_100_000,
        1000_704_800,
    )
    .unwrap();

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_000_000);
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

    // Step 1: Bond some LP tokens for the user
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
        b.time = Timestamp::from_seconds(1000_302_400);
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

    // Validate that user balance is still zero after bonding till unlock happens
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr,
        Uint128::from(0 as u64),
    );

    // Step 2: Instantly unbond half of the remaining tokens
    instant_unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        Uint128::from(25_000_000 as u64),
    )
    .unwrap();

    // validatate that user balance has increased post instant unbonding.
    // however, fee is deducted from the amount
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr,
        Uint128::from(23_750_000 as u64),
    );

    // validate that the contract owner received the fee since keeper is not set
    assert_user_lp_token_balance(
        &mut app,
        &keeper_addr,
        &lp_token_addr,
        Uint128::from(1_250_000 as u64),
    );

    // validate no new unlock that must have been issued after second unbonding
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
    assert_eq!(token_locks[0].unlock_time, 1_000_902_400);

    // Step 3: Unbond rest of the tokens normally creating a 2nd lock
    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        Uint128::from(25_000_000 as u64),
    )
    .unwrap();

    // Validate that user balance hasn't updated after unbonding
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr,
        Uint128::from(23_750_000 as u64),
    );

    // fetch current token locks
    let token_lock_info = query_raw_token_locks(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
    );

    // Let's unlock first lock
    let token_lock_to_unlock = token_lock_info[0].clone();

    // Step 4: Instantly unlocks the tokens that were locked in step 2 paying the penalty fee
    instant_unlock_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        vec![token_lock_to_unlock],
    )
    .unwrap();

    // validate user balance is updated after instant unlock and fee is deducted
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr,
        Uint128::from(71_250_000 as u64),
    );

    // validate that keeper received the fee
    assert_user_lp_token_balance(
        &mut app,
        &keeper_addr,
        &lp_token_addr,
        Uint128::from(3_750_000 as u64),
    );

    // validate current locks which must only include the lock created in step 3
    let token_lock_info = query_token_locks(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        None,
    );

    assert_eq!(token_lock_info.locks.len(), 1);
    assert_eq!(
        token_lock_info.locks[0].amount,
        Uint128::from(25_000_000 as u64)
    );
    assert_eq!(token_lock_info.locks[0].unlock_time, 1_000_902_400);

    // skip time to 1_000_902_400
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_902_400);
        b.height = b.height + 100;
    });

    // Step 5: Unlocks the tokens that were locked in step 3
    unlock_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
    );

    // validate user balance is updated after unlock
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr,
        Uint128::from(96_250_000 as u64),
    );

    // try multi-lock scenario. try creating multiple locks of same token amount to be unlocked at the same time
    bond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        Uint128::from(90_000_000 as u64),
    )
    .unwrap();

    // unbond small amount
    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        Uint128::from(10_000_000 as u64),
    )
    .unwrap();

    // unbond same amount again
    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        Uint128::from(10_000_000 as u64),
    )
    .unwrap();

    // validate that 2 locks are created
    let token_lock_info = query_raw_token_locks(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
    );

    assert_eq!(token_lock_info.len(), 2);
    assert_eq!(token_lock_info[0].amount, Uint128::from(10_000_000 as u64));
    assert_eq!(token_lock_info[0].unlock_time, 1_001_502_400);

    assert_eq!(token_lock_info[1].amount, Uint128::from(10_000_000 as u64));
    assert_eq!(token_lock_info[1].unlock_time, 1_001_502_400);

    // fetch current fee tiers for unlock
    let fee_tiers = query_instant_unlock_fee_tiers(&mut app, &multi_staking_instance);

    // validate fee tiers
    assert_eq!(fee_tiers.len(), 7);

    // validate that all the fee tier times are at day boundaries
    for i in 0..fee_tiers.len() {
        let start = fee_tiers[i].seconds_till_unlock_start;
        let end = fee_tiers[i].seconds_till_unlock_end;

        assert!(start < end);

        // validate that the start are at day boundaries
        assert_eq!(start % 86400, 0);

        // validate that end matches the start of the next tier
        if i < fee_tiers.len() - 1 {
            assert_eq!(end, fee_tiers[i + 1].seconds_till_unlock_start);
        }

        // validate that the last tier ends at the unlock period
        if i == fee_tiers.len() - 1 {
            assert_eq!(end, 600_000);
        }
    }

    // validate exact fee tiers
    assert_eq!(fee_tiers[0].unlock_fee_bp, 300u64);
    assert_eq!(fee_tiers[1].unlock_fee_bp, 334u64);
    assert_eq!(fee_tiers[2].unlock_fee_bp, 367u64);
    assert_eq!(fee_tiers[3].unlock_fee_bp, 400u64);
    assert_eq!(fee_tiers[4].unlock_fee_bp, 434u64);
    assert_eq!(fee_tiers[5].unlock_fee_bp, 467u64);
    assert_eq!(fee_tiers[6].unlock_fee_bp, 500u64);

    // validate the fee being charged for instant unlock
    let fee = query_instant_lp_unlock_fee(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        token_lock_info[0].clone(),
    );

    assert_eq!(fee.unlock_fee_bp, 500u64);
    assert_eq!(fee.time_until_lock_expiry, 600_000);
    assert_eq!(fee.unlock_fee, Uint128::from(500_000 as u64));

    // increase time to middle of 2nd tier
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_902_400 + 86400 * 2 + 43200);
        b.height = b.height + 100;
    });

    // fetch the fee again
    let fee = query_instant_lp_unlock_fee(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        token_lock_info[0].clone(),
    );

    assert_eq!(fee.time_until_lock_expiry, 600_000 - 86400 * 2 - 43200);
    assert_eq!(fee.unlock_fee_bp, 434u64);
    assert_eq!(fee.unlock_fee, Uint128::from(434_000 as u64));

    // unlock only one of the similar locks
    instant_unlock_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        vec![token_lock_info[0].clone()],
    )
    .unwrap();

    // validate that fee is correctly transferred to the keeper
    assert_user_lp_token_balance(
        &mut app,
        &keeper_addr,
        &lp_token_addr,
        Uint128::from(3_750_000 + 434_000 as u64),
    );

    // validate that only one lock is left
    let token_lock_info = query_raw_token_locks(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
    );

    assert_eq!(token_lock_info.len(), 1);
    assert_eq!(token_lock_info[0].amount, Uint128::from(10_000_000 as u64));
    assert_eq!(token_lock_info[0].unlock_time, 1_001_502_400);

    // skip time to 1_000_303_500
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_001_502_400);
        b.height = b.height + 100;
    });

    // unlock the remaining lock
    unlock_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
    );

    // validate user balance is updated after unlock
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr,
        Uint128::from(25_816_000 as u64),
    );

    // validate rest is bonded for the user in the staking contract
    assert_user_bonded_amount(
        &mut app,
        &user_addr,
        &multi_staking_instance,
        &lp_token_addr,
        Uint128::from(70_000_000 as u64),
    );
}
