use crate::contract::{
    accumulate_prices, assert_max_spread, compute_offer_amount, compute_swap, execute, instantiate,
    query_pair_info, query_pool, query_share, query_simulation, reply,
};
use crate::state::CONFIG;
use crate::error::ContractError;
use crate::math::{calc_ask_amount, calc_offer_amount, AMP_PRECISION};
use crate::mock_querier::mock_dependencies;

use crate::response::MsgInstantiateContractResponse;
use crate::state::Config;
use dexter::asset::{Asset, AssetInfo};
use dexter::vault::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, PoolConfig, PoolType, PairsResponse, QueryMsg, PoolInfo
};

use dexter::pool::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolResponse, SimulationResponse, StablePoolParams,
    TWAP_PRECISION,
};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
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

    let msg = ExecuteMsg {
        vault_addr: String::from("vault"),
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
    assert_eq!(
        res.messages,
        vec![SubMsg {
            msg: WasmMsg::Instantiate {
                code_id: 10u64,
                msg: to_binary(&TokenInstantiateMsg {
                    name: "UUSD-MAPP-LP".to_string(),
                    symbol: "uLP".to_string(),
                    decimals: 6,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: String::from(MOCK_CONTRACT_ADDR),
                        cap: None,
                    }),
                    marketing: None
                })
                .unwrap(),
                funds: vec![],
                admin: None,
                label: String::from("Astroport LP token"),
            }
            .into(),
            id: 1,
            gas_limit: None,
            reply_on: ReplyOn::Success
        },]
    );

    // Store liquidity token
    store_liquidity_token(deps.as_mut(), 1, "liquidity0000".to_string());

    // It worked, let's query the state
    let pair_info: PoolInfo = query_pair_info(deps.as_ref()).unwrap();
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
fn try_native_to_token() {
    let total_share = Uint128::new(30000000000u128);
    let asset_pool_amount = Uint128::new(20000000000u128);
    let collateral_pool_amount = Uint128::new(30000000000u128);
    let offer_amount = Uint128::new(1500000000u128);

    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: collateral_pool_amount + offer_amount, /* user deposit must be pre-applied */
    }]);

    deps.querier.with_token_balances(&[
        ( 
            &String::from("liquidity0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &total_share)],
        ),
        (
            &String::from("asset0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &asset_pool_amount)],
        ),
    ]);

    let msg = ExecuteMsg {
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

    let env = mock_env_with_block_time(100);
    let info = mock_info("addr0000", &[]);
    // We can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    // Store the liquidity token
    store_liquidity_token(deps.as_mut(), 1, "liquidity0000".to_string());

    // Normal swap
    let msg = ExecuteMsg::swap_request {
        offer_asset: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: offer_amount,
        },
        belief_price: None,
        max_spread: Some(Decimal::percent(50)),
        to: None,
    };
    let env = mock_env_with_block_time(1000);
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: offer_amount,
        }],
    );

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    let msg_transfer = res.messages.get(0).expect("no message");

    let model: StableSwapModel = StableSwapModel::new(
        100,
        vec![collateral_pool_amount.into(), asset_pool_amount.into()],
        2,
    );

    let sim_result = model.sim_exchange(0, 1, offer_amount.into());

    let expected_ret_amount = Uint128::new(sim_result);
    let expected_spread_amount = offer_amount.saturating_sub(expected_ret_amount);
    let expected_commission_amount = expected_ret_amount.multiply_ratio(3u128, 1000u128); // 0.3%
    let expected_maker_fee_amount = expected_commission_amount.multiply_ratio(166u128, 1000u128);

    let expected_return_amount = expected_ret_amount
        .checked_sub(expected_commission_amount)
        .unwrap();
    let expected_tax_amount = Uint128::zero(); // no tax for token

    // Check simulation result
    deps.querier.with_balance(&[(
        &String::from(MOCK_CONTRACT_ADDR),
        &[Coin {
            denom: "uusd".to_string(),
            amount: collateral_pool_amount, /* user deposit must be pre-applied */
        }],
    )]);

    let simulation_res: SimulationResponse = query_simulation(
        deps.as_ref(),
        env.clone(),
        Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: offer_amount,
        },
    )
    .unwrap();
    assert_eq!(expected_return_amount, simulation_res.return_amount);
    assert_eq!(expected_commission_amount, simulation_res.commission_amount);
    assert_eq!(expected_spread_amount, simulation_res.spread_amount);

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "swap"),
            attr("sender", "addr0000"),
            attr("receiver", "addr0000"),
            attr("offer_asset", "uusd"),
            attr("ask_asset", "asset0000"),
            attr("offer_amount", offer_amount.to_string()),
            attr("return_amount", expected_return_amount.to_string()),
            attr("tax_amount", expected_tax_amount.to_string()),
            attr("spread_amount", expected_spread_amount.to_string()),
            attr("commission_amount", expected_commission_amount.to_string()),
            attr("maker_fee_amount", expected_maker_fee_amount.to_string()),
        ]
    );

    assert_eq!(
        &SubMsg {
            msg: WasmMsg::Execute {
                contract_addr: String::from("asset0000"),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: String::from("addr0000"),
                    amount: Uint128::from(expected_return_amount),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            id: 0,
            gas_limit: None,
            reply_on: ReplyOn::Never,
        },
        msg_transfer,
    );
}

#[test]
fn try_token_to_native() {
    let total_share = Uint128::new(30000000000u128);
    let asset_pool_amount = Uint128::new(20000000000u128);
    let collateral_pool_amount = Uint128::new(30000000000u128);
    let offer_amount = Uint128::new(1500000000u128);

    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: collateral_pool_amount,
    }]);

    deps.querier.with_token_balances(&[
        (
            &String::from("liquidity0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &total_share)],
        ),
        (
            &String::from("asset0000"),
            &[(
                &String::from(MOCK_CONTRACT_ADDR),
                &(asset_pool_amount + offer_amount),
            )],
        ),
    ]);

    let msg = ExecuteMsg {
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

    let env = mock_env_with_block_time(100);
    let info = mock_info("addr0000", &[]);
    // We can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    // Store the liquidity token
    store_liquidity_token(deps.as_mut(), 1, "liquidity0000".to_string());

    // Unauthorized access; can not execute swap directy for token swap
    let msg = ExecuteMsg::swap_request {
        offer_asset: Asset {
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
            amount: offer_amount,
        },
        belief_price: None,
        max_spread: None,
        to: None,
    };
    let env = mock_env_with_block_time(1000);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});

    // Normal sell
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: String::from("addr0000"),
        amount: offer_amount,
        msg: to_binary(&Cw20HookMsg::Swap {
            belief_price: None,
            max_spread: None,
            to: None,
        })
        .unwrap(),
    });
    let env = mock_env_with_block_time(1000);
    let info = mock_info("asset0000", &[]);

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    let msg_transfer = res.messages.get(0).expect("no message");

    let model: StableSwapModel = StableSwapModel::new(
        100,
        vec![collateral_pool_amount.into(), asset_pool_amount.into()],
        2,
    );

    let sim_result = model.sim_exchange(1, 0, offer_amount.into());

    let expected_ret_amount = Uint128::new(sim_result);
    let expected_spread_amount = offer_amount.saturating_sub(expected_ret_amount);
    let expected_commission_amount = expected_ret_amount.multiply_ratio(3u128, 1000u128); // 0.3%
    let expected_maker_fee_amount = expected_commission_amount.multiply_ratio(166u128, 1000u128);

    let expected_return_amount = expected_ret_amount
        .checked_sub(expected_commission_amount)
        .unwrap();

    // Check simulation result
    // Return asset token balance as normal
    deps.querier.with_token_balances(&[
        (
            &String::from("liquidity0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &total_share)],
        ),
        (
            &String::from("asset0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &(asset_pool_amount))],
        ),
    ]);

    let simulation_res: SimulationResponse = query_simulation(
        deps.as_ref(),
        env.clone(),
        Asset {
            amount: offer_amount,
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
        },
    )
    .unwrap();
    assert_eq!(expected_return_amount, simulation_res.return_amount);
    assert_eq!(expected_commission_amount, simulation_res.commission_amount);
    assert_eq!(expected_spread_amount, simulation_res.spread_amount);

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "swap"),
            attr("sender", "addr0000"),
            attr("receiver", "addr0000"),
            attr("offer_asset", "asset0000"),
            attr("ask_asset", "uusd"),
            attr("offer_amount", offer_amount.to_string()),
            attr("return_amount", expected_return_amount.to_string()),
            attr("tax_amount", Uint128::zero().to_string()),
            attr("spread_amount", expected_spread_amount.to_string()),
            attr("commission_amount", expected_commission_amount.to_string()),
            attr("maker_fee_amount", expected_maker_fee_amount.to_string()),
        ]
    );

    assert_eq!(
        &SubMsg {
            msg: CosmosMsg::Bank(BankMsg::Send {
                to_address: String::from("addr0000"),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: expected_return_amount,
                }],
            })
            .into(),
            id: 0,
            gas_limit: None,
            reply_on: ReplyOn::Never,
        },
        msg_transfer,
    );

    // Failed due to non asset token contract being used in a swap
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: String::from("addr0000"),
        amount: offer_amount,
        msg: to_binary(&Cw20HookMsg::Swap {
            belief_price: None,
            max_spread: None,
            to: None,
        })
        .unwrap(),
    });
    let env = mock_env_with_block_time(1000);
    let info = mock_info("liquidtity0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});
}

#[test]
fn test_max_spread() {
    assert_max_spread(
        Some(Decimal::from_ratio(1200u128, 1u128)),
        Some(Decimal::percent(1)),
        Uint128::from(1200000000u128),
        Uint128::from(989999u128),
        Uint128::zero(),
    )
    .unwrap_err();

    assert_max_spread(
        Some(Decimal::from_ratio(1200u128, 1u128)),
        Some(Decimal::percent(1)),
        Uint128::from(1200000000u128),
        Uint128::from(990000u128),
        Uint128::zero(),
    )
    .unwrap();

    assert_max_spread(
        None,
        Some(Decimal::percent(1)),
        Uint128::zero(),
        Uint128::from(989999u128),
        Uint128::from(10001u128),
    )
    .unwrap_err();

    assert_max_spread(
        None,
        Some(Decimal::percent(1)),
        Uint128::zero(),
        Uint128::from(990000u128),
        Uint128::from(10000u128),
    )
    .unwrap();
}

#[test]
fn test_query_pool() {
    let total_share_amount = Uint128::from(111u128);
    let asset_0_amount = Uint128::from(222u128);
    let asset_1_amount = Uint128::from(333u128);
    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: asset_0_amount,
    }]);

    deps.querier.with_token_balances(&[
        (
            &String::from("asset0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &asset_1_amount)],
        ),
        (
            &String::from("liquidity0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &total_share_amount)],
        ),
    ]);

    let msg = ExecuteMsg {
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

    let res: PoolResponse = query_pool(deps.as_ref()).unwrap();

    assert_eq!(
        res.assets,
        [
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: asset_0_amount
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0000"),
                },
                amount: asset_1_amount
            }
        ]
    );
    assert_eq!(res.total_share, total_share_amount);
}

#[test]
fn test_query_share() {
    let total_share_amount = Uint128::from(500u128);
    let asset_0_amount = Uint128::from(250u128);
    let asset_1_amount = Uint128::from(1000u128);
    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: asset_0_amount,
    }]);

    deps.querier.with_token_balances(&[
        (
            &String::from("asset0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &asset_1_amount)],
        ),
        (
            &String::from("liquidity0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &total_share_amount)],
        ),
    ]);

    let msg = ExecuteMsg {
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

    let res = query_share(deps.as_ref(), Uint128::new(250)).unwrap();

    assert_eq!(res[0].amount, Uint128::new(125));
    assert_eq!(res[1].amount, Uint128::new(500));
}

#[test]
fn test_accumulate_prices() {
    struct Case {
        block_time: u64,
        block_time_last: u64,
        last0: u128,
        last1: u128,
        x_amount: u128,
        y_amount: u128,
    }

    struct Result {
        block_time_last: u64,
        cumulative_price_x: u128,
        cumulative_price_y: u128,
        is_some: bool,
    }

    let price_precision = 10u128.pow(TWAP_PRECISION.into());

    let test_cases: Vec<(Case, Result)> = vec![
        (
            Case {
                block_time: 1000,
                block_time_last: 0,
                last0: 0,
                last1: 0,
                x_amount: 250_000000,
                y_amount: 500_000000,
            },
            Result {
                block_time_last: 1000,
                cumulative_price_x: 1008,
                cumulative_price_y: 991,
                is_some: true,
            },
        ),
        // Same block height, no changes
        (
            Case {
                block_time: 1000,
                block_time_last: 1000,
                last0: 1 * price_precision,
                last1: 2 * price_precision,
                x_amount: 250_000000,
                y_amount: 500_000000,
            },
            Result {
                block_time_last: 1000,
                cumulative_price_x: 1,
                cumulative_price_y: 2,
                is_some: false,
            },
        ),
        (
            Case {
                block_time: 1500,
                block_time_last: 1000,
                last0: 500 * price_precision,
                last1: 2000 * price_precision,
                x_amount: 250_000000,
                y_amount: 500_000000,
            },
            Result {
                block_time_last: 1500,
                cumulative_price_x: 1004,
                cumulative_price_y: 2495,
                is_some: true,
            },
        ),
    ];

    for test_case in test_cases {
        let (case, result) = test_case;

        let env = mock_env_with_block_time(case.block_time);
        let config = accumulate_prices(
            env.clone(),
            &Config {
                pair_info: PoolInfo {
                    asset_infos: [
                        AssetInfo::NativeToken {
                            denom: "uusd".to_string(),
                        },
                        AssetInfo::Token {
                            contract_addr: Addr::unchecked("asset0000"),
                        },
                    ],
                    contract_addr: Addr::unchecked("pair"),
                    liquidity_token: Addr::unchecked("lp_token"),
                    pair_type: PoolType::Stable {},
                },
                vault_addr: Addr::unchecked("vault"),
                block_time_last: case.block_time_last,
                price0_cumulative_last: Uint128::new(case.last0),
                price1_cumulative_last: Uint128::new(case.last1),
                init_amp: 100 * AMP_PRECISION,
                init_amp_time: env.block.time.seconds(),
                next_amp: 100 * AMP_PRECISION,
                next_amp_time: env.block.time.seconds(),
            },
            Uint128::new(case.x_amount),
            6,
            Uint128::new(case.y_amount),
            6,
        )
        .unwrap();

        assert_eq!(result.is_some, config.is_some());

        if let Some(config) = config {
            assert_eq!(config.2, result.block_time_last);
            assert_eq!(
                config.0 / Uint128::from(price_precision),
                Uint128::new(result.cumulative_price_x)
            );
            assert_eq!(
                config.1 / Uint128::from(price_precision),
                Uint128::new(result.cumulative_price_y)
            );
        }
    }
}

fn mock_env_with_block_time(time: u64) -> Env {
    let mut env = mock_env();
    env.block = BlockInfo {
        height: 1,
        time: Timestamp::from_seconds(time),
        chain_id: "columbus".to_string(),
    };
    env
}

use astroport::factory::PoolType;
use proptest::prelude::*;
use sim::StableSwapModel;

proptest! {
    #[test]
    fn constant_product_swap_no_fee(
        balance_in in 100..1_000_000_000_000_000_000u128,
        balance_out in 100..1_000_000_000_000_000_000u128,
        amount_in in 100..100_000_000_000u128,
        amp in 1..150u64
    ) {
        prop_assume!(amount_in < balance_in);

        let model: StableSwapModel = StableSwapModel::new(
            amp.into(),
            vec![balance_in, balance_out],
            2,
        );

        let result = calc_ask_amount(
            balance_in,
            balance_out,
            amount_in,
            amp * AMP_PRECISION
        ).unwrap();

        let sim_result = model.sim_exchange(0, 1, amount_in);

        let diff = (sim_result as i128 - result as i128).abs();

        assert!(
            diff <= 1,
            "result={}, sim_result={}, amp={}, amount_in={}, balance_in={}, balance_out={}, diff={}",
            result,
            sim_result,
            amp,
            amount_in,
            balance_in,
            balance_out,
            diff
        );

        let reverse_result = calc_offer_amount(
            balance_in,
            balance_out,
            result,
            amp * AMP_PRECISION
        ).unwrap();

        let amount_in_f = amount_in as f64;
        let reverse_diff = (reverse_result as f64 - amount_in_f) / amount_in_f * 100.;

        assert!(
            reverse_diff <= 0.0001,
            "result={}, sim_result={}, amp={}, amount_out={}, balance_in={}, balance_out={}, diff(%)={}",
            reverse_result,
            amount_in,
            amp,
            result,
            balance_in,
            balance_out,
            reverse_diff
        );
    }
}

#[test]
fn ensure_useful_error_messages_are_given_on_swaps() {
    const OFFER: Uint128 = Uint128::new(1_000_000_000000);
    const ASK: Uint128 = Uint128::new(1_000_000_000000);
    const AMOUNT: Uint128 = Uint128::new(1_000000);
    const ZERO: Uint128 = Uint128::zero();
    const DZERO: Decimal = Decimal::zero();
    const AMP: u64 = 100;
    const PRS: u8 = 6;

    // Computing ask
    assert_eq!(
        compute_swap(ZERO, PRS, ZERO, PRS, ZERO, DZERO, AMP).unwrap_err(),
        StdError::generic_err("One of the pools is empty")
    );
    assert_eq!(
        compute_swap(ZERO, PRS, ZERO, PRS, AMOUNT, DZERO, AMP).unwrap_err(),
        StdError::generic_err("One of the pools is empty")
    );
    assert_eq!(
        compute_swap(ZERO, PRS, ASK, PRS, ZERO, DZERO, AMP).unwrap_err(),
        StdError::generic_err("One of the pools is empty")
    );
    assert_eq!(
        compute_swap(ZERO, PRS, ASK, PRS, AMOUNT, DZERO, AMP).unwrap_err(),
        StdError::generic_err("One of the pools is empty")
    );
    assert_eq!(
        compute_swap(OFFER, PRS, ZERO, PRS, ZERO, DZERO, AMP).unwrap_err(),
        StdError::generic_err("One of the pools is empty")
    );
    assert_eq!(
        compute_swap(OFFER, PRS, ZERO, PRS, AMOUNT, DZERO, AMP).unwrap_err(),
        StdError::generic_err("One of the pools is empty")
    );
    assert_eq!(
        compute_swap(OFFER, PRS, ASK, PRS, ZERO, DZERO, AMP).unwrap_err(),
        StdError::generic_err("Swap amount must not be zero")
    );
    compute_swap(OFFER, PRS, ASK, PRS, AMOUNT, DZERO, AMP).unwrap();

    // Computing offer
    assert_eq!(
        compute_offer_amount(ZERO, PRS, ZERO, PRS, ZERO, DZERO, AMP).unwrap_err(),
        StdError::generic_err("One of the pools is empty")
    );
    assert_eq!(
        compute_offer_amount(ZERO, PRS, ZERO, PRS, AMOUNT, DZERO, AMP).unwrap_err(),
        StdError::generic_err("One of the pools is empty")
    );
    assert_eq!(
        compute_offer_amount(ZERO, PRS, ASK, PRS, ZERO, DZERO, AMP).unwrap_err(),
        StdError::generic_err("One of the pools is empty")
    );
    assert_eq!(
        compute_offer_amount(ZERO, PRS, ASK, PRS, AMOUNT, DZERO, AMP).unwrap_err(),
        StdError::generic_err("One of the pools is empty")
    );
    assert_eq!(
        compute_offer_amount(OFFER, PRS, ZERO, PRS, ZERO, DZERO, AMP).unwrap_err(),
        StdError::generic_err("One of the pools is empty")
    );
    assert_eq!(
        compute_offer_amount(OFFER, PRS, ZERO, PRS, AMOUNT, DZERO, AMP).unwrap_err(),
        StdError::generic_err("One of the pools is empty")
    );
    assert_eq!(
        compute_offer_amount(OFFER, PRS, ASK, PRS, ZERO, DZERO, AMP).unwrap_err(),
        StdError::generic_err("Swap amount must not be zero")
    );
    compute_offer_amount(OFFER, PRS, ASK, PRS, AMOUNT, DZERO, AMP).unwrap();
}
