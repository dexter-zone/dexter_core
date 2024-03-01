use cosmwasm_std::{from_json, to_json_binary, Addr, Coin, Decimal, Timestamp, Uint128};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};
use cw_multi_test::Executor;

use dexter::asset::{native_asset_info, Asset, AssetExchangeRate, AssetInfo};
use dexter::pool::{AfterExitResponse, AfterJoinResponse, ConfigResponse, CumulativePricesResponse, ExecuteMsg, ExitType, QueryMsg, ResponseType, SwapResponse};
use dexter::vault;
use dexter::vault::{
    Cw20HookMsg, ExecuteMsg as VaultExecuteMsg, PoolInfoResponse, QueryMsg as VaultQueryMsg, SingleSwapRequest,
    SwapType,
};

use stable_pool::math::{MAX_AMP, MAX_AMP_CHANGE, MIN_AMP_CHANGING_TIME};
use stable_pool::state::{StablePoolParams, StablePoolUpdateParams};

use crate::utils::*;
pub mod utils;

/// Tests Pool::ExecuteMsg::UpdateConfig for stableswap Pool which supports [`StartChangingAmp`] and [`StopChangingAmp`] messages
#[test]
fn test_update_config() {
    let owner = Addr::unchecked("owner");
    let alice_address = Addr::unchecked("alice");
    let mut app = mock_app(
        owner.clone(),
        vec![Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(100000000_000_000_000u128),
        }],
    );
    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(10000000_000_000u128),
        }],
    )
    .unwrap();

    let (vault_instance, pool_addr, _lp_token_addr, token_instance0, token_instance1, _) =
        instantiate_contracts_instance(&mut app, &owner);
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance0.clone(),
        Uint128::new(10000000_000_000u128),
        alice_address.to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        Uint128::new(10000000_000_000u128),
        alice_address.to_string(),
    );

    //// -----x----- Successfully provide liquidity and mint LP tokens -----x----- ////
    // asset:"axlusd" Provided amount:"10000" Pool Liquidity:"0"
    // asset:"contract1" Provided amount:"10000" Pool Liquidity:"0"
    // asset:"contract2" Provided amount:"10000" Pool Liquidity:"0"
    // amp: 1000
    // n_coins: 3
    // ----------x-----------x-----------x-----------x-----------
    // compute_d() Function
    // init_d (Initial invariant (D)): Decimal256(Uint256(0))
    // ----------x-----------x-----------x-----------x-----------
    // compute_d() Function
    // ann ((amp * n_coins) / AMP_PRECISION ) : Decimal256(Uint256(30000000000000000000))
    // sum_x = d: Decimal256(Uint256(30000000000000000))
    // ann_sum_x = ann * sum_x: Decimal256(Uint256(900000000000000000))
    // Start Loop: D_P = D_P * D / (_x * N_COINS)
    // -----
    // acc: 0.03
    // pool_liq: 0.01 n_coins: 3
    // denominator (pool_liq * n_coins) : Uint256(30000000000000000)
    // print_calc_: Ok(Decimal256(Uint256(30000000000000000)))
    // ------
    // ------
    // acc: 0.03
    // pool_liq: 0.01 n_coins: 3
    // denominator (pool_liq * n_coins) : Uint256(30000000000000000)
    // print_calc_: Ok(Decimal256(Uint256(30000000000000000)))
    // ------
    // ------
    // acc: 0.03
    // pool_liq: 0.01 n_coins: 3
    // denominator (pool_liq * n_coins) : Uint256(30000000000000000)
    // print_calc_: Ok(Decimal256(Uint256(30000000000000000)))
    // ------
    // d_prev: 0.03
    // d: 0.03
    // deposit_d (Invariant (D) after deposit added): Decimal256(Uint256(30000000000000000))
    // EMPTY POOL, mint deposit_d number of LP tokens:0.03 (adj to 6 decimals)
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "axlusd".to_string(),
            },
            amount: Uint128::from(10_000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(10_000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(10_000u128),
        },
    ];
    let msg = VaultExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: None,
        min_lp_to_receive: None,
        auto_stake: None,
        assets: Some(assets_msg.clone()),
    };
    app.execute_contract(
        alice_address.clone(),
        token_instance0.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(1000000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        alice_address.clone(),
        token_instance1.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(1000000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        alice_address.clone(),
        vault_instance.clone(),
        &msg,
        &[Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(10000u128),
        }],
    )
    .unwrap();

    //  ###########  Check :: Failure ::  Start changing amp with incorrect next amp   ###########

    let msg = ExecuteMsg::UpdateConfig {
        params: to_json_binary(&StablePoolUpdateParams::StartChangingAmp {
                next_amp: MAX_AMP + 1,
                next_amp_time: app.block_info().time.seconds(),
            })
            .unwrap(),
    };
    let resp = app
        .execute_contract(owner.clone(), pool_addr.clone(), &msg, &[])
        .unwrap_err();
    assert_eq!(
        resp.root_cause().to_string(),
        format!(
            "Amp coefficient must be greater than 0 and less than or equal to {}",
            MAX_AMP
        )
    );

    //  ###########  Check :: Failure ::  Start changing amp with big difference between the old and new amp value   ###########

    let msg = ExecuteMsg::UpdateConfig {
        params: to_json_binary(&StablePoolUpdateParams::StartChangingAmp {
                next_amp: 100 * MAX_AMP_CHANGE + 1,
                next_amp_time: app.block_info().time.seconds(),
            })
            .unwrap(),
    };
    let resp = app
        .execute_contract(owner.clone(), pool_addr.clone(), &msg, &[])
        .unwrap_err();
    assert_eq!(
        resp.root_cause().to_string(),
        format!(
            "The difference between the old and new amp value must not exceed {} times",
            MAX_AMP_CHANGE
        )
    );

    //  ########### Check :: Failure ::   Start changing amp earlier than the MIN_AMP_CHANGING_TIME has elapsed    ###########

    let msg = ExecuteMsg::UpdateConfig {
        params: to_json_binary(&StablePoolUpdateParams::StartChangingAmp {
                next_amp: 25,
                next_amp_time: app.block_info().time.seconds(),
            })
            .unwrap(),
        
    };
    let resp = app
        .execute_contract(owner.clone(), pool_addr.clone(), &msg, &[])
        .unwrap_err();
    assert_eq!(
        resp.root_cause().to_string(),
        format!(
            "Amp coefficient cannot be changed more often than once per {} seconds",
            MIN_AMP_CHANGING_TIME
        )
    );

    // Start increasing amp
    app.update_block(|b| {
        b.time = b.time.plus_seconds(MIN_AMP_CHANGING_TIME);
    });

    let msg = ExecuteMsg::UpdateConfig {
        params: to_json_binary(&StablePoolUpdateParams::StartChangingAmp {
                next_amp: 25,
                next_amp_time: app.block_info().time.seconds() + MIN_AMP_CHANGING_TIME,
            })
            .unwrap(),
    };

    app.execute_contract(owner.clone(), pool_addr.clone(), &msg, &[])
        .unwrap();

    app.update_block(|b| {
        b.time = b.time.plus_seconds(MIN_AMP_CHANGING_TIME / 2);
    });

    let res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(pool_addr.clone(), &QueryMsg::Config {})
        .unwrap();
    let params: StablePoolParams = from_json(&res.additional_params.unwrap()).unwrap();
    assert_eq!(params.amp, 17u64);

    app.update_block(|b| {
        b.time = b.time.plus_seconds(MIN_AMP_CHANGING_TIME / 2);
    });

    let res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(pool_addr.clone(), &QueryMsg::Config {})
        .unwrap();
    let params: StablePoolParams = from_json(&res.additional_params.unwrap()).unwrap();
    assert_eq!(params.amp, 25u64);

    // Start decreasing amp
    app.update_block(|b| {
        b.time = b.time.plus_seconds(MIN_AMP_CHANGING_TIME);
    });

    let msg = ExecuteMsg::UpdateConfig {
        params: to_json_binary(&StablePoolUpdateParams::StartChangingAmp {
                next_amp: 15,
                next_amp_time: app.block_info().time.seconds() + MIN_AMP_CHANGING_TIME,
            })
            .unwrap(),
    };

    app.execute_contract(owner.clone(), pool_addr.clone(), &msg, &[])
        .unwrap();

    app.update_block(|b| {
        b.time = b.time.plus_seconds(MIN_AMP_CHANGING_TIME / 2);
    });

    let res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(pool_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    let params: StablePoolParams = from_json(&res.additional_params.unwrap()).unwrap();
    assert_eq!(params.amp, 20u64);

    // Stop changing amp
    let msg = ExecuteMsg::UpdateConfig {
        params: to_json_binary(&StablePoolUpdateParams::StopChangingAmp {}).unwrap(),
    };
    app.execute_contract(owner.clone(), pool_addr.clone(), &msg, &[])
        .unwrap();

    app.update_block(|b| {
        b.time = b.time.plus_seconds(MIN_AMP_CHANGING_TIME / 2);
    });

    let res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(pool_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    let params: StablePoolParams = from_json(&res.additional_params.unwrap()).unwrap();
    assert_eq!(params.amp, 20u64);

    // Change max allowed spread limits for trades
    let msg = ExecuteMsg::UpdateConfig {
        params: to_json_binary(&StablePoolUpdateParams::UpdateMaxAllowedSpread {
                max_allowed_spread: Decimal::percent(90),
            })
            .unwrap(),
    };

    app.execute_contract(owner.clone(), pool_addr.clone(), &msg, &[]).unwrap();
    
    // validate max allowed spread change
    let res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(pool_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    let params: StablePoolParams = from_json(&res.additional_params.unwrap()).unwrap();
    assert_eq!(params.max_allowed_spread, Decimal::percent(90));

    // try updating max spread to an invalid value
    let msg = ExecuteMsg::UpdateConfig {
        params: to_json_binary(&StablePoolUpdateParams::UpdateMaxAllowedSpread {
                max_allowed_spread: Decimal::percent(100),
            })
            .unwrap(),
    };

    let resp = app
        .execute_contract(owner.clone(), pool_addr.clone(), &msg, &[])
        .unwrap_err();

    assert_eq!(
        resp.root_cause().to_string(),
        "Invalid max allowed spread. Max allowed spread should be positive non-zero value less than 1"
    );

}

/// Tests the following -
/// Pool::QueryMsg::OnJoinPool for StablePool and the returned  [`AfterJoinResponse`] struct to check if the math calculations are correct
/// Vault::ExecuteMsg::JoinPool - Token transfer from user to vault and LP token minting to user are processed as expected and Balances are updated correctly
/// Vault::ExecuteMsg::UpdateLiquidity - Executed by the Vault at the end of join pool tx execution to update pool balances as stored in the Pool contract which are used for computations
#[test]
fn test_query_on_join_pool() {
    let owner = Addr::unchecked("owner");
    let alice_address = Addr::unchecked("alice");
    let mut app = mock_app(
        owner.clone(),
        vec![Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(100000_000_000_000u128),
        }],
    );

    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(10000000_000_000u128),
        }],
    )
    .unwrap();

    let (vault_instance, pool_addr, lp_token_addr, token_instance0, token_instance1, _) =
        instantiate_contracts_instance(&mut app, &owner);

    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance0.clone(),
        Uint128::new(9000000_000_000_000),
        alice_address.to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        Uint128::new(9000000_000_000_000),
        alice_address.to_string(),
    );

    //// -----x----- Check #1 :: Error ::: When no asset info is provided -----x----- ////

    let empty_assets: Vec<Asset> = vec![];
    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnJoinPool {
                assets_in: None,
                mint_amount: Some(Uint128::from(1000_000_000u128)),
            },
        )
        .unwrap();
    assert_eq!(
        ResponseType::Failure("No assets provided".to_string()),
        join_pool_query_res.response
    );
    assert_eq!(Uint128::zero(), join_pool_query_res.new_shares);
    assert_eq!(empty_assets, join_pool_query_res.provided_assets);
    assert_eq!(None, join_pool_query_res.fee);

    //// -----x----- Check #2 :: Success ::: Liquidity being provided when pool is empty -----x----- ////
    let assets_msg = vec![
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(1000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(1000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: "axlusd".to_string(),
            },
            amount: Uint128::from(1000u128),
        },
    ];

    // -------x---- StableSwap Pool -::- QueryOnJoinPool ----x---------
    // assets_in: Some([Asset { info: Token { contract_addr: Addr("contract1") }, amount: Uint128(1000) }, Asset { info: Token { contract_addr: Addr("contract2") }, amount: Uint128(1000) }, Asset { info: AssetInfo::NativeToken { denom: "axlusd".to_string() }, amount: Uint128(1000) }])
    // assets_in sorted
    // act_assets_in: [Asset { info: AssetInfo::NativeToken { denom: "axlusd".to_string() }, amount: Uint128(1000) }, Asset { info: Token { contract_addr: Addr("contract1") }, amount: Uint128(1000) }, Asset { info: Token { contract_addr: Addr("contract2") }, amount: Uint128(1000) }]
    // Asset pools stored in a hashmap
    // asset:"axlusd" Provided amount:"1000" Pool Liquidity:"0"
    // asset:"contract1" Provided amount:"1000" Pool Liquidity:"0"
    // asset:"contract2" Provided amount:"1000" Pool Liquidity:"0"
    // amp: 1000
    // n_coins: 3
    // compute_d() Function
    // init_d (Initial invariant (D)): Decimal256(Uint256(0))
    // compute_d() Function
    // deposit_d (Invariant (D) after deposit added): Decimal256(Uint256(3000000000000000))
    // current total LP token supply (Total share of LP tokens minted by the pool): Uint128(0)
    // EMPTY POOL, mint deposit_d number of LP tokens:0.003
    // mint_amount (adj): Uint128(3000)
    // provided_assets: [Asset { info: AssetInfo::NativeToken { denom: "axlusd".to_string() }, amount: Uint128(1000) }, Asset { info: Token { contract_addr: Addr("contract1") }, amount: Uint128(1000) }, Asset { info: Token { contract_addr: Addr("contract2") }, amount: Uint128(1000) }]
    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnJoinPool {
                assets_in: Some(assets_msg.clone()),
                mint_amount: None,
            },
        )
        .unwrap();
    assert_eq!(Some(vec![]), join_pool_query_res.fee);
    assert_eq!(ResponseType::Success {}, join_pool_query_res.response);
    assert_eq!(Uint128::from(3000_000000_000000u128), join_pool_query_res.new_shares);

    // Returned assets are in sorted order
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                amount: Uint128::from(1000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(1000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(1000u128),
            },
        ],
        join_pool_query_res.provided_assets
    );
    // Execute AddLiquidity via the Vault contract
    let msg = VaultExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: None,
        min_lp_to_receive: None,
        auto_stake: None,
        assets: Some(assets_msg.clone()),
    };

    //// -----x----- Check #2.1 :: Execution Error ::: If insufficient number of Native tokens were sent -----x----- ////
    let add_liq_res = app
        .execute_contract(alice_address.clone(), vault_instance.clone(), &msg, &[])
        .unwrap_err();
    assert_eq!(
        add_liq_res.root_cause().to_string(),
        "Insufficient number of axlusd tokens sent. Tokens sent = 0. Tokens needed = 1000"
    );

    //// -----x----- Check #2.2 :: Execution Error ::: CW20 tokens were not approved for transfer via the Vault contract -----x----- ////
    let add_liq_res = app
        .execute_contract(
            alice_address.clone(),
            vault_instance.clone(),
            &msg,
            &[Coin {
                denom: "axlusd".to_string(),
                amount: Uint128::new(1100u128),
            }],
        )
        .unwrap_err();
    assert_eq!(
        add_liq_res.root_cause().to_string(),
        "No allowance for this account"
    );

    //// -----x----- Check #2.2 :: Success ::: Successfully provide liquidity and mint LP tokens -----x----- ////
    let _current_block = app.block_info();
    app.update_block(|b| {
        b.height += 10;
        b.time = Timestamp::from_seconds(_current_block.time.seconds() + 90)
    });

    app.execute_contract(
        alice_address.clone(),
        token_instance0.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(100000000000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        alice_address.clone(),
        token_instance1.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(100000000000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        alice_address.clone(),
        vault_instance.clone(),
        &msg,
        &[Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(1100u128),
        }],
    )
    .unwrap();

    // Checks -
    // 1. LP tokens minted & transferred to Alice
    // 2. Liquidity Pool balance updated
    // 3. Tokens transferred to the Vault
    // 4. TWAP updated
    let alice_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: alice_address.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(join_pool_query_res.new_shares, alice_bal_res.balance);

    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance0.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1000u128), vault_bal_res.balance);

    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1000u128), vault_bal_res.balance);

    let vault_pool_config_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &vault_instance.clone(),
            &VaultQueryMsg::GetPoolById {
                pool_id: Uint128::from(1u128),
            },
        )
        .unwrap();
    let pool_config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&pool_addr.clone(), &QueryMsg::Config {})
        .unwrap();
    assert_eq!(pool_config_res.assets, vault_pool_config_res.assets);
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                amount: Uint128::from(1000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(1000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(1000u128),
            },
        ],
        vault_pool_config_res.assets
    );

    let _current_block = app.block_info();
    app.update_block(|b| {
        b.height += 10;
        b.time = Timestamp::from_seconds(_current_block.time.seconds() + 90)
    });

    let pool_twap_res: CumulativePricesResponse = app
        .wrap()
        .query_wasm_smart(&pool_addr.clone(), &QueryMsg::CumulativePrices {})
        .unwrap();

    assert_eq!(Uint128::from(3000_000000_000000u128), pool_twap_res.total_share);
    assert_eq!(
        vec![
            AssetExchangeRate {
                offer_info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                rate: Uint128::from(970173599820u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                rate: Uint128::from(970173599820u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                ask_info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                rate: Uint128::from(970173599820u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                rate: Uint128::from(970173599820u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                ask_info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                rate: Uint128::from(970173599820u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                rate: Uint128::from(970173599820u128),
            },
        ],
        pool_twap_res.exchange_infos
    );

    //// -----x----- Check #3.3 :: Success -----x----- ////
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "axlusd".to_string(),
            },
            amount: Uint128::from(109u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(111u128),
        },
    ];

    // -------x---- StableSwap Pool -::- QueryOnJoinPool ----x---------
    // asset:"axlusd" Provided amount:"109" Pool Liquidity:"1000"
    // asset:"contract1" Provided amount:"111" Pool Liquidity:"1000"
    // amp: 1000
    // n_coins: 3
    // ---x---x---x---
    // Calculate current d, which is: 0.003
    // - init_d (Initial invariant (D)): Decimal256(Uint256(3000000000000000))
    // Calculate new d after liq. is added, which is:  0.00322
    // deposit_d (Invariant (D) after deposit added): Decimal256(Uint256(3219649422713044))
    // ---x---x---x---
    // Current LP shares: Uint128(3000)
    // fee (total_fee_bps * N_COINS / (4 * (N_COINS - 1))): Decimal(Uint128(11250000000000000))
    // Start loop for fee stuff
    // ---x---x---x---
    // -- deposit_d:0.003219649422713044, old_balances[i]:0.001, init_d:0.003
    // ideal_balance (ideal_balance = deposit_d * old_balances[i] / init_d): Decimal256(Uint256(1073216474237681))
    // ideal_balance:0.001073216474237681 , new_balances:0.001109, difference:0.000035783525762319
    // new_balances[i] (new_balances[i] -= fee * difference): Decimal256(Uint256(1108597435335174))
    // -- deposit_d:0.003219649422713044, old_balances[i]:0.001, init_d:0.003
    // ideal_balance (ideal_balance = deposit_d * old_balances[i] / init_d): Decimal256(Uint256(1073216474237681))
    // ideal_balance:0.001073216474237681 , new_balances:0.001111, difference:0.000037783525762319
    // new_balances[i] (new_balances[i] -= fee * difference): Decimal256(Uint256(1110574935335174))
    // -- deposit_d:0.003219649422713044, old_balances[i]:0.001, init_d:0.003
    // ideal_balance (ideal_balance = deposit_d * old_balances[i] / init_d): Decimal256(Uint256(1073216474237681))
    // ideal_balance:0.001073216474237681 , new_balances:0.001, difference:0.000073216474237681
    // new_balances[i] (new_balances[i] -= fee * difference): Decimal256(Uint256(999176314664827))
    // ---x---x---x---
    // Calculate d after fee, which is: 0.003
    // after_fee_d (Invariant (D) after fee): Decimal256(Uint256(3217995268828152))
    // total_share:3000, init_d:0.003, after_fee_d:0.003217995268828152
    // tokens_to_mint (Total share of LP tokens minted by the pool): Decimal256(Uint256(217995268828152))
    // mint_amount (adj): Uint128(217)
    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnJoinPool {
                assets_in: Some(assets_msg.clone()),
                mint_amount: None,
            },
        )
        .unwrap();
    assert_eq!(ResponseType::Success {}, join_pool_query_res.response);
    assert_eq!(Uint128::from(217_995_261_723_840u128), join_pool_query_res.new_shares);

    // Execute AddLiquidity via the Vault contract
    let msg = VaultExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: Some("recipient".to_string()),
        min_lp_to_receive: None,
        auto_stake: None,
        assets: Some(assets_msg.clone()),
    };

    let _current_block = app.block_info();
    app.update_block(|b| {
        b.height += 10;
        b.time = Timestamp::from_seconds(_current_block.time.seconds() + 90)
    });

    app.execute_contract(
        alice_address.clone(),
        vault_instance.clone(),
        &msg,
        &[Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(217u128),
        }],
    )
    .unwrap();

    // Checks -
    // 1. LP tokens minted & transferred to Alice
    // 2. Liquidity Pool balance updated
    // 3. Tokens transferred to the Vault
    // 4. TWAP updated
    let recepient_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: "recipient".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(217_995_261_723_840u128), recepient_bal_res.balance);

    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance0.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1111u128), vault_bal_res.balance);

    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1000u128), vault_bal_res.balance);

    let vault_pool_config_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &vault_instance.clone(),
            &VaultQueryMsg::GetPoolById {
                pool_id: Uint128::from(1u128),
            },
        )
        .unwrap();

    let pool_config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&pool_addr.clone(), &QueryMsg::Config {})
        .unwrap();
    assert_eq!(pool_config_res.assets, vault_pool_config_res.assets);
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                amount: Uint128::from(1109u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(1111u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(1000u128),
            },
        ],
        vault_pool_config_res.assets
    );

    //// -----x----- Check #3.4 :: Success -----x----- ////
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "axlusd".to_string(),
            },
            amount: Uint128::from(1090_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(1110_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(1500_000000u128),
        },
    ];

    // -------x---- StableSwap Pool -::- QueryOnJoinPool ----x---------
    // assets_in: Some([Asset { info: AssetInfo::NativeToken { denom: "axlusd".to_string() }, amount: Uint128(1090000000) }, Asset { info: Token { contract_addr: Addr("contract1") }, amount: Uint128(1110000000) }, Asset { info: Token { contract_addr: Addr("contract2") }, amount: Uint128(1500000000) }])
    // assets_in sorted
    // act_assets_in: [Asset { info: AssetInfo::NativeToken { denom: "axlusd".to_string() }, amount: Uint128(1090000000) }, Asset { info: Token { contract_addr: Addr("contract1") }, amount: Uint128(1110000000) }, Asset { info: Token { contract_addr: Addr("contract2") }, amount: Uint128(1500000000) }]
    // Asset pools stored in a hashmap
    // asset:"axlusd" Provided amount:"1090000000" Pool Liquidity:"1109"
    // asset:"contract1" Provided amount:"1110000000" Pool Liquidity:"1111"
    // asset:"contract2" Provided amount:"1500000000" Pool Liquidity:"1000"
    // amp: 1000
    // n_coins: 3

    // compute_d() Function
    // init_d (Initial invariant (D)): Decimal256(Uint256(3219649422713044))

    // compute_d() Function
    // deposit_d (Invariant (D) after deposit added): Decimal256(Uint256(3696237765829431864057))
    // current total LP token supply (Total share of LP tokens minted by the pool): Uint128(3217)
    // fee (total_fee_bps * N_COINS / (4 * (N_COINS - 1))): Decimal(Uint128(11250000000000000))
    // /nStart loop for fee stuff
    // deposit_d:3696.237765829431864057, old_balances[i]:0.001109, init_d:0.003219649422713044
    // ideal_balance (ideal_balance = deposit_d * old_balances[i] / init_d): "1273.159634520301849482"
    // ideal_balance:1273.159634520301849482 ,
    // new_balances:1090.001109,
    // difference:183.158525520301849482
    // new_balances[i] (new_balances[i] -= fee * difference): "1087.940575587896604194"
    // deposit_d:3696.237765829431864057, old_balances[i]:0.001111, init_d:0.003219649422713044
    // ideal_balance (ideal_balance = deposit_d * old_balances[i] / init_d): "1275.455684357128363367"
    // ideal_balance:1275.455684357128363367 ,
    // new_balances:1110.001111,
    // difference:165.454573357128363367
    // new_balances[i] (new_balances[i] -= fee * difference): "1108.139747049732305913"
    // deposit_d:3696.237765829431864057, old_balances[i]:0.001, init_d:0.003219649422713044
    // ideal_balance (ideal_balance = deposit_d * old_balances[i] / init_d): "1148.024918413256850137"
    // ideal_balance:1148.024918413256850137 ,
    // new_balances:1500.001,
    // difference:351.976081586743149863
    // new_balances[i] (new_balances[i] -= fee * difference): "1496.041269082149139565"
    // End loop for fee stuff

    // compute_d() Function
    // after_fee_d (Invariant (D) after fee): Decimal256(Uint256(3688385217245853637444))
    // total_share:3217, init_d:0.003219649422713044, after_fee_d:3688.385217245853637444
    // tokens_to_mint (Total share of LP tokens minted by the pool): Decimal256(Uint256(3685346858748742308952))
    // mint_amount (adj): Uint128(3685346858)
    // mint_amount (adj): Uint128(3685346858)
    // provided_assets: [Asset { info: AssetInfo::NativeToken { denom: "axlusd".to_string() }, amount: Uint128(1090000000) }, Asset { info: Token { contract_addr: Addr("contract1") }, amount: Uint128(1110000000) }, Asset { info: Token { contract_addr: Addr("contract2") }, amount: Uint128(1500000000) }]
    // Join Query Over
    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnJoinPool {
                assets_in: Some(assets_msg.clone()),
                mint_amount: None,
            },
        )
        .unwrap();
    assert_eq!(ResponseType::Success {}, join_pool_query_res.response);
    assert_eq!(
        Uint128::from(3686487023559696294465u128),
        join_pool_query_res.new_shares
    );

    // Execute AddLiquidity via the Vault contract
    let msg = VaultExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: Some("recipient".to_string()),
        min_lp_to_receive: None,
        auto_stake: None,
        assets: Some(assets_msg.clone()),
    };

    let _current_block = app.block_info();
    app.update_block(|b| {
        b.height += 10;
        b.time = Timestamp::from_seconds(_current_block.time.seconds() + 90)
    });

    app.execute_contract(
        alice_address.clone(),
        vault_instance.clone(),
        &msg,
        &[Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(1090000000u128),
        }],
    )
    .unwrap();

    //// -----x----- Check #3.5 :: Success -----x----- ////
    let assets_msg = vec![Asset {
        info: AssetInfo::Token {
            contract_addr: token_instance1.clone(),
        },
        amount: Uint128::from(1500_000000u128),
    }];

    let _current_block = app.block_info();
    app.update_block(|b| {
        b.height += 10;
        b.time = Timestamp::from_seconds(_current_block.time.seconds() + 90)
    });

    // -------x---- StableSwap Pool -::- QueryOnJoinPool ----x---------
    // --- StableSwap Pool:OnJoinPool Query : Begin ---
    // init_d: 3691.212147126202104076
    // deposit_d: 5129.699875790924368109
    // Fee will be charged only during imbalanced provide i.e. if invariant D was changed
    // For axlusd, fee is charged on 424.266112539587866702 amount, which is difference b/w 1512.948481539587866702 (ideal_balance) and 1088.682369 (new_balance). Fee charged:4.7729937660703635
    // For contract1, fee is charged on 432.109909885426293184 amount, which is difference b/w 1540.919749885426293184 (ideal_balance) and 1108.80984 (new_balance). Fee charged:4.861236486211045798
    // For contract2, fee is charged on 916.428129128471638874 amount, which is difference b/w 2081.038644871528361126 (ideal_balance) and 2997.466774 (new_balance). Fee charged:10.309816452695305937
    // after_fee_d (Invariant computed for - total tokens provided as liquidity - total fee): 5109.864501140504492175
    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnJoinPool {
                assets_in: Some(assets_msg.clone()),
                mint_amount: None,
            },
        )
        .unwrap();
    assert_eq!(ResponseType::Success {}, join_pool_query_res.response);
    assert_eq!(
        Uint128::from(1416837573911032858392u128),
        join_pool_query_res.new_shares
    );
}

/// Tests the following -
/// Pool::QueryMsg::OnExitPool for StableSwap Pool and the returned  [`AfterExitResponse`] struct to check if the math calculations are correct
/// Vault::ExecuteMsg::ExitPool - Token transfer from vault to recepient and LP tokens to be burnt are processed as expected and Balances are updated correctly
/// Vault::ExecuteMsg::UpdateLiquidity - Executed by the Vault at the end of join pool tx execution to update pool balances as stored in the Pool contract which are used for computations
#[test]
fn test_on_exit_pool() {
    let owner = Addr::unchecked("owner");
    let alice_address = Addr::unchecked("alice");
    let mut app = mock_app(
        owner.clone(),
        vec![Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(100000000000_000_000_000u128),
        }],
    );
    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(10000000000_000_000u128),
        }],
    )
    .unwrap();

    let (vault_instance, pool_addr, lp_token_addr, token_instance0, token_instance1, _) =
        instantiate_contracts_instance(&mut app, &owner);
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance0.clone(),
        Uint128::new(10000000000_000_000u128),
        alice_address.to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        Uint128::new(10000000000_000_000u128),
        alice_address.to_string(),
    );

    //// -----x----- Successfully provide liquidity and mint LP tokens -----x----- ////
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "axlusd".to_string(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
    ];
    let msg = VaultExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: None,
        min_lp_to_receive: None,
        auto_stake: None,
        assets: Some(assets_msg.clone()),
    };
    app.execute_contract(
        alice_address.clone(),
        token_instance0.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(1000000000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        alice_address.clone(),
        token_instance1.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(1000000000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        alice_address.clone(),
        vault_instance.clone(),
        &msg,
        &[Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(1000000000_000000u128),
        }],
    )
    .unwrap();

    let lp_supply: cw20::TokenInfoResponse = app
        .wrap()
        .query_wasm_smart(&lp_token_addr.clone(), &Cw20QueryMsg::TokenInfo {})
        .unwrap();
    assert_eq!(Uint128::from(300_0000000_000000_000000u128), lp_supply.total_supply);

    //// -----x----- Check #1 :: Error ::: Wrong token -----x----- ////

    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(50u8),
        msg: to_json_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(1u128),
            recipient: None,
            exit_type: vault::ExitType::ExactLpBurn {
                lp_to_burn: Uint128::from(50u8),
                min_assets_out: None,
            },
        })
        .unwrap(),
    };
    let res = app
        .execute_contract(
            alice_address.clone(),
            token_instance0.clone(),
            &exit_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "Unauthorized");

    //// -----x----- Check #2 :: Error ::: Burn amount not provided -----x----- ////

    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(50u8),
        msg: to_json_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(1u128),
            recipient: None,
            exit_type: vault::ExitType::ExactLpBurn {
                lp_to_burn: Uint128::from(0u8),
                min_assets_out: None,
            },
        })
        .unwrap(),
    };
    let res = app
        .execute_contract(alice_address.clone(), lp_token_addr.clone(), &exit_msg, &[])
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "ReceivedUnexpectedLpTokens - expected: 0, received: 50");

    //// -----x----- Check #3 :: Success ::: Successfully exit the pool - Imbalanced_withdraw() -----x----- ////

    let exit_pool_query_res: AfterExitResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnExitPool {
                exit_type: ExitType::ExactLpBurn(Uint128::from(5000_000000_000000u128)),
            },
        )
        .unwrap();
    assert_eq!(Some(vec![]), exit_pool_query_res.fee);
    assert_eq!(ResponseType::Success {}, exit_pool_query_res.response);
    assert_eq!(Uint128::from(5000_000000_000000u128), exit_pool_query_res.burn_shares);
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                amount: Uint128::from(1666u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(1666u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(1666u128),
            },
        ],
        exit_pool_query_res.assets_out
    );

    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(5000_000000_000000u128),
        msg: to_json_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(1u128),
            recipient: None,
            exit_type: vault::ExitType::ExactLpBurn {
                lp_to_burn: Uint128::from(5000_000000_000000u128),
                min_assets_out: None,
            },
        })
        .unwrap(),
    };
    app.execute_contract(alice_address.clone(), lp_token_addr.clone(), &exit_msg, &[])
        .unwrap();
    let _current_block = app.block_info();

    // Checks -
    // 1. LP tokens burnt
    // 2. Liquidity Pool balance updated
    // 3. Tokens transferred to the Vault
    // 4. TWAP updated
    let lp_supply: cw20::TokenInfoResponse = app
        .wrap()
        .query_wasm_smart(&lp_token_addr.clone(), &Cw20QueryMsg::TokenInfo {})
        .unwrap();
    assert_eq!(Uint128::from(2999995000000000000000u128), lp_supply.total_supply);

    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance0.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(999998334u128), vault_bal_res.balance);

    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(999998334u128), vault_bal_res.balance);

    let vault_pool_config_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &vault_instance.clone(),
            &VaultQueryMsg::GetPoolById {
                pool_id: Uint128::from(1u128),
            },
        )
        .unwrap();
    let pool_config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&pool_addr.clone(), &QueryMsg::Config {})
        .unwrap();
    assert_eq!(pool_config_res.assets, vault_pool_config_res.assets);
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                amount: Uint128::from(999998334u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(999998334u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(999998334u128),
            },
        ],
        vault_pool_config_res.assets
    );

    let _current_block = app.block_info();
    app.update_block(|b| {
        b.height += 10;
        b.time = Timestamp::from_seconds(_current_block.time.seconds() + 90)
    });

    let pool_twap_res: CumulativePricesResponse = app
        .wrap()
        .query_wasm_smart(&pool_addr.clone(), &QueryMsg::CumulativePrices {})
        .unwrap();
    assert_eq!(Uint128::from(29_99_995_000_000_000_000_000u128), pool_twap_res.total_share);
    assert_eq!(
        vec![
            AssetExchangeRate {
                offer_info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string()
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                rate: Uint128::from(999999991900u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string()
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                rate: Uint128::from(999999991900u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                ask_info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string()
                },
                rate: Uint128::from(999999991900u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                rate: Uint128::from(999999991900u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                ask_info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string()
                },
                rate: Uint128::from(999999991900u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                rate: Uint128::from(999999991900u128),
            }
        ],
        pool_twap_res.exchange_infos
    );

    //// -----x----- Check #2 :: Success ::: Successfully exit the pool - Imbalanced_withdraw() -----x----- ////

    let mut assets_out = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "axlusd".to_string(),
            },
            amount: Uint128::from(463_000000u128),
        },
        // Asset {
        //     info: AssetInfo::Token {
        //         contract_addr: token_instance0.clone(),
        //     },
        //     amount: Uint128::from(1110_000000u128),
        // },
        // Asset {
        //     info: AssetInfo::Token {
        //         contract_addr: token_instance1.clone(),
        //     },
        //     amount: Uint128::from(1500_000000u128),
        // },
    ];

    // Exit Query -::- Started
    // burn_amount:500000000
    // Current supply of LP tokens:2999995000
    // assets_out Some([Asset { info: NativeToken { denom: "axlusd" }, amount: Uint128(463000000) }])
    // Imbalanced withdraw
    // assets_collection - properly created
    // n_coins:3 amp:1000
    // init_d (Current Value):2999.995002
    // withdraw_d (After withdrawals):2527.430282023354549873
    // fee = total_fee_bps * N_COINS / (4 * (N_COINS - 1)):0.01125
    // ideal_balance:842.476760674451515781, new_balance:533.561701699912420448, difference:305.478426674451515781
    // ideal_balance:842.476760674451515781, new_balance:998.226216300087579553, difference:157.521573325548484219
    // ideal_balance:842.476760674451515781, new_balance:998.226216300087579553, difference:157.521573325548484219
    // after_fee_d (After fee applied):2520.332903880655139814
    // Current LP supply:2999995000
    // Lp tokens to be burnt (Calculated for imbalanced withdraw) Uint128(479662098)
    let exit_pool_query_res: AfterExitResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnExitPool {
                exit_type: ExitType::ExactAssetsOut(assets_out.clone()),
            },
        )
        .unwrap();
    assert_eq!(
        Asset {
            info: AssetInfo::NativeToken {
                denom: "axlusd".to_string()
            },
            amount: Uint128::from(3436632u128)
        },
        exit_pool_query_res.fee.clone().unwrap()[0]
    );
    assert_eq!(ResponseType::Success {}, exit_pool_query_res.response);
    assert_eq!(
        Uint128::from(479_662_097_799_569_595_515u128),
        exit_pool_query_res.burn_shares
    );
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                amount: Uint128::from(463000000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(0u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(0u128),
            }
        ],
        exit_pool_query_res.assets_out.clone()
    );

    Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(5000u128),
        msg: to_json_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(1u128),
            recipient: None,
            exit_type: vault::ExitType::ExactAssetsOut {
                assets_out: assets_out.clone(),
                max_lp_to_burn: Some(Uint128::from(500_000_000u128)),
            },
        })
        .unwrap(),
    };

    //// -----x----- Check #3 :: Success ::: Successfully exit the pool - Imbalanced_withdraw() -----x----- ////

    assets_out = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "axlusd".to_string(),
            },
            amount: Uint128::from(463_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(977_000000u128),
        },
    ];

    let _current_block = app.block_info();
    app.update_block(|b| {
        b.height += 10;
        b.time = Timestamp::from_seconds(_current_block.time.seconds() + 90)
    });

    // Exit Query -::- Started
    // burn_amount:500000000
    // Current supply of LP tokens:2999995000
    // assets_out:  "axlusd", amount: Uint128(463000000)
    //              "contract2", amount: Uint128(977000000)
    // Imbalanced withdraw
    // assets_collection - properly created
    // n_coins:3 amp:1000
    // init_d (Current Value):2999.995002
    // withdraw_d (After withdrawals):1309.606415609756312639
    // fee = total_fee_bps * N_COINS / (4 * (N_COINS - 1)):0.01125
    // ideal_balance:436.535471869918770443, new_balance:535.868126801036586168, difference:100.462862130081229557
    // ideal_balance:436.535471869918770443, new_balance:18.346041198963413833, difference:413.537137869918770443
    // ideal_balance:436.535471869918770443, new_balance:993.659376801036586168, difference:563.462862130081229557
    // after_fee_d (After fee applied):1265.711910224449560738
    // Current LP supply:2999995000
    // LP tokens to burn:1734283091
    // Lp tokens to be burnt (Calculated for imbalanced withdraw) Uint128(1734283091)
    // refund_assets:  "axlusd", amount: Uint128(463000000)
    //              "contract2", amount: Uint128(977000000)
    //              "contract1", amount: Uint128(0)
    let exit_pool_query_res: AfterExitResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnExitPool {
                exit_type: ExitType::ExactAssetsOut(assets_out.clone()),
            },
        )
        .unwrap();
    assert_eq!(
        Asset {
            info: AssetInfo::NativeToken {
                denom: "axlusd".to_string()
            },
            amount: Uint128::from(1130207u128)
        },
        exit_pool_query_res.fee.clone().unwrap()[0]
    );
    assert_eq!(ResponseType::Success {}, exit_pool_query_res.response);
    assert_eq!(
        Uint128::from(1734283090619359785679u128),
        exit_pool_query_res.burn_shares
    );
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                amount: Uint128::from(463000000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(0u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(977000000u128),
            }
        ],
        exit_pool_query_res.assets_out.clone()
    );

    Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(5000_000000u128),
        msg: to_json_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(1u128),
            recipient: None,
            exit_type: vault::ExitType::ExactAssetsOut {
                assets_out: assets_out.clone(),
                max_lp_to_burn: Some(Uint128::from(5000_000_000u128)),
            },
        })
        .unwrap(),
    };

    //// -----x----- Check #3 :: Success ::: Successfully exit the pool - Imbalanced_withdraw() -----x----- ////

    assets_out = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "axlusd".to_string(),
            },
            amount: Uint128::from(463_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(977_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(357_000000u128),
        },
    ];

    let _current_block = app.block_info();
    app.update_block(|b| {
        b.height += 10;
        b.time = Timestamp::from_seconds(_current_block.time.seconds() + 90)
    });

    // burn_amount:50000000000
    // Current supply of LP tokens:2999995000
    // assets_out:  "axlusd", amount: Uint128(463000000)
    //              "contract1", amount: Uint128(357000000)
    //              "contract2", amount: Uint128(977000000)
    // Imbalanced withdraw()
    // assets_collection - properly created
    // n_coins:3 amp:1000
    // init_d (Current Value):2999.995002
    // withdraw_d (After withdrawals):1049.421655898757097765
    // fee = total_fee_bps * N_COINS / (4 * (N_COINS - 1)):0.01125
    // ideal_balance:349.807218632919032238, new_balance:534.892433952120339113, difference:187.191115367080967762
    // ideal_balance:349.807218632919032238, new_balance:19.321734047879660888, difference:326.808884632919032238
    // ideal_balance:349.807218632919032238, new_balance:639.699933952120339113, difference:293.191115367080967762
    // after_fee_d (After fee applied):1023.281579916945584519
    // Current LP supply:2999995000
    // LP tokens to burn:1976713421
    let exit_pool_query_res: AfterExitResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnExitPool {
                exit_type: ExitType::ExactAssetsOut(assets_out.clone()),
            },
        )
        .unwrap();
    assert_eq!(
        Asset {
            info: AssetInfo::NativeToken {
                denom: "axlusd".to_string()
            },
            amount: Uint128::from(2105900u128)
        },
        exit_pool_query_res.fee.clone().unwrap()[0]
    );
    assert_eq!(ResponseType::Success {}, exit_pool_query_res.response);
    assert_eq!(
        Uint128::from(1_976_713_420_765_243_272_248u128),
        exit_pool_query_res.burn_shares
    );
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                amount: Uint128::from(463000000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(357000000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(977000000u128),
            }
        ],
        exit_pool_query_res.assets_out.clone()
    );

    Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(5000_000000u128),
        msg: to_json_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(1u128),
            recipient: None,
            exit_type: vault::ExitType::ExactAssetsOut {
                assets_out: assets_out.clone(),
                max_lp_to_burn: Some(Uint128::from(5000_000_000u128)),
            },
        })
        .unwrap(),
    };
}

/// Tests the following -
/// Pool::QueryMsg::OnSwap - for StableSwap Pool and the returned  [`SwapResponse`] struct to check if the math calculations are correct
/// Vault::ExecuteMsg::Swap - Token transfers of [`OfferAsset`], [`AskAsset`], and the fee charged are processed as expected and Balances are updated correctly
/// Vault::ExecuteMsg::UpdateLiquidity - Executed by the Vault at the end of join pool tx execution to update pool balances as stored in the Pool contract which are used for computations
#[test]
fn test_swap() {
    let owner = Addr::unchecked("owner");
    let alice_address = Addr::unchecked("alice");
    let mut app = mock_app(
        owner.clone(),
        vec![Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(10000000_000000u128),
        }],
    );
    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(10000000_000000u128),
        }],
    )
    .unwrap();

    let (vault_instance, pool_addr, _, token_instance0, token_instance1, _) =
        instantiate_contracts_instance(&mut app, &owner);
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance0.clone(),
        Uint128::new(10000000_000000u128),
        alice_address.to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        Uint128::new(10000000_000000u128),
        alice_address.to_string(),
    );

    //// -----x----- Successfully provide liquidity and mint LP tokens -----x----- ////
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "axlusd".to_string(),
            },
            amount: Uint128::from(56535_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(53335_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(59335_000000u128),
        },
    ];
    let msg = VaultExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: None,
        min_lp_to_receive: None,
        auto_stake: None,
        assets: Some(assets_msg.clone()),
    };
    app.execute_contract(
        alice_address.clone(),
        token_instance0.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(10000000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        alice_address.clone(),
        token_instance1.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(10000000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        alice_address.clone(),
        vault_instance.clone(),
        &msg,
        &[Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(10000000_000000u128),
        }],
    )
    .unwrap();

    //// -----x----- Check #1 :: Error ::: assets mismatch || SwapType not supported -----x----- ////
    let swap_offer_asset_res: SwapResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnSwap {
                swap_type: SwapType::GiveIn {},
                offer_asset: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                ask_asset: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                amount: Uint128::from(1000u128),
                max_spread: None,
                belief_price: None,
            },
        )
        .unwrap();
    assert_eq!(
        swap_offer_asset_res.response,
        ResponseType::Failure("Error during pool selection: Source and target assets are the same".to_string())
    );

    let swap_offer_asset_res: SwapResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnSwap {
                swap_type: SwapType::Custom("()".to_string()),
                offer_asset: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                ask_asset: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(1000u128),
                max_spread: None,
                belief_price: None,
            },
        )
        .unwrap();
    assert_eq!(
        swap_offer_asset_res.response,
        ResponseType::Failure("SwapType not supported".to_string())
    );

    //// -----x----- Check #1 :: QUERY Success :::  -----x----- ////

    let _current_block = app.block_info();
    app.update_block(|b| {
        b.height += 10;
        b.time = Timestamp::from_seconds(_current_block.time.seconds() + 90)
    });

    // SwapType:: axlUSD --> token0 ::: GiveIn {},
    // Swap Query Started -::- GiveIn
    // pools configured to decimal precision
    // offer_pool: DecimalAsset { info: NativeToken { denom: "axlusd" }, amount: Decimal256(Uint256(56535000000000000000000)) }
    // ask_pool: DecimalAsset { info: Token { contract_addr: Addr("contract1") }, amount: Decimal256(Uint256(53335000000000000000000)) }
    // offer_asset: Asset { info: NativeToken { denom: "axlusd" }, amount: Uint128(1000) }
    // Compute Swap Function
    // offer_asset_amount: 0.001
    // calc_y() Function
    // Returned y: 53334999005
    // new_ask_pool (calc_y() fn): 53334999005
    // return_amount: 995
    // spread_amount: 5
    // calc_amount: Uint128(995)
    // spread_amount: Uint128(5)
    // total_fee: Uint128(29)
    // ask_asset: "966contract1"
    // swap success
    // offer_asset: "axlusd" | amount:"1000"
    // ask_asset: "contract1" | amount:"966"
    // total_fee: "29" | "contract1"
    let swap_offer_asset_res: SwapResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnSwap {
                swap_type: SwapType::GiveIn {},
                offer_asset: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                ask_asset: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(1000u128),
                max_spread: Some(Decimal::from_ratio(1u128, 10u128)),
                belief_price: None,
            },
        )
        .unwrap();
    assert_eq!(swap_offer_asset_res.response, ResponseType::Success {});
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_in,
        Uint128::from(1000u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_out,
        Uint128::from(965u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(0u128)
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().info,
        AssetInfo::NativeToken {
            denom: "axlusd".to_string()
        }
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().amount,
        Uint128::from(30u128)
    );

    let _current_block = app.block_info();
    app.update_block(|b| {
        b.height += 10;
        b.time = Timestamp::from_seconds(_current_block.time.seconds() + 90)
    });

    // Swap Query Started -::- GiveOut
    // pools configured to decimal precision
    // offer_pool: DecimalAsset { info: NativeToken { denom: "axlusd" }, amount: Decimal256(Uint256(56535000000000000000000)) }
    // ask_pool: DecimalAsset { info: Token { contract_addr: Addr("contract1") }, amount: Decimal256(Uint256(53335000000000000000000)) }
    // Swap Type: GiveOut
    // ask_asset: Asset { info: Token { contract_addr: Addr("contract1") }, amount: Uint128(784600000) }
    // Compute compute_offer_amount Function
    // ask_amount before_commission: 808.865979
    // calc_y() Function
    // new_offer_pool (calc_y() ): 57349406287
    // offer_amount = new_offer_pool - offer_pool: 814406287
    // spread_amount: 5540308
    // commission_amount: 24265979
    // calc_amount: Uint128(814406287)
    // spread_amount: Uint128(5540308)
    // offer_asset: "814406287axlusd"
    // swap success
    // offer_asset: "axlusd" | amount:"814406287"
    // ask_asset: "contract1" | amount:"784600000"
    // total_fee: "24265979" | "contract1"
    // test test_swap ... ok
    let swap_offer_asset_res: SwapResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnSwap {
                swap_type: SwapType::GiveOut {},
                offer_asset: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                ask_asset: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(7846_00000u128),
                max_spread: Some(Decimal::from_ratio(2u128, 10u128)),
                belief_price: None,
            },
        )
        .unwrap();
    assert_eq!(swap_offer_asset_res.response, ResponseType::Success {});
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_out,
        Uint128::from(784600000u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_in,
        Uint128::from(814372347u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(0u128)
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().info,
        AssetInfo::NativeToken {
            denom: "axlusd".to_string()
        }
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().amount,
        Uint128::from(24431170u128)
    );

    //// -----x----- Check #2 :: QUERY Failure : Spread check failed :::  -----x----- ////
    // SwapType::GiveIn {},
    let swap_offer_asset_res: SwapResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnSwap {
                swap_type: SwapType::GiveIn {},
                offer_asset: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                ask_asset: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(30000_000000u128),
                max_spread: Some(Decimal::from_ratio(1u128, 100u128)),
                belief_price: None,
            },
        )
        .unwrap();

    // Success: since the max_spread field is ignored
    assert_eq!(
        swap_offer_asset_res.response,
        ResponseType::Success {}
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_in,
        Uint128::from(30000_000000u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_out,
        Uint128::from(27150_746218u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(0u128)
    );
    assert_eq!(swap_offer_asset_res.fee.clone(), Some(Asset {
        info: AssetInfo::NativeToken {
            denom: "axlusd".to_string()
        },
        amount: Uint128::from(900_000000u128)
    }));

    // SwapType::GiveOut {},
    let swap_offer_asset_res: SwapResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnSwap {
                swap_type: SwapType::GiveOut {},
                offer_asset: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                ask_asset: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(50000_000000u128),
                max_spread: Some(Decimal::from_ratio(2u128, 100u128)),
                belief_price: None,
            },
        )
        .unwrap();
    // Success: since we removed the check
    assert_eq!(
        swap_offer_asset_res.response,
        ResponseType::Success {  }
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_in,
        Uint128::from(63455_748363u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_out,
        Uint128::from(50000_000000u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(0u128)
    );
    assert_eq!(swap_offer_asset_res.fee.clone(), Some(Asset {
        info: native_asset_info("axlusd".to_string()),
        amount: Uint128::from(1903_672450u128)
    }));

    //// -----x----- Check #3 :: EXECUTE Success :::  -----x----- ////

    // Execute Swap :: GiveIn Type
    let swap_msg = VaultExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(1u128),
            swap_type: SwapType::GiveIn {},
            asset_in: AssetInfo::NativeToken {
                denom: "axlusd".to_string(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(1000u128),
            max_spread: Some(Decimal::from_ratio(20u128, 100u128)),
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };
    app.execute_contract(
        alice_address.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(10000u128),
        }],
    )
    .unwrap();

    // Checks -
    // 1. Tokens transferred as expected
    // 2. Liquidity Pool balance updated
    // 3. Tokens transferred to the Vault
    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance0.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(53334999035u128), vault_bal_res.balance);
    let keeper_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance0.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();

    let keeper_balance_axlusd = app
        .wrap()
        .query_balance(&"fee_collector".to_string(), "axlusd")
        .unwrap();


    assert_eq!(Uint128::from(0u128), keeper_bal_res.balance);
    assert_eq!(Uint128::from(19u128), keeper_balance_axlusd.amount);

    let vault_pool_config_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &vault_instance.clone(),
            &VaultQueryMsg::GetPoolById {
                pool_id: Uint128::from(1u128),
            },
        )
        .unwrap();
    let pool_config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&pool_addr.clone(), &QueryMsg::Config {})
        .unwrap();
    assert_eq!(pool_config_res.assets, vault_pool_config_res.assets);
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                amount: Uint128::from(56535000981u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(53334999035u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(59335000000u128),
            },
        ],
        vault_pool_config_res.assets
    );

    // Execute Swap :: GiveOut Type
    let swap_msg = VaultExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(1u128),
            swap_type: SwapType::GiveOut {},
            asset_in: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            asset_out: AssetInfo::NativeToken {
                denom: "axlusd".to_string(),
            },
            amount: Uint128::from(1000u128),
            max_spread: Some(Decimal::from_ratio(20u128, 100u128)),
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };
    app.execute_contract(
        alice_address.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(10000u128),
        }],
    )
    .unwrap();
    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(59335001016u128), vault_bal_res.balance);
}
