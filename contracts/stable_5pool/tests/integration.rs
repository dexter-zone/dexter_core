use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{attr, from_binary, to_binary, Addr, Coin, Decimal, Timestamp, Uint128};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse};
use cw_multi_test::{App, ContractWrapper, Executor};

use dexter::asset::{Asset, AssetInfo};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{
    AfterExitResponse, AfterJoinResponse, ConfigResponse, ExecuteMsg, FeeResponse, FeeStructs,
    QueryMsg, ResponseType, SwapResponse,
};
use dexter::vault::{
    Cw20HookMsg, ExecuteMsg as VaultExecuteMsg, FeeInfo, InstantiateMsg as VaultInstantiateMsg,
    PoolConfig, PoolInfo, PoolInfoResponse, PoolType, QueryMsg as VaultQueryMsg, SingleSwapRequest,
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
    let pool_contract = Box::new(
        ContractWrapper::new_with_empty(
            stable5pool::contract::execute,
            stable5pool::contract::instantiate,
            stable5pool::contract::query,
        )
        .with_reply_empty(stable5pool::contract::reply),
    );
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

    let pool_configs = vec![PoolConfig {
        code_id: stable5pool_code_id,
        pool_type: PoolType::Stable5Pool {},
        fee_info: FeeInfo {
            total_fee_bps: 300u16,
            protocol_fee_percent: 49u16,
            dev_fee_percent: 15u16,
            developer_addr: Some(Addr::unchecked("dev".to_string())),
        },
        is_disabled: false,
        is_generator_disabled: false,
    }];

    let vault_init_msg = VaultInstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: token_code_id,
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
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
        lp_token_name: None,
        lp_token_symbol: None,
    };
    let res = app
        .execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

    assert_eq!(res.events[1].attributes[1], attr("action", "create_pool"));
    assert_eq!(
        res.events[1].attributes[2],
        attr("pool_type", "stable-3-pool")
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
    assert_eq!(
        Some(Addr::unchecked("dev".to_string())),
        pool_res.developer_addr
    );

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
        .query_wasm_smart(pool_res.pool_addr.clone().unwrap(), &QueryMsg::Config {})
        .unwrap();
    assert_eq!(
        FeeStructs {
            total_fee_bps: 300u16,
        },
        pool_config_res.fee_info
    );
    assert_eq!(Uint128::from(1u128), pool_config_res.pool_id);
    assert_eq!(
        pool_res.lp_token_addr.clone().unwrap(),
        pool_config_res.lp_token_addr.unwrap()
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
        .query_wasm_smart(pool_res.pool_addr.clone().unwrap(), &QueryMsg::FeeParams {})
        .unwrap();
    assert_eq!(300u16, pool_fee_res.total_fee_bps);

    //// -----x----- Check :: Pool-ID for Stable Pool -----x----- ////
    let pool_id_res: Uint128 = app
        .wrap()
        .query_wasm_smart(pool_res.pool_addr.clone().unwrap(), &QueryMsg::PoolId {})
        .unwrap();
    assert_eq!(Uint128::from(1u128), pool_id_res);

    return (
        vault_instance,
        pool_res.pool_addr.unwrap(),
        pool_res.lp_token_addr.unwrap(),
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
            amount: Uint128::new(100_000_000_000u128),
        }],
    );
    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(1000_000_000u128),
        }],
    )
    .unwrap();

    let (vault_instance, pool_addr, _lp_token_addr, token_instance0, token_instance1, _) =
        instantiate_contracts_instance(&mut app, &owner);
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance0.clone(),
        Uint128::new(900_000_000_000),
        alice_address.to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        Uint128::new(900_000_000_000),
        alice_address.to_string(),
    );

    //// -----x----- Successfully provide liquidity and mint LP tokens -----x----- ////
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

    //  ###########  Check :: Faulure ::  Start changing amp with incorrect next amp   ###########

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

    //  ###########  Check :: Faulure ::  Start changing amp with big difference between the old and new amp value   ###########

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

    //  ########### Check :: Faulure ::   Start changing amp earlier than the MIN_AMP_CHANGING_TIME has elapsed    ###########

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
            amount: Uint128::new(100_000_000_000u128),
        }],
    );

    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(1000_000_000u128),
        }],
    )
    .unwrap();

    let (vault_instance, pool_addr, lp_token_addr, token_instance0, token_instance1, _) =
        instantiate_contracts_instance(&mut app, &owner);

    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance0.clone(),
        Uint128::new(900_000_000_000),
        alice_address.to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        Uint128::new(900_000_000_000),
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
    // Check Query Response
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
    assert_eq!(None, join_pool_query_res.fee);
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

    // let pool_twap_res: CumulativePricesResponse = app
    //     .wrap()
    //     .query_wasm_smart(&pool_addr.clone(), &QueryMsg::CumulativePrices {})
    //     .unwrap();
    // let pool_twap_res_t1: CumulativePriceResponse = app
    //     .wrap()
    //     .query_wasm_smart(
    //         &pool_addr.clone(),
    //         &QueryMsg::CumulativePrice {
    //             offer_asset: AssetInfo::NativeToken {
    //                 denom: "axlusd".to_string(),
    //             },
    //             ask_asset: AssetInfo::Token {
    //                 contract_addr: token_instance0.clone(),
    //             },
    //         },
    //     )
    //     .unwrap();
    // let pool_twap_res_t2: CumulativePriceResponse = app
    //     .wrap()
    //     .query_wasm_smart(
    //         &pool_addr.clone(),
    //         &QueryMsg::CumulativePrice {
    //             ask_asset: AssetInfo::NativeToken {
    //                 denom: "axlusd".to_string(),
    //             },
    //             offer_asset: AssetInfo::Token {
    //                 contract_addr: token_instance0.clone(),
    //             },
    //         },
    //     )
    //     .unwrap();

    // assert_eq!(Uint128::from(3000u128), pool_twap_res.total_share);
    // assert_eq!(
    //     vec![
    //         AssetExchangeRate {
    //             offer_info: AssetInfo::NativeToken {
    //                 denom: "axlusd".to_string(),
    //             },
    //             ask_info: AssetInfo::Token {
    //                 contract_addr: token_instance0.clone(),
    //             },
    //             rate: Uint128::from(1000000000000000u128),
    //         },
    //         AssetExchangeRate {
    //             offer_info: AssetInfo::NativeToken {
    //                 denom: "axlusd".to_string(),
    //             },
    //             ask_info: AssetInfo::Token {
    //                 contract_addr: token_instance1.clone(),
    //             },
    //             rate: Uint128::from(1000000000000000u128),
    //         },
    //         AssetExchangeRate {
    //             offer_info: AssetInfo::Token {
    //                 contract_addr: token_instance0.clone(),
    //             },
    //             ask_info: AssetInfo::NativeToken {
    //                 denom: "axlusd".to_string(),
    //             },
    //             rate: Uint128::from(1000000000000000u128),
    //         },
    //         AssetExchangeRate {
    //             offer_info: AssetInfo::Token {
    //                 contract_addr: token_instance0.clone(),
    //             },
    //             ask_info: AssetInfo::Token {
    //                 contract_addr: token_instance1.clone(),
    //             },
    //             rate: Uint128::from(1000000000000000u128),
    //         },
    //         AssetExchangeRate {
    //             offer_info: AssetInfo::Token {
    //                 contract_addr: token_instance1.clone(),
    //             },
    //             ask_info: AssetInfo::NativeToken {
    //                 denom: "axlusd".to_string(),
    //             },
    //             rate: Uint128::from(1000000000000000u128),
    //         },
    //         AssetExchangeRate {
    //             offer_info: AssetInfo::Token {
    //                 contract_addr: token_instance1.clone(),
    //             },
    //             ask_info: AssetInfo::Token {
    //                 contract_addr: token_instance0.clone(),
    //             },
    //             rate: Uint128::from(1000000000000000u128),
    //         },
    //     ],
    //     pool_twap_res.exchange_infos
    // );

    // assert_eq!(Uint128::from(1000u128), pool_twap_res_t1.total_share);
    // assert_eq!(
    //     AssetExchangeRate {
    //         offer_info: AssetInfo::NativeToken {
    //             denom: "axlusd".to_string(),
    //         },
    //         ask_info: AssetInfo::Token {
    //             contract_addr: token_instance.clone(),
    //         },
    //         rate: Uint128::from(1000000000000000u128),
    //     },
    //     pool_twap_res_t1.exchange_info
    // );

    // assert_eq!(Uint128::from(1000u128), pool_twap_res_t2.total_share);
    // assert_eq!(
    //     AssetExchangeRate {
    //         offer_info: AssetInfo::Token {
    //             contract_addr: token_instance.clone(),
    //         },
    //         ask_info: AssetInfo::NativeToken {
    //             denom: "axlusd".to_string(),
    //         },
    //         rate: Uint128::from(1000000000000000u128),
    //     },
    //     pool_twap_res_t2.exchange_info
    // );
    // assert_eq!((current_block.time.seconds() as u128) as u128, 1000000u128);

    // //// -----x----- Check #3.3 :: Success -----x----- ////
    // let assets_msg = vec![
    //     Asset {
    //         info: AssetInfo::NativeToken {
    //             denom: "axlusd".to_string(),
    //         },
    //         amount: Uint128::from(109u128),
    //     },
    //     Asset {
    //         info: AssetInfo::Token {
    //             contract_addr: token_instance.clone(),
    //         },
    //         amount: Uint128::from(111u128),
    //     },
    // ];
    // let join_pool_query_res: AfterJoinResponse = app
    //     .wrap()
    //     .query_wasm_smart(
    //         pool_addr.clone(),
    //         &QueryMsg::OnJoinPool {
    //             assets_in: Some(assets_msg.clone()),
    //             mint_amount: None,
    //             slippage_tolerance: Some(Decimal::from_ratio(49u128, 100u128)),
    //         },
    //     )
    //     .unwrap();
    // assert_eq!(ResponseType::Success {}, join_pool_query_res.response);
    // assert_eq!(Uint128::from(110u128), join_pool_query_res.new_shares);

    // // Execute AddLiquidity via the Vault contract
    let msg = VaultExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: Some("recipient".to_string()),
        lp_to_mint: None,
        auto_stake: None,
        slippage_tolerance: Some(Decimal::from_ratio(49u128, 100u128)),
        assets: Some(assets_msg.clone()),
    };

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(EPOCH_START + 900_00)
    });

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
    let recepient_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: "recipient".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(3000u128), recepient_bal_res.balance);

    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance0.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(2000u128), vault_bal_res.balance);

    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(2000u128), vault_bal_res.balance);

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
                amount: Uint128::from(2000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(2000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(2000u128),
            },
        ],
        vault_pool_config_res.assets
    );

    // let pool_twap_res: CumulativePricesResponse = app
    //     .wrap()
    //     .query_wasm_smart(&pool_addr.clone(), &QueryMsg::CumulativePrices {})
    //     .unwrap();
    // let pool_twap_res_t1: CumulativePriceResponse = app
    //     .wrap()
    //     .query_wasm_smart(
    //         &pool_addr.clone(),
    //         &QueryMsg::CumulativePrice {
    //             offer_asset: AssetInfo::NativeToken {
    //                 denom: "axlusd".to_string(),
    //             },
    //             ask_asset: AssetInfo::Token {
    //                 contract_addr: token_instance.clone(),
    //             },
    //         },
    //     )
    //     .unwrap();
    // let pool_twap_res_t2: CumulativePriceResponse = app
    //     .wrap()
    //     .query_wasm_smart(
    //         &pool_addr.clone(),
    //         &QueryMsg::CumulativePrice {
    //             ask_asset: AssetInfo::NativeToken {
    //                 denom: "axlusd".to_string(),
    //             },
    //             offer_asset: AssetInfo::Token {
    //                 contract_addr: token_instance.clone(),
    //             },
    //         },
    //     )
    //     .unwrap();

    // assert_eq!(Uint128::from(1110u128), pool_twap_res.total_share);
    // assert_eq!(
    //     vec![
    //         AssetExchangeRate {
    //             offer_info: AssetInfo::NativeToken {
    //                 denom: "axlusd".to_string(),
    //             },
    //             ask_info: AssetInfo::Token {
    //                 contract_addr: token_instance.clone(),
    //             },
    //             rate: Uint128::from(1090162308385933u128),
    //         },
    //         AssetExchangeRate {
    //             offer_info: AssetInfo::Token {
    //                 contract_addr: token_instance.clone(),
    //             },
    //             ask_info: AssetInfo::NativeToken {
    //                 denom: "axlusd".to_string(),
    //             },
    //             rate: Uint128::from(1089837983798379u128),
    //         },
    //     ],
    //     pool_twap_res.exchange_infos
    // );

    // assert_eq!(Uint128::from(1110u128), pool_twap_res_t1.total_share);
    // assert_eq!(
    //     AssetExchangeRate {
    //         offer_info: AssetInfo::NativeToken {
    //             denom: "axlusd".to_string(),
    //         },
    //         ask_info: AssetInfo::Token {
    //             contract_addr: token_instance.clone(),
    //         },
    //         rate: Uint128::from(1090162308385933u128),
    //     },
    //     pool_twap_res_t1.exchange_info
    // );

    // assert_eq!(Uint128::from(1110u128), pool_twap_res_t2.total_share);
    // assert_eq!(
    //     AssetExchangeRate {
    //         offer_info: AssetInfo::Token {
    //             contract_addr: token_instance.clone(),
    //         },
    //         ask_info: AssetInfo::NativeToken {
    //             denom: "axlusd".to_string(),
    //         },
    //         rate: Uint128::from(1089837983798379u128),
    //     },
    //     pool_twap_res_t2.exchange_info
    // );
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
            amount: Uint128::new(100_000_000_000u128),
        }],
    );
    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(1000_000_000u128),
        }],
    )
    .unwrap();

    let (vault_instance, pool_addr, lp_token_addr, token_instance0, token_instance1, _) =
        instantiate_contracts_instance(&mut app, &owner);
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance0.clone(),
        Uint128::new(900_000_000_000),
        alice_address.to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        Uint128::new(900_000_000_000),
        alice_address.to_string(),
    );

    //// -----x----- Successfully provide liquidity and mint LP tokens -----x----- ////
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

    let lp_supply: cw20::TokenInfoResponse = app
        .wrap()
        .query_wasm_smart(&lp_token_addr.clone(), &Cw20QueryMsg::TokenInfo {})
        .unwrap();
    assert_eq!(Uint128::from(30000u128), lp_supply.total_supply);

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
            burn_amount: None,
        })
        .unwrap(),
    };
    let res = app
        .execute_contract(alice_address.clone(), lp_token_addr.clone(), &exit_msg, &[])
        .unwrap_err();
    assert_eq!(
        res.root_cause().to_string(),
        "Pool logic not satisfied. Reason : error : Burn amount is zero"
    );

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
    assert_eq!(
        res.root_cause().to_string(),
        "Pool logic not satisfied. Reason : error : Burn amount is zero"
    );

    //// -----x----- Check #2 :: Success ::: Successfully exit the pool -----x----- ////

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
    assert_eq!(None, exit_pool_query_res.fee);
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
    assert_eq!(Uint128::from(25000u128), lp_supply.total_supply);

    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance0.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(8334u128), vault_bal_res.balance);

    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(8334u128), vault_bal_res.balance);

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
                amount: Uint128::from(8334u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(8334u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(8334u128),
            },
        ],
        vault_pool_config_res.assets
    );

    // let pool_twap_res: CumulativePricesResponse = app
    //     .wrap()
    //     .query_wasm_smart(&pool_addr.clone(), &QueryMsg::CumulativePrices {})
    //     .unwrap();
    // let pool_twap_res_t1: CumulativePriceResponse = app
    //     .wrap()
    //     .query_wasm_smart(
    //         &pool_addr.clone(),
    //         &QueryMsg::CumulativePrice {
    //             offer_asset: AssetInfo::NativeToken {
    //                 denom: "axlusd".to_string(),
    //             },
    //             ask_asset: AssetInfo::Token {
    //                 contract_addr: token_instance.clone(),
    //             },
    //         },
    //     )
    //     .unwrap();
    // let pool_twap_res_t2: CumulativePriceResponse = app
    //     .wrap()
    //     .query_wasm_smart(
    //         &pool_addr.clone(),
    //         &QueryMsg::CumulativePrice {
    //             ask_asset: AssetInfo::NativeToken {
    //                 denom: "axlusd".to_string(),
    //             },
    //             offer_asset: AssetInfo::Token {
    //                 contract_addr: token_instance.clone(),
    //             },
    //         },
    //     )
    //     .unwrap();

    // assert_eq!(Uint128::from(5000u128), pool_twap_res.total_share);
    // assert_eq!(
    //     vec![
    //         AssetExchangeRate {
    //             offer_info: AssetInfo::NativeToken {
    //                 denom: "axlusd".to_string(),
    //             },
    //             ask_info: AssetInfo::Token {
    //                 contract_addr: token_instance.clone(),
    //             },
    //             rate: Uint128::from(1000000000000000u128),
    //         },
    //         AssetExchangeRate {
    //             offer_info: AssetInfo::Token {
    //                 contract_addr: token_instance.clone(),
    //             },
    //             ask_info: AssetInfo::NativeToken {
    //                 denom: "axlusd".to_string(),
    //             },
    //             rate: Uint128::from(1000000000000000u128),
    //         },
    //     ],
    //     pool_twap_res.exchange_infos
    // );

    // assert_eq!(Uint128::from(5000u128), pool_twap_res_t1.total_share);
    // assert_eq!(
    //     AssetExchangeRate {
    //         offer_info: AssetInfo::NativeToken {
    //             denom: "axlusd".to_string(),
    //         },
    //         ask_info: AssetInfo::Token {
    //             contract_addr: token_instance.clone(),
    //         },
    //         rate: Uint128::from(1000000000000000u128),
    //     },
    //     pool_twap_res_t1.exchange_info
    // );

    // assert_eq!(Uint128::from(5000u128), pool_twap_res_t2.total_share);
    // assert_eq!(
    //     AssetExchangeRate {
    //         offer_info: AssetInfo::Token {
    //             contract_addr: token_instance.clone(),
    //         },
    //         ask_info: AssetInfo::NativeToken {
    //             denom: "axlusd".to_string(),
    //         },
    //         rate: Uint128::from(1000000000000000u128),
    //     },
    //     pool_twap_res_t2.exchange_info
    // );
    // assert_eq!((current_block.time.seconds() as u128) as u128, 1000000u128);
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
            amount: Uint128::new(100_000_000_000u128),
        }],
    );
    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[Coin {
            denom: "axlusd".to_string(),
            amount: Uint128::new(1000_000_000u128),
        }],
    )
    .unwrap();

    let (vault_instance, pool_addr, _, token_instance0, token_instance1, _) =
        instantiate_contracts_instance(&mut app, &owner);
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance0.clone(),
        Uint128::new(900_000_000_000),
        alice_address.to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        Uint128::new(900_000_000_000),
        alice_address.to_string(),
    );

    //// -----x----- Successfully provide liquidity and mint LP tokens -----x----- ////
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
        ResponseType::Failure("assets mismatch".to_string())
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

    // SwapType:: axlUSD --> token0 ::: GiveIn {},
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
        Uint128::from(962u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(9u128)
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

    // // SwapType:: axlUSD --> token0 ::: GiveOut {},
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
                amount: Uint128::from(1000u128),
                max_spread: Some(Decimal::from_ratio(2u128, 10u128)),
                belief_price: None,
            },
        )
        .unwrap();
    assert_eq!(swap_offer_asset_res.response, ResponseType::Success {});
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_out,
        Uint128::from(1000u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_in,
        Uint128::from(1039u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(9u128)
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().info,
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().amount,
        Uint128::from(30u128)
    );

    // SwapType:: axlUSD --> token1 ::: GiveIn {},
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
                    contract_addr: token_instance1.clone(),
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
        Uint128::from(962u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(9u128)
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().info,
        AssetInfo::Token {
            contract_addr: token_instance1.clone(),
        },
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().amount,
        Uint128::from(29u128)
    );

    // // SwapType:: axlUSD --> token1 ::: GiveOut {},
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
                amount: Uint128::from(1000u128),
                max_spread: Some(Decimal::from_ratio(2u128, 10u128)),
                belief_price: None,
            },
        )
        .unwrap();
    assert_eq!(swap_offer_asset_res.response, ResponseType::Success {});
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_out,
        Uint128::from(1000u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_in,
        Uint128::from(1039u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(9u128)
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().info,
        AssetInfo::Token {
            contract_addr: token_instance1.clone(),
        },
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().amount,
        Uint128::from(30u128)
    );

    // SwapType:: token1 --> axlUSD  ::: GiveIn {},
    let swap_offer_asset_res: SwapResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnSwap {
                swap_type: SwapType::GiveIn {},
                offer_asset: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                ask_asset: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
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
        Uint128::from(962u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(9u128)
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().info,
        AssetInfo::NativeToken {
            denom: "axlusd".to_string(),
        }
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().amount,
        Uint128::from(29u128)
    );

    // SwapType:: token1 --> axlUSD  ::: GiveOut {},
    let swap_offer_asset_res: SwapResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnSwap {
                swap_type: SwapType::GiveOut {},
                offer_asset: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                ask_asset: AssetInfo::NativeToken {
                    denom: "axlusd".to_string(),
                },
                amount: Uint128::from(1000u128),
                max_spread: Some(Decimal::from_ratio(2u128, 10u128)),
                belief_price: None,
            },
        )
        .unwrap();
    assert_eq!(swap_offer_asset_res.response, ResponseType::Success {});
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_out,
        Uint128::from(1000u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_in,
        Uint128::from(1039u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(9u128)
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().info,
        AssetInfo::NativeToken {
            denom: "axlusd".to_string(),
        }
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().amount,
        Uint128::from(30u128)
    );

    // SwapType:: token1 --> token0  ::: GiveIn {},
    let swap_offer_asset_res: SwapResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnSwap {
                swap_type: SwapType::GiveIn {},
                offer_asset: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
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
        Uint128::from(962u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(9u128)
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().info,
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        }
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().amount,
        Uint128::from(29u128)
    );

    // SwapType:: token1 --> token0  ::: GiveOut {},
    let swap_offer_asset_res: SwapResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnSwap {
                swap_type: SwapType::GiveOut {},
                offer_asset: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                ask_asset: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(1000u128),
                max_spread: Some(Decimal::from_ratio(2u128, 10u128)),
                belief_price: None,
            },
        )
        .unwrap();
    assert_eq!(swap_offer_asset_res.response, ResponseType::Success {});
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_out,
        Uint128::from(1000u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.amount_in,
        Uint128::from(1039u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(9u128)
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().info,
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        }
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().amount,
        Uint128::from(30u128)
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
                amount: Uint128::from(3000u128),
                max_spread: Some(Decimal::from_ratio(1u128, 100u128)),
                belief_price: None,
            },
        )
        .unwrap();
    assert_eq!(
        swap_offer_asset_res.response,
        ResponseType::Failure(
            "error : Operation exceeds max spread limit. Current spread = 0.028333333333333333"
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

    // // SwapType::GiveOut {},
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
                amount: Uint128::from(5000u128),
                max_spread: Some(Decimal::from_ratio(2u128, 100u128)),
                belief_price: None,
            },
        )
        .unwrap();
    assert_eq!(
        swap_offer_asset_res.response,
        ResponseType::Failure(
            "error : Operation exceeds max spread limit. Current spread = 0.060517681370761939"
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
    assert_eq!(Uint128::from(9020u128), vault_bal_res.balance);
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
                amount: Uint128::from(11000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(9020u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(10000u128),
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
    assert_eq!(Uint128::from(11030u128), vault_bal_res.balance);
}
