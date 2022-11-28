use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{attr, to_binary, Addr, Coin, Decimal, Timestamp, Uint128};
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

fn store_xyk_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(ContractWrapper::new_with_empty(
        xyk_pool::contract::execute,
        xyk_pool::contract::instantiate,
        xyk_pool::contract::query,
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

/// Initialize a new vault and a XYK Pool with the given assets - Tests the following:
/// Vault::ExecuteMsg::{ Config, PoolId, FeeParams}
/// Pool::QueryMsg::{ CreatePoolInstance}
fn instantiate_contracts_instance(app: &mut App, owner: &Addr) -> (Addr, Addr, Addr, Addr, u128) {
    let xyk_pool_code_id = store_xyk_pool_code(app);
    let vault_code_id = store_vault_code(app);
    let token_code_id = store_token_code(app);

    let pool_configs = vec![PoolTypeConfig {
        code_id: xyk_pool_code_id,
        pool_type: PoolType::Xyk {},
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
        decimals: 18,
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

    let asset_infos = vec![
        AssetInfo::NativeToken {
            denom: "xprt".to_string(),
        },
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
    ];

    // Initialize XYK Pool contract instance
    let current_block = app.block_info();
    let msg = VaultExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Xyk {},
        asset_infos: asset_infos.to_vec(),
        init_params: None,
        fee_info: None,
    };
    let res = app
        .execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

    assert_eq!(res.events[1].attributes[2], attr("pool_type", "xyk"));
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
    assert_eq!(PoolType::Xyk {}, pool_res.pool_type);

    let assets = vec![
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::zero(),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            amount: Uint128::zero(),
        },
    ];

    //// -----x----- Check :: ConfigResponse for XYK Pool -----x----- ////

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
    assert_eq!(PoolType::Xyk {}, pool_config_res.pool_type);
    assert_eq!(
        current_block.time.seconds(),
        pool_config_res.block_time_last
    );

    //// -----x----- Check :: FeeResponse for XYK Pool -----x----- ////
    let pool_fee_res: FeeResponse = app
        .wrap()
        .query_wasm_smart(pool_res.pool_addr.clone().unwrap(), &QueryMsg::FeeParams {})
        .unwrap();
    assert_eq!(300u16, pool_fee_res.total_fee_bps);

    //// -----x----- Check :: Pool-ID for XYK Pool -----x----- ////
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
        current_block.time.seconds() as u128,
    );
}

/// Tests Pool::ExecuteMsg::UpdateConfig for XYK Pool which is not supported
#[test]
fn test_update_config() {
    let owner = Addr::unchecked("owner");
    let alice_address = Addr::unchecked("alice");
    let mut app = mock_app(
        owner.clone(),
        vec![Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(1000000_000000u128),
        }],
    );

    let (_, pool_addr, _, _, _) = instantiate_contracts_instance(&mut app, &owner);

    //// -----x----- Success :: Function not supported -----x----- ////

    let res = app
        .execute_contract(
            alice_address.clone(),
            pool_addr.clone(),
            &ExecuteMsg::UpdateConfig { params: None },
            &[],
        )
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "Operation non supported");
}

/// Tests the following -
/// Pool::QueryMsg::OnJoinPool for XYK Pool and the returned  [`AfterJoinResponse`] struct to check if the math calculations are correct
/// Vault::ExecuteMsg::JoinPool - Token transfer from user to vault and LP token minting to user are processed as expected and Balances are updated correctly
/// Vault::ExecuteMsg::UpdateLiquidity - Executed by the Vault at the end of join pool tx execution to update pool balances as stored in the Pool contract which are used for computations
#[test]
fn test_query_on_join_pool() {
    let owner = Addr::unchecked("owner");
    let alice_address = Addr::unchecked("alice");
    let mut app = mock_app(
        owner.clone(),
        vec![Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(1000000_000000u128),
        }],
    );

    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(1000000_000000u128),
        }],
    )
    .unwrap();

    let (vault_instance, pool_addr, lp_token_addr, token_instance, _) =
        instantiate_contracts_instance(&mut app, &owner);

    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance.clone(),
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
                mint_amount: Some(Uint128::from(1000000_000000u128)),
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

    //// -----x----- Check #2 :: Success ::: Liquidity being provided when pool is empty -----x----- ////
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            amount: Uint128::from(100u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance.clone(),
            },
            amount: Uint128::from(100u128),
        },
    ];

    // Query :: OnJoinPool
    // assets sorted
    // deposit:100 contract1
    // deposit:100 xprt
    // Current total supply of LP tokens:0
    // Liquidity provided for the first time, mint sqrt(deposit1 * deposit2) LP tokens
    // New shares to be minted:100
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
    assert_eq!(Uint128::from(100u128), join_pool_query_res.new_shares);
    // Returned assets are in sorted order
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
                },
                amount: Uint128::from(100u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(100u128),
            },
        ],
        join_pool_query_res.provided_assets
    );

    // Query :: OnJoinPool
    // assets sorted
    // deposit:153000000 contract1
    // deposit:9674000000 xprt
    // Current total supply of LP tokens:0
    // Liquidity provided for the first time, mint sqrt(deposit1 * deposit2) LP tokens
    // New shares to be minted:1216602646
    let join_pool_query_res_2: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnJoinPool {
                assets_in: Some(vec![
                    Asset {
                        info: AssetInfo::NativeToken {
                            denom: "xprt".to_string(),
                        },
                        amount: Uint128::from(9674_000000u128),
                    },
                    Asset {
                        info: AssetInfo::Token {
                            contract_addr: token_instance.clone(),
                        },
                        amount: Uint128::from(153_000000u128),
                    },
                ]),
                mint_amount: None,
                slippage_tolerance: None,
            },
        )
        .unwrap();
    assert_eq!(ResponseType::Success {}, join_pool_query_res_2.response);
    assert_eq!(
        Uint128::from(1216602646u128),
        join_pool_query_res_2.new_shares
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
        "Insufficient number of xprt tokens sent. Tokens sent = 0. Tokens needed = 100"
    );

    //// -----x----- Check #2.2 :: Execution Error ::: CW20 tokens were not approved for transfer via the Vault contract -----x----- ////
    let add_liq_res = app
        .execute_contract(
            alice_address.clone(),
            vault_instance.clone(),
            &msg,
            &[Coin {
                denom: "xprt".to_string(),
                amount: Uint128::new(110u128),
            }],
        )
        .unwrap_err();
    assert_eq!(
        add_liq_res.root_cause().to_string(),
        "No allowance for this account"
    );

    //// -----x----- Check #2.2 :: Success ::: Successfully provide liquidity and mint LP tokens -----x----- ////

    app.execute_contract(
        alice_address.clone(),
        token_instance.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(1000000_00000u128),
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
            denom: "xprt".to_string(),
            amount: Uint128::new(110u128),
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
            &token_instance.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(100u128), vault_bal_res.balance);

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
                info: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
                },
                amount: Uint128::from(100u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(100u128),
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
    assert_eq!(Uint128::from(100u128), pool_twap_res.total_share);
    assert_eq!(
        vec![
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
                },
                ask_info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                rate: Uint128::from(90000000000u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
                },
                rate: Uint128::from(90000000000u128),
            },
        ],
        pool_twap_res.exchange_infos
    );

    //// -----x----- Check #3.1 :: Error ::: Provided spread amount exceeds allowed limit -----x----- ////

    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnJoinPool {
                assets_in: Some(assets_msg.clone()),
                mint_amount: None,
                slippage_tolerance: Some(Decimal::from_ratio(70u128, 100u128)),
            },
        )
        .unwrap();
    assert_eq!(
        ResponseType::Failure("error : Provided spread amount exceeds allowed limit".to_string()),
        join_pool_query_res.response
    );

    //// -----x----- Check #3.2 :: Error ::: Operation exceeds max slippage limit -----x----- ////
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            amount: Uint128::from(1000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance.clone(),
            },
            amount: Uint128::from(100u128),
        },
    ];
    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnJoinPool {
                assets_in: Some(assets_msg.clone()),
                mint_amount: None,
                slippage_tolerance: None, // Some(Decimal::from_ratio(70u128, 100u128)),
            },
        )
        .unwrap();
    assert_eq!(None, join_pool_query_res.fee);
    assert_eq!(
        ResponseType::Failure("error : Operation exceeds max slippage limit".to_string()),
        join_pool_query_res.response
    );

    //// -----x----- Check #3.3 :: Success -----x----- ////
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            amount: Uint128::from(7525_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance.clone(),
            },
            amount: Uint128::from(8452_000000u128),
        },
    ];

    // Query :: OnJoinPool
    // assets sorted
    // deposit:8452000000 contract1
    // deposit:7525000000 xprt
    // Current total supply of LP tokens:100
    // deposit:8452000000 current_pool_liq:100
    // deposit:7525000000 current_pool_liq:100
    // New shares to be minted:7525000000
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
        Uint128::from(7525000000u128),
        join_pool_query_res.new_shares
    );
    // Returned assets are in sorted order
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
                },
                amount: Uint128::from(8452000000u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(7525000000u128),
            },
        ],
        join_pool_query_res.provided_assets
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
        b.height += 17280;
        b.time = Timestamp::from_seconds(_current_block.time.seconds() + 90)
    });

    app.execute_contract(
        alice_address.clone(),
        vault_instance.clone(),
        &msg,
        &[Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(7525000000u128),
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
    assert_eq!(Uint128::from(7525000000u128), recepient_bal_res.balance);

    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(8452000100u128), vault_bal_res.balance);

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
                info: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
                },
                amount: Uint128::from(8452000100u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(7525000100u128),
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
    assert_eq!(Uint128::from(7525000100u128), pool_twap_res.total_share);
    assert_eq!(
        vec![
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
                },
                ask_info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                rate: Uint128::from(260128963675u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
                },
                rate: Uint128::from(281087043042u128),
            },
        ],
        pool_twap_res.exchange_infos
    );

    //// -----x----- Check #4 :: Error ::: Invalid tokens -----x----- ////
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            amount: Uint128::from(10u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked("token2".to_string()),
            },
            amount: Uint128::from(10u128),
        },
    ];
    let msg = VaultExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: Some("recipient".to_string()),
        lp_to_mint: None,
        auto_stake: None,
        slippage_tolerance: Some(Decimal::from_ratio(49u128, 100u128)),
        assets: Some(assets_msg.clone()),
    };
    let err_res = app
        .execute_contract(
            alice_address.clone(),
            vault_instance.clone(),
            &msg,
            &[Coin {
                denom: "xprt".to_string(),
                amount: Uint128::new(1100u128),
            }],
        )
        .unwrap_err();
    assert_eq!(
        err_res.root_cause().to_string(),
        "Invalid sequence of assets"
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
            denom: "xprt".to_string(),
            amount: Uint128::new(1000000_000000u128),
        }],
    );
    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(1000000_000000u128),
        }],
    )
    .unwrap();

    let (vault_instance, pool_addr, lp_token_addr, token_instance, _) =
        instantiate_contracts_instance(&mut app, &owner);
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance.clone(),
        Uint128::new(900_000_000_000),
        alice_address.to_string(),
    );

    //// -----x----- Successfully provide liquidity and mint LP tokens -----x----- ////
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            amount: Uint128::from(10_000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance.clone(),
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
        token_instance.clone(),
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
            denom: "xprt".to_string(),
            amount: Uint128::new(10000u128),
        }],
    )
    .unwrap();

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
            token_instance.clone(),
            &exit_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "Unauthorized");

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
    assert_eq!(ResponseType::Success {}, exit_pool_query_res.response);
    assert_eq!(Uint128::from(5000u128), exit_pool_query_res.burn_shares);
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
                },
                amount: Uint128::from(5000u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(5000u128),
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

    // Checks -
    // 1. LP tokens burnt
    // 2. Liquidity Pool balance updated
    // 3. Tokens transferred to the Vault
    // 4. TWAP updated
    let lp_supply: cw20::TokenInfoResponse = app
        .wrap()
        .query_wasm_smart(&lp_token_addr.clone(), &Cw20QueryMsg::TokenInfo {})
        .unwrap();
    assert_eq!(Uint128::from(5000u128), lp_supply.total_supply);

    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(5000u128), vault_bal_res.balance);

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
                info: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
                },
                amount: Uint128::from(5000u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(5000u128),
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
    assert_eq!(Uint128::from(5000u128), pool_twap_res.total_share);
    assert_eq!(
        vec![
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
                },
                ask_info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                rate: Uint128::from(90000000000u128),
            },
            AssetExchangeRate {
                offer_info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
                },
                rate: Uint128::from(90000000000u128),
            },
        ],
        pool_twap_res.exchange_infos
    );

    // Query :: OnExitPool
    // Burn amount:573
    // Current total supply of LP tokens:5000
    // Share ratio: 0.1146
    // pool liquidity: 5000 contract1
    // pool liquidity: 5000 xprt
    // Assets to be withdrawn:
    // 573 contract1
    // 573 xprt
    let exit_pool_query_res: AfterExitResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnExitPool {
                assets_out: None,
                burn_amount: Some(Uint128::from(573u128)),
            },
        )
        .unwrap();
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
                },
                amount: Uint128::from(573u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(573u128),
            },
        ],
        exit_pool_query_res.assets_out
    );
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
            denom: "xprt".to_string(),
            amount: Uint128::new(1000000_000000u128),
        }],
    );
    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(1000000_000000u128),
        }],
    )
    .unwrap();

    let (vault_instance, pool_addr, _, token_instance, _) =
        instantiate_contracts_instance(&mut app, &owner);
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance.clone(),
        Uint128::new(900_000_000_000),
        alice_address.to_string(),
    );

    //// -----x----- Successfully provide liquidity and mint LP tokens -----x----- ////
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            amount: Uint128::from(10_000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance.clone(),
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
        token_instance.clone(),
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
            denom: "xprt".to_string(),
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
                    denom: "xprt".to_string(),
                },
                ask_asset: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
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
                    denom: "xprt".to_string(),
                },
                ask_asset: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
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
    // SwapType::GiveIn {},
    // Query :: OnSwap :: give-in
    // Offer asset info:xprt
    // Ask asset info:contract1
    // Amount:1000
    // Current offer asset balance:10000
    // Current ask asset balance:10000
    // --- compute_swap()
    // offer_pool: 10000
    // ask_pool: 10000
    // offer_amount: 1000
    // cp: 100000000
    // return_amount  = (ask_pool - cp / (offer_pool + offer_amount)): 909
    // return amount:909
    // Spread amount:91
    // Total fee:27
    // Swap success
    // Offer asset:1000xprt
    // Ask asset:882contract1
    let swap_offer_asset_res: SwapResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnSwap {
                swap_type: SwapType::GiveIn {},
                offer_asset: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                ask_asset: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
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
        Uint128::from(882u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(91u128)
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().info,
        AssetInfo::Token {
            contract_addr: token_instance.clone(),
        }
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().amount,
        Uint128::from(27u128)
    );

    // SwapType::GiveOut {},
    // Query :: OnSwap :: give-out
    // Offer asset info:xprt
    // Ask asset info:contract1
    // Amount:1000
    // Current offer asset balance:10000
    // Current ask asset balance:10000
    // --- compute_offer_amount()
    // cp: 100000000
    // before_commission_deduction: 1030
    // offer_amount: 1148
    // spread_amount: 118
    // offer amount:1148
    // Spread amount:118
    // Before commission deduction:1030
    // Total fee:30
    // Swap success
    // Offer asset:1148xprt
    // Ask asset:1000contract1
    let swap_offer_asset_res: SwapResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnSwap {
                swap_type: SwapType::GiveOut {},
                offer_asset: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                ask_asset: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
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
        Uint128::from(1148u128)
    );
    assert_eq!(
        swap_offer_asset_res.trade_params.spread,
        Uint128::from(118u128)
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().info,
        AssetInfo::Token {
            contract_addr: token_instance.clone(),
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
                    denom: "xprt".to_string(),
                },
                ask_asset: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
                },
                amount: Uint128::from(1000u128),
                max_spread: Some(Decimal::from_ratio(1u128, 100u128)),
                belief_price: None,
            },
        )
        .unwrap();
    assert_eq!(
        swap_offer_asset_res.response,
        ResponseType::Failure(
            "error : Operation exceeds max spread limit. Current spread = 0.091".to_string()
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
    assert_eq!(swap_offer_asset_res.fee, None);

    // SwapType::GiveOut {},
    let swap_offer_asset_res: SwapResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnSwap {
                swap_type: SwapType::GiveOut {},
                offer_asset: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                ask_asset: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
                },
                amount: Uint128::from(1000u128),
                max_spread: Some(Decimal::from_ratio(2u128, 100u128)),
                belief_price: None,
            },
        )
        .unwrap();
    assert_eq!(
        swap_offer_asset_res.response,
        ResponseType::Failure(
            "error : Operation exceeds max spread limit. Current spread = 0.102787456445993031"
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
    assert_eq!(swap_offer_asset_res.fee, None);

    //// -----x----- Check #3 :: EXECUTE Failure : Spread check failed :::  -----x----- ////

    // Execute Swap :: GiveIn Type
    let swap_msg = VaultExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(1u128),
            swap_type: SwapType::GiveIn {},
            asset_in: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance.clone(),
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
            denom: "xprt".to_string(),
            amount: Uint128::new(10000u128),
        }],
    )
    .unwrap();

    // Checks -
    // 1. Tokens transferred as expected
    // 2. Liquidity Pool balance updated
    // 3. Tokens transferred to the Vault
    // 4. TWAP updated
    // assert_eq!(res.root_cause().to_string(), "Unauthorized");
    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(9101u128), vault_bal_res.balance);
    let dev_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance.clone(),
            &Cw20QueryMsg::Balance {
                address: "dev".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(4u128), dev_bal_res.balance);
    let keeper_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(13u128), keeper_bal_res.balance);
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
                info: AssetInfo::Token {
                    contract_addr: token_instance.clone(),
                },
                amount: Uint128::from(9101u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(11000u128),
            },
        ],
        vault_pool_config_res.assets
    );

    let keeper_bal_before = app
        .wrap()
        .query_balance(&"fee_collector".to_string(), "xprt")
        .unwrap();

    // Execute Swap :: GiveOut Type
    let swap_msg = VaultExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(1u128),
            swap_type: SwapType::GiveOut {},
            asset_in: AssetInfo::Token {
                contract_addr: token_instance.clone(),
            },
            asset_out: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
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
            denom: "xprt".to_string(),
            amount: Uint128::new(10000u128),
        }],
    )
    .unwrap();
    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(10041u128), vault_bal_res.balance);

    let keeper_bal_after = app
        .wrap()
        .query_balance(&"fee_collector".to_string(), "xprt")
        .unwrap();
    assert_eq!(
        keeper_bal_before.amount + Uint128::from(14u128),
        keeper_bal_after.amount
    );
}
