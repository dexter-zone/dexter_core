use cosmwasm_std::{Addr, Coin, Timestamp, Uint128};
use dexter::{
    asset::AssetInfo,
};

use crate::utils::{
    assert_user_lp_token_balance, bond_lp_tokens, create_reward_schedule,
    mint_lp_tokens_to_addr, mock_app, query_token_locks, setup, unbond_lp_tokens, unlock_lp_tokens, instant_unbond_lp_tokens, instant_unlock_lp_tokens
};
pub mod utils;

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
    ).unwrap();

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

    // Step 1: Bond some LP tokens for the user
    bond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        Uint128::from(100_000_000 as u64),
    ).unwrap();

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
    ).unwrap();

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
    ).unwrap();

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
        &admin_addr,
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
    assert_eq!(token_locks[0].unlock_time, 1_000_302_500);


    // Step 3: Unbond rest of the tokens normally creating a 2nd lock
    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        Uint128::from(25_000_000 as u64),
    ).unwrap();

    // Validate that user balance hasn't updated after unbonding
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr,
        Uint128::from(23_750_000 as u64),
    );

    // Step 4: Instantly unlocks the tokens that were locked in step 2 paying the penalty fee
    instant_unlock_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr,
        &user_addr,
        vec![0]
    ).unwrap();

    // validate user balance is updated after instant unlock and fee is deducted
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr,
        Uint128::from(71_250_000 as u64),
    );

    // validate the contract owner received the fee since keeper is not set
    assert_user_lp_token_balance(
        &mut app,
        &admin_addr,
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
    assert_eq!(token_lock_info.locks[0].amount, Uint128::from(25_000_000 as u64));
    assert_eq!(token_lock_info.locks[0].unlock_time, 1_000_302_500);

    // skip time to 1_000_302_500
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_302_500);
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

}