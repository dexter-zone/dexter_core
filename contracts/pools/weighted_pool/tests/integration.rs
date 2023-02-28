use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{attr, to_binary, Addr, Coin, Decimal, Timestamp, Uint128};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse};
use cw_multi_test::{App, ContractWrapper, Executor};

use dexter::asset::{Asset, AssetExchangeRate, AssetInfo};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{AfterExitResponse, AfterJoinResponse, ConfigResponse, CumulativePricesResponse, ExecuteMsg, ExitType, FeeResponse, FeeStructs, QueryMsg, ResponseType, SwapResponse};
use dexter::vault;
use dexter::vault::{
    Cw20HookMsg, ExecuteMsg as VaultExecuteMsg, FeeInfo, InstantiateMsg as VaultInstantiateMsg, PauseInfo,
    PoolTypeConfig, PoolInfo, PoolInfoResponse, PoolType, QueryMsg as VaultQueryMsg, SingleSwapRequest,
    SwapType, PoolCreationFee,
};
use weighted_pool::state::{MathConfig, WeightedAsset, WeightedParams};

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

fn store_weighted_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(ContractWrapper::new_with_empty(
        weighted_pool::contract::execute,
        weighted_pool::contract::instantiate,
        weighted_pool::contract::query,
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

/// Initialize a new vault and a WEIGHTED  Pool with the given assets - Tests the following:
/// Vault::ExecuteMsg::{ Config, PoolId, FeeParams}
/// Pool::QueryMsg::{ CreatePoolInstance}
fn instantiate_contracts_instance(
    app: &mut App,
    owner: &Addr,
) -> (Addr, Addr, Addr, Addr, Addr, u128) {
    let weighted_pool_code_id = store_weighted_pool_code(app);
    let vault_code_id = store_vault_code(app);
    let token_code_id = store_token_code(app);

    let pool_configs = vec![PoolTypeConfig {
        code_id: weighted_pool_code_id,
        pool_type: PoolType::Weighted {},
        default_fee_info: FeeInfo {
            total_fee_bps: 300u16,
            protocol_fee_percent: 64u16,
        },
        allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
        paused: PauseInfo::default(),
    }];

    let vault_init_msg = VaultInstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: Some(token_code_id),
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        pool_creation_fee: PoolCreationFee::default(),
        auto_stake_impl: dexter::vault::AutoStakeImpl::None,
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

    // Create Token y
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
            denom: "xprt".to_string(),
        },
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
        AssetInfo::Token {
            contract_addr: token_instance1.clone(),
        },
    ];

    let asset_infos_with_weights = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            amount: Uint128::from(33u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(33u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(34u128),
        },
    ];

    // Initialize WEIGHTED  Pool contract instance
    let current_block = app.block_info();
    let msg = VaultExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Weighted {},
        asset_infos: asset_infos.to_vec(),
        native_asset_precisions: vec![("xprt".to_string(), 6u8)],
        init_params: Some(
            to_binary(&WeightedParams {
                weights: asset_infos_with_weights,
                exit_fee: Some(Decimal::from_ratio(1u128, 100u128)),
            })
            .unwrap(),
        ),
        fee_info: None
    };
    app.execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

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
    assert_eq!(PoolType::Weighted {}, pool_res.pool_type);

    let assets = vec![
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
        Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            amount: Uint128::zero(),
        },
    ];

    //// -----x----- Check :: ConfigResponse for WEIGHTED Pool -----x----- ////

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
    assert_eq!(PoolType::Weighted {}, pool_config_res.pool_type);
    assert_eq!(
        current_block.time.seconds(),
        pool_config_res.block_time_last
    );
    assert_eq!(
        to_binary(&vec![
            WeightedAsset {
                asset: Asset {
                    info: AssetInfo::Token {
                        contract_addr: token_instance0.clone(),
                    },
                    amount: Uint128::zero(),
                },
                weight: Decimal::from_ratio(33u128, 100u128)
            },
            WeightedAsset {
                asset: Asset {
                    info: AssetInfo::Token {
                        contract_addr: token_instance1.clone(),
                    },
                    amount: Uint128::zero(),
                },
                weight: Decimal::from_ratio(34u128, 100u128)
            },
            WeightedAsset {
                asset: Asset {
                    info: AssetInfo::NativeToken {
                        denom: "xprt".to_string(),
                    },
                    amount: Uint128::zero(),
                },
                weight: Decimal::from_ratio(33u128, 100u128)
            }
        ])
        .unwrap(),
        pool_config_res.additional_params.unwrap()
    );
    assert_eq!(
        to_binary(&MathConfig {
            exit_fee: Some(Decimal::from_ratio(1u128, 100u128)),
            greatest_precision: 6u8
        })
        .unwrap(),
        pool_config_res.math_params.unwrap()
    );

    //// -----x----- Check :: FeeResponse for WEIGHTED Pool -----x----- ////
    let pool_fee_res: FeeResponse = app
        .wrap()
        .query_wasm_smart(pool_res.pool_addr.clone(), &QueryMsg::FeeParams {})
        .unwrap();
    assert_eq!(300u16, pool_fee_res.total_fee_bps);

    //// -----x----- Check :: Pool-ID for WEIGHTED Pool -----x----- ////
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

/// Tests Pool::ExecuteMsg::UpdateConfig for WEIGHTED  Pool which is not supported
#[test]
fn test_update_config() {
    let owner = Addr::unchecked("owner");
    let alice_address = Addr::unchecked("alice");
    let mut app = mock_app(
        owner.clone(),
        vec![Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(100_000_000_000u128),
        }],
    );

    let (_, pool_addr, _, _, _, _) = instantiate_contracts_instance(&mut app, &owner);

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
/// Pool::QueryMsg::OnJoinPool for Weighted Pool and the returned  [`AfterJoinResponse`] struct to check if the math calculations are correct
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
            amount: Uint128::new(10000000000_000_000_000u128),
        }],
    );

    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(10000000000_000_000_000u128),
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
            amount: Uint128::from(46743_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(56742_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(28774_000000u128),
        },
    ];

    // When liquidity is provided for the first time, we mint a fixed number of LP tokens
    // Check Query Response
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
    assert_eq!(None, join_pool_query_res.fee);
    assert_eq!(ResponseType::Success {}, join_pool_query_res.response);
    assert_eq!(
        Uint128::from(100_000_000_000_000_000_000u128),
        join_pool_query_res.new_shares
    );
    // // Returned assets are in sorted order
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(56742_000000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(28774_000000u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(46743_000000u128),
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

    // //// -----x----- Check #2.1 :: Execution Error ::: If insufficient number of Native tokens were sent -----x----- ////
    let add_liq_res = app
        .execute_contract(alice_address.clone(), vault_instance.clone(), &msg, &[])
        .unwrap_err();
    assert_eq!(
        add_liq_res.root_cause().to_string(),
        "Insufficient number of xprt tokens sent. Tokens sent = 0. Tokens needed = 46743000000"
    );

    // //// -----x----- Check #2.2 :: Execution Error ::: CW20 tokens were not approved for transfer via the Vault contract -----x----- ////
    let add_liq_res = app
        .execute_contract(
            alice_address.clone(),
            vault_instance.clone(),
            &msg,
            &[Coin {
                denom: "xprt".to_string(),
                amount: Uint128::new(46743_000000u128),
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
        token_instance0.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(46743_00_000000u128),
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
            amount: Uint128::from(46743_00_000000u128),
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
            amount: Uint128::new(46743_000000u128),
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
    assert_eq!(Uint128::from(56742_000000u128), vault_bal_res.balance);

    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(28774_000000u128), vault_bal_res.balance);

    // validate vault xprt balance
    let vault_bal_res = app.wrap().query_balance(vault_instance.clone().to_string(), "xprt");
    assert_eq!(vault_bal_res.unwrap().amount, Uint128::new(46743_000000u128));

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
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(56742_000000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(28774_000000u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(46743_000000u128),
            },
        ],
        vault_pool_config_res.assets
    );

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(EPOCH_START + 900_00)
    });

    let pool_twap_res: CumulativePricesResponse = app
        .wrap()
        .query_wasm_smart(&pool_addr.clone(), &QueryMsg::CumulativePrices {})
        .unwrap();
    assert_eq!(Uint128::from(100_000_000_000_000_000_000u128), pool_twap_res.total_share);
    // TODO: verify twap rates
    assert_eq!(
        vec![
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone()
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                rate: Uint128::from(44296110000u128)
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone()
                },
                ask_info: AssetInfo::NativeToken {
                    denom: "xprt".to_string()
                },
                rate: Uint128::from(74138940000u128)
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone()
                },
                rate: Uint128::from(182850660000u128)
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                ask_info: AssetInfo::NativeToken {
                    denom: "xprt".to_string()
                },
                rate: Uint128::from(150628950000u128)
            },
            AssetExchangeRate {
                offer_info: AssetInfo::NativeToken {
                    denom: "xprt".to_string()
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone()
                },
                rate: Uint128::from(109249920000u128)
            },
            AssetExchangeRate {
                offer_info: AssetInfo::NativeToken {
                    denom: "xprt".to_string()
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                rate: Uint128::from(53771400000u128)
            }
        ],
        pool_twap_res.exchange_infos
    );

    //// -----x----- Check #2 :: Success ::: Single Asset Join Check -----x----- ////
    // We will join the pool with a single asset = 25774 tokens,
    // here, we solve constant function invariant with the calcs mentioned below,
    //  fee_ratio : 0.980200000000000000
    // token_amount_in_after_fee: 25263.674800 ,
    // weight_ratio = (weightX/weightY): 0.34
    // y = balanceXBefore/balanceXAfter : 1.878003572669771321
    // y_to_weight_ratio: 1.878003572669771321^0.34 = 1.238958556548818616
    // paranthetical: 0.238958556548818616
    // amount_y = (balanceY * (1 - (y ^ weight_ratio))): 23.8958556548818616
    // Num Shares (Single asset join) to be minted = 23.895855
    let single_asset_msg = vec![Asset {
        info: AssetInfo::Token {
            contract_addr: token_instance1.clone(),
        },
        amount: Uint128::from(25774_000000u128),
    }];
    // Check Query Response
    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnJoinPool {
                assets_in: Some(single_asset_msg.clone()),
                mint_amount: None,
            },
        )
        .unwrap();
    assert_eq!(
        Some(vec![Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(510325200u128)
        }]),
        join_pool_query_res.fee
    );
    assert_eq!(ResponseType::Success {}, join_pool_query_res.response);
    assert_eq!(Uint128::from(23895855654881861600u128), join_pool_query_res.new_shares);
    // // Returned assets are in sorted order
    assert_eq!(
        vec![
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
                amount: Uint128::from(25774_000000u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(0u128),
            },
        ],
        join_pool_query_res.provided_assets
    );

    // Execute function -::- Provide single asset join liquidity
    // Execute AddLiquidity via the Vault contract
    let single_join_msg = VaultExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: None,
        min_lp_to_receive: None,
        auto_stake: None,
        assets: Some(single_asset_msg.clone()),
    };

    app.execute_contract(
        alice_address.clone(),
        vault_instance.clone(),
        &single_join_msg,
        &[],
    )
    .unwrap();

    //// -----x----- Check #2 :: Success ::: Single Asset Join Check -----x----- ////
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(EPOCH_START + 1200_00)
    });
    let single_asset_msg = vec![Asset {
        info: AssetInfo::NativeToken {
            denom: "xprt".to_string(),
        },
        amount: Uint128::from(2577_000000u128),
    }];

    // We will join the pool with a single asset = 2577 tokens,
    // here, we solve constant function invariant with the calcs mentioned below,
    //  fee_ratio : 0.979900000000000000
    // token_amount_in_after_fee: 2525.202300
    // weight_ratio = (weightX/weightY): 0.33
    // y = balanceXBefore/balanceXAfter : 1.054023111481933123
    // y_to_weight_ratio:  1.054023111481933123^0.33: 1.017514353102858334
    // paranthetical: 0.017514353102858334
    // amount_y = (balanceY * (1 - (y ^ weight_ratio))): 2.169955752450536234
    // Num Shares (Single asset join) to be minted = 2.169955
    // Check Query Response
    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnJoinPool {
                assets_in: Some(single_asset_msg.clone()),
                mint_amount: None,
            },
        )
        .unwrap();
    assert_eq!(
        Some(vec![Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string().clone(),
            },
            amount: Uint128::from(51797700u128)
        }]),
        join_pool_query_res.fee
    );
    assert_eq!(ResponseType::Success {}, join_pool_query_res.response);
    assert_eq!(Uint128::from(2169955763920368399u128), join_pool_query_res.new_shares);
    // // Returned assets are in sorted order
    assert_eq!(
        vec![
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
                amount: Uint128::from(000000u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(2577_000000u128),
            },
        ],
        join_pool_query_res.provided_assets
    );

    // Execute function -::- Provide single asset join liquidity
    // Execute AddLiquidity via the Vault contract
    let single_join_msg = VaultExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: None,
        min_lp_to_receive: None,
        auto_stake: None,
        assets: Some(single_asset_msg.clone()),
    };

    app.execute_contract(
        alice_address.clone(),
        vault_instance.clone(),
        &single_join_msg,
        &[Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(100_000_000_000u128),
        }],
    )
    .unwrap();

    // Fetch current pool info
    let pool_info: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &VaultQueryMsg::GetPoolById { pool_id: Uint128::from(1u64) })
        .unwrap();

    // Validate the pool balances post liquidity addition
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(56_742_000_000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(54_221_391_872u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(49_286_849_472u128),
            },
        ],
        pool_info.assets
    );

    //// -----x----- Check #3 :: Success ::: Multi Asset Join Check -----x----- ////
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(EPOCH_START + 1500_00)
    });
    let multi_asset_msg = vec![
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(2977_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(3177_000000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            amount: Uint128::from(3477_000000u128),
        },
    ];

    // We will join the pool with all assets = 2977, 3177, 3477 tokens,
    // here, we first execute -maximal_exact_ratio_join()
    // contract1 - provided assets = 2977.000000, pool liquidity = 56742.000000 || share ratio: 0.052465545803813753
    // contract2 - provided assets = 3177.000000, pool liquidity = 54221.391872 || share ratio: 0.058593110400041336
    // xprt - provided assets = 3477.000000, pool liquidity = 49286.849472 || share ratio: 0.070546201212867006
    // ==> Number of shares to be minted = 6.614111603288115245 (Precision 18)
    // "contract2"  used_amount: 2844.754918 new_amount: 332.245082
    // "xprt"  used_amount: 2585.861458 new_amount: 891.138542
    // ------------------------------------------------------------
    // For remaining assets, we execute -calc_single_asset_join()
    // ------ contract2 - remaining assets = 332.245082 | pool balance = 57066.146790 (34%)
    // token_amount_in_after_fee: 325.666629 , fee_ratio :0.980200000000000000
    // weight_ratio = (weightX/weightY): 0.34
    // y = balanceXBefore/balanceXAfter : 1.005706827030015425
    // Calculated pow for 1.005706827030015425^0.34: 1.001936678569752017
    // y_to_weight_ratio: 1.001936678569752017
    // paranthetical: 0.001936678569752017
    // amount_y = (balanceY * (1 - (y ^ weight_ratio))): 0.256958363553229642
    // number of shares to mint for remaining assets of contract2 = 0.256958363553229642
    // ------- xprt - remaining assets = 891.138542 | pool balance = 51872.710930 (33%)
    // token_amount_in_after_fee: 873.226657 , fee_ratio :0.979900000000000000
    // weight_ratio = (weightX/weightY): 0.33
    // y = balanceXBefore/balanceXAfter : 1.0168340277834791
    // Calculated pow for 1.0168340277834791^0.33: 1.005524191288544062
    // y_to_weight_ratio: 1.005509784151347131
    // paranthetical: 0.005524191288544062
    // amount_y = (balanceY * (1 - (y ^ weight_ratio))): 0.734368762076787510
    //  number of shares to mint for remaining assets of xprt : 0.734368762076787510
    //-------------------------------------------------------------
    // Num Shares (Multi asset join) to be minted = 6.614111 + 0.242272 + 0.732372 = 7.588755
    // JoinPool-QueryResponse - Tokens to provide : 2977000000 contract1
    // JoinPool-QueryResponse - Tokens to provide : 3177000000 contract2
    // JoinPool-QueryResponse - Tokens to provide : 3477000000 xprt
    // JoinPool-QueryResponse New shares to be minted : 7.605438728918132397
    // JoinPool-QueryResponse Response : success
    // JoinPool-QueryResponse Fee : Token0: 6578453u128 xprt: 17911885u128

    // Check Query Response
    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnJoinPool {
                assets_in: Some(multi_asset_msg.clone()),
                mint_amount: None,
            },
        )
        .unwrap();
    assert_eq!(
        Some(vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(6578453u128)
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string()
                },
                amount: Uint128::from(17911885u128)
            }
        ]),
        join_pool_query_res.fee
    );
    assert_eq!(ResponseType::Success {}, join_pool_query_res.response);
    assert_eq!(Uint128::from(7605438728918132397u128), join_pool_query_res.new_shares);
    // // Returned assets are in sorted order
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(2977000000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(3177000000u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(3477000000u128),
            },
        ],
        join_pool_query_res.provided_assets
    );



    // Execute function -::- Provide single asset join liquidity
    // Execute AddLiquidity via the Vault contract
    let multi_asset_join_msg = VaultExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: None,
        min_lp_to_receive: None,
        auto_stake: None,
        assets: Some(multi_asset_msg.clone()),
    };
    app.execute_contract(
        alice_address.clone(),
        vault_instance.clone(),
        &multi_asset_join_msg,
        &[Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(3477000000u128),
        }],
    )
    .unwrap();

    // Lets test for TWAP
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(EPOCH_START + 1700_00)
    });
    let pool_twap_res: CumulativePricesResponse = app
        .wrap()
        .query_wasm_smart(&pool_addr.clone(), &QueryMsg::CumulativePrices {})
        .unwrap();
    // assert_eq!(Uint128::from(100_000000u128), pool_twap_res.total_share);
    assert_eq!(
        vec![
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone()
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                rate: Uint128::from(118599230000u128)
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone()
                },
                ask_info: AssetInfo::NativeToken {
                    denom: "xprt".to_string()
                },
                rate: Uint128::from(142576390000u128)
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone()
                },
                rate: Uint128::from(268981680000u128)
            },
            AssetExchangeRate {
                offer_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                ask_info: AssetInfo::NativeToken {
                    denom: "xprt".to_string()
                },
                rate: Uint128::from(224309220000u128)
            },
            AssetExchangeRate {
                offer_info: AssetInfo::NativeToken {
                    denom: "xprt".to_string()
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance0.clone()
                },
                rate: Uint128::from(202844490000u128)
            },
            AssetExchangeRate {
                offer_info: AssetInfo::NativeToken {
                    denom: "xprt".to_string()
                },
                ask_info: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                rate: Uint128::from(140698510000u128)
            }
        ],
        pool_twap_res.exchange_infos
    );


    //// -----x----- Check #4 :: Success ::: Multi Asset Join Check -----x----- ////
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(EPOCH_START + 2100_00)
    });
    let multi_asset_msg = vec![
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(63770_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(63770_000000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            amount: Uint128::from(54670_000000u128),
        },
    ];

    // We will join the pool with all assets =
    // here, we first execute -maximal_exact_ratio_join()
    // contract1 - provided assets = 63770.000000, pool liquidity = 59719.000000 || share ratio: 1.06783435757464123
    // contract2 - provided assets = 63770.0000000, pool liquidity = 57394.181663 || share ratio: 1.111088234944035532
    // xprt - provided assets = 54670.000000, pool liquidity = 52752.385866 || share ratio: 1.036351230423417528
    // ==> Number of shares to be minted = 138.530364562826429567
    // "contract1"  used_amount: 61889.859129 new_amount: 1880.140871
    // "contract2"  used_amount: 59480.530785 new_amount: 4289.469215
    // "xprt"  used_amount: 54670.000000 new_amount: 0
    // ------------------------------------------------------------
    // For remaining assets, we execute -calc_single_asset_join()
    // ------ contract1 - remaining assets = 1880.140871 | pool balance = 121608.859129 (33%)
    // token_amount_in_after_fee: 1842.350039 , fee_ratio : 0.979900000000000000
    // fee charged: 37.790832
    // weight_ratio = (weightX/weightY): 0.33
    // y = balanceXBefore/balanceXAfter : 1.004974273163847065
    // Calculated pow for 1.004974273163847065^0.33: 1.004974273163847065
    // y_to_weight_ratio: 1.004974273163847065
    // paranthetical: 0.004974273163847065
    // amount_y = (balanceY * (1 - (y ^ weight_ratio))): 1.354005187210511381
    // number of shares to mint for remaining assets of contract2 = 1.354005187210511381
    // ------ contract2 - remaining assets = 4289.469215 | pool balance = 116874.712448 (34%)
    // token_amount_in_after_fee: 4204.537724 , fee_ratio : 0.980200000000000000
    // fee charged: 84.931491
    // weight_ratio = (weightX/weightY): 0.34
    // y = balanceXBefore/balanceXAfter : 1.03597474283302033
    // Calculated pow for 1.051323594629709831^0.34: 1.012089028498860541
    // y_to_weight_ratio: 1.012089028498860541
    // paranthetical: 1.012089028498860541
    // amount_y = (balanceY * (1 - (y ^ weight_ratio))): 3.307021684967449713
    // number of shares to mint for remaining assets of contract2 = 1.354005187210511381
    
    //-------------------------------------------------------------
    // Num Shares (Multi asset join) to be minted = 138.530364562826429567 + 1.354005187210511381 + 1.354005187210511381 = 143.191391435004390661
    // JoinPool-QueryResponse - Tokens to provide : 63770000000 contract1
    // JoinPool-QueryResponse - Tokens to provide : 63770000000 contract2
    // JoinPool-QueryResponse - Tokens to provide : 54670000000 xprt
    // JoinPool-QueryResponse New shares to be minted : 142.759135

    // Check Query Response
    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnJoinPool {
                assets_in: Some(multi_asset_msg.clone()),
                mint_amount: None,
            },
        )
        .unwrap();
    assert_eq!(
        Some(vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(37790832u128)
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(84931491u128)
            },
        ]),
        join_pool_query_res.fee
    );
    assert_eq!(ResponseType::Success {}, join_pool_query_res.response);
    assert_eq!(Uint128::from(143191391435004390661u128), join_pool_query_res.new_shares);
    // // Returned assets are in sorted order
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(63770_000000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(63770_000000u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(54670_000000u128),
            },
        ],
        join_pool_query_res.provided_assets
    );

    // Execute function -::- Provide single asset join liquidity
    // Execute AddLiquidity via the Vault contract
    let multi_asset_join_msg = VaultExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: None,
        min_lp_to_receive: None,
        auto_stake: None,
        assets: Some(multi_asset_msg.clone()),
    };
    app.execute_contract(
        alice_address.clone(),
        vault_instance.clone(),
        &multi_asset_join_msg,
        &[Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(54670_000000u128),
        }],
    )
    .unwrap();
}

/// Tests the following -
/// Pool::QueryMsg::OnExitPool for Weighted Pool and the returned  [`AfterExitResponse`] struct to check if the math calculations are correct
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
            amount: Uint128::new(10000000_000_000_000u128),
        }],
    );
    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(1000900_000_000_000),
        }],
    )
    .unwrap();

    let (vault_instance, pool_addr, lp_token_addr, token_instance0, token_instance1, _) =
        instantiate_contracts_instance(&mut app, &owner);
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance0.clone(),
        Uint128::new(1000900_000_000_000),
        alice_address.to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        Uint128::new(1000900_000_000_000),
        alice_address.to_string(),
    );

    //// -----x----- Successfully provide liquidity and mint LP tokens -----x----- ////
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            amount: Uint128::from(46743_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(56742_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(28774_000000u128),
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
            amount: Uint128::from(10046743_000000u128),
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
            amount: Uint128::from(10046743_000000u128),
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
            amount: Uint128::new(46743_000000u128),
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

    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(50u8),
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(1u128),
            recipient: None,
            exit_type: vault::ExitType::ExactLpBurn {
                lp_to_burn: Uint128::from(0u128),
                min_assets_out: None,
            },
        })
        .unwrap(),
    };
    let res = app
        .execute_contract(alice_address.clone(), lp_token_addr.clone(), &exit_msg, &[])
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "Amount cannot be 0");

    //// -----x----- Check #2 :: Success ::: Successfully exit the pool -----x----- ////

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(EPOCH_START + 900_00)
    });

    let exit_pool_query_res: AfterExitResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &QueryMsg::OnExitPool {
                exit_type: ExitType::ExactLpBurn(Uint128::from(5000_000_000_000_000u128)),
            },
        )
        .unwrap();
    assert_eq!(ResponseType::Success {}, exit_pool_query_res.response);
    assert_eq!(Uint128::from(5000_000_000_000_000u128), exit_pool_query_res.burn_shares);
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(2808729u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(1424313u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(2313778u128),
            },
        ],
        exit_pool_query_res.assets_out
    );

    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(5000_000_000_000_000u128),
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(1u128),
            recipient: None,
            exit_type: vault::ExitType::ExactLpBurn {
                lp_to_burn: Uint128::from(5000_000_000_000_000u128),
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
    assert_eq!(Uint128::from(9_9995_000_000_000_000_000u128), lp_supply.total_supply);

    let vault_bal_res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance0.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(56739191271u128), vault_bal_res.balance);

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
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::from(56739191271u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone(),
                },
                amount: Uint128::from(28772575687u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::from(46740686222u128),
            },
        ],
        vault_pool_config_res.assets
    );
}

/// Tests the following -
/// Pool::QueryMsg::OnSwap - for Weighted Pool and the returned  [`SwapResponse`] struct to check if the math calculations are correct
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
            amount: Uint128::new(10000000_000_000_000u128),
        }],
    );
    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(1000900_000_000_000),
        }],
    )
    .unwrap();

    let (vault_instance, pool_addr, _lp_token_addr, token_instance0, token_instance1, _) =
        instantiate_contracts_instance(&mut app, &owner);
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance0.clone(),
        Uint128::new(1000900_000_000_000),
        alice_address.to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        Uint128::new(1000900_000_000_000),
        alice_address.to_string(),
    );

    //// -----x----- Successfully provide liquidity and mint LP tokens -----x----- ////
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            amount: Uint128::from(46743_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(56742_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(28774_000000u128),
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
            amount: Uint128::from(10046743_000000u128),
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
            amount: Uint128::from(10046743_000000u128),
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
            amount: Uint128::new(46743_000000u128),
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
        ResponseType::Failure("Error during pool selection: Source and target assets are the same".to_string())
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
    // SwapType::GiveIn {}, XPRT --> Token0
    // offer_asset_info : xprt, ask_asset_info : contract1, amount : 1000
    // offer_pool : xprt 46743
    // ask_pool : contract1 56742
    // offer_weight : 0.33 ask_weight : 0.33
    // ---------- SwapType::GiveIn
    // offer_asset : 1000xprt || amount = 1000
    // offer asset after fee: 970xprt || amount = 970
    // fee: 30xprt || amount = 30
    // pool_post_swap_in_balance: 46743.00097
    // weight_ratio = (weightX/weightY): 1
    // y = balanceXBefore/balanceXAfter : 0.999999979248230112
    // y_to_weight_ratio: 0.999999979248230112
    // paranthetical: 0.000000020751769888
    // amount_y = (balanceY * (1 - (y ^ weight_ratio))): 0.001177496926984896
    // return_amount: 0.001177496926984896
    // return_amount (adjusted to correct precision): 1177
    // calc_amount : 1177 || spread_amount = 0
    // ask_asset : 1177, contract1 amount: 1177
    // offer_asset:xprt , amount_in : 1000 || ask_asset:contract1 , amount_out = 1177
    // total_fee : 30xprt 
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
        Uint128::from(1177u128)
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().info,
        AssetInfo::NativeToken {
            denom: "xprt".to_string(),
        }
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().amount,
        Uint128::from(30u128)
    );

    // SwapType::GiveOut {},  XPRT --> Token0
    // offer_asset_info : xprt  ask_asset_info : contract1  amount : 1000
    // offer_pool : xprt 46743.000000
    // ask_pool : contract1 56742.000000
    // offer_weight : 0.33 ask_weight : 0.33
    // ---------- SwapType::GiveOut
    // ask_asset : contract1 || amount = 1000 (0.001000)
    // weight_ratio = (weightX/weightY): 1
    // y = balanceXBefore/balanceXAfter : 1.000000017623630073
    // y_to_weight_ratio: 1.000000017623630073
    // paranthetical: 0.000000017623630073
    // amount_y = (balanceY * (1 - (y ^ weight_ratio))): 0.000823781340502239
    // calc_amount : "848" || spread_amount = "0"
    // total_fee : 25
    // offer amount excluding fee: 823
    // ask_asset : 1000contract1 amount: 1000
    // offer_asset:xprt , amount_in : 848 || ask_asset:contract1 , amount_out = 1000
    // total_fee : 25 xprt

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
        Uint128::from(848u128)
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().info,
        AssetInfo::NativeToken { denom: "xprt".to_string() }
    );
    assert_eq!(
        swap_offer_asset_res.fee.clone().unwrap().amount,
        Uint128::from(25u128)
    );

    // ----- Execute GiveIn Swap----- //
    let swap_msg = VaultExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(1u128),
            swap_type: SwapType::GiveIn {},
            asset_in: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(1000u128),
            max_spread: None,
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

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(EPOCH_START + 900_00)
    });

    // ----- Execute GiveOut Swap----- //
    let swap_msg = VaultExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(1u128),
            swap_type: SwapType::GiveOut {},
            asset_in: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(1000u128),
            max_spread: None,
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
}

#[test]
fn test_join_pool_large_liquidity() {

    let owner = Addr::unchecked("owner");
    let alice_address = Addr::unchecked("alice");
    let mut app = mock_app(
        owner.clone(),
        vec![Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(10000000_000_000_000u128),
        }],
    );

    let weighted_pool_code_id = store_weighted_pool_code(&mut app);
    let vault_code_id = store_vault_code(&mut app);
    let token_code_id = store_token_code(&mut app);

    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(1000900_000_000_000),
        }],
    )
    .unwrap();

    // Create a CW20 token
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

    let pool_configs = vec![PoolTypeConfig {
        code_id: weighted_pool_code_id,
        pool_type: PoolType::Weighted {},
        default_fee_info: FeeInfo {
            total_fee_bps: 30u16,
            protocol_fee_percent: 20u16,
        },
        allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
        paused: PauseInfo::default(),
    }];

    let vault_init_msg = VaultInstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: Some(token_code_id),
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        pool_creation_fee: PoolCreationFee::default(),
        auto_stake_impl: dexter::vault::AutoStakeImpl::None,
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

    // Asset infos
    let asset_infos = vec![
        AssetInfo::NativeToken {
            denom: "xprt".to_string(),
        },
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
    ];

    // ----- Create Pool ----- //
    let asset_infos_with_weights = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            amount: Uint128::from(50u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(50u128),
        },
    ];

    let pool_msg = VaultExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Weighted {},
        asset_infos: asset_infos.to_vec(),
        native_asset_precisions: vec![("xprt".to_string(), 6u8)],
        init_params: Some(
            to_binary(&WeightedParams {
                weights: asset_infos_with_weights,
                exit_fee: None,
            })
            .unwrap(),
        ),
        fee_info: None
    };

    // create pool
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &pool_msg,
        &[Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(10000u128),
        }],
    ).unwrap();

    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance0.clone(),
        Uint128::new(1000900_000_000_000),
        alice_address.to_string(),
    );

    // Join pool with small liquditity
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
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

    // Set allowance to spend token
    app.execute_contract(
        alice_address.clone(),
        token_instance0.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(1000_000000u128),
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
            amount: Uint128::new(1000_000000u128),
        }],
    ).unwrap();

    let assets_msg_large = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "xprt".to_string(),
            },
            amount: Uint128::from(10_000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(10_000_000000u128),
        },
    ];

    let msg = VaultExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: None,
        min_lp_to_receive: None,
        auto_stake: None,
        assets: Some(assets_msg_large.clone()),
    };

    app.execute_contract(
        alice_address.clone(),
        token_instance0.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(10_000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();

    let response = app.execute_contract(
        alice_address.clone(),
        vault_instance.clone(),
        &msg,
        &[Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(10_000_000000u128),
        }],
    ).unwrap();

    let event = response.events.iter().find(|event| event.ty == "wasm-dexter-vault::join_pool").unwrap();
    let new_shares = event.attributes.iter().find(|attr| attr.key == "lp_tokens_minted").unwrap().value.parse::<Uint128>().unwrap();

    // Shares should exactly be 10 times the default minted in first run since we exactly supplied 10x the liquidity
    assert_eq!(new_shares, Uint128::from(1000_000_000_000_000_000_000u128));
}