use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{attr, to_binary, Addr, Coin, Decimal, Timestamp, Uint128};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse};
use cw_multi_test::{App, ContractWrapper, Executor};
use dexter::asset::{Asset, AssetInfo};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{
    AfterJoinResponse, ConfigResponse as Pool_ConfigResponse, QueryMsg as PoolQueryMsg,
};
use dexter::vault::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, FeeInfo, InstantiateMsg, PoolTypeConfig,
    PoolConfigResponse, PoolInfo, PoolInfoResponse, PoolType, QueryMsg, SingleSwapRequest,
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

fn store_token_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        lp_token::contract::execute,
        lp_token::contract::instantiate,
        lp_token::contract::query,
    ));
    app.store_code(token_contract)
}

fn store_stable5_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(ContractWrapper::new_with_empty(
        stable5pool::contract::execute,
        stable5pool::contract::instantiate,
        stable5pool::contract::query,
    ));
    app.store_code(pool_contract)
}

fn store_weighted_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(ContractWrapper::new_with_empty(
        weighted_pool::contract::execute,
        weighted_pool::contract::instantiate,
        weighted_pool::contract::query,
    ));
    app.store_code(pool_contract)
}

fn store_stable_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(ContractWrapper::new_with_empty(
        stableswap_pool::contract::execute,
        stableswap_pool::contract::instantiate,
        stableswap_pool::contract::query,
    ));
    app.store_code(pool_contract)
}

fn store_xyk_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(ContractWrapper::new_with_empty(
        xyk_pool::contract::execute,
        xyk_pool::contract::instantiate,
        xyk_pool::contract::query,
    ));
    app.store_code(pool_contract)
}

// Initialize a vault with XYK, Stable, Stable5, Weighted pools
fn instantiate_contract(app: &mut App, owner: &Addr) -> Addr {
    let xyk_pool_code_id = store_xyk_pool_code(app);
    let stable_pool_code_id = store_stable_pool_code(app);
    let weighted_pool_code_id = store_weighted_pool_code(app);
    let stable5_pool_code_id = store_stable5_pool_code(app);

    let vault_code_id = store_vault_code(app);
    let token_code_id = store_token_code(app);

    let pool_configs = vec![
        PoolTypeConfig {
            code_id: xyk_pool_code_id,
            pool_type: PoolType::Xyk {},
            fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: Some(Addr::unchecked(&"xyk_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
        },
        PoolTypeConfig {
            code_id: stable_pool_code_id,
            pool_type: PoolType::Stable2Pool {},
            fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: Some(Addr::unchecked(&"stable_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
        },
        PoolTypeConfig {
            code_id: weighted_pool_code_id,
            pool_type: PoolType::Weighted {},
            fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: Some(Addr::unchecked(&"weighted_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
        },
        PoolTypeConfig {
            code_id: stable5_pool_code_id,
            pool_type: PoolType::Stable5Pool {},
            fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: Some(Addr::unchecked(&"stable5_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
        },
    ];

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: token_code_id,
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        generator_address: None,
    };

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

    return vault_instance;
}

fn initialize_3_tokens(app: &mut App, owner: &Addr) -> (Addr, Addr, Addr) {
    let token_code_id = store_token_code(app);

    // Initialize 3 tokens
    let token_instance0 = app
        .instantiate_contract(
            token_code_id,
            Addr::unchecked(owner.clone()),
            &TokenInstantiateMsg {
                name: "x_token".to_string(),
                symbol: "X-Tok".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: owner.to_string(),
                    cap: None,
                }),
                marketing: None,
            },
            &[],
            "x_token",
            None,
        )
        .unwrap();
    let token_instance2 = app
        .instantiate_contract(
            token_code_id,
            Addr::unchecked(owner.clone()),
            &TokenInstantiateMsg {
                name: "y_token".to_string(),
                symbol: "y-Tok".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: owner.to_string(),
                    cap: None,
                }),
                marketing: None,
            },
            &[],
            "y_token",
            None,
        )
        .unwrap();
    let token_instance3 = app
        .instantiate_contract(
            token_code_id,
            Addr::unchecked(owner.clone()),
            &TokenInstantiateMsg {
                name: "z_token".to_string(),
                symbol: "z-Tok".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: owner.to_string(),
                    cap: None,
                }),
                marketing: None,
            },
            &[],
            "x_token",
            None,
        )
        .unwrap();
    (token_instance0, token_instance2, token_instance3)
}

// Mints some Tokens to "to" recipient
fn mint_some_tokens(app: &mut App, owner: Addr, token_instance: Addr, amount: Uint128, to: String) {
    let msg = cw20::Cw20ExecuteMsg::Mint {
        recipient: to.clone(),
        amount: amount,
    };
    app.execute_contract(owner.clone(), token_instance.clone(), &msg, &[])
        .unwrap();
}

/// Initialize a STABLE-5-POOL
/// --------------------------
fn initialize_stable_5_pool(
    app: &mut App,
    owner: &Addr,
    vault_instance: Addr,
    token_instance0: Addr,
    token_instance1: Addr,
    token_instance2: Addr,
    denom0: String,
    denom1: String,
) -> (Addr, Addr, Uint128) {
    let asset_infos = vec![
        AssetInfo::NativeToken { denom: denom0 },
        AssetInfo::NativeToken { denom: denom1 },
        AssetInfo::Token {
            contract_addr: token_instance1.clone(),
        },
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
        AssetInfo::Token {
            contract_addr: token_instance2.clone(),
        },
    ];

    // Initialize Stable-5-Pool contract instance
    // ------------------------------------------

    let vault_config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &QueryMsg::Config {})
        .unwrap();
    let next_pool_id = vault_config_res.next_pool_id;
    let msg = ExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Stable5Pool {},
        asset_infos: asset_infos.to_vec(),
        init_params: Some(to_binary(&stable5pool::state::StablePoolParams { amp: 10u64 }).unwrap()),
    };
    app.execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

    let pool_info_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::GetPoolById {
                pool_id: next_pool_id,
            },
        )
        .unwrap();

    let pool_addr = pool_info_res.pool_addr.unwrap();
    let lp_token_addr = pool_info_res.lp_token_addr.unwrap();
    let pool_id = pool_info_res.pool_id;

    return (pool_addr, lp_token_addr, pool_id);
}

/// Initialize a WEIGHTED POOL
/// --------------------------
fn initialize_weighted_pool(
    app: &mut App,
    owner: &Addr,
    vault_instance: Addr,
    token_instance0: Addr,
    token_instance1: Addr,
    token_instance2: Addr,
    denom0: String,
    denom1: String,
) -> (Addr, Addr, Uint128) {
    let asset_infos = vec![
        AssetInfo::NativeToken {
            denom: denom0.clone(),
        },
        AssetInfo::NativeToken {
            denom: denom1.clone(),
        },
        AssetInfo::Token {
            contract_addr: token_instance1.clone(),
        },
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
        AssetInfo::Token {
            contract_addr: token_instance2.clone(),
        },
    ];
    let asset_infos_with_weights = vec![
        Asset {
            info: AssetInfo::NativeToken { denom: denom0 },
            amount: Uint128::from(20u128),
        },
        Asset {
            info: AssetInfo::NativeToken { denom: denom1 },
            amount: Uint128::from(20u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::from(20u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(20u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(20u128),
        },
    ];
    // Initialize Stable-5-Pool contract instance
    // ------------------------------------------

    let vault_config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &QueryMsg::Config {})
        .unwrap();
    let next_pool_id = vault_config_res.next_pool_id;
    let msg = ExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Weighted {},
        asset_infos: asset_infos.to_vec(),
        init_params: Some(
            to_binary(&weighted_pool::state::WeightedParams {
                weights: asset_infos_with_weights,
                exit_fee: Some(Decimal::from_ratio(1u128, 100u128)),
            })
            .unwrap(),
        ),
    };
    app.execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

    let pool_info_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::GetPoolById {
                pool_id: next_pool_id,
            },
        )
        .unwrap();

    let pool_addr = pool_info_res.pool_addr.unwrap();
    let lp_token_addr = pool_info_res.lp_token_addr.unwrap();
    let pool_id = pool_info_res.pool_id;

    return (pool_addr, lp_token_addr, pool_id);
}

/// Initialize a STABLE POOL
/// --------------------------
fn initialize_stable_pool(
    app: &mut App,
    owner: &Addr,
    vault_instance: Addr,
    token_instance0: Addr,
    denom0: String,
) -> (Addr, Addr, Uint128) {
    let asset_infos = vec![
        AssetInfo::NativeToken {
            denom: denom0.clone(),
        },
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
    ];

    // Initialize Stable-Pool contract instance
    // ------------------------------------------

    let vault_config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &QueryMsg::Config {})
        .unwrap();
    let next_pool_id = vault_config_res.next_pool_id;
    let msg = ExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Stable2Pool {},
        asset_infos: asset_infos.to_vec(),
        init_params: Some(to_binary(&stable5pool::state::StablePoolParams { amp: 10u64 }).unwrap()),
    };
    app.execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

    let pool_info_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::GetPoolById {
                pool_id: next_pool_id,
            },
        )
        .unwrap();

    let pool_addr = pool_info_res.pool_addr.unwrap();
    let lp_token_addr = pool_info_res.lp_token_addr.unwrap();
    let pool_id = pool_info_res.pool_id;

    return (pool_addr, lp_token_addr, pool_id);
}

/// Initialize a XYK POOL
/// --------------------------
fn initialize_xyk_pool(
    app: &mut App,
    owner: &Addr,
    vault_instance: Addr,
    token_instance0: Addr,
    denom0: String,
) -> (Addr, Addr, Uint128) {
    let asset_infos = vec![
        AssetInfo::NativeToken {
            denom: denom0.clone(),
        },
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
    ];

    // Initialize XYK Pool contract instance
    // ------------------------------------------

    let vault_config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &QueryMsg::Config {})
        .unwrap();
    let next_pool_id = vault_config_res.next_pool_id;
    let msg = ExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Xyk {},
        asset_infos: asset_infos.to_vec(),
        init_params: None,
    };
    app.execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

    let pool_info_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::GetPoolById {
                pool_id: next_pool_id,
            },
        )
        .unwrap();

    let pool_addr = pool_info_res.pool_addr.unwrap();
    let lp_token_addr = pool_info_res.lp_token_addr.unwrap();
    let pool_id = pool_info_res.pool_id;

    return (pool_addr, lp_token_addr, pool_id);
}

#[test]
fn proper_initialization() {
    let owner = Addr::unchecked("owner");
    let mut app = mock_app(Addr::unchecked(owner.clone()), vec![]);
    let vault_code_id = store_vault_code(&mut app);
    let xyk_pool_code_id = store_xyk_pool_code(&mut app);
    let stable_pool_code_id = store_stable_pool_code(&mut app);
    let weighted_pool_code_id = store_weighted_pool_code(&mut app);
    let stable5_pool_code_id = store_stable5_pool_code(&mut app);
    let token_code_id = store_token_code(&mut app);

    let pool_configs = vec![
        PoolTypeConfig {
            code_id: xyk_pool_code_id,
            pool_type: PoolType::Xyk {},
            fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: Some(Addr::unchecked(&"xyk_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
        },
        PoolTypeConfig {
            code_id: stable_pool_code_id,
            pool_type: PoolType::Stable2Pool {},
            fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: Some(Addr::unchecked(&"stable_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
        },
        PoolTypeConfig {
            code_id: stable5_pool_code_id,
            pool_type: PoolType::Stable5Pool {},
            fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: Some(Addr::unchecked(&"stable5_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
        },
        PoolTypeConfig {
            code_id: weighted_pool_code_id,
            pool_type: PoolType::Weighted {},
            fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: Some(Addr::unchecked(&"weighted_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
        },
    ];

    //// -----x----- Success :: Initialize Vault Contract -----x----- ////

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: token_code_id,
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        generator_address: None,
    };

    let vault_instance = app
        .instantiate_contract(
            vault_code_id,
            Addr::unchecked(owner.clone()),
            &vault_init_msg,
            &[],
            "vault",
            None,
        )
        .unwrap();

    let msg = QueryMsg::Config {};
    let config_res: ConfigResponse = app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();

    assert_eq!(owner, config_res.owner);
    assert_eq!(token_code_id, config_res.lp_token_code_id);
    assert_eq!(
        Some(Addr::unchecked("fee_collector".to_string())),
        config_res.fee_collector
    );
    assert_eq!(None, config_res.generator_address);

    // Check XYK Pool Config
    // ---------------------
    let xyk_pool_config_res: PoolConfigResponse = app
        .wrap()
        .query_wasm_smart(
            &vault_instance,
            &QueryMsg::QueryRegistry {
                pool_type: PoolType::Xyk {},
            },
        )
        .unwrap();
    assert_eq!(xyk_pool_code_id, xyk_pool_config_res.code_id);
    assert_eq!(PoolType::Xyk {}, xyk_pool_config_res.pool_type);
    assert_eq!(pool_configs[0].fee_info, xyk_pool_config_res.fee_info);
    assert_eq!(pool_configs[0].allow_instantiation, xyk_pool_config_res.allow_instantiation);
    assert_eq!(
        pool_configs[0].is_generator_disabled,
        xyk_pool_config_res.is_generator_disabled
    );

    // Check Stabl Pool Config
    // ---------------------
    let stablepool_config_res: PoolConfigResponse = app
        .wrap()
        .query_wasm_smart(
            &vault_instance,
            &QueryMsg::QueryRegistry {
                pool_type: PoolType::Stable2Pool {},
            },
        )
        .unwrap();
    assert_eq!(stable_pool_code_id, stablepool_config_res.code_id);
    assert_eq!(PoolType::Stable2Pool {}, stablepool_config_res.pool_type);
    assert_eq!(pool_configs[1].fee_info, stablepool_config_res.fee_info);
    assert_eq!(
        pool_configs[1].allow_instantiation,
        stablepool_config_res.allow_instantiation
    );
    assert_eq!(
        pool_configs[1].is_generator_disabled,
        stablepool_config_res.is_generator_disabled
    );

    // Check Stabl-5-Pool Config
    // ---------------------
    let stable5pool_config_res: PoolConfigResponse = app
        .wrap()
        .query_wasm_smart(
            &vault_instance,
            &QueryMsg::QueryRegistry {
                pool_type: PoolType::Stable5Pool {},
            },
        )
        .unwrap();
    assert_eq!(stable5_pool_code_id, stable5pool_config_res.code_id);
    assert_eq!(PoolType::Stable5Pool {}, stable5pool_config_res.pool_type);
    assert_eq!(pool_configs[2].fee_info, stable5pool_config_res.fee_info);
    assert_eq!(
        pool_configs[2].allow_instantiation,
        stable5pool_config_res.allow_instantiation
    );
    assert_eq!(
        pool_configs[2].is_generator_disabled,
        stable5pool_config_res.is_generator_disabled
    );

    // Check Weighted Config
    // ---------------------
    let weightedpool_config_res: PoolConfigResponse = app
        .wrap()
        .query_wasm_smart(
            &vault_instance,
            &QueryMsg::QueryRegistry {
                pool_type: PoolType::Weighted {},
            },
        )
        .unwrap();
    assert_eq!(weighted_pool_code_id, weightedpool_config_res.code_id);
    assert_eq!(PoolType::Weighted {}, weightedpool_config_res.pool_type);
    assert_eq!(pool_configs[3].fee_info, weightedpool_config_res.fee_info);
    assert_eq!(
        pool_configs[3].allow_instantiation,
        weightedpool_config_res.allow_instantiation
    );
    assert_eq!(
        pool_configs[3].is_generator_disabled,
        weightedpool_config_res.is_generator_disabled
    );

    //// -----x----- Error :: PoolConfigDuplicate Error -----x----- ////

    let pool_configs = vec![
        PoolTypeConfig {
            code_id: xyk_pool_code_id,
            pool_type: PoolType::Xyk {},
            fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: None,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
        },
        PoolTypeConfig {
            code_id: xyk_pool_code_id,
            pool_type: PoolType::Xyk {},
            fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: None,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
        },
    ];

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: token_code_id,
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        generator_address: None,
    };

    let res = app
        .instantiate_contract(
            vault_code_id,
            Addr::unchecked(owner.clone()),
            &vault_init_msg,
            &[],
            "vault",
            None,
        )
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "Duplicate of Pool Configs");

    //// -----x----- Error :: InvalidFeeInfo Error -----x----- ////

    let pool_configs = vec![PoolTypeConfig {
        code_id: xyk_pool_code_id,
        pool_type: PoolType::Xyk {},
        fee_info: FeeInfo {
            total_fee_bps: 30000u16,
            protocol_fee_percent: 49u16,
            dev_fee_percent: 15u16,
            developer_addr: None,
        },
        allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
        is_generator_disabled: false,
    }];

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: token_code_id,
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        generator_address: None,
    };

    let res = app
        .instantiate_contract(
            vault_code_id,
            Addr::unchecked(owner.clone()),
            &vault_init_msg,
            &[],
            "vault",
            None,
        )
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "Invalid FeeInfo params");
}

#[test]
fn update_config() {
    let owner = String::from("owner");
    let mut app = mock_app(Addr::unchecked(owner.clone()), vec![]);
    let vault_instance = instantiate_contract(&mut app, &Addr::unchecked(owner.clone()));

    let msg = QueryMsg::Config {};
    let after_init_config_res: ConfigResponse =
        app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();

    assert_eq!(owner, after_init_config_res.owner);
    assert_eq!(
        Some(Addr::unchecked("fee_collector".to_string())),
        after_init_config_res.fee_collector
    );
    assert_eq!(None, after_init_config_res.generator_address);

    //// -----x----- Success :: update config -----x----- ////

    let msg = ExecuteMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: Some("fee_address".to_string()),
        generator_address: Some("generator_address".to_string()),
    };

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    let msg = QueryMsg::Config {};
    let config_res: ConfigResponse = app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();

    assert_eq!(owner, config_res.owner);
    assert_eq!(
        Some(Addr::unchecked("fee_address".to_string())),
        config_res.fee_collector
    );
    assert_eq!(
        Some(Addr::unchecked("generator_address".to_string())),
        config_res.generator_address
    );
    assert_eq!(
        after_init_config_res.lp_token_code_id,
        config_res.lp_token_code_id
    );
}

#[test]
fn test_add_to_registery() {
    let owner = Addr::unchecked("owner");
    let mut app = mock_app(Addr::unchecked(owner.clone()), vec![]);
    let vault_code_id = store_vault_code(&mut app);
    let xyk_pool_code_id = store_xyk_pool_code(&mut app);
    let token_code_id = store_token_code(&mut app);

    let pool_configs = vec![PoolTypeConfig {
        code_id: xyk_pool_code_id,
        pool_type: PoolType::Xyk {},
        fee_info: FeeInfo {
            total_fee_bps: 300u16,
            protocol_fee_percent: 49u16,
            dev_fee_percent: 15u16,
            developer_addr: None,
        },
        allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
        is_generator_disabled: false,
    }];

    //// -----x----- Success :: Initialize Vault Contract -----x----- ////

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: token_code_id,
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        generator_address: None,
    };

    let vault_instance = app
        .instantiate_contract(
            vault_code_id,
            Addr::unchecked(owner.clone()),
            &vault_init_msg,
            &[],
            "vault",
            None,
        )
        .unwrap();

    let msg = QueryMsg::QueryRegistry {
        pool_type: PoolType::Xyk {},
    };
    let registery_res: PoolConfigResponse =
        app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();
    assert_eq!(xyk_pool_code_id, registery_res.code_id);
    assert_eq!(PoolType::Xyk {}, registery_res.pool_type);
    assert_eq!(pool_configs[0].fee_info, registery_res.fee_info);
    assert_eq!(pool_configs[0].allow_instantiation, registery_res.allow_instantiation);
    assert_eq!(
        pool_configs[0].is_generator_disabled,
        registery_res.is_generator_disabled
    );

    //// -----x----- Error :: Only Owner can add new PoolType to registery || Pool Type already exists -----x----- ////

    let msg = ExecuteMsg::AddToRegistry {
        new_pool_config: PoolTypeConfig {
            code_id: xyk_pool_code_id,
            pool_type: PoolType::Xyk {},
            fee_info: FeeInfo {
                total_fee_bps: 10_0u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: None,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
        },
    };

    let err_res = app
        .execute_contract(
            Addr::unchecked("not_owner".to_string().clone()),
            vault_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Unauthorized");

    let err_res = app
        .execute_contract(
            Addr::unchecked(owner.clone()),
            vault_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Pool Type already exists");

    //// -----x----- Error :: Only Owner can add new PoolType to registery || Pool Type already exists -----x----- ////

    let msg = ExecuteMsg::AddToRegistry {
        new_pool_config: PoolTypeConfig {
            code_id: xyk_pool_code_id,
            pool_type: PoolType::Stable2Pool {},
            fee_info: FeeInfo {
                total_fee_bps: 10_001u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: None,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
        },
    };

    let err_res = app
        .execute_contract(
            Addr::unchecked(owner.clone()),
            vault_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Invalid FeeInfo params");

    //// -----x----- Success :: Add new PoolType to registery  -----x----- ////
    let stable_pool_code_id = 2u64;
    let msg = ExecuteMsg::AddToRegistry {
        new_pool_config: PoolTypeConfig {
            code_id: stable_pool_code_id,
            pool_type: PoolType::Stable2Pool {},
            fee_info: FeeInfo {
                total_fee_bps: 1_001u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: None,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
        },
    };

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    let msg = QueryMsg::QueryRegistry {
        pool_type: PoolType::Stable2Pool {},
    };
    let registery_res: PoolConfigResponse =
        app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();
    assert_eq!(stable_pool_code_id, registery_res.code_id);
    assert_eq!(PoolType::Stable2Pool {}, registery_res.pool_type);
    assert_eq!(
        FeeInfo {
            total_fee_bps: 1_001u16,
            protocol_fee_percent: 49u16,
            dev_fee_percent: 15u16,
            developer_addr: None,
        },
        registery_res.fee_info
    );
    assert_eq!(dexter::vault::AllowPoolInstantiation::Everyone, registery_res.allow_instantiation);
    assert_eq!(false, registery_res.is_generator_disabled);
}

#[test]
fn test_create_pool_instance() {
    let owner = String::from("owner");
    let mut app = mock_app(Addr::unchecked(owner.clone()), vec![]);

    let token_code_id = store_token_code(&mut app);
    let vault_instance = instantiate_contract(&mut app, &Addr::unchecked(owner.clone()));

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

    // Create Token Y
    let init_msg = TokenInstantiateMsg {
        name: "y_token".to_string(),
        symbol: "Y-Tok".to_string(),
        decimals: 18,
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
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
        AssetInfo::Token {
            contract_addr: token_instance1.clone(),
        },
    ];

    let msg = ExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Xyk {},
        asset_infos: asset_infos.to_vec(),
        init_params: None,
    };

    let res = app
        .execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

    assert_eq!(res.events[1].attributes[2], attr("pool_type", "xyk"));

    let pool_res: PoolInfo = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::GetPoolById {
                pool_id: Uint128::from(1u128),
            },
        )
        .unwrap();

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
    ];

    assert_eq!(Uint128::from(1u128), pool_res.pool_id);
    assert_eq!(
        Some(Addr::unchecked("contract3".to_string())),
        pool_res.pool_addr
    );
    assert_eq!(
        Some(Addr::unchecked("contract4".to_string())),
        pool_res.lp_token_addr
    );
    assert_eq!(assets, pool_res.assets);
    assert_eq!(PoolType::Xyk {}, pool_res.pool_type);
}

#[test]
fn test_update_owner() {
    let owner = String::from("owner");
    let mut app = mock_app(Addr::unchecked(owner.clone()), vec![]);
    let vault_instance = instantiate_contract(&mut app, &Addr::unchecked(owner.clone()));

    let new_owner = String::from("new_owner");

    // New owner
    let msg = ExecuteMsg::ProposeNewOwner {
        owner: new_owner.clone(),
        expires_in: 100, // seconds
    };

    // Unauthed check
    let err = app
        .execute_contract(
            Addr::unchecked("not_owner"),
            vault_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    // Claim before proposal
    let err = app
        .execute_contract(
            Addr::unchecked(new_owner.clone()),
            vault_instance.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Ownership proposal not found"
    );

    // Propose new owner
    app.execute_contract(Addr::unchecked("owner"), vault_instance.clone(), &msg, &[])
        .unwrap();

    // Claim from invalid addr
    let err = app
        .execute_contract(
            Addr::unchecked("invalid_addr"),
            vault_instance.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap_err();
    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    // Drop ownership proposal
    let err = app
        .execute_contract(
            Addr::unchecked(new_owner.clone()),
            vault_instance.clone(),
            &ExecuteMsg::DropOwnershipProposal {},
            &[],
        )
        .unwrap_err();
    // new_owner is not an owner yet
    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &ExecuteMsg::DropOwnershipProposal {},
        &[],
    )
    .unwrap();

    // Try to claim ownership
    let err = app
        .execute_contract(
            Addr::unchecked(new_owner.clone()),
            vault_instance.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Ownership proposal not found"
    );

    // Propose new owner again
    app.execute_contract(Addr::unchecked("owner"), vault_instance.clone(), &msg, &[])
        .unwrap();
    // Claim ownership
    app.execute_contract(
        Addr::unchecked(new_owner.clone()),
        vault_instance.clone(),
        &ExecuteMsg::ClaimOwnership {},
        &[],
    )
    .unwrap();

    // Let's query the contract state
    let msg = QueryMsg::Config {};
    let res: ConfigResponse = app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();

    assert_eq!(res.owner, new_owner)
}

#[test]
fn test_join_pool() {
    let owner = Addr::unchecked("owner".to_string());
    let denom0 = "token0".to_string();
    let denom1 = "token1".to_string();

    let mut app = mock_app(
        owner.clone(),
        vec![
            Coin {
                denom: denom0.clone(),
                amount: Uint128::new(100000000_000_000_000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(100000000_000_000_000u128),
            },
        ],
    );
    let vault_instance = instantiate_contract(&mut app, &owner.clone());

    let (token_instance1, token_instance2, token_instance3) =
        initialize_3_tokens(&mut app, &owner.clone());

    // Mint Tokens
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        Uint128::new(10000000_000000u128),
        owner.clone().to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance2.clone(),
        Uint128::new(10000000_000000u128),
        owner.clone().to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance3.clone(),
        Uint128::new(10000000_000000u128),
        owner.clone().to_string(),
    );

    // Increase Allowances
    app.execute_contract(
        owner.clone(),
        token_instance1.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(1000000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        token_instance2.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(1000000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        token_instance3.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(1000000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();

    // Create STABLE-5-POOL pool
    let (stable5_pool_addr, stable5_lp_token_addr, stable5_pool_id) = initialize_stable_5_pool(
        &mut app,
        &Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        token_instance1.clone(),
        token_instance2.clone(),
        token_instance3.clone(),
        denom0.clone(),
        denom1.clone(),
    );
    // Create WEIGHTED pool
    let (_, weighted_lp_token_addr, weighted_pool_id) = initialize_weighted_pool(
        &mut app,
        &Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        token_instance1.clone(),
        token_instance2.clone(),
        token_instance3.clone(),
        denom0.clone(),
        denom1.clone(),
    );

    // Create STABLE pool
    let (_, _, stable_pool_id) = initialize_stable_pool(
        &mut app,
        &Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        token_instance1.clone(),
        denom0.clone(),
    );
    // Create XYK pool
    let (_, _, xyk_pool_id) = initialize_xyk_pool(
        &mut app,
        &Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        token_instance1.clone(),
        denom0.clone(),
    );

    // -------x---------- STABLE-5-POOL -::- PROVIDE LIQUIDITY -------x----------
    // -------x---------- -------x---------- -------x---------- -------x----------

    // VAULT -::- Join Pool -::- Execution Function
    // --- Stable5Pool:OnJoinPool Query : Begin ---
    // init_d: 0
    // deposit_d: 5000
    // Fee will be charged only during imbalanced provide i.e. if invariant D was changed
    // --- Stable5Pool:OnJoinPool Query :: End ---
    // Following assets are to be transferred by the user to the Vault:
    // ::: "contract1" "1000000000"
    // ::: "contract2" "1000000000"
    // ::: "contract3" "1000000000"
    // ::: "token0" "1000000000"
    // ::: "token1" "1000000000"
    // LP tokens to be minted: "5000000000"
    // Transfering total "1000000000" "contract1" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "contract1"
    // Transfering total "1000000000" "contract2" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "contract2"
    // Transfering total "1000000000" "contract3" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "contract3"
    // Transfering total "1000000000" "token0" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "token0"
    // Transfering total "1000000000" "token1" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "token1"
    let mut assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance3.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
    ];
    // Check Query Response
    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            stable5_pool_addr.clone(),
            &PoolQueryMsg::OnJoinPool {
                assets_in: Some(assets_msg.clone()),
                mint_amount: None,
                slippage_tolerance: None,
            },
        )
        .unwrap();
    // Provide liquidity to empty stable 5 pool. No fee is charged
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(stable5_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: None,
            assets: Some(assets_msg.clone()),
        },
        &[
            Coin {
                denom: denom0.clone(),
                amount: Uint128::new(1000_000000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(1000_000000u128),
            },
        ],
    )
    .unwrap();

    // Pool Config
    let pool_config_res: Pool_ConfigResponse = app
        .wrap()
        .query_wasm_smart(stable5_pool_addr.clone(), &PoolQueryMsg::Config {})
        .unwrap();

    // Checks -
    // - Pool Liquidity balances updates correctly
    // - Tokens transferred to the Vault.
    // - Fee transferred correctly - 0 fee charged
    // - LP tokens minted & transferred correctly
    let mut cur_user_lp_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &stable5_lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(join_pool_query_res.new_shares, cur_user_lp_balance.balance);
    assert_eq!(join_pool_query_res.provided_assets, pool_config_res.assets);

    let vault_token1_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1000_000000u128), vault_token1_balance.balance);
    let mut vault_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1000_000000u128), vault_token2_balance.balance);
    let vault_token3_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance3.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1000_000000u128), vault_token3_balance.balance);

    // VAULT -::- Join Pool -::- Execution Function
    // --- Stable5Pool:OnJoinPool Query : Begin ---
    // init_d: 5000
    // deposit_d: 11036.237493754238660601
    // Fee will be charged only during imbalanced provide i.e. if invariant D was changed
    // For contract1, fee is charged on 245.75250124915226788 amount, which is difference b/w 2207.24749875084773212 (ideal_balance) and 2453 (new_balance). Fee charged:2.303929699210802511
    // For contract2, fee is charged on 262.24749875084773212 amount, which is difference b/w 2207.24749875084773212 (ideal_balance) and 1945 (new_balance). Fee charged:2.458570300789197488
    // For contract3, fee is charged on 355.75250124915226788 amount, which is difference b/w 2207.24749875084773212 (ideal_balance) and 2563 (new_balance). Fee charged:3.335179699210802511
    // For token0, fee is charged on 73.24749875084773212 amount, which is difference b/w 2207.24749875084773212 (ideal_balance) and 2134 (new_balance). Fee charged:0.686695300789197488
    // For token1, fee is charged on 259.24749875084773212 amount, which is difference b/w 2207.24749875084773212 (ideal_balance) and 1948 (new_balance). Fee charged:2.430445300789197488
    // after_fee_d (Invariant computed for - total tokens provided as liquidity - total fee): 11025.030251953726515704
    // --- Stable5Pool:OnJoinPool Query :: End ---
    // Following assets are to be transferred by the user to the Vault:
    // ::: "contract1" "1453000000"
    // ::: "contract2" "945000000"
    // ::: "contract3" "1563000000"
    // ::: "token0" "1134000000"
    // ::: "token1" "948000000"
    // LP tokens to be minted: "6025030251"
    // Transfering total "1453000000" "contract1" to the Vault. Total Fee : "2303929" (protocol_fee="1128925", dev_fee="345589" LP fee="829415"). Liquidity updated with "1451525486" "contract1"
    // Transfering total "945000000" "contract2" to the Vault. Total Fee : "2458570" (protocol_fee="1204699", dev_fee="368785" LP fee="885086"). Liquidity updated with "943426516" "contract2"
    // Transfering total "1563000000" "contract3" to the Vault. Total Fee : "3335179" (protocol_fee="1634237", dev_fee="500276" LP fee="1200666"). Liquidity updated with "1560865487" "contract3"
    // Transfering total "1134000000" "token0" to the Vault. Total Fee : "686695" (protocol_fee="336480", dev_fee="103004" LP fee="247211"). Liquidity updated with "1133560516" "token0"
    // Transfering total "948000000" "token1" to the Vault. Total Fee : "2430445" (protocol_fee="1190918", dev_fee="364566" LP fee="874961"). Liquidity updated with "946444516" "token1"
    assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            amount: Uint128::from(1134_000000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom1.clone(),
            },
            amount: Uint128::from(948_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(1453_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(945_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance3.clone(),
            },
            amount: Uint128::from(1563_000000u128),
        },
    ];

    // Check Query Response
    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            stable5_pool_addr.clone(),
            &PoolQueryMsg::OnJoinPool {
                assets_in: Some(assets_msg.clone()),
                mint_amount: None,
                slippage_tolerance: None,
            },
        )
        .unwrap();

    // Provide imbalanced liquidity to stable 5 pool. Fee is charged
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(stable5_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: None,
            assets: Some(assets_msg.clone()),
        },
        &[
            Coin {
                denom: denom0.clone(),
                amount: Uint128::new(1155_000000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(10000_000000u128),
            },
        ],
    )
    .unwrap();

    // Checks -
    // - Pool Liquidity balances updates correctly
    // - Tokens transferred to the Vault.
    // - Fee transferred correctly - 0 fee charged
    // - LP tokens minted & transferred correctly
    let mut new_user_lp_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &stable5_lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        join_pool_query_res.new_shares,
        new_user_lp_balance.balance - cur_user_lp_balance.balance
    );

    let new_vault_token1_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(1451525486u128),
        new_vault_token1_balance.balance - vault_token1_balance.balance
    );

    let mut new_vault_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(943426516u128),
        new_vault_token2_balance.balance - vault_token2_balance.balance
    );

    let new_vault_token3_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance3.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(1560865487u128),
        new_vault_token3_balance.balance - vault_token3_balance.balance
    );

    // FEE CHECKS
    let keeper_token1_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1128925u128), keeper_token1_balance.balance);
    let mut keeper_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1204699u128), keeper_token2_balance.balance);
    let keeper_token3_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance3.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1634237u128), keeper_token3_balance.balance);

    let dev_token1_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: "stable5_dev".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(345589u128), dev_token1_balance.balance);
    let mut dev_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "stable5_dev".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(368785u128), dev_token2_balance.balance);
    let dev_token3_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance3.clone(),
            &Cw20QueryMsg::Balance {
                address: "stable5_dev".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(500276u128), dev_token3_balance.balance);

    // Provide only 2 of 5 assets liquidity to stable 5 pool. Fee is charged
    // VAULT -::- Join Pool -::- Execution Function
    // --- Stable5Pool:OnJoinPool Query : Begin ---
    // init_d: 11029.064881254479322174
    // deposit_d: 12287.337905948465585852
    // Fee will be charged only during imbalanced provide i.e. if invariant D was changed
    // For contract1, fee is charged on 279.687210257190258222 amount, which is difference b/w 2731.212696257190258222 (ideal_balance) and 2451.525486 (new_balance). Fee charged:2.62206759616115867
    // For contract2, fee is charged on 523.280281521050150215 amount, which is difference b/w 2165.146234478949849785 (ideal_balance) and 2688.426516 (new_balance). Fee charged:4.905752639259845158
    // For contract3, fee is charged on 292.161483938556503376 amount, which is difference b/w 2853.026970938556503376 (ideal_balance) and 2560.865487 (new_balance). Fee charged:2.739013911923967219
    // For token0, fee is charged on 270.588462146246137888 amount, which is difference b/w 2376.972053853753862112 (ideal_balance) and 2647.560516 (new_balance). Fee charged:2.536766832621057542
    // For token1, fee is charged on 222.064033072200704588 amount, which is difference b/w 2168.508549072200704588 (ideal_balance) and 1946.444516 (new_balance). Fee charged:2.081850310051881605
    // after_fee_d (Invariant computed for - total tokens provided as liquidity - total fee): 12272.475667670006139966
    // --- Stable5Pool:OnJoinPool Query :: End ---
    // Following assets are to be transferred by the user to the Vault:
    // ::: "contract1" "0"
    // ::: "contract2" "745000000"
    // ::: "contract3" "0"
    // ::: "token0" "514000000"
    // ::: "token1" "0"
    // LP tokens to be minted: "1242955924"
    // Transfering total "0" "contract1" to the Vault. Total Fee : "2622067" (protocol_fee="1284812", dev_fee="393310" LP fee="943945"). Liquidity updated by subtracting  "1678122" "contract1"
    // Transfering total "745000000" "contract2" to the Vault. Total Fee : "4905752" (protocol_fee="2403818", dev_fee="735862" LP fee="1766072"). Liquidity updated with "741860320" "contract2"
    // Transfering total "0" "contract3" to the Vault. Total Fee : "2739013" (protocol_fee="1342116", dev_fee="410851" LP fee="986046"). Liquidity updated by subtracting  "1752967" "contract3"
    // Transfering total "514000000" "token0" to the Vault. Total Fee : "2536766" (protocol_fee="1243015", dev_fee="380514" LP fee="913237"). Liquidity updated with "512376471" "token0"
    // Transfering total "0" "token1" to the Vault. Total Fee : "2081850" (protocol_fee="1020106", dev_fee="312277" LP fee="749467"). Liquidity updated by subtracting  "1332383" "token1"
    assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            amount: Uint128::from(514_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(745_000000u128),
        },
    ];
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(stable5_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: None,
            assets: Some(assets_msg.clone()),
        },
        &[Coin {
            denom: denom0.clone(),
            amount: Uint128::new(1155_000000u128),
        }],
    )
    .unwrap();

    // -------x---------- WEIGHTED-POOL -::- PROVIDE LIQUIDITY -------x----------
    // -------x---------- -------x---------- -------x---------- -------x----------

    // --- Liquidity provided to empty pool - No fee is charged ---
    // VAULT -::- Join Pool -::- Execution Function
    // --- WeightedPool:OnJoinPool Query : Begin ---
    // Lp shares to mint (exact-ratio-join): 100000000
    // Following assets are to be transferred by the user to the Vault:
    // ::: "contract1" "1000000000"
    // ::: "contract2" "1000000000"
    // ::: "contract3" "1000000000"
    // ::: "token0" "1000000000"
    // ::: "token1" "1000000000"
    // LP tokens to be minted: "100000000"
    // Transfering total "1000000000" "contract1" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "contract1"
    // Transfering total "1000000000" "contract2" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "contract2"
    // Transfering total "1000000000" "contract3" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "contract3"
    // Transfering total "1000000000" "token0" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "token0"
    // Transfering total "1000000000" "token1" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "token1"
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance3.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
    ];
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(weighted_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: None,
            assets: Some(assets_msg.clone()),
        },
        &[
            Coin {
                denom: denom0.clone(),
                amount: Uint128::new(1000_000000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(1000_000000u128),
            },
        ],
    )
    .unwrap();

    // --- Liquidity added with all tokens in imbalanced manner ---
    // VAULT -::- Join Pool -::- Execution Function
    // --- WeightedPool:OnJoinPool Query : Begin ---
    // Lp shares to mint (exact-ratio-join): 94500000
    // We need to charge fee during single asset join for :  "contract1"
    // "contract1" - Tokens in = "508000000" Tokens in (after fee) = "495808000" Fee charged = "12192000"
    // contract1 new_num_shares_from_single: 9036549 | in_asset : 508000000, fee_charged: 12192000
    // We need to charge fee during single asset join for :  "contract3"
    // "contract3" - Tokens in = "618000000" Tokens in (after fee) = "603168000" Fee charged = "14832000"
    // contract3 new_num_shares_from_single: 11297986 | in_asset : 618000000, fee_charged: 14832000
    // We need to charge fee during single asset join for :  "token0"
    // "token0" - Tokens in = "189000000" Tokens in (after fee) = "184464000" Fee charged = "4536000"
    // token0 new_num_shares_from_single: 3928648 | in_asset : 189000000, fee_charged: 4536000
    // We need to charge fee during single asset join for :  "token1"
    // "token1" - Tokens in = "3000000" Tokens in (after fee) = "2928000" Fee charged = "72000"
    // token1 new_num_shares_from_single: 65825 | in_asset : 3000000, fee_charged: 72000
    // --- WeightedPool:OnJoinPool Query :: End ---
    // Following assets are to be transferred by the user to the Vault:
    // ::: "contract1" "1453000000"
    // ::: "contract2" "945000000"
    // ::: "contract3" "1563000000"
    // ::: "token0" "1134000000"
    // ::: "token1" "948000000"
    // LP tokens to be minted: "118829008"
    // Transfering total "1453000000" "contract1" to the Vault. Total Fee : "12192000" (protocol_fee="5974080", dev_fee="1828800" LP fee="4389120"). Liquidity updated with "1445197120" "contract1"
    // Transfering total "945000000" "contract2" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "945000000" "contract2"
    // Transfering total "1563000000" "contract3" to the Vault. Total Fee : "14832000" (protocol_fee="7267680", dev_fee="2224800" LP fee="5339520"). Liquidity updated with "1553507520" "contract3"
    // Transfering total "1134000000" "token0" to the Vault. Total Fee : "4536000" (protocol_fee="2222640", dev_fee="680400" LP fee="1632960"). Liquidity updated with "1131096960" "token0"
    // Transfering total "948000000" "token1" to the Vault. Total Fee : "72000" (protocol_fee="35280", dev_fee="10800" LP fee="25920"). Liquidity updated with "947953920" "token1"
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            amount: Uint128::from(1134_000000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom1.clone(),
            },
            amount: Uint128::from(948_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(1453_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(945_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance3.clone(),
            },
            amount: Uint128::from(1563_000000u128),
        },
    ];
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(weighted_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: None,
            assets: Some(assets_msg.clone()),
        },
        &[
            Coin {
                denom: denom0.clone(),
                amount: Uint128::new(1500_000000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(1000_000000u128),
            },
        ],
    )
    .unwrap();

    // --- Liquidity added with a single token ---
    // VAULT -::- Join Pool -::- Execution Function
    // --- WeightedPool:OnJoinPool Query : Begin ---
    // ---- Single asset join
    // We need to charge fee during single asset join for :  "contract2"
    // "contract2" - Tokens in = "945000000" Tokens in (after fee) = "922320000" Fee charged = "22680000"
    // Following assets are to be transferred by the user to the Vault:
    // ::: "contract1" "0"
    // ::: "contract2" "945000000"
    // ::: "contract3" "0"
    // ::: "token0" "0"
    // ::: "token1" "0"
    // LP tokens to be minted: "17662854"
    // Transfering total "945000000" "contract2" to the Vault. Total Fee : "22680000" (protocol_fee="11113200", dev_fee="3402000" LP fee="8164800"). Liquidity updated with "930484800" "contract2"

    cur_user_lp_balance = app
        .wrap()
        .query_wasm_smart(
            &weighted_lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();

    vault_token2_balance = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    keeper_token2_balance = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    dev_token2_balance = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "weighted_dev".to_string(),
            },
        )
        .unwrap();

    let assets_msg = vec![Asset {
        info: AssetInfo::Token {
            contract_addr: token_instance2.clone(),
        },
        amount: Uint128::from(945_000000u128),
    }];

    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(weighted_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: None,
            assets: Some(assets_msg.clone()),
        },
        &[],
    )
    .unwrap();

    // Checks -
    // - Tokens transferred to the Vault.
    // - Fee transferred correctly - 0 fee charged
    // - LP tokens minted & transferred correctly
    new_user_lp_balance = app
        .wrap()
        .query_wasm_smart(
            &weighted_lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();

    assert_eq!(
        Uint128::from(17662854u128),
        new_user_lp_balance.balance - cur_user_lp_balance.balance
    );

    new_vault_token2_balance = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(930484800u128),
        new_vault_token2_balance.balance - vault_token2_balance.balance
    );

    // // FEE CHECKS
    let new_keeper_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(11113200u128),
        new_keeper_token2_balance.balance - keeper_token2_balance.balance
    );

    let new_dev_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "weighted_dev".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(3402000u128),
        new_dev_token2_balance.balance - dev_token2_balance.balance
    );

    // -------x---------- XYK-POOL -::- PROVIDE LIQUIDITY -------x----------
    // -------x---------- -------x---------- -------x---------- -------x----

    // Provided to empty XYK Pool
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(xyk_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: None,
            assets: Some(vec![
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: denom0.clone(),
                    },
                    amount: Uint128::from(1000_000000u128),
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: token_instance1.clone(),
                    },
                    amount: Uint128::from(1000_000000u128),
                },
            ]),
        },
        &[Coin {
            denom: denom0.clone(),
            amount: Uint128::new(1000_000000u128),
        }],
    )
    .unwrap();

    // Provided to non-empty XYK Pool
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(xyk_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: Some(Decimal::from_ratio(50u128, 100u128)),
            assets: Some(vec![
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: token_instance1.clone(),
                    },
                    amount: Uint128::from(563_000000u128),
                },
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: denom0.clone(),
                    },
                    amount: Uint128::from(557_000000u128),
                },
            ]),
        },
        &[Coin {
            denom: denom0.clone(),
            amount: Uint128::new(1000_000000u128),
        }],
    )
    .unwrap();

    // -------x---------- Stableswap-POOL -::- PROVIDE LIQUIDITY -------x---------
    // -------x---------- -------x---------- -------x---------- -------x----------

    // Provided to empty Stable Pool
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(stable_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: None,
            assets: Some(vec![
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: denom0.clone(),
                    },
                    amount: Uint128::from(1000_000000u128),
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: token_instance1.clone(),
                    },
                    amount: Uint128::from(1000_000000u128),
                },
            ]),
        },
        &[Coin {
            denom: denom0.clone(),
            amount: Uint128::new(1000_000000u128),
        }],
    )
    .unwrap();

    // Provided to non-empty Stable-Pool
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(stable_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: Some(Decimal::from_ratio(50u128, 100u128)),
            assets: Some(vec![
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: token_instance1.clone(),
                    },
                    amount: Uint128::from(563_000000u128),
                },
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: denom0.clone(),
                    },
                    amount: Uint128::from(557_000000u128),
                },
            ]),
        },
        &[Coin {
            denom: denom0.clone(),
            amount: Uint128::new(1000_000000u128),
        }],
    )
    .unwrap();
}

#[test]
fn test_exit_pool() {
    let owner = Addr::unchecked("owner".to_string());
    let denom0 = "token0".to_string();
    let denom1 = "token1".to_string();

    let mut app = mock_app(
        owner.clone(),
        vec![
            Coin {
                denom: denom0.clone(),
                amount: Uint128::new(100000000_000_000_000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(100000000_000_000_000u128),
            },
        ],
    );
    let vault_instance = instantiate_contract(&mut app, &owner.clone());

    let (token_instance1, token_instance2, token_instance3) =
        initialize_3_tokens(&mut app, &owner.clone());

    // Mint Tokens
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        Uint128::new(10000000_000000u128),
        owner.clone().to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance2.clone(),
        Uint128::new(10000000_000000u128),
        owner.clone().to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance3.clone(),
        Uint128::new(10000000_000000u128),
        owner.clone().to_string(),
    );

    // Increase Allowances
    app.execute_contract(
        owner.clone(),
        token_instance1.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(1000000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        token_instance2.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(1000000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        token_instance3.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(1000000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();

    // Create STABLE-5-POOL pool
    let (_, stable5_lp_token_addr, stable5_pool_id) = initialize_stable_5_pool(
        &mut app,
        &Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        token_instance1.clone(),
        token_instance2.clone(),
        token_instance3.clone(),
        denom0.clone(),
        denom1.clone(),
    );
    // Create WEIGHTED pool
    let (_, weighted_lp_token_addr, weighted_pool_id) = initialize_weighted_pool(
        &mut app,
        &Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        token_instance1.clone(),
        token_instance2.clone(),
        token_instance3.clone(),
        denom0.clone(),
        denom1.clone(),
    );

    // Create STABLE pool
    let (_, stable_lp_token_addr, stable_pool_id) = initialize_stable_pool(
        &mut app,
        &Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        token_instance1.clone(),
        denom0.clone(),
    );
    // Create XYK pool
    let (_, xyk_lp_token_addr, xyk_pool_id) = initialize_xyk_pool(
        &mut app,
        &Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        token_instance1.clone(),
        denom0.clone(),
    );

    // Provide liquidity to empty stable 5 pool. No fee is charged
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance3.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
    ];
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(stable5_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: None,
            assets: Some(assets_msg.clone()),
        },
        &[
            Coin {
                denom: denom0.clone(),
                amount: Uint128::new(1000_000000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(1000_000000u128),
            },
        ],
    )
    .unwrap();

    // Liquidity provided to empty Weighted pool - No fee is charged
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance3.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
    ];
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(weighted_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: None,
            assets: Some(assets_msg.clone()),
        },
        &[
            Coin {
                denom: denom0.clone(),
                amount: Uint128::new(1000_000000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(1000_000000u128),
            },
        ],
    )
    .unwrap();

    // Liquidity Provided to empty XYK Pool
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(xyk_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: None,
            assets: Some(vec![
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: denom0.clone(),
                    },
                    amount: Uint128::from(1000_000000u128),
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: token_instance1.clone(),
                    },
                    amount: Uint128::from(1000_000000u128),
                },
            ]),
        },
        &[Coin {
            denom: denom0.clone(),
            amount: Uint128::new(1000_000000u128),
        }],
    )
    .unwrap();

    // Liquidity Provided to empty Stable Pool
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(stable_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: None,
            assets: Some(vec![
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: denom0.clone(),
                    },
                    amount: Uint128::from(1000_000000u128),
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: token_instance1.clone(),
                    },
                    amount: Uint128::from(1000_000000u128),
                },
            ]),
        },
        &[Coin {
            denom: denom0.clone(),
            amount: Uint128::new(1000_000000u128),
        }],
    )
    .unwrap();

    // -------x---------- Stable-5-swap-POOL -::- WITHDRAW LIQUIDITY -------x---------
    // -------x---------- -------x---------- -------x---------- -------x--------------

    // When you withdraw only 1 token from a stable-5-pool
    // VAULT -::- Exit Pool -::- Execution Function
    // Stable-5-Pool : Imbalanced Withdraw
    // Stable-5-Pool : Initial D : 5000
    // Stable-5-Pool : Withdraw D : 4899.612056677904874438
    // For token0, fee is charged on 79.922411335580974887 amount, which is difference b/w 979.922411335580974887 (ideal_balance) and 900 (new_balance). Fee charged:0.749272606271071639
    // For contract1, fee is charged on 20.077588664419025113 amount, which is difference b/w 979.922411335580974887 (ideal_balance) and 1000 (new_balance). Fee charged:0.18822739372892836
    // For contract2, fee is charged on 20.077588664419025113 amount, which is difference b/w 979.922411335580974887 (ideal_balance) and 1000 (new_balance). Fee charged:0.18822739372892836
    // For contract3, fee is charged on 20.077588664419025113 amount, which is difference b/w 979.922411335580974887 (ideal_balance) and 1000 (new_balance). Fee charged:0.18822739372892836
    // For token1, fee is charged on 20.077588664419025113 amount, which is difference b/w 979.922411335580974887 (ideal_balance) and 1000 (new_balance). Fee charged:0.18822739372892836
    // Stable-5-Pool : After Fee D : 4898.105276502220535553
    // Stable-5-Pool : Total Share : 5000000000
    // Stable-5-Pool : Burn Amount : 101894724
    // act_burn_amount: 101894724
    // Transfering total "0" "contract1" to the User. Total Fee : "188227" (protocol_fee="92231", dev_fee="28234" LP fee="67762"). Liquidity withdrawn = "120465" "contract1"
    // Transfering total "0" "contract2" to the User. Total Fee : "188227" (protocol_fee="92231", dev_fee="28234" LP fee="67762"). Liquidity withdrawn = "120465" "contract2"
    // Transfering total "0" "contract3" to the User. Total Fee : "188227" (protocol_fee="92231", dev_fee="28234" LP fee="67762"). Liquidity withdrawn = "120465" "contract3"
    // Transfering total "100000000" "token0" to the User. Total Fee : "749272" (protocol_fee="367143", dev_fee="112390" LP fee="269739"). Liquidity withdrawn = "100479533" "token0"
    // Transfering total "0" "token1" to the User. Total Fee : "188227" (protocol_fee="92231", dev_fee="28234" LP fee="67762"). Liquidity withdrawn = "120465" "token1"
    // test test_exit_pool ... ok
    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(500_000000u128),
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(stable5_pool_id),
            recipient: None,
            assets: Some(vec![Asset {
                info: AssetInfo::NativeToken {
                    denom: denom0.clone(),
                },
                amount: Uint128::from(100_000000u128),
            }]),
            burn_amount: Some(Uint128::from(500_000000u128)),
        })
        .unwrap(),
    };
    app.execute_contract(owner.clone(), stable5_lp_token_addr.clone(), &exit_msg, &[])
        .unwrap();

    // When you withdraw multiple tokens from a stable-5-pool
    // VAULT -::- Exit Pool -::- Execution Function
    // Stable-5-Pool : Imbalanced Withdraw
    // Stable-5-Pool : Initial D : 4898.647724421098219454
    // Stable-5-Pool : Withdraw D : 4508.309241082299087282
    // For token0, fee is charged on 53.32359569956993967 amount, which is difference b/w 827.84406269956993967 (ideal_balance) and 774.520467 (new_balance). Fee charged:0.499908709683468184
    // For token1, fee is charged on 74.673306424484064835 amount, which is difference b/w 920.206228575515935165 (ideal_balance) and 994.879535 (new_balance). Fee charged:0.700062247729538107
    // For contract2, fee is charged on 177.326693575515935165 amount, which is difference b/w 920.206228575515935165 (ideal_balance) and 742.879535 (new_balance). Fee charged:1.662437752270461892
    // For contract1, fee is charged on 79.673306424484064835 amount, which is difference b/w 920.206228575515935165 (ideal_balance) and 999.879535 (new_balance). Fee charged:0.746937247729538107
    // For contract3, fee is charged on 79.673306424484064835 amount, which is difference b/w 920.206228575515935165 (ideal_balance) and 999.879535 (new_balance). Fee charged:0.746937247729538107
    // Stable-5-Pool : After Fee D : 4503.934926829800769923
    // Stable-5-Pool : Total Share : 4898105276
    // Stable-5-Pool : Burn Amount : 394669090
    // act_burn_amount: 394669090
    // Transfering total "0" "contract1" to the User. Total Fee : "746937" (protocol_fee="365999", dev_fee="112040" LP fee="268898"). Liquidity withdrawn = "478039" "contract1"
    // Transfering total "257000000" "contract2" to the User. Total Fee : "1662437" (protocol_fee="814594", dev_fee="249365" LP fee="598478"). Liquidity withdrawn = "258063959" "contract2"
    // Transfering total "0" "contract3" to the User. Total Fee : "746937" (protocol_fee="365999", dev_fee="112040" LP fee="268898"). Liquidity withdrawn = "478039" "contract3"
    // Transfering total "125000000" "token0" to the User. Total Fee : "499908" (protocol_fee="244954", dev_fee="74986" LP fee="179968"). Liquidity withdrawn = "125319940" "token0"
    // Transfering total "5000000" "token1" to the User. Total Fee : "700062" (protocol_fee="343030", dev_fee="105009" LP fee="252023"). Liquidity withdrawn = "5448039" "token1"

    let cur_user_lp_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &stable5_lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();

    let vault_token1_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    let vault_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    let vault_token3_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance3.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    let keeper_token1_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    let dev_token1_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: "stable5_dev".to_string(),
            },
        )
        .unwrap();
    let keeper_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    let dev_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "stable5_dev".to_string(),
            },
        )
        .unwrap();

    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(500_000000u128),
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(stable5_pool_id),
            recipient: None,
            assets: Some(vec![
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: denom0.clone(),
                    },
                    amount: Uint128::from(125_000000u128),
                },
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: denom1.clone(),
                    },
                    amount: Uint128::from(5_000000u128),
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: token_instance2.clone(),
                    },
                    amount: Uint128::from(257_000000u128),
                },
            ]),
            burn_amount: Some(Uint128::from(500_000000u128)),
        })
        .unwrap(),
    };
    app.execute_contract(owner.clone(), stable5_lp_token_addr.clone(), &exit_msg, &[])
        .unwrap();

    // Checks -
    // - Tokens transferred to the Vault.
    // - Fee transferred correctly
    // - LP tokens burnt & returned correctly
    let new_user_lp_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &stable5_lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(394669090u128),
        cur_user_lp_balance.balance - new_user_lp_balance.balance
    );

    let new_vault_token1_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(478039u128),
        vault_token1_balance.balance - new_vault_token1_balance.balance
    );

    let new_vault_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(258063959u128),
        vault_token2_balance.balance - new_vault_token2_balance.balance
    );
    let new_vault_token3_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance3.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(478039u128),
        vault_token3_balance.balance - new_vault_token3_balance.balance
    );

    // FEE CHECKS
    let new_keeper_token1_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(365999u128),
        new_keeper_token1_balance.balance - keeper_token1_balance.balance
    );

    let new_dev_token1_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: "stable5_dev".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(112040u128),
        new_dev_token1_balance.balance - dev_token1_balance.balance
    );

    let new_keeper_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(814594u128),
        new_keeper_token2_balance.balance - keeper_token2_balance.balance
    );

    let new_dev_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "stable5_dev".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(249365u128),
        new_dev_token2_balance.balance - dev_token2_balance.balance
    );

    // When its normal withdraw from a stable-5-pool. No fee charged
    // VAULT -::- Exit Pool -::- Execution Function
    // act_burn_amount: 50000000
    // Transfering total "11095988" "contract1" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "11095988" "contract1"
    // Transfering total "8236106" "contract2" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "8236106" "contract2"
    // Transfering total "11095988" "contract3" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "11095988" "contract3"
    // Transfering total "8595664" "token0" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "8595664" "token0"
    // Transfering total "11040808" "token1" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "11040808" "token1"
    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(50_000000u128),
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(stable5_pool_id),
            recipient: None,
            assets: None,
            burn_amount: Some(Uint128::from(50_000000u128)),
        })
        .unwrap(),
    };
    app.execute_contract(owner.clone(), stable5_lp_token_addr.clone(), &exit_msg, &[])
        .unwrap();

    // -------x---------- Weighted POOL -::- WITHDRAW LIQUIDITY -------x---------
    // -------x---------- -------x---------- -------x---------- -------x---------

    // No Fee charged by weighted pool
    // VAULT -::- Exit Pool -::- Execution Function
    // Transfering total "49500000" "contract1" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "49500000" "contract1"
    // Transfering total "49500000" "contract2" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "49500000" "contract2"
    // Transfering total "49500000" "contract3" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "49500000" "contract3"
    // Transfering total "49500000" "token0" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "49500000" "token0"
    // Transfering total "49500000" "token1" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "49500000" "token1"
    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(5000_000u128),
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(weighted_pool_id),
            recipient: None,
            assets: None,
            burn_amount: Some(Uint128::from(5000_000u128)),
        })
        .unwrap(),
    };
    app.execute_contract(
        owner.clone(),
        weighted_lp_token_addr.clone(),
        &exit_msg,
        &[],
    )
    .unwrap();

    // -------x---------- XYK POOL -::- WITHDRAW LIQUIDITY -------x--------------
    // -------x---------- -------x---------- -------x---------- -------x---------

    // No Fee charged by XYK pool
    // VAULT -::- Exit Pool -::- Execution Function
    // Transfering total "5000000" "contract1" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "5000000" "contract1"
    // Transfering total "5000000" "token0" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "5000000" "token0"
    // test test_exit_pool ... ok
    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(5000_000u128),
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(xyk_pool_id),
            recipient: None,
            assets: None,
            burn_amount: Some(Uint128::from(5000_000u128)),
        })
        .unwrap(),
    };
    app.execute_contract(owner.clone(), xyk_lp_token_addr.clone(), &exit_msg, &[])
        .unwrap();

    // -------x---------- StableSwap POOL -::- WITHDRAW LIQUIDITY -------x--------------
    // -------x---------- -------x---------- -------x---------- -------x----------------

    // No Fee charged by Stableswap pool
    // VAULT -::- Exit Pool -::- Execution Function
    // Transfering total "5000000" "contract1" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "5000000" "contract1"
    // Transfering total "5000000" "token0" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "5000000" "token0"
    // test test_exit_pool ... ok
    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(5000_000u128),
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(stable_pool_id),
            recipient: None,
            assets: None,
            burn_amount: Some(Uint128::from(5000_000u128)),
        })
        .unwrap(),
    };
    app.execute_contract(owner.clone(), stable_lp_token_addr.clone(), &exit_msg, &[])
        .unwrap();
}

#[test]
fn test_swap() {
    let owner = Addr::unchecked("owner".to_string());
    let denom0 = "token0".to_string();
    let denom1 = "token1".to_string();

    let mut app = mock_app(
        owner.clone(),
        vec![
            Coin {
                denom: denom0.clone(),
                amount: Uint128::new(100000000_000_000_000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(100000000_000_000_000u128),
            },
        ],
    );
    let vault_instance = instantiate_contract(&mut app, &owner.clone());

    let (token_instance1, token_instance2, token_instance3) =
        initialize_3_tokens(&mut app, &owner.clone());

    // Mint Tokens
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        Uint128::new(10000000_000000u128),
        owner.clone().to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance2.clone(),
        Uint128::new(10000000_000000u128),
        owner.clone().to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance3.clone(),
        Uint128::new(10000000_000000u128),
        owner.clone().to_string(),
    );

    // Increase Allowances
    app.execute_contract(
        owner.clone(),
        token_instance1.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(1000000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        token_instance2.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(1000000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        token_instance3.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(1000000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();

    // Create STABLE-5-POOL pool
    let (_, _, stable5_pool_id) = initialize_stable_5_pool(
        &mut app,
        &Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        token_instance1.clone(),
        token_instance2.clone(),
        token_instance3.clone(),
        denom0.clone(),
        denom1.clone(),
    );
    // Create WEIGHTED pool
    let (_, _, weighted_pool_id) = initialize_weighted_pool(
        &mut app,
        &Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        token_instance1.clone(),
        token_instance2.clone(),
        token_instance3.clone(),
        denom0.clone(),
        denom1.clone(),
    );

    // Create STABLE pool
    let (_, _, stable_pool_id) = initialize_stable_pool(
        &mut app,
        &Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        token_instance1.clone(),
        denom0.clone(),
    );
    // Create XYK pool
    let (_, _, xyk_pool_id) = initialize_xyk_pool(
        &mut app,
        &Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        token_instance1.clone(),
        denom0.clone(),
    );

    // Provide liquidity to empty stable 5 pool. No fee is charged
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance3.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
    ];
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(stable5_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: None,
            assets: Some(assets_msg.clone()),
        },
        &[
            Coin {
                denom: denom0.clone(),
                amount: Uint128::new(1000_000000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(1000_000000u128),
            },
        ],
    )
    .unwrap();

    // Liquidity provided to empty Weighted pool - No fee is charged
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance3.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
    ];
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(weighted_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: None,
            assets: Some(assets_msg.clone()),
        },
        &[
            Coin {
                denom: denom0.clone(),
                amount: Uint128::new(1000_000000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(1000_000000u128),
            },
        ],
    )
    .unwrap();

    // Liquidity Provided to empty XYK Pool
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(xyk_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: None,
            assets: Some(vec![
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: denom0.clone(),
                    },
                    amount: Uint128::from(1000_000000u128),
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: token_instance1.clone(),
                    },
                    amount: Uint128::from(1000_000000u128),
                },
            ]),
        },
        &[Coin {
            denom: denom0.clone(),
            amount: Uint128::new(1000_000000u128),
        }],
    )
    .unwrap();

    // Liquidity Provided to empty Stable Pool
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(stable_pool_id),
            recipient: None,
            lp_to_mint: None,
            auto_stake: None,
            slippage_tolerance: None,
            assets: Some(vec![
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: denom0.clone(),
                    },
                    amount: Uint128::from(1000_000000u128),
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: token_instance1.clone(),
                    },
                    amount: Uint128::from(1000_000000u128),
                },
            ]),
        },
        &[Coin {
            denom: denom0.clone(),
            amount: Uint128::new(1000_000000u128),
        }],
    )
    .unwrap();

    let _current_block = app.block_info();
    app.update_block(|b| {
        b.height += 10;
        b.time = Timestamp::from_seconds(_current_block.time.seconds() + 90)
    });

    // -------x---------- Stable-5-swap-POOL -::- SWAP TOKENS -------x---------
    // -------x---------- -------x---------- -------x---------- -------x--------------
    // Execute Swap :: GiveIn Type
    // VAULT -::- Swap -::- Execution Function
    // Offer asset: token1 Ask asset: contract2 Swap Type: "give-in" Amount: 252000000
    // --- Stable5Pool:OnSwap Query :: Start ---
    // SwapType::GiveIn
    // In compute_swap() fn, we calculate the new ask pool balance which is 753939768 and calculate the return amount (cur_pool_balance - new_pool_balance) which is 246060232
    // fee yet to be charged: 7381806, hence return amount (actual return amount - total_fee) = 238678426
    // VAULT -::- Swap -::- Pool Swap Transition Query Response returned - amount_in:252000000 amount_out:238678426 spread:5939768. Response: success
    // Fee: 7381806 contract2
    // Protocol Fee: 3617084 Dev Fee: 1107270
    // Ask Asset ::: Pool Liquidity being updated. Current pool balance: 1000000000. Ask Asset Amount: 238678426
    // Ask Asset ::: Pool Liquidity after subtracting the ask asset amount to be transferred 761321574
    // Fee Asset ::: Pool Liquidity being updated. Protocol and dev fee to be subtracted. Current pool liquidity 761321574
    // Fee Asset ::: Pool Liquidity after being updated: 756597220
    // Offer Asset ::: Pool Liquidity being updated. Current pool balance: 1000000000. Offer Asset Amount: 252000000
    // Offer Asset ::: Pool Liquidity after adding offer asset amount provided 1252000000
    let swap_msg = ExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(stable5_pool_id),
            swap_type: SwapType::GiveIn {},
            asset_in: AssetInfo::NativeToken {
                denom: denom1.to_string(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(252_000000u128),
            max_spread: Some(Decimal::percent(50)),
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: denom1.to_string(),
            amount: Uint128::new(252_000000u128),
        }],
    )
    .unwrap();

    let user_ask_token_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();
    let vault_ask_token_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    let keeper_ask_token_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    let dev_ask_token_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "stable5_dev".to_string(),
            },
        )
        .unwrap();

    // Execute Swap :: GiveOut Type
    // VAULT -::- Swap -::- Execution Function
    // Offer asset: token1 Ask asset: contract2 Swap Type: "give-out" Amount: 252000000
    // --- Stable5Pool:OnSwap Query :: Start ---
    // SwapType::GiveOut
    // In compute_offer_amount() fn, we calculate the new ask offer pool balance which is 1537249235 based on updated ask_pool balance which includes ask_amount + total fee yet to be charged. ask_amount = 252000000, ask_amount_before_commission = 259.793814
    // offer amount = 285249235, total fee = 7793814
    // VAULT -::- Swap -::- Pool Swap Transition Query Response returned - amount_in:285249235 amount_out:252000000 spread:25455421. Response: success
    // Fee: 7793814 contract2
    // Protocol Fee: 3818968 Dev Fee: 1169072
    // Ask Asset ::: Pool Liquidity being updated. Current pool balance: 756597220. Ask Asset Amount: 252000000
    // Ask Asset ::: Pool Liquidity after subtracting the ask asset amount to be transferred 504597220
    // Fee Asset ::: Pool Liquidity being updated. Protocol and dev fee to be subtracted. Current pool liquidity 504597220
    // Fee Asset ::: Pool Liquidity after being updated: 499609180
    // Offer Asset ::: Pool Liquidity being updated. Current pool balance: 1252000000. Offer Asset Amount: 285249235
    // Offer Asset ::: Pool Liquidity after adding offer asset amount provided 1537249235
    let swap_msg = ExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(stable5_pool_id),
            swap_type: SwapType::GiveOut {},
            asset_in: AssetInfo::NativeToken {
                denom: denom1.to_string(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(252_000000u128),
            max_spread: Some(Decimal::percent(50)),
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: denom1.to_string(),
            amount: Uint128::new(292_000000u128),
        }],
    )
    .unwrap();

    // Checks if tokens are transferred correctly
    let new_user_ask_token_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();
    let new_vault_ask_token_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    let new_keeper_ask_token_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    let new_dev_ask_token_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "stable5_dev".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(256988040u128),
        vault_ask_token_balance.balance - new_vault_ask_token_balance.balance
    );

    assert_eq!(
        Uint128::from(252000000u128),
        new_user_ask_token_balance.balance - user_ask_token_balance.balance
    );
    assert_eq!(
        Uint128::from(1169072u128),
        new_dev_ask_token_balance.balance - dev_ask_token_balance.balance
    );
    assert_eq!(
        Uint128::from(3818968u128),
        new_keeper_ask_token_balance.balance - keeper_ask_token_balance.balance
    );

    // VAULT -::- Swap -::- Execution Function
    // Offer asset: token1 Ask asset: contract2 Swap Type: "give-out" Amount: 252000000
    // --- Stable5Pool:OnSwap Query :: Start ---
    // SwapType::GiveOut
    // In compute_offer_amount() fn, we calculate the new ask offer pool balance which is 1537249235 based on updated ask_pool balance which includes ask_amount + total fee yet to be charged. ask_amount = 252000000, ask_amount_before_commission = 259.793814
    // offer amount = 285249235, total fee = 7793814
    // VAULT -::- Swap -::- Pool Swap Transition Query Response returned - amount_in:285249235 amount_out:252000000 spread:25455421. Response: success
    // Fee: 7793814 contract2
    // Protocol Fee: 3818968 Dev Fee: 1169072
    // Ask Asset ::: Pool Liquidity being updated. Current pool balance: 756597220. Ask Asset Amount: 252000000
    // Ask Asset ::: Pool Liquidity after subtracting the ask asset amount to be transferred 504597220
    // Fee Asset ::: Pool Liquidity being updated. Protocol and dev fee to be subtracted. Current pool liquidity 504597220
    // Fee Asset ::: Pool Liquidity after being updated: 499609180
    // Offer Asset ::: Pool Liquidity being updated. Current pool balance: 1252000000. Offer Asset Amount: 285249235
    // Offer Asset ::: Pool Liquidity after adding offer asset amount provided 1537249235

    // -------x---------- Weighted POOL -::- SWAP TOKENS -------x----------------
    // -------x---------- -------x---------- -------x---------- -------x---------

    // VAULT -::- Swap -::- Execution Function
    // Offer asset: token1 Ask asset: contract2 Swap Type: "give-in" Amount: 252000000
    // --- Weighted:OnSwap Query :: Start ---
    // SwapType::GiveIn
    // In compute_swap() fn in weighted pool, we solve for constant function variant with updated offer pool balance and calculate the return amount, which is 201277955
    // fee yet to be charged: 6038338, hence return amount (actual return amount - total_fee) = 195239617
    // VAULT -::- Swap -::- Pool Swap Transition Query Response returned - amount_in:252000000 amount_out:195239617 spread:0. Response: success
    // Fee: 6038338 contract2
    // Protocol Fee: 2958785 Dev Fee: 905750
    // Ask Asset ::: Pool Liquidity being updated. Current pool balance: 1000000000. Ask Asset Amount: 195239617
    // Ask Asset ::: Pool Liquidity after subtracting the ask asset amount to be transferred 804760383
    // Fee Asset ::: Pool Liquidity being updated. Protocol and dev fee to be subtracted. Current pool liquidity 804760383
    // Fee Asset ::: Pool Liquidity after being updated: 800895848
    // Offer Asset ::: Pool Liquidity being updated. Current pool balance: 1000000000. Offer Asset Amount: 252000000
    // Offer Asset ::: Pool Liquidity after adding offer asset amount provided 1252000000
    let swap_msg = ExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(weighted_pool_id),
            swap_type: SwapType::GiveIn {},
            asset_in: AssetInfo::NativeToken {
                denom: denom1.to_string(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(252_000000u128),
            max_spread: Some(Decimal::percent(50)),
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: denom1.to_string(),
            amount: Uint128::new(252_000000u128),
        }],
    )
    .unwrap();

    // VAULT -::- Swap -::- Execution Function
    // Offer asset: token1 Ask asset: contract2 Swap Type: "give-out" Amount: 252000000
    // --- Weighted:OnSwap Query :: Start ---
    // SwapType::GiveOut
    // In compute_offer_amount() fn, we calculate the new ask offer pool balance which is 541.102033567010309468 based on updated ask_pool balance which includes ask_amount + total fee yet to be charged. ask_amount = 252, ask_amount_before_commission = 259.793814432989690532
    // VAULT -::- Swap -::- Pool Swap Transition Query Response returned - amount_in:601110022 amount_out:252000000 spread:0. Response: success
    // Fee: 7793814 contract2
    // Protocol Fee: 3818968 Dev Fee: 1169072
    // Ask Asset ::: Pool Liquidity being updated. Current pool balance: 800895848. Ask Asset Amount: 252000000
    // Ask Asset ::: Pool Liquidity after subtracting the ask asset amount to be transferred 548895848
    // Fee Asset ::: Pool Liquidity being updated. Protocol and dev fee to be subtracted. Current pool liquidity 548895848
    // Fee Asset ::: Pool Liquidity after being updated: 543907808
    // Offer Asset ::: Pool Liquidity being updated. Current pool balance: 1252000000. Offer Asset Amount: 601110022
    // Offer Asset ::: Pool Liquidity after adding offer asset amount provided 1853110022
    let swap_msg = ExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(weighted_pool_id),
            swap_type: SwapType::GiveOut {},
            asset_in: AssetInfo::NativeToken {
                denom: denom1.to_string(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(252_000000u128),
            max_spread: Some(Decimal::percent(50)),
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: denom1.to_string(),
            amount: Uint128::new(792_000000u128),
        }],
    )
    .unwrap();

    // -------x---------- XYK POOL -::- SWAP TOKENS -------x--------------
    // -------x---------- -------x---------- -------x---------- -------x---------

    // VAULT -::- Swap -::- Execution Function
    // Offer asset: token0 Ask asset: contract1 Swap Type: "give-in" Amount: 252000000
    // --- XYK:OnSwap Query :: Start ---
    // SwapType::GiveIn
    // In compute_swap() fn in XYK pool, we calculate the return amount via (ask_amount = (ask_pool - cp / (offer_pool + offer_amount))), which is 201277955
    // fee yet to be charged: 6038338, hence return amount (actual return amount - total_fee) = 195239617
    // VAULT -::- Swap -::- Pool Swap Transition Query Response returned - amount_in:252000000 amount_out:195239617 spread:50722045. Response: success
    // Fee: 6038338 contract1
    // Protocol Fee: 2958785 Dev Fee: 905750
    // Ask Asset ::: Pool Liquidity being updated. Current pool balance: 1000000000. Ask Asset Amount: 195239617
    // Ask Asset ::: Pool Liquidity after subtracting the ask asset amount to be transferred 804760383
    // Fee Asset ::: Pool Liquidity being updated. Protocol and dev fee to be subtracted. Current pool liquidity 804760383
    // Fee Asset ::: Pool Liquidity after being updated: 800895848
    // Offer Asset ::: Pool Liquidity being updated. Current pool balance: 1000000000. Offer Asset Amount: 252000000
    // Offer Asset ::: Pool Liquidity after adding offer asset amount provided 1252000000
    let swap_msg = ExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(xyk_pool_id),
            swap_type: SwapType::GiveIn {},
            asset_in: AssetInfo::NativeToken {
                denom: denom0.to_string(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(252_000000u128),
            max_spread: Some(Decimal::percent(50)),
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: denom0.to_string(),
            amount: Uint128::new(252_000000u128),
        }],
    )
    .unwrap();

    // VAULT -::- Swap -::- Execution Function
    // Offer asset: token0 Ask asset: contract1 Swap Type: "give-out" Amount: 252000000
    // --- XYK:OnSwap Query :: Start ---
    // SwapType::GiveOut
    // In compute_offer_amount() fn, we calculate the offer_amount which is 601110021 based on updated ask_pool balance which includes ask_amount + total fee yet to be charged. ask_amount = 252000000, ask_amount_before_commission = 259793814
    // VAULT -::- Swap -::- Pool Swap Transition Query Response returned - amount_in:601110021 amount_out:252000000 spread:124732160. Response: success
    // Fee: 7793814 contract1
    // Protocol Fee: 3818968 Dev Fee: 1169072
    // Ask Asset ::: Pool Liquidity being updated. Current pool balance: 800895848. Ask Asset Amount: 252000000
    // Ask Asset ::: Pool Liquidity after subtracting the ask asset amount to be transferred 548895848
    // Fee Asset ::: Pool Liquidity being updated. Protocol and dev fee to be subtracted. Current pool liquidity 548895848
    // Fee Asset ::: Pool Liquidity after being updated: 543907808
    // Offer Asset ::: Pool Liquidity being updated. Current pool balance: 1252000000. Offer Asset Amount: 601110021
    // Offer Asset ::: Pool Liquidity after adding offer asset amount provided 1853110021
    let swap_msg = ExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(xyk_pool_id),
            swap_type: SwapType::GiveOut {},
            asset_in: AssetInfo::NativeToken {
                denom: denom0.to_string(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(252_000000u128),
            max_spread: Some(Decimal::percent(50)),
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: denom0.to_string(),
            amount: Uint128::new(792_000000u128),
        }],
    )
    .unwrap();

    // -------x---------- StableSwap POOL -::- SWAP TOKENS -------x---------------------
    // -------x---------- -------x---------- -------x---------- -------x----------------

    // VAULT -::- Swap -::- Execution Function
    // Offer asset: token0 Ask asset: contract1 Swap Type: "give-in" Amount: 252000000
    // --- Stableswap:OnSwap Query :: Start ---
    // SwapType::GiveIn
    // In compute_swap() fn in Stableswap pool, we calculate new ask pool balance based on offer amount and calculate the total return amount (with fee included) by subtracting it from current ask pool balance, total return amount: 246060232
    // fee yet to be charged: 7381806, hence return amount (actual return amount - total_fee) = 238678426
    // VAULT -::- Swap -::- Pool Swap Transition Query Response returned - amount_in:252000000 amount_out:238678426 spread:5939768. Response: success
    // Fee: 7381806 contract1
    // Protocol Fee: 3617084 Dev Fee: 1107270
    // Ask Asset ::: Pool Liquidity being updated. Current pool balance: 1000000000. Ask Asset Amount: 238678426
    // Ask Asset ::: Pool Liquidity after subtracting the ask asset amount to be transferred 761321574
    // Fee Asset ::: Pool Liquidity being updated. Protocol and dev fee to be subtracted. Current pool liquidity 761321574
    // Fee Asset ::: Pool Liquidity after being updated: 756597220
    // Offer Asset ::: Pool Liquidity being updated. Current pool balance: 1000000000. Offer Asset Amount: 252000000
    // Offer Asset ::: Pool Liquidity after adding offer asset amount provided 1252000000
    let swap_msg = ExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(stable_pool_id),
            swap_type: SwapType::GiveIn {},
            asset_in: AssetInfo::NativeToken {
                denom: denom0.to_string(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(252_000000u128),
            max_spread: Some(Decimal::percent(50)),
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: denom0.to_string(),
            amount: Uint128::new(252_000000u128),
        }],
    )
    .unwrap();

    // VAULT -::- Swap -::- Execution Function
    // Offer asset: token0 Ask asset: contract1 Swap Type: "give-out" Amount: 252000000
    // --- Stableswap:OnSwap Query :: Start ---
    // SwapType::GiveOut
    // In compute_offer_amount() fn, we calculate the offer_amount which is 285268305 based on updated ask_pool balance which includes ask_amount + total fee yet to be charged. ask_amount = 252000000, ask_amount_before_commission = 259793814
    // VAULT -::- Swap -::- Pool Swap Transition Query Response returned - amount_in:285268305 amount_out:252000000 spread:25474491. Response: success
    // Fee: 7793814 contract1
    // Protocol Fee: 3818968 Dev Fee: 1169072
    // Ask Asset ::: Pool Liquidity being updated. Current pool balance: 756597220. Ask Asset Amount: 252000000
    // Ask Asset ::: Pool Liquidity after subtracting the ask asset amount to be transferred 504597220
    // Fee Asset ::: Pool Liquidity being updated. Protocol and dev fee to be subtracted. Current pool liquidity 504597220
    // Fee Asset ::: Pool Liquidity after being updated: 499609180
    // Offer Asset ::: Pool Liquidity being updated. Current pool balance: 1252000000. Offer Asset Amount: 285268305
    // Offer Asset ::: Pool Liquidity after adding offer asset amount provided 1537268305
    let swap_msg = ExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(stable_pool_id),
            swap_type: SwapType::GiveOut {},
            asset_in: AssetInfo::NativeToken {
                denom: denom0.to_string(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(252_000000u128),
            max_spread: Some(Decimal::percent(50)),
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: denom0.to_string(),
            amount: Uint128::new(792_000000u128),
        }],
    )
    .unwrap();
}
