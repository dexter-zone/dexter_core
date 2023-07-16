use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{to_binary, Addr, Coin, Decimal, Timestamp, Uint128};
use cw20::MinterResponse;
use cw_multi_test::{App, ContractWrapper, Executor};
use dexter::asset::{Asset, AssetInfo};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;

use dexter::vault::{
    ConfigResponse, ExecuteMsg, FeeInfo, InstantiateMsg, PoolInfoResponse, PoolType,
    PoolTypeConfig, QueryMsg, PoolCreationFee, PauseInfo, NativeAssetPrecisionInfo,
};
use stable_pool::state::StablePoolParams;

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
        stable_pool::contract::execute,
        stable_pool::contract::instantiate,
        stable_pool::contract::query,
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

// Initialize a vault with StableSwap, Weighted pools
pub fn instantiate_contract(app: &mut App, owner: &Addr) -> Addr {
    let weighted_pool_code_id = store_weighted_pool_code(app);
    let stable5_pool_code_id = store_stable5_pool_code(app);

    let vault_code_id = store_vault_code(app);
    let token_code_id = store_token_code(app);

    let pool_configs = vec![
        PoolTypeConfig {
            code_id: weighted_pool_code_id,
            pool_type: PoolType::Weighted {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 64u16,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            paused: PauseInfo::default(),
        },
        PoolTypeConfig {
            code_id: stable5_pool_code_id,
            pool_type: PoolType::StableSwap {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 64u16,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
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

pub fn store_multistaking_code(app: &mut App) -> u64 {
    let multistaking_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_multi_staking::contract::execute,
        dexter_multi_staking::contract::instantiate,
        dexter_multi_staking::contract::query,
    ));
    app.store_code(multistaking_contract)
}

pub fn initialize_multistaking_contract(
    app: &mut App,
    owner: &Addr,
    keeper_addr: &Addr,
) -> Addr {
    let multistaking_code_id = store_multistaking_code(app);

    let multistaking_init_msg = dexter::multi_staking::InstantiateMsg {
        owner: owner.clone(),
        unlock_period: 86400u64,
        keeper_addr: keeper_addr.clone(),
        minimum_reward_schedule_proposal_start_delay: 3 * 24 * 60 * 60,
        instant_unbond_fee_bp: 500u64,
        instant_unbond_min_fee_bp: 200u64,
        fee_tier_interval: 86400u64,
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
        pool_type: PoolType::StableSwap {},
        asset_infos: asset_infos.to_vec(),
        native_asset_precisions: vec![],
        init_params: Some(to_binary(&StablePoolParams { 
            amp: 100u64,
            scaling_factor_manager: None,
            scaling_factors: vec![],
            supports_scaling_factors_update: false,
            max_allowed_spread: Decimal::from_ratio(50u64, 100u64)
         }).unwrap()),
        fee_info: Some(FeeInfo {
            total_fee_bps: 1_000u16,
            protocol_fee_percent: 49u16,
        }),
    }
}

/// Initialize a STABLE-5-POOL with 2 tokens
/// --------------------------
pub fn initialize_stable_5_pool_2_asset(
    app: &mut App,
    owner: &Addr,
    vault_instance: Addr,
    token_instance0: Addr,
    denom0: String,
) -> (Addr, Addr, Uint128) {
    let asset_infos = vec![
        AssetInfo::NativeToken { denom: denom0.clone() },
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
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
        pool_type: PoolType::StableSwap {},
        asset_infos: asset_infos.to_vec(),
        native_asset_precisions: vec![NativeAssetPrecisionInfo {
            denom: denom0.clone(),
            precision: 6u8,
        }],
        init_params: Some(to_binary(&StablePoolParams {
            amp: 10u64,
            scaling_factor_manager: None,
            scaling_factors: vec![],
            supports_scaling_factors_update: false,
            max_allowed_spread: Decimal::from_ratio(50u64, 100u64)
        }).unwrap()),
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
        AssetInfo::NativeToken { denom: denom0.clone() },
        AssetInfo::NativeToken { denom: denom1.clone() },
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
        pool_type: PoolType::StableSwap {},
        asset_infos: asset_infos.to_vec(),
        native_asset_precisions: vec![
            NativeAssetPrecisionInfo {
                denom: denom0.clone(),
                precision: 6u8,
            },
            NativeAssetPrecisionInfo {
                denom: denom1.clone(),
                precision: 6u8,
            },],
        init_params: Some(to_binary(&StablePoolParams {
            amp: 10u64,
            scaling_factor_manager: None,
            scaling_factors: vec![],
            supports_scaling_factors_update: false,
            max_allowed_spread: Decimal::from_ratio(50u64, 100u64)
        }).unwrap()),
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
            info: AssetInfo::NativeToken { denom: denom0.clone() },
            amount: Uint128::from(20u128),
        },
        Asset {
            info: AssetInfo::NativeToken { denom: denom1.clone() },
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
        native_asset_precisions: vec![
            NativeAssetPrecisionInfo {
                denom: denom0.clone(),
                precision: 6u8,
            },
            NativeAssetPrecisionInfo {
                denom: denom1.clone(),
                precision: 6u8,
            },],
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