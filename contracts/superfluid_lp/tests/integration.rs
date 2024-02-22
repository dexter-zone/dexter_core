use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{Addr, Coin, Timestamp, Uint128, to_json_binary};
use cw20::MinterResponse;
use cw_multi_test::{App, ContractWrapper, Executor};
use dexter::asset::{Asset, AssetInfo};
use dexter::vault::{FeeInfo, PauseInfo, PoolCreationFee, PoolTypeConfig, NativeAssetPrecisionInfo};

const EPOCH_START: u64 = 1_000_000;

#[macro_export]
macro_rules! uint128_with_precision {
    ($value:expr, $precision:expr) => {
        cosmwasm_std::Uint128::from($value)
            .checked_mul(cosmwasm_std::Uint128::from(10u64).pow($precision as u32))
            .unwrap()
    };
}

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

fn store_multi_staking_code(app: &mut App) -> u64 {
    let multi_staking_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_multi_staking::contract::execute,
        dexter_multi_staking::contract::instantiate,
        dexter_multi_staking::contract::query,
    ));
    app.store_code(multi_staking_contract)
}

fn store_vault_code(app: &mut App) -> u64 {
    let dexter_vault = Box::new(
        ContractWrapper::new_with_empty(
            dexter_vault::contract::execute,
            dexter_vault::contract::instantiate,
            dexter_vault::contract::query,
        )
        .with_reply_empty(dexter_vault::contract::reply),
    );

    app.store_code(dexter_vault)
}

fn store_weighted_pool_code(app: &mut App) -> u64 {
    let weighted_pool_contract = Box::new(ContractWrapper::new_with_empty(
        weighted_pool::contract::execute,
        weighted_pool::contract::instantiate,
        weighted_pool::contract::query,
    ));
    app.store_code(weighted_pool_contract)
}

fn store_token_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        lp_token::contract::execute,
        lp_token::contract::instantiate,
        lp_token::contract::query,
    ));
    app.store_code(token_contract)
}

fn store_superfluid_lp_code(app: &mut App) -> u64 {
    let superfluid_lp_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_superfluid_lp::contract::execute,
        dexter_superfluid_lp::contract::instantiate,
        dexter_superfluid_lp::contract::query,
    ));
    app.store_code(superfluid_lp_contract)
}

pub fn create_cw20_token(app: &mut App, code_id: u64, sender: Addr, token_name: String) -> Addr {
    let lp_token_instantiate_msg = dexter::lp_token::InstantiateMsg {
        name: token_name,
        symbol: "abcde".to_string(),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: sender.to_string(),
            cap: None,
        }),
        marketing: None,
    };

    let lp_token_instance = app
        .instantiate_contract(
            code_id,
            sender.clone(),
            &lp_token_instantiate_msg,
            &[],
            "lp_token",
            Some(sender.to_string()),
        )
        .unwrap();

    return lp_token_instance;
}

fn instantiate_contract(app: &mut App, owner: &Addr) -> (Addr, Addr, Addr) {
    let token_code_id = store_token_code(app);
    let multistaking_code = store_multi_staking_code(app);
    let superfluid_lp_code = store_superfluid_lp_code(app);
    let vault_code = store_vault_code(app);
    let weighted_pool_code = store_weighted_pool_code(app);

    let keeper = String::from("keeper");

    let keeper_addr = Addr::unchecked(keeper.clone());

    // instantiate multistaking contract
    let msg = dexter::multi_staking::InstantiateMsg {
        owner: owner.clone(),
        unlock_period: 1000,
        minimum_reward_schedule_proposal_start_delay: 3 * 24 * 60 * 60,
        keeper_addr: keeper_addr.clone(),
        instant_unbond_fee_bp: 500,
        instant_unbond_min_fee_bp: 200,
        fee_tier_interval: 1000,
    };

    let multi_staking_instance = app
        .instantiate_contract(
            multistaking_code,
            owner.clone(),
            &msg,
            &[],
            "multi_staking",
            None,
        )
        .unwrap();

    let vault_instantiate_msg = dexter::vault::InstantiateMsg {
        owner: owner.to_string(),
        pool_configs: vec![PoolTypeConfig {
            code_id: weighted_pool_code,
            pool_type: dexter::vault::PoolType::Weighted {},
            default_fee_info: FeeInfo {
                total_fee_bps: 30,
                protocol_fee_percent: 30,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            paused: PauseInfo {
                deposit: false,
                imbalanced_withdraw: true,
                swap: false,
            },
        }],
        lp_token_code_id: Some(token_code_id),
        fee_collector: None,
        pool_creation_fee: PoolCreationFee::Disabled,
        auto_stake_impl: dexter::vault::AutoStakeImpl::Multistaking {
            contract_addr: multi_staking_instance.clone(),
        },
    };

    let vault_instance = app
        .instantiate_contract(
            vault_code,
            owner.clone(),
            &vault_instantiate_msg,
            &[],
            "vault",
            None,
        )
        .unwrap();

    let superfluid_lp_instantiate_msg = dexter::superfluid_lp::InstantiateMsg {
        vault_addr: vault_instance.clone(),
        owner: owner.clone(),
        allowed_lockable_tokens: vec![
            AssetInfo::NativeToken {
                denom: "stk/uxprt".to_string(),
            },
        ],
    };

    let superfluid_lp_instance = app
        .instantiate_contract(
            superfluid_lp_code,
            owner.clone(),
            &superfluid_lp_instantiate_msg,
            &[],
            "superfluid_lp",
            None,
        )
        .unwrap();

    return (
        multi_staking_instance,
        superfluid_lp_instance,
        vault_instance
    );
}

#[test]
fn test_superfluid_lp_locking() {
    let coins = vec![
        Coin {
            denom: "stk/uxprt".to_string(),
            amount: Uint128::from(100000000000u128),
        },
        Coin {
            denom: "uxprt".to_string(),
            amount: Uint128::from(100000000000u128),
        },
        Coin {
            denom: "ustake".to_string(),
            amount: Uint128::from(100000000000u128),
        }
    ];

    let owner = Addr::unchecked("owner");
    let mut app = mock_app(owner.clone(), coins);
    let (multi_staking_instance, superfluid_lp_instance, vault_instance) =
        instantiate_contract(&mut app, &Addr::unchecked("owner"));

    // create a CW20 token
    let cw20_code_id = store_token_code(&mut app);

    // let's assume that there is a CW20 token which is also an LST for XPRT, and let's have a 3 Pool with XPRT and LST as assets
    let cw20_lst_addr = create_cw20_token(&mut app, cw20_code_id, owner.clone(), "Staked XPRT".to_string());
    

    let create_pool_msg = dexter::vault::ExecuteMsg::CreatePoolInstance {
        pool_type: dexter::vault::PoolType::Weighted {},
        asset_infos: vec![
            AssetInfo::NativeToken {
                denom: "uxprt".to_string(),
            },
            AssetInfo::NativeToken { 
                denom: "stk/uxprt".to_string(),
            },
            AssetInfo::Token {
                contract_addr: cw20_lst_addr.clone(),
            },
        ],
        native_asset_precisions: vec![
            NativeAssetPrecisionInfo { 
                denom: "uxprt".to_string(),
                precision: 6,
            },
            NativeAssetPrecisionInfo { 
                denom: "stk/uxprt".to_string(),
                precision: 6,
            }
        ],
        fee_info: Some(FeeInfo {
            total_fee_bps: 30,
            protocol_fee_percent: 30,
        }),
        init_params: to_json_binary(
            &weighted_pool::state::WeightedParams {
                weights: vec![
                    Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uxprt".to_string(),
                        },
                        amount: Uint128::from(1u128),
                    },
                    Asset {
                        info: AssetInfo::NativeToken {
                            denom: "stk/uxprt".to_string(),
                        },
                        amount: Uint128::from(1u128),
                    },
                    Asset {
                        info: AssetInfo::Token {
                            contract_addr: cw20_lst_addr.clone(),
                        },
                        amount: Uint128::from(1u128),
                    },
                ],
                exit_fee: None,
            }
        ).ok()
    };

    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &create_pool_msg,
        &[],
    ).unwrap();

    // get LP token address for the pool
    let query_msg = dexter::vault::QueryMsg::GetPoolById {
        pool_id: Uint128::from(1u128),
    };

    let res: dexter::vault::PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &query_msg)
        .unwrap();


    let lp_token_addr = res.lp_token_addr;

    // allow LP token in the multistaking contract
    let msg = dexter::multi_staking::ExecuteMsg::AllowLpToken {
        lp_token: lp_token_addr.clone(),
    };

    app.execute_contract(
        owner.clone(),
        multi_staking_instance.clone(),
        &msg,
        &[],
    ).unwrap();
    

    let user = String::from("user");
    let user_addr = Addr::unchecked(user.clone());

     // mint Cw20 tokens for the user
     let mint_msg = cw20::Cw20ExecuteMsg::Mint {
        recipient: user_addr.to_string(),
        amount: Uint128::from(10000000u128),
    };

    app.execute_contract(
        owner.clone(),
        cw20_lst_addr.clone(),
        &mint_msg,
        &[],
    ).unwrap();

    // send XPRT and stkXPRT to the user
    app.send_tokens(
        owner,
        user_addr.clone(),
        &[
            Coin {
                denom: "uxprt".to_string(),
                amount: Uint128::from(20000000u128),
            },
            Coin {
                denom: "stk/uxprt".to_string(),
                amount: Uint128::from(10000000u128),
            },
            Coin {
                denom: "ustake".to_string(),
                amount: Uint128::from(10000000u128),
            }
        ],
    ).unwrap();

    let invalid_msg = dexter::superfluid_lp::ExecuteMsg::LockLstAsset {
        asset: Asset {
            info: AssetInfo::NativeToken {
                denom: "uxprt".to_string(),
            },
            amount: Uint128::from(10000000u128),
        },
    };

    // locking of uxprt should fail since it's not allowed
    let res = app
        .execute_contract(
            user_addr.clone(),
            superfluid_lp_instance.clone(),
            &invalid_msg,
            &[],
        );

    assert!(res.is_err());
    assert_eq!(res.unwrap_err().root_cause().to_string(), "Only whitelisted assets can be locked");

    // locking for CW20 staked XPRT should fail since it's not allowed

    let invalid_msg = dexter::superfluid_lp::ExecuteMsg::LockLstAsset {
        asset: Asset {
            info: AssetInfo::Token {
                contract_addr: cw20_lst_addr.clone(),
            },
            amount: Uint128::from(10000000u128),
        },
    };

    let res = app
        .execute_contract(
            user_addr.clone(),
            superfluid_lp_instance.clone(),
            &invalid_msg,
            &[],
        );

    assert!(res.is_err());
    assert_eq!(res.unwrap_err().root_cause().to_string(), "Only whitelisted assets can be locked");

    let msg = dexter::superfluid_lp::ExecuteMsg::LockLstAsset {
        asset: Asset {
            info: AssetInfo::NativeToken {
                denom: "stk/uxprt".to_string(),
            },
            amount: Uint128::from(10000000u128),
        },
    };

    app
        .execute_contract(
            user_addr.clone(),
            superfluid_lp_instance.clone(),
            &msg,
            &[Coin {
                denom: "stk/uxprt".to_string(),
                amount: Uint128::from(10000000u128),
            }],
        )
        .unwrap();

    // query the locked tokens
    let query_msg = dexter::superfluid_lp::QueryMsg::TotalAmountLocked {
        user: Addr::unchecked(user.clone()),
        asset_info: AssetInfo::NativeToken {
            denom: "stk/uxprt".to_string(),
        },
    };

    let res: Uint128 = app
        .wrap()
        .query_wasm_smart(superfluid_lp_instance.clone(), &query_msg)
        .unwrap();

    assert_eq!(res, Uint128::from(10000000u128));

    // join pool using the locked tokens
    let join_pool_msg = dexter::superfluid_lp::ExecuteMsg::JoinPoolAndBondUsingLockedLst { 
        pool_id: Uint128::from(1u128),
        total_assets: vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "stk/uxprt".to_string(),
                },
                amount: Uint128::from(10000000u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uxprt".to_string(),
                },
                amount: Uint128::from(10000000u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: cw20_lst_addr.clone(),
                },
                amount: Uint128::from(10000000u128),
            },
        ],
        min_lp_to_receive: None,
    };

    let response = app
        .execute_contract(
            user_addr.clone(),
            superfluid_lp_instance.clone(),
            &join_pool_msg,
            // add funds to the message
            &[Coin {
                denom: "uxprt".to_string(),
                amount: Uint128::from(10000000u128),
            }],
        );

    // confirm error
    assert!(response.is_err());
    // error should be about the spending of CW20 tokens
    assert_eq!(response.unwrap_err().root_cause().to_string(), "No allowance for this account");

    // set allowance and try again
    let allowance_msg = cw20::Cw20ExecuteMsg::IncreaseAllowance {
        spender: superfluid_lp_instance.to_string(),
        amount: Uint128::from(10000000u128),
        expires: None,
    };

    app.execute_contract(
        user_addr.clone(),
        cw20_lst_addr.clone(),
        &allowance_msg,
        &[],
    ).unwrap();

    // query and store uxprt and ustable balances for the user
    let uxprt_balance = app
        .wrap()
        .query_balance(user_addr.clone(), "uxprt".to_string()).unwrap();

    let ustake_balance = app
        .wrap()
        .query_balance(user_addr.clone(), "ustake".to_string()).unwrap();

    // try again
    let response = app
        .execute_contract(
            user_addr.clone(),
            superfluid_lp_instance.clone(),
            &join_pool_msg,
            // add funds to the message
            &[
                // send extra XPRT and make sure it returns the extra XPRT back
                Coin {
                    denom: "uxprt".to_string(),
                    amount: Uint128::from(12000000u128),
                },
                // add an extra denom to the message, and make sure it returns the extra denom back
                Coin {
                    denom: "ustake".to_string(),
                    amount: Uint128::from(10000000u128),
                }
            ],
        );
    
    // confirm success
    assert!(response.is_ok());

    // validate no extra uxprt was spent
    let uxprt_balance_after = app
        .wrap()
        .query_balance(user_addr.clone(), "uxprt".to_string()).unwrap();

    assert_eq!(uxprt_balance_after.amount, uxprt_balance.amount - Uint128::from(10000000u128));

    // validate no extra ustake was spent
    let ustake_balance_after = app
        .wrap()
        .query_balance(user_addr.clone(), "ustake".to_string()).unwrap();

    assert_eq!(ustake_balance_after.amount, ustake_balance.amount);

    // confirm LP tokens are minted for the user and bonded
    let query_msg = dexter::multi_staking::QueryMsg::BondedLpTokens {
        lp_token: lp_token_addr.clone(),
        user: user_addr.clone(),
    };

    let res: Uint128 = app
        .wrap()
        .query_wasm_smart(multi_staking_instance.clone(), &query_msg)
        .unwrap();

    // validate that it must be equal to 100 (Decimal 18) since that's the default amount of LP tokens minted for the first time user
    assert_eq!(res, uint128_with_precision!(100u128, 18));

}
