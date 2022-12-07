use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{attr, from_binary, to_binary, Addr, Coin, Decimal, Timestamp, Uint128};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse};
use cw_multi_test::{App, ContractWrapper, Executor};

use dexter::asset::{Asset, AssetExchangeRate, AssetInfo};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{
    AfterExitResponse, AfterJoinResponse, ConfigResponse, CumulativePricesResponse, ExecuteMsg,
    FeeResponse, FeeStructs, QueryMsg, ResponseType, SwapResponse,
};
use dexter::vault::{
    Cw20HookMsg, ExecuteMsg as VaultExecuteMsg, FeeInfo, InstantiateMsg as VaultInstantiateMsg,
    PoolTypeConfig, PoolInfo, PoolInfoResponse, PoolType, QueryMsg as VaultQueryMsg, SingleSwapRequest,
    SwapType,
};

use stable5pool::math::{MAX_AMP, MAX_AMP_CHANGE, MIN_AMP_CHANGING_TIME};
use stable5pool::state::{MathConfig, StablePoolParams, StablePoolUpdateParams};

const EPOCH_START: u64 = 1_000_000;

fn mock_app(owner: Addr, coins: Vec<Coin>) -> App {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(EPOCH_START);

    let mut app = App::new(|router, _, storage| {
        // initialization  moved to App construction
        router.bank.init_balance(storage, &owner, coins).unwrap();
    });
    app.set_block(env.block);
    app
}

fn store_vault_code(app: &mut App) -> u64 {
    let factory_contract = Box::new(
        ContractWrapper::new_with_empty(
            dexter_vault::contract::execute,
            dexter_vault::contract::instantiate,
            dexter_vault::contract::query,
        )
        .with_reply_empty(dexter_vault::contract::reply),
    );
    app.store_code(factory_contract)
}

fn store_stable_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(ContractWrapper::new_with_empty(
        stable5pool::contract::execute,
        stable5pool::contract::instantiate,
        stable5pool::contract::query,
    ));
    app.store_code(pool_contract)
}

fn store_token_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        lp_token::contract::execute,
        lp_token::contract::instantiate,
        lp_token::contract::query,
    ));
    app.store_code(token_contract)
}

// Mints some Tokens to "to" recipient
fn mint_some_tokens(app: &mut App, owner: Addr, token_instance: Addr, amount: Uint128, to: String) {
    let msg = cw20::Cw20ExecuteMsg::Mint {
        recipient: to.clone(),
        amount: amount,
    };
    let res = app
        .execute_contract(owner.clone(), token_instance.clone(), &msg, &[])
        .unwrap();
    assert_eq!(res.events[1].attributes[1], attr("action", "mint"));
    assert_eq!(res.events[1].attributes[2], attr("to", to));
    assert_eq!(res.events[1].attributes[3], attr("amount", amount));
}

/// Initialize a new vault and a Stable-5-Pool with the given assets - Tests the following:
/// Vault::ExecuteMsg::{ Config, PoolId, FeeParams}
/// Pool::QueryMsg::{ CreatePoolInstance}
fn instantiate_contracts_instance(
    app: &mut App,
    owner: &Addr,
) -> (Addr, Addr, Addr, Addr, Addr, u128) {
    let stable5pool_code_id = store_stable_pool_code(app);
    let vault_code_id = store_vault_code(app);
    let token_code_id = store_token_code(app);

    let pool_configs = vec![PoolTypeConfig {
        code_id: stable5pool_code_id,
        pool_type: PoolType::Stable5Pool {},
        default_fee_info: FeeInfo {
            total_fee_bps: 300u16,
            protocol_fee_percent: 49u16,
            dev_fee_percent: 15u16,
            developer_addr: Some(Addr::unchecked("dev".to_string())),
        },
        allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
        is_generator_disabled: false,
    }];

    let vault_init_msg = VaultInstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: token_code_id,
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        pool_creation_fee: None,
        auto_stake_impl: None,
        multistaking_address: None,
        generator_address: None,
    };

    // Initialize Vault contract instance
    let vault_instance = app
        .instantiate_contract(
            vault_code_id,
            owner.to_owned(),
            &vault_init_msg,
            &[],
            "vault",
            None,
        )
        .unwrap();

    // Create Token X
    let init_msg = TokenInstantiateMsg {
        name: "x_token".to_string(),
        symbol: "X-Tok".to_string(),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
        marketing: None,
    };
    let token_instance0 = app
        .instantiate_contract(
            token_code_id,
            Addr::unchecked(owner.clone()),
            &init_msg,
            &[],
            "x_token",
            None,
        )
        .unwrap();

    // Create Token Y
    let init_msg = TokenInstantiateMsg {
        name: "y_token".to_string(),
        symbol: "Y-Tok".to_string(),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
        marketing: None,
    };
    let token_instance1 = app
        .instantiate_contract(
            token_code_id,
            Addr::unchecked(owner.clone()),
            &init_msg,
            &[],
            "y_token",
            None,
        )
        .unwrap();

    let asset_infos = vec![
        AssetInfo::NativeToken {
            denom: "axlusd".to_string(),
        },
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
        AssetInfo::Token {
            contract_addr: token_instance1.clone(),
        },
    ];

    // Initialize Stable-3-Pool contract instance
    let current_block = app.block_info();
    let msg = VaultExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Stable5Pool {},
        asset_infos: asset_infos.to_vec(),
        init_params: Some(to_binary(&StablePoolParams { amp: 10u64 }).unwrap()),
        fee_info: None,
    };
    let res = app
        .execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

    assert_eq!(
        res.events[1].attributes[2],
        attr("pool_type", "stable-5-pool")
    );
    let pool_res: PoolInfo = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &VaultQueryMsg::GetPoolById {
                pool_id: Uint128::from(1u128),
            },
        )
        .unwrap();

    assert_eq!(Uint128::from(1u128), pool_res.pool_id);
    assert_eq!(PoolType::Stable5Pool {}, pool_res.pool_type);

    let assets = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "axlusd".to_string(),
            },
            amount: Uint128::zero(),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::zero(),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::zero(),
        },
    ];

    //// -----x----- Check :: ConfigResponse for Stable 3 Pool -----x----- ////

    let pool_config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(pool_res.pool_addr.clone(), &QueryMsg::Config {})
        .unwrap();
    assert_eq!(
        FeeStructs {
            total_fee_bps: 300u16,
        },
        pool_config_res.fee_info
    );
    assert_eq!(Uint128::from(1u128), pool_config_res.pool_id);
    assert_eq!(
        pool_res.lp_token_addr,
        pool_config_res.lp_token_addr
    );
    assert_eq!(vault_instance, pool_config_res.vault_addr);
    assert_eq!(assets, pool_config_res.assets);
    assert_eq!(PoolType::Stable5Pool {}, pool_config_res.pool_type);
    assert_eq!(
        current_block.time.seconds(),
        pool_config_res.block_time_last
    );
    assert_eq!(
        Some(
            to_binary(&MathConfig {
                init_amp: 10u64 * 100,
                init_amp_time: EPOCH_START,
                next_amp: 10u64 * 100,
                next_amp_time: EPOCH_START,
                greatest_precision: 6u8,
            })
            .unwrap()
        ),
        pool_config_res.math_params
    );

    //// -----x----- Check :: FeeResponse for Stable Pool -----x----- ////
    let pool_fee_res: FeeResponse = app
        .wrap()
        .query_wasm_smart(pool_res.pool_addr.clone(), &QueryMsg::FeeParams {})
        .unwrap();
    assert_eq!(300u16, pool_fee_res.total_fee_bps);

    //// -----x----- Check :: Pool-ID for Stable Pool -----x----- ////
    let pool_id_res: Uint128 = app
        .wrap()
        .query_wasm_smart(pool_res.pool_addr.clone(), &QueryMsg::PoolId {})
        .unwrap();
    assert_eq!(Uint128::from(1u128), pool_id_res);

    return (
        vault_instance,
        pool_res.pool_addr,
        pool_res.lp_token_addr,
        token_instance0,
        token_instance1,
        current_block.time.seconds() as u128,
    );
}

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
        lp_to_mint: None,
        auto_stake: None,
        slippage_tolerance: None,
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
        params: Some(
            to_binary(&StablePoolUpdateParams::StartChangingAmp {
                next_amp: MAX_AMP + 1,
                next_amp_time: app.block_info().time.seconds(),
            })
            .unwrap(),
        ),
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
        params: Some(
            to_binary(&StablePoolUpdateParams::StartChangingAmp {
                next_amp: 100 * MAX_AMP_CHANGE + 1,
                next_amp_time: app.block_info().time.seconds(),
            })
            .unwrap(),
        ),
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
        params: Some(
            to_binary(&StablePoolUpdateParams::StartChangingAmp {
                next_amp: 25,
                next_amp_time: app.block_info().time.seconds(),
            })
            .unwrap(),
        ),
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
        params: Some(
            to_binary(&StablePoolUpdateParams::StartChangingAmp {
                next_amp: 25,
                next_amp_time: app.block_info().time.seconds() + MIN_AMP_CHANGING_TIME,
            })
            .unwrap(),
        ),
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
    let params: StablePoolParams = from_binary(&res.additional_params.unwrap()).unwrap();
    assert_eq!(params.amp, 17u64);

    app.update_block(|b| {
        b.time = b.time.plus_seconds(MIN_AMP_CHANGING_TIME / 2);
    });

    let res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(pool_addr.clone(), &QueryMsg::Config {})
        .unwrap();
    let params: StablePoolParams = from_binary(&res.additional_params.unwrap()).unwrap();
    assert_eq!(params.amp, 25u64);

    // Start decreasing amp
    app.update_block(|b| {
        b.time = b.time.plus_seconds(MIN_AMP_CHANGING_TIME);
    });

    let msg = ExecuteMsg::UpdateConfig {
        params: Some(
            to_binary(&StablePoolUpdateParams::StartChangingAmp {
                next_amp: 15,
                next_amp_time: app.block_info().time.seconds() + MIN_AMP_CHANGING_TIME,
            })
            .unwrap(),
        ),
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

    let params: StablePoolParams = from_binary(&res.additional_params.unwrap()).unwrap();
    assert_eq!(params.amp, 20u64);

    // Stop changing amp
    let msg = ExecuteMsg::UpdateConfig {
        params: Some(to_binary(&StablePoolUpdateParams::StopChangingAmp {}).unwrap()),
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

    let params: StablePoolParams = from_binary(&res.additional_params.unwrap()).unwrap();
    assert_eq!(params.amp, 20u64);
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
                slippage_tolerance: None,
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

    // -------x---- Stable5Pool -::- QueryOnJoinPool ----x---------
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
                slippage_tolerance: None,
            },
        )
        .unwrap();
    assert_eq!(Some(vec![]), join_pool_query_res.fee);
    assert_eq!(ResponseType::Success {}, join_pool_query_res.response);
    assert_eq!(Uint128::from(3000u128), join_pool_query_res.new_shares);

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
        lp_to_mint: None,
        auto_stake: None,
        slippage_tolerance: None,
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

    assert_eq!(Uint128::from(3000u128), pool_twap_res.total_share);
    assert_eq!(
        vec![
            AssetExchangeRate {
                offer_info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                rate: Uint128::from(1000180000u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                rate: Uint128::from(1000180000u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                ask_info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                rate: Uint128::from(1000180000u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                rate: Uint128::from(1000180000u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                ask_info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                rate: Uint128::from(1000180000u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                rate: Uint128::from(1000180000u128),
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

    // -------x---- Stable5Pool -::- QueryOnJoinPool ----x---------
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
                slippage_tolerance: Some(Decimal::from_ratio(49u128, 100u128)),
            },
        )
        .unwrap();
    assert_eq!(ResponseType::Success {}, join_pool_query_res.response);
    assert_eq!(Uint128::from(217u128), join_pool_query_res.new_shares);

    // Execute AddLiquidity via the Vault contract
    let msg = VaultExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: Some("recipient".to_string()),
        lp_to_mint: None,
        auto_stake: None,
        slippage_tolerance: Some(Decimal::from_ratio(49u128, 100u128)),
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
    assert_eq!(Uint128::from(217u128), recepient_bal_res.balance);

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

    // -------x---- Stable5Pool -::- QueryOnJoinPool ----x---------
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
                slippage_tolerance: Some(Decimal::from_ratio(49u128, 100u128)),
            },
        )
        .unwrap();
    assert_eq!(ResponseType::Success {}, join_pool_query_res.response);
    assert_eq!(
        Uint128::from(3685346858u128),
        join_pool_query_res.new_shares
    );

    // Execute AddLiquidity via the Vault contract
    let msg = VaultExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: Some("recipient".to_string()),
        lp_to_mint: None,
        auto_stake: None,
        slippage_tolerance: Some(Decimal::from_ratio(49u128, 100u128)),
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

    // -------x---- Stable5Pool -::- QueryOnJoinPool ----x---------
    // --- Stable5Pool:OnJoinPool Query : Begin ---
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
                slippage_tolerance: Some(Decimal::from_ratio(49u128, 100u128)),
            },
        )
        .unwrap();
    assert_eq!(ResponseType::Success {}, join_pool_query_res.response);
    assert_eq!(
        Uint128::from(1416399369u128),
        join_pool_query_res.new_shares
    );
}

/// Tests the following -
/// Pool::QueryMsg::OnExitPool for XYK Pool and the returned  [`AfterExitResponse`] struct to check if the math calculations are correct
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
        lp_to_mint: None,
        auto_stake: None,
        slippage_tolerance: None,
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
    assert_eq!(Uint128::from(3000000000u128), lp_supply.total_supply);

    //// -----x----- Check #1 :: Error ::: Wrong token -----x----- ////

    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(50u8),
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(1u128),
            recipient: None,
            assets: None,
            burn_amount: Some(Uint128::from(50u8)),
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
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(1u128),
            recipient: None,
            assets: None,
            burn_amount: Some(Uint128::from(0u128)),
        })
        .unwrap(),
    };
    let res = app
        .execute_contract(alice_address.clone(), lp_token_addr.clone(), &exit_msg, &[])
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "Amount cannot be 0");

    //// -----x----- Check #3 :: Success ::: Successfully exit the pool - Imbalanced_withdraw() -----x----- ////

    let exit_pool_query_res: AfterExitResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnExitPool {
                assets_out: None,
                burn_amount: Some(Uint128::from(5000u128)),
            },
        )
        .unwrap();
    assert_eq!(Some(vec![]), exit_pool_query_res.fee);
    assert_eq!(ResponseType::Success {}, exit_pool_query_res.response);
    assert_eq!(Uint128::from(5000u128), exit_pool_query_res.burn_shares);
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
        amount: Uint128::from(5000u128),
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(1u128),
            recipient: None,
            assets: None,
            burn_amount: Some(Uint128::from(5000u128)),
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
    assert_eq!(Uint128::from(2999995000u128), lp_supply.total_supply);

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
    assert_eq!(Uint128::from(2999995000u128), pool_twap_res.total_share);
    assert_eq!(
        vec![
            AssetExchangeRate {
                offer_info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string()
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                rate: Uint128::from(999999991810u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string()
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                rate: Uint128::from(999999991810u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                ask_info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string()
                },
                rate: Uint128::from(999999991810u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                rate: Uint128::from(999999991810u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                ask_info: AssetInfo::NativeToken {
                    denom: "axlusd".to_string()
                },
                rate: Uint128::from(999999991810u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                rate: Uint128::from(999999991810u128),
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
                assets_out: Some(assets_out.clone()),
                burn_amount: Some(Uint128::from(500_000000u128)),
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
        Uint128::from(479662098u128),
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
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(1u128),
            recipient: None,
            assets: Some(assets_out.clone()),
            burn_amount: Some(Uint128::from(500_000000u128)),
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
                assets_out: Some(assets_out.clone()),
                burn_amount: Some(Uint128::from(5000_000000u128)),
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
        Uint128::from(1734283091u128),
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
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(1u128),
            recipient: None,
            assets: Some(assets_out.clone()),
            burn_amount: Some(Uint128::from(5000_000000u128)),
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
                assets_out: Some(assets_out.clone()),
                burn_amount: Some(Uint128::from(50000_000000u128)),
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
        Uint128::from(1976713421u128),
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
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(1u128),
            recipient: None,
            assets: Some(assets_out.clone()),
            burn_amount: Some(Uint128::from(5000_000000u128)),
        })
        .unwrap(),
    };
}

/// Tests the following -
/// Pool::QueryMsg::OnSwap - for XYK Pool and the returned  [`SwapResponse`] struct to check if the math calculations are correct
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
        lp_to_mint: None,
        auto_stake: None,
        slippage_tolerance: None,
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
        Uint128::from(966u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(5u128)
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().info,
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().amount,
        Uint128::from(29u128)
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
        Uint128::from(814406287u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(5540308u128)
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().info,
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().amount,
        Uint128::from(24265979u128)
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
    assert_eq!(
        swap_offer_asset_res.response,
        ResponseType::Failure(
            "error : Operation exceeds max spread limit. Current spread = 0.0697911555".to_string()
        )
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_in,
        Uint128::from(0u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_out,
        Uint128::from(0u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(0u128)
    );
    assert_eq!(swap_offer_asset_res.fee.clone(), None);

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
    assert_eq!(
        swap_offer_asset_res.response,
        ResponseType::Failure(
            "error : Operation exceeds max spread limit. Current spread = 0.216606015387939536"
                .to_string()
        )
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_in,
        Uint128::from(0u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_out,
        Uint128::from(0u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(0u128)
    );
    assert_eq!(swap_offer_asset_res.fee.clone(), None);

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
    assert_eq!(Uint128::from(53334999016u128), vault_bal_res.balance);
    let dev_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance0.clone(),
            &Cw20QueryMsg::Balance {
                address: "dev".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(4u128), dev_bal_res.balance);
    let keeper_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance0.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(14u128), keeper_bal_res.balance);
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
                amount: Uint128::from(56535001000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(53334999016u128),
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
    assert_eq!(Uint128::from(59335001034u128), vault_bal_res.balance);
}