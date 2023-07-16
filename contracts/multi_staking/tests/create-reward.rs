use crate::utils::{
     mock_app, create_reward_schedule, setup,
};
use cosmwasm_std::{Addr, Coin, Uint128};
use cw_multi_test::Executor;
use dexter::asset::AssetInfo;
use dexter::multi_staking::{
    QueryMsg, SudoMsg, RewardSchedule,
};

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

    // Allow lp token for reward
    let response = app.wasm_sudo(
        multi_staking_instance.clone(),
        &SudoMsg::AllowLpTokenForReward {
            lp_token: new_lp_token_addr.clone(),
        },
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
    app.wasm_sudo(
        multi_staking_instance.clone(),
        &SudoMsg::AllowLpTokenForReward {
            lp_token: lp_token1_addr.clone(),
        },
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
        &user1_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-1".to_string(),
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(10_000_000 as u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();
    create_reward_schedule(
        &mut app,
        &user1_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-2".to_string(),
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(10_000_000 as u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();
    create_reward_schedule(
        &mut app,
        &user1_addr,
        &multi_staking_instance,
        &lp_token1_addr,
        "prop-3".to_string(),
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(10_000_000 as u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();
    create_reward_schedule(
        &mut app,
        &user2_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-4".to_string(),
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(10_000_000 as u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();

    // validate reward schedule is created
    let res: RewardSchedule = app
        .wrap()
        .query_wasm_smart(
            multi_staking_instance.clone(),
            &QueryMsg::RewardSchedule { id: 1 },
        )
        .unwrap();

    let expected_reward_schedule =  RewardSchedule {
        staking_lp_token: lp_token_addr.clone(),
        title: "prop-1".to_string(),
        creator: user1_addr.clone(),
        amount: Uint128::from(10_000_000 as u64),
        asset: AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        start_block_time: 1000_301_000,
        end_block_time: 1000_302_000,
    };

    assert_eq!(res, expected_reward_schedule);
}
