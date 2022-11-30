use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{to_binary, Addr, Coin, Decimal, Timestamp, Uint128};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse};
use cw_multi_test::{App, ContractWrapper, Executor};
use dexter::asset::{Asset, AssetInfo};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;

use dexter::router::{
    ConfigResponse, ExecuteMsg, HopSwapRequest, InstantiateMsg, QueryMsg, SimulateMultiHopResponse,
    SimulatedTrade,
};
use dexter::vault::{
    ConfigResponse as VaultConfigResponse, ExecuteMsg as VaultExecuteMsg, FeeInfo,
    InstantiateMsg as VaultInstantiateMsg, PoolTypeConfig, PoolInfoResponse, PoolType,
    QueryMsg as VaultQueryMsg, SwapType,
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

fn store_router_code(app: &mut App) -> u64 {
    let router_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_router::contract::execute,
        dexter_router::contract::instantiate,
        dexter_router::contract::query,
    ));
    app.store_code(router_contract)
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
            default_fee_info: FeeInfo {
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
            default_fee_info: FeeInfo {
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
            default_fee_info: FeeInfo {
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
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: Some(Addr::unchecked(&"stable5_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
        },
    ];

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

    let vault_config_res: VaultConfigResponse = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &VaultQueryMsg::Config {})
        .unwrap();
    let next_pool_id = vault_config_res.next_pool_id;
    let msg = VaultExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Stable5Pool {},
        asset_infos: asset_infos.to_vec(),
        init_params: Some(to_binary(&stable5pool::state::StablePoolParams { amp: 10u64 }).unwrap()),
        fee_info: None,
    };
    app.execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

    let pool_info_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &VaultQueryMsg::GetPoolById {
                pool_id: next_pool_id,
            },
        )
        .unwrap();

    let pool_addr = pool_info_res.pool_addr;
    let lp_token_addr = pool_info_res.lp_token_addr;
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

    let vault_config_res: VaultConfigResponse = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &VaultQueryMsg::Config {})
        .unwrap();
    let next_pool_id = vault_config_res.next_pool_id;
    let msg = VaultExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Weighted {},
        asset_infos: asset_infos.to_vec(),
        init_params: Some(
            to_binary(&weighted_pool::state::WeightedParams {
                weights: asset_infos_with_weights,
                exit_fee: Some(Decimal::from_ratio(1u128, 100u128)),
            })
            .unwrap(),
        ),
        fee_info: None,
    };
    app.execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

    let pool_info_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &VaultQueryMsg::GetPoolById {
                pool_id: next_pool_id,
            },
        )
        .unwrap();

    let pool_addr = pool_info_res.pool_addr;
    let lp_token_addr = pool_info_res.lp_token_addr;
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

    let vault_config_res: VaultConfigResponse = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &VaultQueryMsg::Config {})
        .unwrap();
    let next_pool_id = vault_config_res.next_pool_id;
    let msg = VaultExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Stable2Pool {},
        asset_infos: asset_infos.to_vec(),
        init_params: Some(to_binary(&stable5pool::state::StablePoolParams { amp: 10u64 }).unwrap()),
        fee_info: None,
    };
    app.execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

    let pool_info_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &VaultQueryMsg::GetPoolById {
                pool_id: next_pool_id,
            },
        )
        .unwrap();

    let pool_addr = pool_info_res.pool_addr;
    let lp_token_addr = pool_info_res.lp_token_addr;
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

    let vault_config_res: VaultConfigResponse = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &VaultQueryMsg::Config {})
        .unwrap();
    let next_pool_id = vault_config_res.next_pool_id;
    let msg = VaultExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Xyk {},
        asset_infos: asset_infos.to_vec(),
        init_params: None,
        fee_info: None,
    };
    app.execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

    let pool_info_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &VaultQueryMsg::GetPoolById {
                pool_id: next_pool_id,
            },
        )
        .unwrap();

    let pool_addr = pool_info_res.pool_addr;
    let lp_token_addr = pool_info_res.lp_token_addr;
    let pool_id = pool_info_res.pool_id;

    return (pool_addr, lp_token_addr, pool_id);
}

/// Initialize the Router contract
/// --------------------------
fn initialize_router(app: &mut App, owner: &Addr, vault_instance: Addr) -> Addr {
    let router_code_id = store_router_code(app);
    let router_instance = app
        .instantiate_contract(
            router_code_id,
            Addr::unchecked(owner),
            &InstantiateMsg {
                dexter_vault: vault_instance.clone().to_string(),
            },
            &[],
            "Router",
            None,
        )
        .unwrap();
    return router_instance;
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
    let router_code_id = store_router_code(&mut app);

    let pool_configs = vec![
        PoolTypeConfig {
            code_id: xyk_pool_code_id,
            pool_type: PoolType::Xyk {},
            default_fee_info: FeeInfo {
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
            default_fee_info: FeeInfo {
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
            default_fee_info: FeeInfo {
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
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
                dev_fee_percent: 15u16,
                developer_addr: Some(Addr::unchecked(&"weighted_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
        },
    ];

    //// -----x----- Success :: Initialize Vault & Router Contracts -----x----- ////

    // Vault contract instance
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

    let msg = VaultQueryMsg::Config {};
    let config_res: VaultConfigResponse =
        app.wrap().query_wasm_smart(&vault_instance, &msg).unwrap();

    assert_eq!(owner, config_res.owner);
    assert_eq!(token_code_id, config_res.lp_token_code_id);
    assert_eq!(
        Some(Addr::unchecked("fee_collector".to_string())),
        config_res.fee_collector
    );
    assert_eq!(None, config_res.generator_address);

    // Router contract instance
    let router_init_msg = InstantiateMsg {
        dexter_vault: vault_instance.clone().to_string(),
    };

    let router_instance = app
        .instantiate_contract(
            router_code_id,
            Addr::unchecked(owner.clone()),
            &router_init_msg,
            &[],
            "vault",
            None,
        )
        .unwrap();

    let msg = QueryMsg::Config {};
    let r_config_res: ConfigResponse = app.wrap().query_wasm_smart(&router_instance, &msg).unwrap();

    assert_eq!(
        vault_instance.clone().to_string(),
        r_config_res.dexter_vault
    );
}

#[test]
fn test_router_functionality() {
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
    let router_instance = initialize_router(&mut app, &owner.clone(), vault_instance.clone());

    let (token_instance1, token_instance2, token_instance3) =
        initialize_3_tokens(&mut app, &owner.clone());

    // Mint Tokens
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        Uint128::new(100000000_000000u128),
        owner.clone().to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance2.clone(),
        Uint128::new(100000000_000000u128),
        owner.clone().to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance3.clone(),
        Uint128::new(100000000_000000u128),
        owner.clone().to_string(),
    );

    // Increase Allowances
    app.execute_contract(
        owner.clone(),
        token_instance1.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(100000000_000000u128),
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
            amount: Uint128::from(100000000_000000u128),
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
            amount: Uint128::from(100000000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();

    // Increase Allowances
    app.execute_contract(
        owner.clone(),
        token_instance1.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: router_instance.clone().to_string(),
            amount: Uint128::from(100000000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        token_instance2.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: router_instance.clone().to_string(),
            amount: Uint128::from(100000000_000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        token_instance3.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: router_instance.clone().to_string(),
            amount: Uint128::from(100000000_000000u128),
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

    // -------x---------- STABLE-5-POOL -::- PROVIDE LIQUIDITY -------x----------
    // -------x---------- -------x---------- -------x---------- -------x----------

    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            amount: Uint128::from(100000_000000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom1.clone(),
            },
            amount: Uint128::from(100000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(100000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(100000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance3.clone(),
            },
            amount: Uint128::from(100000_000000u128),
        },
    ];

    // Provide liquidity to empty stable 5 pool. No fee is charged
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &VaultExecuteMsg::JoinPool {
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
                amount: Uint128::new(100000_000000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(100000_000000u128),
            },
        ],
    )
    .unwrap();

    // -------x---------- WEIGHTED-POOL -::- PROVIDE LIQUIDITY -------x----------
    // -------x---------- -------x---------- -------x---------- -------x----------

    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            amount: Uint128::from(100000_000000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom1.clone(),
            },
            amount: Uint128::from(100000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(100000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(100000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance3.clone(),
            },
            amount: Uint128::from(100000_000000u128),
        },
    ];
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &VaultExecuteMsg::JoinPool {
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
                amount: Uint128::new(100000_000000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(100000_000000u128),
            },
        ],
    )
    .unwrap();

    // -------x---------- XYK-POOL -::- PROVIDE LIQUIDITY -------x----------
    // -------x---------- -------x---------- -------x---------- -------x----

    // Provided to empty XYK Pool
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &VaultExecuteMsg::JoinPool {
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
                    amount: Uint128::from(100000_000000u128),
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: token_instance1.clone(),
                    },
                    amount: Uint128::from(100000_000000u128),
                },
            ]),
        },
        &[Coin {
            denom: denom0.clone(),
            amount: Uint128::new(100000_000000u128),
        }],
    )
    .unwrap();

    // -------x---------- Stableswap-POOL -::- PROVIDE LIQUIDITY -------x---------
    // -------x---------- -------x---------- -------x---------- -------x----------

    // Provided to empty Stable Pool
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &VaultExecuteMsg::JoinPool {
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
                    amount: Uint128::from(100000_000000u128),
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: token_instance1.clone(),
                    },
                    amount: Uint128::from(100000_000000u128),
                },
            ]),
        },
        &[Coin {
            denom: denom0.clone(),
            amount: Uint128::new(100000_000000u128),
        }],
    )
    .unwrap();

    // -------x---------- DEXTER ROUTER -::- Test swap simulations -------x-------
    // -------x---------- -------x---------- -------x---------- -------x----------

    // SWAP TYPE -::- GIVE IN {}
    // Pool ID: Uint128(4)  | asset_in: "token0" | asset_out: "contract2" | amount_provided "1000000"
    // Number of "contract2" tokens returned and are to be used for next swap: Uint128(969991)

    // Pool ID: Uint128(2)  | asset_in: "contract2" | asset_out: "contract3" | amount_provided "969991"
    // Number of "contract3" tokens returned and are to be used for next swap: Uint128(940882)

    // Pool ID: Uint128(1)  | asset_in: "contract3" | asset_out: "token0" | amount_provided "940882"
    // Number of "token0" tokens returned and are to be used for next swap: Uint128(912656)

    // Pool ID: Uint128(3)  | asset_in: "token0" | asset_out: "contract2" | amount_provided "912656"
    // Number of "contract2" tokens returned and are to be used for next swap: Uint128(885277)

    // multihop_sim_response: SimulateMultiHopResponse {
    //        swap_operations: [SimulatedTrade { pool_id: Uint128(4), asset_in: NativeToken { denom: "token0" }, offered_amount: Uint128(1000000), asset_out: Token { contract_addr: Addr("contract2") }, received_amount: Uint128(969991) },
    //                          SimulatedTrade { pool_id: Uint128(2), asset_in: Token { contract_addr: Addr("contract2") }, offered_amount: Uint128(969991), asset_out: Token { contract_addr: Addr("contract3") }, received_amount: Uint128(940882) },
    //                          SimulatedTrade { pool_id: Uint128(1), asset_in: Token { contract_addr: Addr("contract3") }, offered_amount: Uint128(940882), asset_out: NativeToken { denom: "token0" }, received_amount: Uint128(912656) },
    //                          SimulatedTrade { pool_id: Uint128(3), asset_in: NativeToken { denom: "token0" }, offered_amount: Uint128(912656), asset_out: Token { contract_addr: Addr("contract2") }, received_amount: Uint128(885277) }]
    //       , response: Success }
    let multiswap_request_msg: Vec<HopSwapRequest> = [
        HopSwapRequest {
            pool_id: Uint128::from(xyk_pool_id),
            asset_in: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            max_spread: None,
            belief_price: None,
        },
        HopSwapRequest {
            pool_id: Uint128::from(weighted_pool_id),
            asset_in: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            max_spread: None,
            belief_price: None,
        },
        HopSwapRequest {
            pool_id: Uint128::from(stable5_pool_id),
            asset_in: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            asset_out: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            max_spread: None,
            belief_price: None,
        },
        HopSwapRequest {
            pool_id: Uint128::from(stable_pool_id),
            asset_in: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            max_spread: None,
            belief_price: None,
        },
    ]
    .to_vec();
    let multihop_sim_response: SimulateMultiHopResponse = app
        .wrap()
        .query_wasm_smart(
            &router_instance.clone(),
            &dexter::router::QueryMsg::SimulateMultihopSwap {
                multiswap_request: multiswap_request_msg.clone(),
                swap_type: SwapType::GiveIn {},
                amount: Uint128::from(1000000u128),
            },
        )
        .unwrap();
    assert_eq!(
        dexter::pool::ResponseType::Success {},
        multihop_sim_response.response
    );
    assert_eq!(
        vec![
            SimulatedTrade {
                pool_id: Uint128::from(4u128),
                asset_in: AssetInfo::NativeToken {
                    denom: denom0.clone()
                },
                offered_amount: Uint128::from(1000000u128),
                asset_out: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                received_amount: Uint128::from(969991u128)
            },
            SimulatedTrade {
                pool_id: Uint128::from(2u128),
                asset_in: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                offered_amount: Uint128::from(969991u128),
                asset_out: AssetInfo::Token {
                    contract_addr: token_instance2.clone()
                },
                received_amount: Uint128::from(940882u128)
            },
            SimulatedTrade {
                pool_id: Uint128::from(1u128),
                asset_in: AssetInfo::Token {
                    contract_addr: token_instance2.clone()
                },
                offered_amount: Uint128::from(940882u128),
                asset_out: AssetInfo::NativeToken {
                    denom: denom0.clone()
                },
                received_amount: Uint128::from(912656u128)
            },
            SimulatedTrade {
                pool_id: Uint128::from(3u128),
                asset_in: AssetInfo::NativeToken {
                    denom: denom0.clone()
                },
                offered_amount: Uint128::from(912656u128),
                asset_out: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                received_amount: Uint128::from(885277u128)
            }
        ],
        multihop_sim_response.swap_operations
    );
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                amount: Uint128::from(29999u128)
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance2.clone()
                },
                amount: Uint128::from(29099u128)
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: denom0.clone()
                },
                amount: Uint128::from(28226u128)
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                amount: Uint128::from(27379u128)
            }
        ],
        multihop_sim_response.fee
    );

    // SWAP TYPE -::- GIVE OUT {}
    // Pool ID: Uint128(3)  | asset_in: "token0" | asset_out: "contract2" | amount_to_receive "885277"
    // asset_in: "token0" offered_amount: Uint128(912656) || asset_out: "contract2" received_amount: Uint128(885277)
    // Number of "token0" tokens to be provided for this swap and should be returned by previous swap: Uint128(912656)

    // Pool ID: Uint128(1)  | asset_in: "contract3" | asset_out: "token0" | amount_to_receive "912656"
    // asset_in: "contract3" offered_amount: Uint128(940882) || asset_out: "token0" received_amount: Uint128(912656)
    // Number of "contract3" tokens to be provided for this swap and should be returned by previous swap: Uint128(940882)

    // Pool ID: Uint128(2)  | asset_in: "contract2" | asset_out: "contract3" | amount_to_receive "940882"
    // asset_in: "contract2" offered_amount: Uint128(969990) || asset_out: "contract3" received_amount: Uint128(940882)
    // Number of "contract2" tokens to be provided for this swap and should be returned by previous swap: Uint128(969990)

    // Pool ID: Uint128(4)  | asset_in: "token0" | asset_out: "contract2" | amount_to_receive "969990"
    // asset_in: "token0" offered_amount: Uint128(999998) || asset_out: "contract2" received_amount: Uint128(969990)
    // Number of "token0" tokens to be provided for this swap and should be returned by previous swap: Uint128(999998)

    // multihop_sim_response: SimulateMultiHopResponse { swap_operations: [ SimulatedTrade { pool_id: Uint128(4), asset_in: NativeToken { denom: "token0" }, offered_amount: Uint128(999998), asset_out: Token { contract_addr: Addr("contract2") }, received_amount: Uint128(969990) },
    //                                                                      SimulatedTrade { pool_id: Uint128(2), asset_in: Token { contract_addr: Addr("contract2") }, offered_amount: Uint128(969990), asset_out: Token { contract_addr: Addr("contract3") }, received_amount: Uint128(940882) },
    //                                                                      SimulatedTrade { pool_id: Uint128(1), asset_in: Token { contract_addr: Addr("contract3") }, offered_amount: Uint128(940882), asset_out: NativeToken { denom: "token0" }, received_amount: Uint128(912656) },
    //                                                                      SimulatedTrade { pool_id: Uint128(3), asset_in: NativeToken { denom: "token0" }, offered_amount: Uint128(912656), asset_out: Token { contract_addr: Addr("contract2") }, received_amount: Uint128(885277) }],
    //                                                  response: Success }
    let multihop_sim_response: SimulateMultiHopResponse = app
        .wrap()
        .query_wasm_smart(
            &router_instance.clone(),
            &dexter::router::QueryMsg::SimulateMultihopSwap {
                multiswap_request: multiswap_request_msg.clone(),
                swap_type: SwapType::GiveOut {},
                amount: Uint128::from(885277u128),
            },
        )
        .unwrap();
    assert_eq!(
        dexter::pool::ResponseType::Success {},
        multihop_sim_response.response
    );
    assert_eq!(
        vec![
            SimulatedTrade {
                pool_id: Uint128::from(4u128),
                asset_in: AssetInfo::NativeToken {
                    denom: denom0.clone()
                },
                offered_amount: Uint128::from(999998u128),
                asset_out: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                received_amount: Uint128::from(969990u128)
            },
            SimulatedTrade {
                pool_id: Uint128::from(2u128),
                asset_in: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                offered_amount: Uint128::from(969990u128),
                asset_out: AssetInfo::Token {
                    contract_addr: token_instance2.clone()
                },
                received_amount: Uint128::from(940882u128)
            },
            SimulatedTrade {
                pool_id: Uint128::from(1u128),
                asset_in: AssetInfo::Token {
                    contract_addr: token_instance2.clone()
                },
                offered_amount: Uint128::from(940882u128),
                asset_out: AssetInfo::NativeToken {
                    denom: denom0.clone()
                },
                received_amount: Uint128::from(912656u128)
            },
            SimulatedTrade {
                pool_id: Uint128::from(3u128),
                asset_in: AssetInfo::NativeToken {
                    denom: denom0.clone()
                },
                offered_amount: Uint128::from(912656u128),
                asset_out: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                received_amount: Uint128::from(885277u128)
            }
        ],
        multihop_sim_response.swap_operations
    );
    assert_eq!(
        vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                amount: Uint128::from(29999u128)
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance2.clone()
                },
                amount: Uint128::from(29099u128)
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: denom0.clone()
                },
                amount: Uint128::from(28226u128)
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance1.clone()
                },
                amount: Uint128::from(27379u128)
            }
        ],
        multihop_sim_response.fee
    );

    // -------x---------- DEXTER ROUTER -::- Test Multi-Hop Function ----------x----------
    // -------x---------- ---------x---------- -------------x---------- -------x----------

    let current_block = app.block_info();
    app.update_block(|b| {
        b.height += 10;
        b.time = Timestamp::from_seconds(current_block.time.seconds() + 90)
    });

    let multiswap_request_msg: Vec<HopSwapRequest> = [
        HopSwapRequest {
            pool_id: Uint128::from(xyk_pool_id),
            asset_in: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            asset_out: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            max_spread: None,
            belief_price: None,
        },
        HopSwapRequest {
            pool_id: Uint128::from(weighted_pool_id),
            asset_in: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            max_spread: None,
            belief_price: None,
        },
        HopSwapRequest {
            pool_id: Uint128::from(stable5_pool_id),
            asset_in: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance3.clone(),
            },
            max_spread: None,
            belief_price: None,
        },
    ]
    .to_vec();

    let cur_sender_offer_asset_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();
    let cur_sender_ask_asset_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance3.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();

    let cur_vault_offer_asset_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    let cur_vault_ask_asset_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance3.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();

    // Multi-Hop Swap
    // Offer token is not native. Transferring tokens to router from the user and providing allowance
    // First hop swap request: SingleSwapRequest { pool_id: Uint128(4), asset_in: Token { contract_addr: Addr("contract2") }, asset_out: NativeToken { denom: "token0" }, swap_type: GiveIn, amount: Uint128(885277), max_spread: None, belief_price: None }
    // Current ask balance (before swap): Uint128(0)

    // Current offer asset ("token0") balance (after swap): Uint128(858711)
    // Amount returned from the last hop swap: Uint128(858711)
    // Next hop swap request: SingleSwapRequest { pool_id: Uint128(2), asset_in: NativeToken { denom: "token0" }, asset_out: Token { contract_addr: Addr("contract3") }, swap_type: GiveIn, amount: Uint128(858711), max_spread: None, belief_price: None }
    // Current ask asset ("contract3") balance: Uint128(0)

    // Current offer asset ("contract3") balance (after swap): Uint128(832942)
    // Amount returned from the last hop swap: Uint128(832942)
    // Next hop swap request: SingleSwapRequest { pool_id: Uint128(1), asset_in: Token { contract_addr: Addr("contract3") }, asset_out: Token { contract_addr: Addr("contract4") }, swap_type: GiveIn, amount: Uint128(832942), max_spread: None, belief_price: None }
    // Current ask asset ("contract4") balance: Uint128(0)

    // Current offer asset ("contract4") balance (after swap): Uint128(807954)
    // Amount returned from the last hop swap: Uint128(807954)
    // Hop is over. Checking if minimum receive amount is met. Minimum receive amount: "0" Amount returned from the last hop swap: "807954"
    let multihop_swap_msg = ExecuteMsg::ExecuteMultihopSwap {
        multiswap_request: multiswap_request_msg,
        recipient: None,
        offer_amount: Uint128::from(885277u128),
        minimum_receive: None,
    };
    app.execute_contract(
        owner.clone(),
        router_instance.clone(),
        &multihop_swap_msg,
        &[],
    )
    .unwrap();

    let new_sender_offer_asset_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();
    let new_sender_ask_asset_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance3.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();

    let new_vault_offer_asset_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    let new_vault_ask_asset_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance3.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();

    assert_eq!(
        Uint128::from(807954u128),
        new_sender_ask_asset_balance.balance - cur_sender_ask_asset_balance.balance
    );
    assert_eq!(
        Uint128::from(823946u128), // Fees are also deducted
        cur_vault_ask_asset_balance.balance - new_vault_ask_asset_balance.balance
    );

    assert_eq!(
        Uint128::from(885277u128),
        cur_sender_offer_asset_balance.balance - new_sender_offer_asset_balance.balance
    );
    assert_eq!(
        Uint128::from(885277u128),
        new_vault_offer_asset_balance.balance - cur_vault_offer_asset_balance.balance
    );
}
