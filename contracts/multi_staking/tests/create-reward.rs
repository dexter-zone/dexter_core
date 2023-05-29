use crate::utils::{
    drop_reward_schedule, mock_app, propose_reward_schedule, review_reward_schedule, setup,
};
use cosmwasm_std::{Addr, Coin, StdResult, Uint128};
use cw_multi_test::Executor;
use dexter::asset::{Asset, AssetInfo};
use dexter::multi_staking::{
    ExecuteMsg, ProposedRewardSchedule, ProposedRewardSchedulesResponse, QueryMsg,
    ReviewProposedRewardSchedule,
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
fn test_reward_schedule_proposal_flow() {
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

    // trying to propose a reward schedule for unallowed LP token should fail
    let res = propose_reward_schedule(
        &mut app,
        &user1_addr,
        &multi_staking_instance,
        &Addr::unchecked("unknown_token"),
        "prop-1".to_string(),
        None,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000 as u64),
        1000_001_000,
        1000_002_000,
    );
    assert_eq!(res.is_err(), true);
    assert_eq!(
        res.unwrap_err().root_cause().to_string(),
        "LP Token is not allowed for staking"
    );

    // trying to propose a reward schedule with invalid start & end block time should fail
    let res = propose_reward_schedule(
        &mut app,
        &user1_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-1".to_string(),
        None,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000 as u64),
        1000_002_000,
        1000_001_000,
    );
    assert_eq!(res.is_err(), true);
    assert_eq!(res.unwrap_err().root_cause().to_string(), "Invalid block times. Start block time 1000002000 is greater than end block time 1000001000");

    // trying to propose a reward schedule too soon in future should fail
    let res = propose_reward_schedule(
        &mut app,
        &user1_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-1".to_string(),
        None,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000 as u64),
        1000_001_000,
        1000_002_000,
    );
    assert_eq!(res.is_err(), true);
    assert_eq!(res.unwrap_err().root_cause().to_string(), "Start block time must be at least 259200 seconds in future at the time of proposal to give enough time to review");

    // proposing a valid reward schedule should succeed
    let prop1_id = propose_reward_schedule(
        &mut app,
        &user1_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-1".to_string(),
        None,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000 as u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();

    // propose few more reward schedules by user2
    let prop2_id = propose_reward_schedule(
        &mut app,
        &user2_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-2".to_string(),
        None,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(100_000_000 as u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();

    let prop3_id = propose_reward_schedule(
        &mut app,
        &user2_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-3".to_string(),
        None,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(10_000_000 as u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();

    // reviewing proposals by non-admin should fail
    let res = review_reward_schedule(
        &mut app,
        &user1_addr,
        &multi_staking_instance,
        vec![ReviewProposedRewardSchedule {
            proposal_id: prop1_id,
            approve: true,
        }],
    );
    assert_eq!(res.is_err(), true);
    assert_eq!(res.unwrap_err().root_cause().to_string(), "Unauthorized");

    // reviewing proposals by admin should work
    review_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        vec![
            ReviewProposedRewardSchedule {
                proposal_id: prop1_id,
                approve: false,
            },
            ReviewProposedRewardSchedule {
                proposal_id: prop2_id,
                approve: true,
            },
        ],
    )
    .unwrap();

    // dropping proposal by non-proposer should fail
    let res = drop_reward_schedule(&mut app, &user2_addr, &multi_staking_instance, prop1_id);
    assert_eq!(res.is_err(), true);
    assert_eq!(res.unwrap_err().root_cause().to_string(), "Unauthorized");

    // dropping an approved proposal should fail
    let res = drop_reward_schedule(&mut app, &user2_addr, &multi_staking_instance, prop2_id);
    assert_eq!(res.is_err(), true);
    assert_eq!(
        res.unwrap_err().root_cause().to_string(),
        "dexter::multi_staking::ProposedRewardSchedule not found"
    );

    // dropping proposal by the proposer should work

    // 1. dropping a rejected proposal

    // assert user1 balance
    let user1_balance = app
        .wrap()
        .query_balance(user1_addr.clone(), "uxprt")
        .unwrap()
        .amount;
    assert_eq!(user1_balance, Uint128::from(100_000_000u128)); // 200 - 100

    drop_reward_schedule(&mut app, &user1_addr, &multi_staking_instance, prop1_id).unwrap();

    // ensure user got the refund
    let user1_balance = app
        .wrap()
        .query_balance(user1_addr.clone(), "uxprt")
        .unwrap()
        .amount;
    assert_eq!(user1_balance, Uint128::from(200_000_000u128)); // 200 - 100 + 100

    // 2. dropping a non-reviewed proposal

    // assert user1 balance
    let user2_balance = app
        .wrap()
        .query_balance(user2_addr.clone(), "uxprt")
        .unwrap()
        .amount;
    assert_eq!(user2_balance, Uint128::from(90_000_000u128)); // 200 - 100 - 10

    drop_reward_schedule(&mut app, &user2_addr, &multi_staking_instance, prop3_id).unwrap();

    // ensure user got the refund
    let user2_balance = app
        .wrap()
        .query_balance(user2_addr.clone(), "uxprt")
        .unwrap()
        .amount;
    assert_eq!(user2_balance, Uint128::from(100_000_000u128)); // 200 - 100 - 10 + 10
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
    propose_reward_schedule(
        &mut app,
        &user1_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-1".to_string(),
        Some("This is proposal 1".to_string()),
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(10_000_000 as u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();
    propose_reward_schedule(
        &mut app,
        &user1_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-2".to_string(),
        None,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(10_000_000 as u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();
    propose_reward_schedule(
        &mut app,
        &user1_addr,
        &multi_staking_instance,
        &lp_token1_addr,
        "prop-3".to_string(),
        None,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(10_000_000 as u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();
    propose_reward_schedule(
        &mut app,
        &user2_addr,
        &multi_staking_instance,
        &lp_token_addr,
        "prop-4".to_string(),
        None,
        AssetInfo::NativeToken {
            denom: "uxprt".to_string(),
        },
        Uint128::from(10_000_000 as u64),
        1000_301_000,
        1000_302_000,
    )
    .unwrap();

    // ensure get query works
    let res: ProposedRewardSchedule = app
        .wrap()
        .query_wasm_smart(
            multi_staking_instance.clone(),
            &QueryMsg::ProposedRewardSchedule { proposal_id: 1 },
        )
        .unwrap();
    let expected_reward_schedule = ProposedRewardSchedule {
        lp_token: lp_token_addr.clone(),
        proposer: user1_addr.clone(),
        title: "prop-1".to_string(),
        description: Some("This is proposal 1".to_string()),
        asset: Asset {
            info: AssetInfo::NativeToken {
                denom: "uxprt".to_string(),
            },
            amount: Uint128::from(10_000_000 as u64),
        },
        start_block_time: 1000_301_000,
        end_block_time: 1000_302_000,
        rejected: false,
    };
    assert_eq!(res, expected_reward_schedule);

    // ensure get query fails for non-existent proposal
    let res: StdResult<ProposedRewardSchedule> = app.wrap().query_wasm_smart(
        multi_staking_instance.clone(),
        &QueryMsg::ProposedRewardSchedule { proposal_id: 5 },
    );
    assert_eq!(res.is_err(), true);
    assert_eq!(res.unwrap_err().to_string().contains("not found"), true);

    // reject a proposal
    review_reward_schedule(
        &mut app,
        &admin_addr,
        &multi_staking_instance,
        vec![ReviewProposedRewardSchedule {
            proposal_id: 2,
            approve: false,
        }],
    )
    .unwrap();

    // ensure list query works with pagination

    // combo-1: start_after & limit
    let res: Vec<ProposedRewardSchedulesResponse> = app
        .wrap()
        .query_wasm_smart(
            multi_staking_instance.clone(),
            &QueryMsg::ProposedRewardSchedules {
                start_after: Some(1),
                limit: Some(1),
            },
        )
        .unwrap();
    assert_eq!(
        res,
        vec![ProposedRewardSchedulesResponse {
            proposal_id: 2,
            proposal: ProposedRewardSchedule {
                lp_token: lp_token_addr.clone(),
                proposer: user1_addr.clone(),
                title: "prop-2".to_string(),
                description: None,
                asset: Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uxprt".to_string()
                    },
                    amount: Uint128::from(10_000_000 as u64),
                },
                start_block_time: 1000_301_000,
                end_block_time: 1000_302_000,
                rejected: true,
            }
        },]
    );

    // combo-2: no params
    let res: Vec<ProposedRewardSchedulesResponse> = app
        .wrap()
        .query_wasm_smart(
            multi_staking_instance.clone(),
            &QueryMsg::ProposedRewardSchedules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        vec![
            ProposedRewardSchedulesResponse {
                proposal_id: 1,
                proposal: ProposedRewardSchedule {
                    lp_token: lp_token_addr.clone(),
                    proposer: user1_addr.clone(),
                    title: "prop-1".to_string(),
                    description: Some("This is proposal 1".to_string()),
                    asset: Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uxprt".to_string()
                        },
                        amount: Uint128::from(10_000_000 as u64),
                    },
                    start_block_time: 1000_301_000,
                    end_block_time: 1000_302_000,
                    rejected: false,
                }
            },
            ProposedRewardSchedulesResponse {
                proposal_id: 2,
                proposal: ProposedRewardSchedule {
                    lp_token: lp_token_addr.clone(),
                    proposer: user1_addr.clone(),
                    title: "prop-2".to_string(),
                    description: None,
                    asset: Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uxprt".to_string()
                        },
                        amount: Uint128::from(10_000_000 as u64),
                    },
                    start_block_time: 1000_301_000,
                    end_block_time: 1000_302_000,
                    rejected: true,
                }
            },
            ProposedRewardSchedulesResponse {
                proposal_id: 3,
                proposal: ProposedRewardSchedule {
                    lp_token: lp_token1_addr.clone(),
                    proposer: user1_addr.clone(),
                    title: "prop-3".to_string(),
                    description: None,
                    asset: Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uxprt".to_string()
                        },
                        amount: Uint128::from(10_000_000 as u64),
                    },
                    start_block_time: 1000_301_000,
                    end_block_time: 1000_302_000,
                    rejected: false,
                }
            },
            ProposedRewardSchedulesResponse {
                proposal_id: 4,
                proposal: ProposedRewardSchedule {
                    lp_token: lp_token_addr.clone(),
                    proposer: user2_addr.clone(),
                    title: "prop-4".to_string(),
                    description: None,
                    asset: Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uxprt".to_string()
                        },
                        amount: Uint128::from(10_000_000 as u64),
                    },
                    start_block_time: 1000_301_000,
                    end_block_time: 1000_302_000,
                    rejected: false,
                }
            },
        ]
    );

    // drop a proposal
    drop_reward_schedule(&mut app, &user1_addr, &multi_staking_instance, 3).unwrap();
    // query again, the query should reflect the updated state
    let res: Vec<ProposedRewardSchedulesResponse> = app
        .wrap()
        .query_wasm_smart(
            multi_staking_instance.clone(),
            &QueryMsg::ProposedRewardSchedules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        vec![
            ProposedRewardSchedulesResponse {
                proposal_id: 1,
                proposal: ProposedRewardSchedule {
                    lp_token: lp_token_addr.clone(),
                    proposer: user1_addr.clone(),
                    title: "prop-1".to_string(),
                    description: Some("This is proposal 1".to_string()),
                    asset: Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uxprt".to_string()
                        },
                        amount: Uint128::from(10_000_000 as u64),
                    },
                    start_block_time: 1000_301_000,
                    end_block_time: 1000_302_000,
                    rejected: false,
                }
            },
            ProposedRewardSchedulesResponse {
                proposal_id: 2,
                proposal: ProposedRewardSchedule {
                    lp_token: lp_token_addr.clone(),
                    proposer: user1_addr.clone(),
                    title: "prop-2".to_string(),
                    description: None,
                    asset: Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uxprt".to_string()
                        },
                        amount: Uint128::from(10_000_000 as u64),
                    },
                    start_block_time: 1000_301_000,
                    end_block_time: 1000_302_000,
                    rejected: true,
                }
            },
            ProposedRewardSchedulesResponse {
                proposal_id: 4,
                proposal: ProposedRewardSchedule {
                    lp_token: lp_token_addr.clone(),
                    proposer: user2_addr.clone(),
                    title: "prop-4".to_string(),
                    description: None,
                    asset: Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uxprt".to_string()
                        },
                        amount: Uint128::from(10_000_000 as u64),
                    },
                    start_block_time: 1000_301_000,
                    end_block_time: 1000_302_000,
                    rejected: false,
                }
            },
        ]
    );
}
