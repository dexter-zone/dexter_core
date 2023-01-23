use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{to_binary, Addr, Coin, Decimal, Timestamp, Uint128, BlockInfo, Uint64};
use cw20::MinterResponse;
use cw_multi_test::{App, ContractWrapper, Executor};
use dexter::asset::{Asset, AssetInfo};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;

use dexter::vault::{
    ConfigResponse, ExecuteMsg, FeeInfo, InstantiateMsg, PoolInfoResponse, PoolType,
    PoolTypeConfig, QueryMsg, PoolCreationFee, PauseInfo,
};

const EPOCH_START: u64 = 1_000_000;

pub fn mock_app(owner: Addr, coins: Vec<Coin>) -> App {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(EPOCH_START);

    let mut app = App::new(|router, _, storage| {
        // initialization  moved to App construction
        router.bank.init_balance(storage, &owner, coins).unwrap();
    });
    app.set_block(env.block);
    app
}

pub fn store_vault_code(app: &mut App) -> u64 {
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

pub fn store_token_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        lp_token::contract::execute,
        lp_token::contract::instantiate,
        lp_token::contract::query,
    ));
    app.store_code(token_contract)
}

pub fn store_stable5_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(ContractWrapper::new_with_empty(
        stable5pool::contract::execute,
        stable5pool::contract::instantiate,
        stable5pool::contract::query,
    ));
    app.store_code(pool_contract)
}

pub fn store_weighted_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(ContractWrapper::new_with_empty(
        weighted_pool::contract::execute,
        weighted_pool::contract::instantiate,
        weighted_pool::contract::query,
    ));
    app.store_code(pool_contract)
}

pub fn store_stable_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(ContractWrapper::new_with_empty(
        stableswap_pool::contract::execute,
        stableswap_pool::contract::instantiate,
        stableswap_pool::contract::query,
    ));
    app.store_code(pool_contract)
}

pub fn store_xyk_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(ContractWrapper::new_with_empty(
        xyk_pool::contract::execute,
        xyk_pool::contract::instantiate,
        xyk_pool::contract::query,
    ));
    app.store_code(pool_contract)
}

// Initialize a vault with XYK, Stable, Stable5, Weighted pools
pub fn instantiate_contract(app: &mut App, owner: &Addr) -> Addr {
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
                protocol_fee_percent: 64u16,
                dev_fee_percent: 0u16,
                developer_addr: Some(Addr::unchecked(&"xyk_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
            paused: PauseInfo::default(),
        },
        PoolTypeConfig {
            code_id: stable_pool_code_id,
            pool_type: PoolType::Stable2Pool {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 64u16,
                dev_fee_percent: 0u16,
                developer_addr: Some(Addr::unchecked(&"stable_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
            paused: PauseInfo::default(),
        },
        PoolTypeConfig {
            code_id: weighted_pool_code_id,
            pool_type: PoolType::Weighted {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 64u16,
                dev_fee_percent: 0u16,
                developer_addr: Some(Addr::unchecked(&"weighted_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
            paused: PauseInfo::default(),
        },
        PoolTypeConfig {
            code_id: stable5_pool_code_id,
            pool_type: PoolType::Stable5Pool {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 64u16,
                dev_fee_percent: 0u16,
                developer_addr: Some(Addr::unchecked(&"stable5_dev".to_string())),
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            is_generator_disabled: false,
            paused: PauseInfo::default(),
        },
    ];

    let vault_init_msg = InstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: Some(token_code_id),
        fee_collector: None,
        owner: owner.to_string(),
        auto_stake_impl: dexter::vault::AutoStakeImpl::None,
        pool_creation_fee: PoolCreationFee::default(),
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

pub fn store_generator_code(app: &mut App) -> u64 {
    let generator_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_generator::contract::execute,
        dexter_generator::contract::instantiate,
        dexter_generator::contract::query,
    ));
    app.store_code(generator_contract)
}

pub fn store_multistaking_code(app: &mut App) -> u64 {
    let multistaking_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_multi_staking::contract::execute,
        dexter_multi_staking::contract::instantiate,
        dexter_multi_staking::contract::query,
    ));
    app.store_code(multistaking_contract)
}

pub fn initialize_generator_contract(
    app: &mut App,
    owner: &Addr,
    vault: &Addr,
    current_block: BlockInfo,
) -> Addr {
    let generator_code_id = store_generator_code(app);

    let generator_init_msg = dexter::generator::InstantiateMsg {
        owner: owner.to_string(),
        vault: vault.clone().to_string(),
        dex_token: None,
        tokens_per_block: Uint128::zero(),
        start_block: Uint64::from(current_block.height),
        unbonding_period: 8640u64,
    };

    let generator_instance = app
        .instantiate_contract(
            generator_code_id,
            owner.to_owned(),
            &generator_init_msg,
            &[],
            "generator",
            None,
        )
        .unwrap();

    return generator_instance;
}

pub fn initialize_multistaking_contract(
    app: &mut App,
    owner: &Addr,
) -> Addr {
    let multistaking_code_id = store_multistaking_code(app);

    let multistaking_init_msg = dexter::multi_staking::InstantiateMsg {
        owner: owner.clone(),
        unlock_period: 86400u64,
    };

    let multistaking_instance = app
        .instantiate_contract(
            multistaking_code_id,
            owner.to_owned(),
            &multistaking_init_msg,
            &[],
            "multistaking",
            None,
        )
        .unwrap();

    return multistaking_instance;
}

pub fn initialize_3_tokens(app: &mut App, owner: &Addr) -> (Addr, Addr, Addr) {
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
pub fn mint_some_tokens(
    app: &mut App,
    owner: Addr,
    token_instance: Addr,
    amount: Uint128,
    to: String,
) {
    let msg = cw20::Cw20ExecuteMsg::Mint {
        recipient: to.clone(),
        amount: amount,
    };
    app.execute_contract(owner.clone(), token_instance.clone(), &msg, &[])
        .unwrap();
}

// increase token allowance
pub fn increase_token_allowance(
    app: &mut App,
    owner: Addr,
    token_instance: Addr,
    spender: String,
    amount: Uint128,
) {
    let msg = cw20::Cw20ExecuteMsg::IncreaseAllowance {
        spender: spender.clone(),
        amount: amount,
        expires: None,
    };
    app.execute_contract(owner.clone(), token_instance.clone(), &msg, &[])
        .unwrap();
}

pub fn dummy_pool_creation_msg(asset_infos: &[AssetInfo]) -> ExecuteMsg {
    ExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Xyk {},
        asset_infos: asset_infos.to_vec(),
        init_params: None,
        fee_info: Some(FeeInfo {
            total_fee_bps: 1_000u16,
            protocol_fee_percent: 49u16,
            dev_fee_percent: 0u16,
            developer_addr: None,
        }),
    }
}

/// Initialize a STABLE-5-POOL
/// --------------------------
pub fn initialize_stable_5_pool(
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
        fee_info: None,
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

    let pool_addr = pool_info_res.pool_addr;
    let lp_token_addr = pool_info_res.lp_token_addr;
    let pool_id = pool_info_res.pool_id;

    return (pool_addr, lp_token_addr, pool_id);
}

/// Initialize a WEIGHTED POOL
/// --------------------------
pub fn initialize_weighted_pool(
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
        fee_info: None,
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

    let pool_addr = pool_info_res.pool_addr;
    let lp_token_addr = pool_info_res.lp_token_addr;
    let pool_id = pool_info_res.pool_id;

    return (pool_addr, lp_token_addr, pool_id);
}

/// Initialize a STABLE POOL
/// --------------------------
pub fn initialize_stable_pool(
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
        fee_info: None,
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

    let pool_addr = pool_info_res.pool_addr;
    let lp_token_addr = pool_info_res.lp_token_addr;
    let pool_id = pool_info_res.pool_id;

    return (pool_addr, lp_token_addr, pool_id);
}

/// Initialize a XYK POOL
/// --------------------------
pub fn initialize_xyk_pool(
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
        fee_info: None,
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

    let pool_addr = pool_info_res.pool_addr;
    let lp_token_addr = pool_info_res.lp_token_addr;
    let pool_id = pool_info_res.pool_id;

    return (pool_addr, lp_token_addr, pool_id);
}


pub fn set_keeper_contract_in_config(app: &mut App, owner: Addr, vault_addr: Addr) {
    let msg = ExecuteMsg::UpdateConfig { 
        lp_token_code_id: None,
        fee_collector: Some("fee_collector".to_string()),
        pool_creation_fee: None,
        auto_stake_impl: None,
        paused: None 
    };

    app.execute_contract(
        owner,
        vault_addr,
        &msg,
        &[],
    ).unwrap();
}