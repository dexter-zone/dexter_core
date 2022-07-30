use cosmwasm_std::{attr, Addr};

use dexter::asset:: AssetInfo;
use dexter::vault::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, PoolConfig, PoolType, QueryMsg, PoolInfo,FeeInfo
};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use cw20::MinterResponse;
use cw_multi_test::{App, BasicApp, ContractWrapper, Executor};
use cosmwasm_std::{Coin,Uint128};

type TerraApp = App;
fn mock_app() -> TerraApp {
    BasicApp::default()
}

fn store_factory_code(app: &mut TerraApp) -> u64 {
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

fn store_pool_code(app: &mut TerraApp) -> u64 {
    let pool_contract = Box::new(
        ContractWrapper::new_with_empty(
            dexter_vault::contract::execute,
            dexter_vault::contract::instantiate,
            dexter_vault::contract::query,
        )
        .with_reply_empty(dexter_pool::contract::reply),
    );

    app.store_code(pool_contract)
}

fn store_token_code(app: &mut TerraApp) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
          dexter_vault::contract::execute,
            dexter_vault::contract::instantiate,
            dexter_vault::contract::query,
    ));

    app.store_code(token_contract)
}
#[test]
fn proper_initialization() {
    let mut app = mock_app();

    let owner = Addr::unchecked("owner");

    let factory_code_id = store_factory_code(&mut app);

    let pool_configs = vec![PoolConfig {
        code_id: 321,
        pool_type: PoolType::Xyk {},
        total_fee_bps: 100,
        is_disabled: false,
        is_generator_disabled: false,
    }];

    let msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: 123,
        fee_collector: None,
        owner: owner.to_string(),
        generator_address: Some(String::from("generator")),
        //whitelist_code_id: 234u64,
    };

    let vault_instance = app
        .instantiate_contract(
            factory_code_id,
            Addr::unchecked(owner.clone()),
            &msg,
            &[],
            "factory",
            None,
        )
        .unwrap();

    let msg = QueryMsg::Config {};
    let config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&vault_instance, &msg)
        .unwrap();

    assert_eq!(123, config_res.lp_token_code_id);
    assert_eq!(pool_configs, config_res.pool_configs);
    assert_eq!(owner, config_res.owner);
}

#[test]
fn update_config() {
    let mut app = mock_app();

    let owner = String::from("owner");

    let lp_token_code_id = store_token_code(&mut app);
    let vault_instance =
        instantiate_contract(&mut app, &Addr::unchecked(owner.clone()), lp_token_code_id);

    // Update config
    let fee_address = Some(String::from("fee"));
    let generator_address = Some(String::from("generator"));

    let msg = ExecuteMsg::UpdateConfig {
        lp_token_code_id: Some(200u64),
        fee_collector: fee_address.clone(),
        generator_address: generator_address.clone(),
        //whitelist_code_id: None,
    };

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    let msg = QueryMsg::Config {};
    let config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&vault_instance, &msg)
        .unwrap();

    assert_eq!(200u64, config_res.lp_token_code_id);
    assert_eq!(
        fee_address.unwrap(),
        config_res.fee_address.unwrap().to_string()
    );
    assert_eq!(
        generator_address.unwrap(),
        config_res.generator_address.unwrap().to_string()
    );

    // Unauthorized err
    let msg = ExecuteMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: None,
        generator_address: None,
        //whitelist_code_id: None,
    };

    let res = app
        .execute_contract(
            Addr::unchecked("invalid_owner"),
            vault_instance,
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "Unauthorized");
}

fn instantiate_contract(app: &mut TerraApp, owner: &Addr, lp_token_code_id: u64) -> Addr {
    let pool_code_id = store_pool_code(app);
    let vault_code_id = store_factory_code(app);

    let pool_configs = vec![PoolConfig {
        code_id: pool_code_id,
        pool_type: PoolType::Xyk {},
        total_fee_bps: 100,
        is_disabled: false,
        is_generator_disabled: false,
    }];

    let msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id,
        fee_collector: None,
        owner: owner.to_string(),
        generator_address: Some(String::from("generator")),
        //whitelist_code_id: 234u64,
    };

    app.instantiate_contract(
        vault_code_id,
        owner.to_owned(),
        &msg,
        &[],
        "factory",
        None,
    )
    .unwrap()
}

#[test]
fn create_pool() {
    let mut app = mock_app();

    let owner = String::from("owner");

    let lp_token_code_id = store_token_code(&mut app);

    let vault_instance =
        instantiate_contract(&mut app, &Addr::unchecked(owner.clone()), lp_token_code_id);

    let owner_addr = Addr::unchecked(owner.clone());

    let lp_token_name = "tokenX";

    let init_msg = TokenInstantiateMsg {
        name: lp_token_name.to_string(),
        symbol: lp_token_name.to_string(),
        decimals: 18,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: owner_addr.to_string(),
            cap: None,
        }),
        marketing: None,
    };

    let token_instance0 = app
        .instantiate_contract(
            lp_token_code_id,
            owner_addr.clone(),
            &init_msg,
            &[],
            lp_token_name,
            None,
        )
        .unwrap();

    let lp_token_name = "tokenY";

    let init_msg = TokenInstantiateMsg {
        name: lp_token_name.to_string(),
        symbol: lp_token_name.to_string(),
        decimals: 18,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: owner_addr.to_string(),
            cap: None,
        }),
        marketing: None,
    };

    let token_instance1 = app
        .instantiate_contract(
            lp_token_code_id,
            owner_addr.clone(),
            &init_msg,
            &[],
            lp_token_name,
            None,
        )
        .unwrap();

    let asset_infos = [
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
        AssetInfo::Token {
            contract_addr: token_instance1.clone(),
        },
    ];

    let msg = ExecuteMsg::CreatePool {
        pool_type: PoolType::Xyk {},
        asset_infos: asset_infos.clone(),
        init_params: None,
    };

    let res = app
        .execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

    assert_eq!(res.events[1].attributes[1], attr("action", "create_pool"));
    assert_eq!(
        res.events[1].attributes[2],
        attr("pool", "contract1-contract2")
    );

    let res: PoolInfo = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::Pool {
                asset_infos: asset_infos.clone(),
            },
        )
        .unwrap();

    // in multitest, contract names are named in the order in which contracts are created.
    assert_eq!("contract0", vault_instance.to_string());
    assert_eq!("contract3", res.pool_addr.to_string());
    assert_eq!("contract4", res.lp_token_addr.to_string());
}
fn test_provide_and_withdraw_liquidity() {
    let owner = Addr::unchecked("owner");
    let alice_address = Addr::unchecked("alice");
    let mut router = TerraApp::new(|router, _, storage| {
        // initialization moved to App construction
        router
            .bank
            .init_balance(
                storage,
                &alice_address,
                vec![
                    Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128::new(233u128),
                    },
                    Coin {
                        denom: "uluna".to_string(),
                        amount: Uint128::new(200u128),
                    },
                ],
            )
            .unwrap();
        router
            .bank
            .init_balance(
                storage,
                &owner,
                vec![
                    Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128::new(100_000000u128),
                    },
                    Coin {
                        denom: "uluna".to_string(),
                        amount: Uint128::new(100_000000u128),
                    },
                ],
            )
            .unwrap()
    });

    // Init pool
    let pool_instance = instantiate_pool(&mut router, &owner);

    let res: Result<PoolInfo, _> = router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: pool_instance.to_string(),
        msg: to_binary(&QueryMsg::Pool {}).unwrap(),
    }));
    let res = res.unwrap();

    assert_eq!(
        res.asset_infos,
        [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        ],
    );

    // When dealing with native tokens transfer should happen before contract call, which cw-multitest doesn't support
    router
        .send_tokens(
            owner.clone(),
            pool_instance.clone(),
            &[coin(100, "uusd"), coin(100, "uluna")],
        )
        .unwrap();

    // Provide liquidity
    let (msg, coins) = provide_liquidity_msg(Uint128::new(100), Uint128::new(100), None);
    let res = router
        .execute_contract(alice_address.clone(), pool_instance.clone(), &msg, &coins)
        .unwrap();

    assert_eq!(
        res.events[1].attributes[1],
        attr("action", "provide_liquidity")
    );
    assert_eq!(res.events[1].attributes[3], attr("receiver", "alice"),);
    assert_eq!(
        res.events[1].attributes[4],
        attr("assets", "100uusd, 100uluna")
    );
    assert_eq!(
        res.events[1].attributes[5],
        attr("share", 100u128.to_string())
    );
    assert_eq!(res.events[3].attributes[1], attr("action", "mint"));
    assert_eq!(res.events[3].attributes[2], attr("to", "alice"));
    assert_eq!(
        res.events[3].attributes[3],
        attr("amount", 100u128.to_string())
    );

    // Provide liquidity for receiver
    let (msg, coins) = provide_liquidity_msg(
        Uint128::new(100),
        Uint128::new(100),
        Some("bob".to_string()),
    );
    let res = router
        .execute_contract(alice_address.clone(), pool_instance.clone(), &msg, &coins)
        .unwrap();

    assert_eq!(
        res.events[1].attributes[1],
        attr("action", "provide_liquidity")
    );
    assert_eq!(res.events[1].attributes[3], attr("receiver", "bob"),);
    assert_eq!(
        res.events[1].attributes[4],
        attr("assets", "100uusd, 100uluna")
    );
    assert_eq!(
        res.events[1].attributes[5],
        attr("share", 50u128.to_string())
    );
    assert_eq!(res.events[3].attributes[1], attr("action", "mint"));
    assert_eq!(res.events[3].attributes[2], attr("to", "bob"));
    assert_eq!(res.events[3].attributes[3], attr("amount", 50.to_string()));
}

fn provide_liquidity_msg(
    uusd_amount: Uint128,
    uluna_amount: Uint128,
    receiver: Option<String>,
) -> (ExecuteMsg, [Coin; 2]) {
    let msg = ExecuteMsg::ProvideLiquidity {
        assets: [
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: uusd_amount.clone(),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
                amount: uluna_amount.clone(),
            },
        ],
        slippage_tolerance: None,
        auto_stake: None,
        receiver,
    };

    let coins = [
        Coin {
            denom: "uluna".to_string(),
            amount: uluna_amount.clone(),
        },
        Coin {
            denom: "uusd".to_string(),
            amount: uusd_amount.clone(),
        },
    ];

    (msg, coins)
}