use cosmwasm_std::{Addr, Coin, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};
use std::vec;

use dexter::asset::{Asset, AssetInfo};
use dexter::keeper::{BalancesResponse, ConfigResponse, ExecuteMsg, QueryMsg};
use dexter::vault::{FeeInfo, PauseInfo, PoolCreationFee, PoolType, PoolTypeConfig};

fn mock_app(owner: Addr, coins: Vec<Coin>) -> App {
    App::new(|router, _, storage| {
        // initialization moved to App construction
        router.bank.init_balance(storage, &owner, coins).unwrap()
    })
}

fn instantiate_contracts(router: &mut App, owner: Addr, keeper_admin: Addr) -> (Addr, Addr) {
    let vault_contract = Box::new(
        ContractWrapper::new_with_empty(
            dexter_vault::contract::execute,
            dexter_vault::contract::instantiate,
            dexter_vault::contract::query,
        )
        .with_reply_empty(dexter_vault::contract::reply),
    );
    let vault_code_id = router.store_code(vault_contract);

    let stable5_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_stable_pool::contract::execute,
        dexter_stable_pool::contract::instantiate,
        dexter_stable_pool::contract::query,
    ));
    let stable5_code_id = router.store_code(stable5_contract);

    // Instantiate Vault Contract
    let msg = dexter::vault::InstantiateMsg {
        pool_configs: vec![PoolTypeConfig {
            code_id: stable5_code_id,
            pool_type: PoolType::StableSwap {},
            default_fee_info: FeeInfo {
                total_fee_bps: 0u16,
                protocol_fee_percent: 50u16,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            paused: PauseInfo::default(),
        }],
        lp_token_code_id: Some(1u64),
        fee_collector: None,
        owner: owner.to_string(),
        pool_creation_fee: PoolCreationFee::default(),
        auto_stake_impl: dexter::vault::AutoStakeImpl::None,
    };
    let vault_instance = router
        .instantiate_contract(
            vault_code_id,
            owner.clone(),
            &msg,
            &[],
            String::from("VAULT"),
            None,
        )
        .unwrap();

    // Instantiate Keeper Contract
    let keeper_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_keeper::contract::execute,
        dexter_keeper::contract::instantiate,
        dexter_keeper::contract::query,
    ));
    let keeper_code_id = router.store_code(keeper_contract);
    let k_msg = dexter::keeper::InstantiateMsg {
        owner: keeper_admin,
        vault_address: vault_instance.clone(),
    };
    let keeper_instance = router
        .instantiate_contract(
            keeper_code_id,
            Addr::unchecked("instantiator"),
            &k_msg,
            &[],
            String::from("KEEPER"),
            None,
        )
        .unwrap();

    (vault_instance, keeper_instance)
}

#[test]
fn update_config() {
    let owner = Addr::unchecked("owner");
    let keeper_admin = Addr::unchecked("keeper_admin");

    let mut router = mock_app(
        owner.clone(),
        vec![
            Coin {
                denom: "ibc/axlusdc".to_string(),
                amount: Uint128::new(100_000_000_000u128),
            },
            Coin {
                denom: "xprt".to_string(),
                amount: Uint128::new(100_000_000_000u128),
            },
        ],
    );

    let (_, keeper_instance) =
        instantiate_contracts(&mut router, owner.clone(), keeper_admin.clone());

    // #########---- Check if Keeper contract is initialzied properly ----#########

    let msg = QueryMsg::Config {};
    let res: ConfigResponse = router
        .wrap()
        .query_wasm_smart(&keeper_instance, &msg)
        .unwrap();

    assert_eq!(res.owner, keeper_admin);

    // #########---- Check if Balances Query is working as expected  ----#########

    router
        .send_tokens(
            owner.clone(),
            keeper_instance.clone(),
            &[Coin {
                denom: "ibc/axlusdc".to_string(),
                amount: Uint128::new(100_000_000u128),
            }],
        )
        .unwrap();
    router
        .send_tokens(
            owner.clone(),
            keeper_instance.clone(),
            &[Coin {
                denom: "xprt".to_string(),
                amount: Uint128::new(1000_000u128),
            }],
        )
        .unwrap();

    let mut assets = vec![];
    assets.push(AssetInfo::NativeToken {
        denom: "ibc/axlusdc".to_string(),
    });
    assets.push(AssetInfo::NativeToken {
        denom: "xprt".to_string(),
    });

    let msg = QueryMsg::Balances { assets };
    let res: BalancesResponse = router
        .wrap()
        .query_wasm_smart(&keeper_instance, &msg)
        .unwrap();

    let mut assets_res = vec![];
    assets_res.push(Asset {
        info: AssetInfo::NativeToken {
            denom: "ibc/axlusdc".to_string(),
        },
        amount: Uint128::new(100_000_000u128),
    });
    assets_res.push(Asset {
        info: AssetInfo::NativeToken {
            denom: "xprt".to_string(),
        },
        amount: Uint128::new(1000_000u128),
    });

    assert_eq!(res.balances, assets_res);
}

#[test]
fn withdraw_funds() {
    let owner = Addr::unchecked("owner");
    let keeper_admin = Addr::unchecked("keeper_admin");

    let mut router = mock_app(
        owner.clone(),
        vec![
            Coin {
                denom: "ibc/axlusdc".to_string(),
                amount: Uint128::new(100_000_000_000u128),
            },
            Coin {
                denom: "xprt".to_string(),
                amount: Uint128::new(100_000_000_000u128),
            },
        ],
    );

    let (_vault_instance, keeper_instance) =
        instantiate_contracts(&mut router, owner.clone(), keeper_admin.clone());

    router
        .send_tokens(
            owner.clone(),
            keeper_instance.clone(),
            &[Coin {
                denom: "ibc/axlusdc".to_string(),
                amount: Uint128::new(100_000_000u128),
            }],
        )
        .unwrap();

    // Check balance query
    let msg = QueryMsg::Balances {
        assets: vec![AssetInfo::NativeToken {
            denom: "ibc/axlusdc".to_string(),
        }],
    };
    let res: BalancesResponse = router
        .wrap()
        .query_wasm_smart(&keeper_instance, &msg)
        .unwrap();

    let mut assets_res = vec![];
    assets_res.push(Asset {
        info: AssetInfo::NativeToken {
            denom: "ibc/axlusdc".to_string(),
        },
        amount: Uint128::new(100_000_000u128),
    });

    assert_eq!(res.balances, assets_res);

    // try withdrawing from a different address
    let msg = ExecuteMsg::Withdraw {
        asset: AssetInfo::NativeToken {
            denom: "ibc/axlusdc".to_string(),
        },
        amount: Uint128::new(50_000_000u128),
        recipient: None,
    };

    let res = router.execute_contract(owner.clone(), keeper_instance.clone(), &msg, &[]);

    assert!(res.is_err());
    assert_eq!(res.unwrap_err().root_cause().to_string(), "Unauthorized");

    // try withdrawing more than available
    let msg = ExecuteMsg::Withdraw {
        asset: AssetInfo::NativeToken {
            denom: "ibc/axlusdc".to_string(),
        },
        amount: Uint128::new(150_000_000u128),
        recipient: None,
    };

    let res = router.execute_contract(keeper_admin.clone(), keeper_instance.clone(), &msg, &[]);

    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().root_cause().to_string(),
        "Insufficient funds to execute this transaction"
    );

    // try withdrawing correct amount
    let msg = ExecuteMsg::Withdraw {
        asset: AssetInfo::NativeToken {
            denom: "ibc/axlusdc".to_string(),
        },
        amount: Uint128::new(50_000_000u128),
        recipient: None,
    };

    router
        .execute_contract(keeper_admin.clone(), keeper_instance.clone(), &msg, &[])
        .unwrap();

    // Check balance query
    let msg = QueryMsg::Balances {
        assets: vec![AssetInfo::NativeToken {
            denom: "ibc/axlusdc".to_string(),
        }],
    };
    let res: BalancesResponse = router
        .wrap()
        .query_wasm_smart(&keeper_instance, &msg)
        .unwrap();

    let mut assets_res = vec![];
    assets_res.push(Asset {
        info: AssetInfo::NativeToken {
            denom: "ibc/axlusdc".to_string(),
        },
        amount: Uint128::new(50_000_000u128),
    });

    assert_eq!(res.balances, assets_res);

    // validate user balances
    let res = router.wrap().query_all_balances(&keeper_admin).unwrap();
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].amount, Uint128::new(50_000_000u128));
    assert_eq!(res[0].denom, "ibc/axlusdc");

    // try withdrawing correct amount to a different address
    let msg = ExecuteMsg::Withdraw {
        asset: AssetInfo::NativeToken {
            denom: "ibc/axlusdc".to_string(),
        },
        amount: Uint128::new(50_000_000u128),
        recipient: Some(Addr::unchecked("recipient")),
    };

    router
        .execute_contract(keeper_admin.clone(), keeper_instance.clone(), &msg, &[])
        .unwrap();

    // Validate if the funds were sent to the recipient
    let res = router
        .wrap()
        .query_all_balances(Addr::unchecked("recipient"))
        .unwrap();
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].amount, Uint128::new(50_000_000u128));
    assert_eq!(res[0].denom, "ibc/axlusdc");
}
