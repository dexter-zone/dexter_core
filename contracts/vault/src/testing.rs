use crate::contract::{
    accumulate_prices, assert_max_spread, compute_offer_amount, compute_swap, execute, instantiate,
    query_pair_info, query_pool, query_share, query_simulation, reply,
};
use crate::error::ContractError;
use crate::math::{calc_ask_amount, calc_offer_amount, AMP_PRECISION};
use crate::mock_querier::mock_dependencies;

use crate::response::MsgInstantiateContractResponse;
use crate::state::Config;
use astroport::asset::{Asset, AssetInfo, PairInfo};

use astroport::pair::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolResponse, SimulationResponse, StablePoolParams,
    TWAP_PRECISION,
};
use astroport::token::InstantiateMsg as TokenInstantiateMsg;
use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    attr, to_binary, Addr, BankMsg, BlockInfo, Coin, CosmosMsg, Decimal, DepsMut, Env, Reply,
    ReplyOn, Response, StdError, SubMsg, SubMsgResponse, SubMsgResult, Timestamp, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};
use protobuf::Message;

fn store_liquidity_token(deps: DepsMut, msg_id: u64, contract_addr: String) {
    let data = MsgInstantiateContractResponse {
        contract_address: contract_addr,
        data: vec![],
        unknown_fields: Default::default(),
        cached_size: Default::default(),
    }
    .write_to_bytes()
    .unwrap();

    let reply_msg = Reply {
        id: msg_id,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: Some(data.into()),
        }),
    };

    let _res = reply(deps, mock_env(), reply_msg.clone()).unwrap();
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    deps.querier.with_token_balances(&[(
        &String::from("asset0000"),
        &[(&String::from(MOCK_CONTRACT_ADDR), &Uint128::new(123u128))],
    )]);

    let msg = InstantiateMsg {
        factory_addr: String::from("factory"),
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
        ],
        token_code_id: 10u64,
        init_params: Some(to_binary(&StablePoolParams { amp: 100 }).unwrap()),
    };

    let sender = "addr0000";
    // We can just call .unwrap() to assert this was a success
    let env = mock_env();
    let info = mock_info(sender, &[]);
    let res = instantiate(deps.as_mut(), env, info, msg).unwrap();
    

    // Store liquidity token
    store_liquidity_token(deps.as_mut(), 1, "liquidity0000".to_string());

    // It worked, let's query the state
    let pair_info: PairInfo = query_pair_info(deps.as_ref()).unwrap();
    assert_eq!(Addr::unchecked("liquidity0000"), pair_info.liquidity_token);
    assert_eq!(
        pair_info.asset_infos,
        [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000")
            }
        ]
    );
}

#[test]
fn provide_liquidity() {
    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: Uint128::new(200_000000000000000000u128),
    }]);

    deps.querier.with_token_balances(&[
        (
            &String::from("asset0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &Uint128::new(0))],
        ),
        (
            &String::from("liquidity0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &Uint128::new(0))],
        ),
    ]);

    let msg = InstantiateMsg {
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
        ],
        token_code_id: 10u64,
        factory_addr: String::from("factory"),
        init_params: Some(to_binary(&StablePoolParams { amp: 100 }).unwrap()),
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);
    // We can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    // Store the liquidity token
    store_liquidity_token(deps.as_mut(), 1, "liquidity0000".to_string());

    // Successfully provide liquidity for the existing pool
    let msg = ExecuteMsg::ProvideLiquidity {
        assets: [
            Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0000"),
                },
                amount: Uint128::from(100_000000000000000000u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(100_000000000000000000u128),
            },
        ],
        slippage_tolerance: None,
        auto_stake: None,
        receiver: None,
    };

    let env = mock_env();
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100_000000000000000000u128),
        }],
    );
    let res = execute(deps.as_mut(), env.clone().clone(), info, msg).unwrap();
    let transfer_from_msg = res.messages.get(0).expect("no message");
    let mint_msg = res.messages.get(1).expect("no message");
    assert_eq!(
        transfer_from_msg,
        &SubMsg {
            msg: WasmMsg::Execute {
                contract_addr: String::from("asset0000"),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: String::from("addr0000"),
                    recipient: String::from(MOCK_CONTRACT_ADDR),
                    amount: Uint128::from(100_000000000000000000u128),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            id: 0,
            gas_limit: None,
            reply_on: ReplyOn::Never
        }
    );
    assert_eq!(
        mint_msg,
        &SubMsg {
            msg: WasmMsg::Execute {
                contract_addr: String::from("liquidity0000"),
                msg: to_binary(&Cw20ExecuteMsg::Mint {
                    recipient: String::from("addr0000"),
                    amount: Uint128::from(100_000000000000000000u128),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            id: 0,
            gas_limit: None,
            reply_on: ReplyOn::Never,
        }
    );

    // Provide more liquidity using a 1:2 ratio
    deps.querier.with_balance(&[(
        &String::from(MOCK_CONTRACT_ADDR),
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(200_000000000000000000 + 200_000000000000000000 /* user deposit must be pre-applied */),
        }],
    )]);

    deps.querier.with_token_balances(&[
        (
            &String::from("liquidity0000"),
            &[(
                &String::from(MOCK_CONTRACT_ADDR),
                &Uint128::new(100_000000000000000000),
            )],
        ),
        (
            &String::from("asset0000"),
            &[(
                &String::from(MOCK_CONTRACT_ADDR),
                &Uint128::new(200_000000000000000000),
            )],
        ),
    ]);

    let msg = ExecuteMsg::ProvideLiquidity {
        assets: [
            Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0000"),
                },
                amount: Uint128::from(100_000000000000000000u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(200_000000000000000000u128),
            },
        ],
        slippage_tolerance: None,
        auto_stake: None,
        receiver: None,
    };

    let env = mock_env_with_block_time(env.block.time.seconds() + 1000);
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(200_000000000000000000u128),
        }],
    );

    let res: Response = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    let transfer_from_msg = res.messages.get(0).expect("no message");
    let mint_msg = res.messages.get(1).expect("no message");
    assert_eq!(
        transfer_from_msg,
        &SubMsg {
            msg: WasmMsg::Execute {
                contract_addr: String::from("asset0000"),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: String::from("addr0000"),
                    recipient: String::from(MOCK_CONTRACT_ADDR),
                    amount: Uint128::from(100_000000000000000000u128),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            id: 0,
            gas_limit: None,
            reply_on: ReplyOn::Never,
        }
    );
    assert_eq!(
        mint_msg,
        &SubMsg {
            msg: WasmMsg::Execute {
                contract_addr: String::from("liquidity0000"),
                msg: to_binary(&Cw20ExecuteMsg::Mint {
                    recipient: String::from("addr0000"),
                    amount: Uint128::from(74_981956874579206461u128),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            id: 0,
            gas_limit: None,
            reply_on: ReplyOn::Never,
        }
    );
        // Check wrong argument
        let msg = ExecuteMsg::ProvideLiquidity {
            assets: [
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("asset0000"),
                    },
                    amount: Uint128::from(100_000000000000000000u128),
                },
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uusd".to_string(),
                    },
                    amount: Uint128::from(50_000000000000000000u128),
                },
            ],
            slippage_tolerance: None,
            auto_stake: None,
            receiver: None,
        };
    
        let env = mock_env();
        let info = mock_info(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(100_000000000000000000u128),
            }],
        );
        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
        match res {
            ContractError::Std(StdError::GenericErr { msg, .. }) => assert_eq!(
                msg,
                "Native token balance mismatch between the argument and the transferred".to_string()
            ),
            _ => panic!("Must return generic error"),
        }
    
        // Initialize token balances with a ratio of 1:1
        deps.querier.with_balance(&[(
            &String::from(MOCK_CONTRACT_ADDR),
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(100_000000000000000000 + 100_000000000000000000 /* user deposit must be pre-applied */),
            }],
        )]);
    
        deps.querier.with_token_balances(&[
            (
                &String::from("liquidity0000"),
                &[(
                    &String::from(MOCK_CONTRACT_ADDR),
                    &Uint128::new(100_000000000000000000),
                )],
            ),
            (
                &String::from("asset0000"),
                &[(
                    &String::from(MOCK_CONTRACT_ADDR),
                    &Uint128::new(100_000000000000000000),
                )],
            ),
        ]);
    
        // Initialize token balances with a ratio of 1:1
        deps.querier.with_balance(&[(
            &String::from(MOCK_CONTRACT_ADDR),
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(100_000000000000000000 + 98_000000000000000000 /* user deposit must be pre-applied */),
            }],
        )]);
    
        // Initialize token balances with a ratio of 1:1
        deps.querier.with_balance(&[(
            &String::from(MOCK_CONTRACT_ADDR),
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(100_000000000000000000 + 100_000000000000000000 /* user deposit must be pre-applied */),
            }],
        )]);
    
        // Successfully provide liquidity
        let msg = ExecuteMsg::ProvideLiquidity {
            assets: [
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("asset0000"),
                    },
                    amount: Uint128::from(99_000000000000000000u128),
                },
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uusd".to_string(),
                    },
                    amount: Uint128::from(100_000000000000000000u128),
                },
            ],
            slippage_tolerance: Some(Decimal::percent(1)),
            auto_stake: None,
            receiver: None,
        };
    
        let env = mock_env_with_block_time(env.block.time.seconds() + 1000);
        let info = mock_info(
            "addr0001",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(100_000000000000000000u128),
            }],
        );
        let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    
        // Initialize token balances with a ratio of 1:1
        deps.querier.with_balance(&[(
            &String::from(MOCK_CONTRACT_ADDR),
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(100_000000000000000000 + 99_000000000000000000 /* user deposit must be pre-applied */),
            }],
        )]);
    
        // Successfully provide liquidity
        let msg = ExecuteMsg::ProvideLiquidity {
            assets: [
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("asset0000"),
                    },
                    amount: Uint128::from(100_000000000000000000u128),
                },
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uusd".to_string(),
                    },
                    amount: Uint128::from(99_000000000000000000u128),
                },
            ],
            slippage_tolerance: Some(Decimal::percent(1)),
            auto_stake: None,
            receiver: None,
        };
    
        let env = mock_env_with_block_time(env.block.time.seconds() + 1000);
        let info = mock_info(
            "addr0001",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(99_000000000000000000u128),
            }],
        );
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();
    }
    
    #[test]
    fn withdraw_liquidity() {
        let mut deps = mock_dependencies(&[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(100u128),
        }]);
    
        deps.querier.with_token_balances(&[
            (
                &String::from("liquidity0000"),
                &[(&String::from("addr0000"), &Uint128::new(100u128))],
            ),
            (
                &String::from("asset0000"),
                &[(&String::from(MOCK_CONTRACT_ADDR), &Uint128::new(100u128))],
            ),
        ]);
    
        let msg = InstantiateMsg {
            asset_infos: [
                AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0000"),
                },
            ],
            token_code_id: 10u64,
            factory_addr: String::from("factory"),
            init_params: Some(to_binary(&StablePoolParams { amp: 100 }).unwrap()),
        };
    
        let env = mock_env();
        let info = mock_info("addr0000", &[]);
        // We can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();
    
        // Store the liquidity token
        store_liquidity_token(deps.as_mut(), 1, "liquidity0000".to_string());
    
        // Withdraw liquidity
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: String::from("addr0000"),
            msg: to_binary(&Cw20HookMsg::WithdrawLiquidity {}).unwrap(),
            amount: Uint128::new(100u128),
        });
    
        let env = mock_env();
        let info = mock_info("liquidity0000", &[]);
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        let log_withdrawn_share = res.attributes.get(2).expect("no log");
        let log_refund_assets = res.attributes.get(3).expect("no log");
        let msg_refund_0 = res.messages.get(0).expect("no message");
        let msg_refund_1 = res.messages.get(1).expect("no message");
        let msg_burn_liquidity = res.messages.get(2).expect("no message");
        assert_eq!(
            msg_refund_0,
            &SubMsg {
                msg: CosmosMsg::Bank(BankMsg::Send {
                    to_address: String::from("addr0000"),
                    amount: vec![Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128::from(100u128),
                    }],
                }),
                id: 0,
                gas_limit: None,
                reply_on: ReplyOn::Never,
            }
        );
        assert_eq!(
            msg_refund_1,
            &SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: String::from("asset0000"),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: String::from("addr0000"),
                        amount: Uint128::from(100u128),
                    })
                    .unwrap(),
                    funds: vec![],
                }
                .into(),
                id: 0,
                gas_limit: None,
                reply_on: ReplyOn::Never,
            }
        );
        assert_eq!(
            msg_burn_liquidity,
            &SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: String::from("liquidity0000"),
                    msg: to_binary(&Cw20ExecuteMsg::Burn {
                        amount: Uint128::from(100u128),
                    })
                    .unwrap(),
                    funds: vec![],
                }
                .into(),
                id: 0,
                gas_limit: None,
                reply_on: ReplyOn::Never,
            }
        );
    
        assert_eq!(
            log_withdrawn_share,
            &attr("withdrawn_share", 100u128.to_string())
        );
        assert_eq!(
            log_refund_assets,
            &attr("refund_assets", "100uusd, 100asset0000")
        );
    }