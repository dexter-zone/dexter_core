use cosmwasm_std::{Addr, Coin, Decimal, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};
use std::vec;

use dexter::asset::{Asset, AssetInfo};
use dexter::keeper::{BalancesResponse, ConfigResponse, ExecuteMsg, QueryMsg};
use dexter::vault::{FeeInfo, PoolConfig, PoolType};

type TerraApp = App;
fn mock_app(owner: Addr, coins: Vec<Coin>) -> App {
    TerraApp::new(|router, _, storage| {
        // initialization moved to App construction
        router.bank.init_balance(storage, &owner, coins).unwrap()
    })
}

fn instantiate_contracts(router: &mut TerraApp, owner: Addr) -> (Addr, Addr) {
    let vault_contract = Box::new(
        ContractWrapper::new_with_empty(
            dexter_vault::contract::execute,
            dexter_vault::contract::instantiate,
            dexter_vault::contract::query,
        )
        .with_reply_empty(dexter_vault::contract::reply),
    );
    let vault_code_id = router.store_code(vault_contract);

    let xyk_contract = Box::new(
        ContractWrapper::new_with_empty(
            xyk_pool::contract::execute,
            xyk_pool::contract::instantiate,
            xyk_pool::contract::query,
        )
        .with_reply_empty(xyk_pool::contract::reply),
    );
    let xyk_code_id = router.store_code(xyk_contract);

    // Instantiate Vault Contract
    let msg = dexter::vault::InstantiateMsg {
        pool_configs: vec![PoolConfig {
            code_id: xyk_code_id,
            pool_type: PoolType::Xyk {},
            fee_info: FeeInfo {
                total_fee_bps: Decimal::zero(),
                protocol_fee_percent: 50u16,
                dev_fee_percent: 15u16,
                developer_addr: None,
            },
            is_disabled: false,
            is_generator_disabled: false,
        }],
        lp_token_code_id: 1u64,
        fee_collector: None,
        owner: owner.to_string(),
        generator_address: None,
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
        vault_contract: vault_instance.to_string(),
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

    let (vault_instance, keeper_instance) = instantiate_contracts(&mut router, owner.clone());

    // #########---- Check if Keeper contract is initialzied properly ----#########

    let msg = QueryMsg::Config {};
    let res: ConfigResponse = router
        .wrap()
        .query_wasm_smart(&keeper_instance, &msg)
        .unwrap();

    assert_eq!(res.dex_token_contract, None);
    assert_eq!(res.vault_contract, vault_instance);
    assert_eq!(res.staking_contract, None);

    // #########---- Error :: Permission check  ----#########

    let new_staking = Addr::unchecked("new_staking");
    let dex_token = Addr::unchecked("dex_token");

    let msg = ExecuteMsg::UpdateConfig {
        dex_token_contract: Some(dex_token.to_string()),
        staking_contract: Some(new_staking.to_string()),
    };

    // Assert cannot update with improper owner
    let e = router
        .execute_contract(
            Addr::unchecked("not_owner"),
            keeper_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();

    assert_eq!(e.root_cause().to_string(), "Unauthorized");

    // #########---- Success :: Check if config updated successfully  ----#########

    // Assert cannot update with improper owner
    router
        .execute_contract(owner.clone(), keeper_instance.clone(), &msg, &[])
        .unwrap();

    let msg = QueryMsg::Config {};
    let res: ConfigResponse = router
        .wrap()
        .query_wasm_smart(&keeper_instance, &msg)
        .unwrap();

    assert_eq!(res.dex_token_contract, Some(dex_token));
    assert_eq!(res.vault_contract, vault_instance);
    assert_eq!(res.staking_contract, Some(new_staking));

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

    let msg = QueryMsg::Balances { assets: assets };
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
