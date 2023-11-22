use crate::utils::{create_reward_schedule, mock_app, setup};
use cosmwasm_std::{Addr, Coin, Uint128};
use cw_multi_test::Executor;
use dexter::asset::AssetInfo;
use dexter::multi_staking::{ExecuteMsg, QueryMsg};

pub mod utils;

#[test]
fn test_allow_lp_token() {
    let admin = String::from("admin");
    let coins = vec![
        Coin::new(1000_000_000, "uxprt"),
        Coin::new(1000_000_000, "uatom"),
    ];
    let admin_addr = Addr::unchecked(admin.clone());
    let mut app = mock_app(admin_addr.clone(), coins);

    let (multi_staking_instance, lp_token_addr) = setup(&mut app, admin_addr.clone());

    // New LP token
    let new_lp_token_addr = Addr::unchecked("new_lp_token".to_string());

    // Create a new reward schedule
    let unauthorized_addr = Addr::unchecked("unauthorized".to_string());
    let response = app.execute_contract(
        unauthorized_addr.clone(),
        multi_staking_instance.clone(),
        &ExecuteMsg::AllowLpToken {
            lp_token: new_lp_token_addr.clone(),
        },
        &vec![],
    );

    assert!(response.is_err());
    // Check the error is amount insufficied fundsinsufficient funds
    assert_eq!(
        response.unwrap_err().root_cause().to_string(),
        "Unauthorized"
    );

    // Allow lp token for reward
    let response = app.execute_contract(
        admin_addr.clone(),
        multi_staking_instance.clone(),
        &ExecuteMsg::AllowLpToken {
            lp_token: new_lp_token_addr.clone(),
        },
        &vec![],
    );

    // response should be ok
    response.unwrap();
    // assert_eq!(response.is_ok(), true);

    // Check if lp token is allowed for reward
    let allowed_lp_tokens: Vec<Addr> = app
        .wrap()
        .query_wasm_smart(
            multi_staking_instance.clone(),
            &QueryMsg::AllowedLPTokensForReward {},
        )
        .unwrap();

    assert_eq!(allowed_lp_tokens.len(), 2);
    assert_eq!(allowed_lp_tokens[0], lp_token_addr);
    assert_eq!(allowed_lp_tokens[1], new_lp_token_addr);
}

#[test]
fn test_reward_schedule_creation() {
    // setup
    let admin_addr = Addr::unchecked("admin");
    let user1_addr = Addr::unchecked("user1");
    let user2_addr = Addr::unchecked("user2");
    let coins = vec![
        Coin::new(1000_000_000, "uxprt"),
        Coin::new(1000_000_000, "uatom"),
    ];
    let mut app = mock_app(admin_addr.clone(), coins);
    let (multi_staking_instance, lp_token_addr) = setup(&mut app, admin_addr.clone());

    // bootstrap user addresses with tokens
    app.send_tokens(
        admin_addr.clone(),
        user1_addr.clone(),
        &[Coin {
            denom: "uxprt".to_string(),
            amount: Uint128::new(200_000_000u128),
        }],
    )
    .unwrap();
    app.send_tokens(
        admin_addr.clone(),
        user2_addr.clone(),
        &[Coin {
            denom: "uxprt".to_string(),
            amount: Uint128::new(200_000_000u128),
        }],
    )
    .unwrap();

    // trying to create a reward schedule by a non-admin user should fail
    let res = create_reward_schedule(
        &mut app,
        &user1_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-1".to_string(),
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000u64),
        1000_001_000,
        1000_002_000,
    );
    assert_eq!(res.is_err(), true);
    assert_eq!(
        res.unwrap_err().root_cause().to_string(),
        "Unauthorized".to_string()
    );

    // trying to create a reward schedule for unallowed LP token should fail
    let res = create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &Addr::unchecked("unknown_token"),
        "prop-1".to_string(),
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000u64),
        1000_001_000,
        1000_002_000,
    );
    assert_eq!(res.is_err(), true);
    assert_eq!(
        res.unwrap_err().root_cause().to_string(),
        "LP Token is not allowed for staking"
    );

    // trying to propose a reward schedule with invalid start & end block time should fail
    let res = create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-1".to_string(),
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000u64),
        1000_002_000,
        1000_001_000,
    );
    assert_eq!(res.is_err(), true);
    assert_eq!(res.unwrap_err().root_cause().to_string(), "Invalid block times. Start block time 1000002000 is greater than end block time 1000001000");

    // trying to propose a reward schedule too soon in future should fail
    let res = create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-1".to_string(),
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000u64),
        1000_001_000,
        1000_002_000,
    );
    assert_eq!(res.is_err(), false);
    // We removed the requirement of a delay in the reward schedule start time so this doesn't apply anymore
    // assert_eq!(res.unwrap_err().root_cause().to_string(), "Start block time must be at least 259200 seconds in future at the time of proposal to give enough time to review");

    // creation of a valid reward schedule should succeed by admin
    let _reward_schedule_id = create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-1".to_string(),
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();

    // TODO: Add more tests for reward schedule creation
}

#[test]
fn test_reward_schedule_queries() {
    // setup
    let admin_addr = Addr::unchecked("admin");
    let user1_addr = Addr::unchecked("user1");
    let user2_addr = Addr::unchecked("user2");
    let coins = vec![
        Coin::new(1000_000_000, "uxprt"),
        Coin::new(1000_000_000, "uatom"),
    ];
    let mut app = mock_app(admin_addr.clone(), coins);
    let (multi_staking_instance, lp_token_addr) = setup(&mut app, admin_addr.clone());

    // Allow another LP token in the multi staking contract
    let lp_token1_addr = Addr::unchecked("lp-token-1");
    app.execute_contract(
        admin_addr.clone(),
        multi_staking_instance.clone(),
        &ExecuteMsg::AllowLpToken {
            lp_token: lp_token1_addr.clone(),
        },
        &vec![],
    )
    .unwrap();

    // bootstrap user addresses with tokens
    app.send_tokens(
        admin_addr.clone(),
        user1_addr.clone(),
        &[Coin {
            denom: "uxprt".to_string(),
            amount: Uint128::new(200_000_000u128),
        }],
    )
    .unwrap();
    app.send_tokens(
        admin_addr.clone(),
        user2_addr.clone(),
        &[Coin {
            denom: "uxprt".to_string(),
            amount: Uint128::new(200_000_000u128),
        }],
    )
    .unwrap();

    // propose some reward schedules
    create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-1".to_string(),
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(10_000_000u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();
    create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-2".to_string(),
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(10_000_000u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();
    create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token1_addr,
        "prop-3".to_string(),
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(10_000_000u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();
    create_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-4".to_string(),
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(10_000_000u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();
}
