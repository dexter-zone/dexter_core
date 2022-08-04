use std::collections::HashMap;
use std::str::FromStr;

use cosmwasm_std::testing::{mock_env, MockApi, MockStorage};
use cosmwasm_std::{to_binary, Addr, Coin, Decimal, Uint128};

use cw_multi_test::{App, AppBuilder, BankKeeper, ContractWrapper, Executor};
use dexter::asset::{Asset, AssetInfo};
use dexter::pool::QueryMsg as PoolQuery;
use dexter::vault::{
    ConfigResponse, ExecuteMsg, FeeInfo, InstantiateMsg as VaultInitMsg, PoolConfig,
    PoolInfoResponse, PoolType, QueryMsg, SingleSwapRequest,
};

pub const TEST_CREATOR: &str = "owner";
pub const RANDOM_USER: &str = "random_user";

pub fn mock_app() -> App {
    let storage = MockStorage::new();

    let env = mock_env();
    let api = MockApi::default();
    let bank = BankKeeper::new();

    let sender = Addr::unchecked(TEST_CREATOR);
    let random_user = Addr::unchecked(RANDOM_USER);

    let funds = vec![
        Coin::new(1_000_000_000, "uusd"),
        Coin::new(1_000_000_000, "uluna"),
        Coin::new(1_000_000_000, "ubtc"),
    ];

    AppBuilder::new()
        .with_api(api)
        .with_block(env.block)
        .with_bank(bank)
        .with_storage(storage)
        .build(|router, _, storage| {
            router
                .bank
                .init_balance(storage, &sender, funds.clone())
                .unwrap();

            router
                .bank
                .init_balance(storage, &random_user, funds)
                .unwrap();
        })
}

fn store_vault_code(app: &mut App) -> u64 {
    let vault_contract = Box::new(
        ContractWrapper::new_with_empty(
            dexter_vault::contract::execute,
            dexter_vault::contract::instantiate,
            dexter_vault::contract::query,
        )
        .with_reply_empty(dexter_vault::contract::reply),
    );

    app.store_code(vault_contract)
}

fn store_pair_code(app: &mut App) -> u64 {
    let pair_contract = Box::new(
        ContractWrapper::new_with_empty(
            crate::contract::execute,
            crate::contract::instantiate,
            crate::contract::query,
        )
        .with_reply(crate::contract::reply),
    );
    app.store_code(pair_contract)
}

fn store_token_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_token::contract::execute,
        dexter_token::contract::instantiate,
        dexter_token::contract::query,
    ));

    app.store_code(token_contract)
}

#[test]
fn proper_initialization() {
    let mut app = mock_app();

    let owner = Addr::unchecked("owner");
    let developer = Addr::unchecked("developer");

    let vault_code_id = store_vault_code(&mut app);
    let token_code = store_token_code(&mut app);
    let weighted_code = store_pair_code(&mut app);

    let pool_configs = vec![PoolConfig {
        code_id: weighted_code,
        pool_type: PoolType::Weighted {},
        is_disabled: false,
        is_generator_disabled: false,
        fee_info: FeeInfo {
            total_fee_bps: Decimal::from_str("0.001").unwrap(),
            dev_fee_percent: 20,
            developer_addr: Some(developer.clone()),
            protocol_fee_percent: 10,
        },
    }];

    let msg = VaultInitMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: token_code,
        fee_collector: None,
        owner: owner.to_string(),
        generator_address: Some(String::from("generator")),
    };

    let vault_instance = app
        .instantiate_contract(vault_code_id, owner.clone(), &msg, &[], "vault", None)
        .unwrap();

    let msg = ExecuteMsg::CreatePool {
        pool_type: PoolType::Weighted {},
        asset_infos: vec![
            AssetInfo::NativeToken {
                denom: "uusd".into(),
            },
            AssetInfo::NativeToken {
                denom: "uluna".into(),
            },
        ],
        lp_token_name: None,
        lp_token_symbol: None,
        init_params: Some(
            to_binary(&vec![
                (
                    AssetInfo::NativeToken {
                        denom: "uusd".into(),
                    },
                    20u128,
                ),
                (
                    AssetInfo::NativeToken {
                        denom: "uluna".into(),
                    },
                    20u128,
                ),
            ])
            .unwrap(),
        ),
    };

    app.execute_contract(owner.clone(), vault_instance.clone(), &msg, &[])
        .unwrap();

    let msg = QueryMsg::Config {};
    let config_res: ConfigResponse = app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();

    assert_eq!(token_code, config_res.lp_token_code_id);
    assert_eq!(pool_configs, config_res.pool_configs);
    assert_eq!(owner, config_res.owner);

    let msg = QueryMsg::GetPoolById {
        pool_id: Uint128::from(1u128),
    };
    let config_res: PoolInfoResponse = app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();
    let pool_instance = config_res.pool_addr.unwrap();

    let _config_res: PoolInfoResponse = app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();

    let msg = ExecuteMsg::JoinPool {
        pool_id: 1u128.into(),
        recipient: None,
        assets: Some(
            [
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uusd".into(),
                    },
                    amount: 100_000u128.into(),
                },
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uluna".into(),
                    },
                    amount: 100_000u128.into(),
                },
            ]
            .to_vec(),
        ),
        lp_to_mint: None,
        auto_stake: None,
    };

    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &msg,
        &[
            Coin::new(100_000u128, "uusd"),
            Coin::new(100_000u128, "uluna"),
        ],
    )
    .unwrap();

    let msg = PoolQuery::Config {};
    let config_res: PoolInfoResponse = app.wrap().query_wasm_smart(&pool_instance, &msg).unwrap();
    assert_eq!(
        config_res.assets,
        vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uluna".into()
                },
                amount: 100_000u128.into()
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".into()
                },
                amount: 100_000u128.into()
            }
        ]
    );
    let liquidity_token = config_res.lp_token_addr.unwrap();
    let msg = cw20::Cw20QueryMsg::Balance {
        address: owner.to_string(),
    };
    let config_res: cw20::BalanceResponse =
        app.wrap().query_wasm_smart(&liquidity_token, &msg).unwrap();
    // init_lp = sqrt(a1^w1 * a2^w2)
    // = sqrt(100_000) because current setting is symmetrical (50/50) pool
    assert_eq!(config_res.balance.u128(), 316_227u128);

    // swap
    let swap_req = SingleSwapRequest {
        pool_id: 1u128.into(),
        amount: 100000u128.into(),
        asset_in: AssetInfo::NativeToken {
            denom: "uluna".into(),
        },
        asset_out: AssetInfo::NativeToken {
            denom: "uusd".into(),
        },
        swap_type: dexter::vault::SwapType::GiveIn {},
    };
    let msg = ExecuteMsg::Swap {
        swap_request: swap_req,
        limit: None,
        deadline: None,
        recipient: None,
    };

    //no funds
    app.execute_contract(owner.clone(), vault_instance.clone(), &msg, &[])
        .unwrap_err();
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &msg,
        &[Coin::new(100000u128, "uluna")],
    )
    .unwrap();

    let balance = app.wrap().query_balance(owner, "uusd").unwrap();
    assert_eq!(balance.amount.u128(), 999949950);
    let balance = app.wrap().query_balance(developer, "uusd").unwrap();
    assert_eq!(balance.amount.u128(), 10);
}
