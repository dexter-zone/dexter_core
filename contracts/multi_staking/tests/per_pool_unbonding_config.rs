use cosmwasm_std::{Addr, Coin, Timestamp, Uint128};
use cw_multi_test::Executor;
use dexter::multi_staking::{UnbondConfig, InstantUnbondConfig, ExecuteMsg, QueryMsg, UnlockFeeTier, InstantLpUnlockFee};
use utils::{store_multi_staking_contract, instantiate_multi_staking_contract, store_lp_token_contract};

use crate::utils::{
    assert_user_lp_token_balance, bond_lp_tokens,
    instant_unbond_lp_tokens, instant_unlock_lp_tokens,
    mint_lp_tokens_to_addr, mock_app, query_token_locks, unbond_lp_tokens, create_lp_token
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

    let multi_staking_code_id = store_multi_staking_contract(&mut app);
    
    let multi_staking_instance = instantiate_multi_staking_contract(
        &mut app,
        multi_staking_code_id,
        admin_addr.clone(),
        keeper_addr.clone(),
        600u64,
        200u64,
        500u64,
        240u64,
    );

    // let cw20_code_id = store_cw20_contract(app);
    let lp_token_code_id = store_lp_token_contract(&mut app);

    let lp_token_addr_1 = create_lp_token(
        &mut app,
        lp_token_code_id,
        admin_addr.clone(),
        "Dummy LP Token".to_string(),
    );

    // Allow LP token in the multi staking contract
    app.execute_contract(
        admin_addr.clone(),
        multi_staking_instance.clone(),
        &ExecuteMsg::AllowLpToken {
            lp_token: lp_token_addr_1.clone(),
        },
        &vec![],
    )
    .unwrap();

     // let cw20_code_id = store_cw20_contract(app);
     let lp_token_code_id = store_lp_token_contract(&mut app);

     let lp_token_addr_2 = create_lp_token(
         &mut app,
         lp_token_code_id,
         admin_addr.clone(),
         "Dummy LP Token".to_string(),
     );

    // Allow LP token in the multi staking contract for user
    app.execute_contract(
        admin_addr.clone(),
        multi_staking_instance.clone(),
        &ExecuteMsg::AllowLpToken {
            lp_token: lp_token_addr_2.clone(),
        },
        &vec![],
    ).unwrap();


    // disbale ILPU for lp token 2 and validate
    let unbond_config = UnbondConfig {
        unlock_period: 1000u64,
        instant_unbond_config: InstantUnbondConfig::Disabled
    };

    app.execute_contract(
        admin_addr.clone(),
        multi_staking_instance.clone(),
        &ExecuteMsg::SetCustomUnbondConfig {
            lp_token: lp_token_addr_2.clone(),
            unbond_config: unbond_config.clone(),
        },
        &vec![],
    ).unwrap();

    // query unbond config and validate
    let query = QueryMsg::UnbondConfig { lp_token: Some(lp_token_addr_2.clone()) };
    let res: UnbondConfig = app.wrap().query_wasm_smart(
        multi_staking_instance.clone(),
        &query,
    ).unwrap();

    assert_eq!(res, unbond_config);

    // query fee tiers and validate that we get error in the query
    let query = QueryMsg::InstantUnlockFeeTiers { lp_token: lp_token_addr_2.clone() };
    let res: Result<Vec<UnlockFeeTier>, _> = app.wrap().query_wasm_smart(
        multi_staking_instance.clone(),
        &query,
    );

    // assert that we get error in the query
    assert!(res.is_err());
    // assert error is that ILPU is disabled
    assert_eq!(res.err().unwrap().to_string(), "Generic error: Querier contract error: Instant unbond/unlock is disabled for this LP");

    // bond some LP tokens and validate that instant unbond fails
    let user_bond_amount = Uint128::from(100000u128);

    // mint some LP tokens to user
    mint_lp_tokens_to_addr(
        &mut app,
        &admin_addr,
        &lp_token_addr_2,
        &user_addr,
        user_bond_amount,
    );

    bond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr_2,
        &user_addr,
        user_bond_amount,
    ).unwrap();

    // try to instant unbond and validate that it fails
    let unbond_amount = Uint128::from(10000u128);
    let res = instant_unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr_2,
        &user_addr,
        unbond_amount,
    );

    // assert that we get error in the response
    assert!(res.is_err());
    // assert error is that ILPU is disabled
    assert_eq!(res.err().unwrap().root_cause().to_string(), "Instant unbond/unlock is disabled for this LP");

    // try to unbond normally and validate that it succeeds
    let unbond_amount = Uint128::from(10000u128);

    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr_2,
        &user_addr,
        unbond_amount,
    ).unwrap();

    // try to instant unlock some LP tokens and validate that it fails
    let token_lock_info = query_token_locks(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr_2,
        &user_addr,
        None
    );

    let locks = token_lock_info.locks;

    let res = instant_unlock_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr_2,
        &user_addr,
        // there's only one lock so we can use the full array
        locks.clone(),
    );

    // assert that we get error in the response
    assert!(res.is_err());
    // assert error is that ILPU is disabled
    assert_eq!(res.err().unwrap().root_cause().to_string(), "Instant unbond/unlock is disabled for this LP");

    // Add a custom unbonding config for LP token 2
    let unbond_config = UnbondConfig {
        unlock_period: 1000u64,
        instant_unbond_config: InstantUnbondConfig::Enabled { 
            min_fee: 200u64,
            max_fee: 500u64,
            fee_tier_interval: 300u64,
        }
    };

    app.execute_contract(
        admin_addr.clone(),
        multi_staking_instance.clone(),
        &ExecuteMsg::SetCustomUnbondConfig {
            lp_token: lp_token_addr_2.clone(),
            unbond_config: unbond_config.clone(),
        },
        &vec![],
    ).unwrap();


    // query fee tiers and validate
    let query = QueryMsg::DefaultInstantUnlockFeeTiers { };
    let res: Vec<UnlockFeeTier> = app.wrap().query_wasm_smart(
        multi_staking_instance.clone(),
        &query,
    ).unwrap();
    
    let default_fee_tiers = vec![
        UnlockFeeTier {
            seconds_till_unlock_end: 240u64,
            seconds_till_unlock_start: 0u64,
            unlock_fee_bp: 200u64,
        },
        UnlockFeeTier {
            seconds_till_unlock_end: 480u64,
            seconds_till_unlock_start: 240u64,
            unlock_fee_bp: 350u64,
        },
        UnlockFeeTier {
            seconds_till_unlock_end: 600u64,
            seconds_till_unlock_start: 480u64,
            unlock_fee_bp: 500u64,
        },
    ];
    assert_eq!(res, default_fee_tiers);

    // query fee tiers for the lp token 2 and validate
    let query = QueryMsg::InstantUnlockFeeTiers { lp_token: lp_token_addr_2.clone() };
    let res: Vec<UnlockFeeTier> = app.wrap().query_wasm_smart(
        multi_staking_instance.clone(),
        &query,
    ).unwrap();

    let expected_fee_tiers = vec![
        UnlockFeeTier {
            seconds_till_unlock_end: 300u64,
            seconds_till_unlock_start: 0u64,
            unlock_fee_bp: 200u64,
        },
        UnlockFeeTier {
            seconds_till_unlock_end: 600u64,
            seconds_till_unlock_start: 300u64,
            unlock_fee_bp: 300u64,
        },
        UnlockFeeTier {
            seconds_till_unlock_end: 900u64,
            seconds_till_unlock_start: 600u64,
            unlock_fee_bp: 400u64,
        },
        UnlockFeeTier {
            seconds_till_unlock_end: 1000u64,
            seconds_till_unlock_start: 900u64,
            unlock_fee_bp: 500u64,
        },
    ];
    assert_eq!(res, expected_fee_tiers);

     // query fee tiers for the lp token 2 and validate
     let query = QueryMsg::InstantUnlockFeeTiers { lp_token: lp_token_addr_1.clone() };
     let res: Vec<UnlockFeeTier> = app.wrap().query_wasm_smart(
         multi_staking_instance.clone(),
         &query,
     ).unwrap();
 
     assert_eq!(res, default_fee_tiers);

    // query instant unlock fee for lp token 2 and validate that we get a fee now and not an error
    let query = QueryMsg::InstantUnlockFee { 
        lp_token: lp_token_addr_2.clone(),
        token_lock: locks[0].clone(),
        user: user_addr.clone(),
    };

    let res: InstantLpUnlockFee = app.wrap().query_wasm_smart(
        multi_staking_instance.clone(),
        &query,
    ).unwrap();

    assert_eq!(res, InstantLpUnlockFee {
        unlock_fee:Uint128::from(500u128),
        time_until_lock_expiry: 1000u64,
        unlock_fee_bp: 500u64,
        unlock_amount: Uint128::from(10000u128), 
    });

    // increase the time by 201 seconds and validate that we moved to the correct tier
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_000_201);
        b.height = b.height + 200;
    });

    // query instant unlock fee for lp token 2 and validate that we get a fee now and not an error
    let query = QueryMsg::InstantUnlockFee { 
        lp_token: lp_token_addr_2.clone(),
        token_lock: locks[0].clone(),
        user: user_addr.clone(),
    };

    let res: InstantLpUnlockFee = app.wrap().query_wasm_smart(
        multi_staking_instance.clone(),
        &query,
    ).unwrap();

    assert_eq!(res, InstantLpUnlockFee {
        unlock_fee:Uint128::from(400u128),
        time_until_lock_expiry: 799u64,
        unlock_fee_bp: 400u64,
        unlock_amount: Uint128::from(10000u128),
    });

    // perform instant unlock and validate that it succeeds
    let res = instant_unlock_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr_2,
        &user_addr,
        // there's only one lock so we can use the full array
        locks.clone(),
    );

    // assert that we get no error in the response
    assert!(res.is_ok());

    // validate that the LP tokens after fee are transferred to the user
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr_2,
        Uint128::from(9600u128),
    );

    // validate that the fee is transferred to the keeper
    assert_user_lp_token_balance(
        &mut app,
        &keeper_addr,
        &lp_token_addr_2,
        Uint128::from(400u128),
    );

    // let's add another token lock before we try the next experiment
    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr_2,
        &user_addr,
        Uint128::from(10000u128),
    ).unwrap();

    // create another one
    unbond_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr_2,
        &user_addr,
        Uint128::from(10000u128),
    ).unwrap();

    // query locks
    let token_lock_info = query_token_locks(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr_2,
        &user_addr,
        None
    );

    let locks = token_lock_info.locks;

    // let's unset the custom unbond config and see what's the fee now,
    // as the lock now would lie outside of the unlock_period range and thus out of every 
    // fee tier range. Ideally, the fee should be max fee and we should not get an error

    // unset the custom unbond config for LP token 2
    app.execute_contract(
        admin_addr.clone(),
        multi_staking_instance.clone(),
        &ExecuteMsg::UnsetCustomUnbondConfig {
            lp_token: lp_token_addr_2.clone(),
        },
        &vec![],
    ).unwrap();

    // query new unlock fee tiers and validate that we get the default fee tiers
    let query = QueryMsg::InstantUnlockFeeTiers { lp_token: lp_token_addr_2.clone() };
    let res: Vec<UnlockFeeTier> = app.wrap().query_wasm_smart(
        multi_staking_instance.clone(),
        &query,
    ).unwrap();

    assert_eq!(res, default_fee_tiers);

    // now let's query the unlock fee for our lock and validate that we get the max fee
    let query = QueryMsg::InstantUnlockFee { 
        lp_token: lp_token_addr_2.clone(),
        token_lock: locks[0].clone(),
        user: user_addr.clone(),
    };

    let res: InstantLpUnlockFee = app.wrap().query_wasm_smart(
        multi_staking_instance.clone(),
        &query,
    ).unwrap();

    assert_eq!(res, InstantLpUnlockFee {
        unlock_fee:Uint128::from(500u128),
        time_until_lock_expiry: 1000u64,
        unlock_fee_bp: 500u64,
        unlock_amount: Uint128::from(10000u128),
    });

    // increase time to near the first tier boundary and validate same fee is being charged
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_000_599);
        b.height = b.height + 300;
    });

    let res: InstantLpUnlockFee = app.wrap().query_wasm_smart(
        multi_staking_instance.clone(),
        &query,
    ).unwrap();

    assert_eq!(res, InstantLpUnlockFee {
        unlock_fee:Uint128::from(500u128),
        time_until_lock_expiry: 602u64,
        unlock_fee_bp: 500u64,
        unlock_amount: Uint128::from(10000u128),
    });

    // now, at this time let's unlock the identical and lock and validate that
    // 1. unlock succeeds
    // 2. user balance is updated as expected

    // perform instant unlock and validate that it succeeds
    let res = instant_unlock_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr_2,
        &user_addr,
        // there's only one lock so we can use the full array
        [locks[1].clone()].to_vec(),
    );

    // assert that we get no error in the response
    assert!(res.is_ok());
    // validate that the LP tokens after fee are transferred to the user
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr_2,
        Uint128::from(19100u128),
    );

    // increase time to be just in the next tier and validate that we get the next tier fee
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_000_722);
        b.height = b.height + 2;
    });

    let res: InstantLpUnlockFee = app.wrap().query_wasm_smart(
        multi_staking_instance.clone(),
        &query,
    ).unwrap();

    assert_eq!(res, InstantLpUnlockFee {
        unlock_fee:Uint128::from(350u128),
        time_until_lock_expiry: 479u64,
        unlock_fee_bp: 350u64,
        unlock_amount: Uint128::from(10000u128),
    });

    // let's unlock the lock and validate fee is charged correctly
    // perform instant unlock and validate that it succeeds
    let res = instant_unlock_lp_tokens(
        &mut app,
        &multi_staking_instance,
        &lp_token_addr_2,
        &user_addr,
        // there's only one lock so we can use the full array
        [locks[0].clone()].to_vec(),
    );

    // assert that we get no error in the response
    assert!(res.is_ok());
    // validate that the LP tokens after fee are transferred to the user
    assert_user_lp_token_balance(
        &mut app,
        &user_addr,
        &lp_token_addr_2,
        Uint128::from(28750u128),
    );
}