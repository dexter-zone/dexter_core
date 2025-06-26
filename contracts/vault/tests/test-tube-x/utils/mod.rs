#![cfg(feature = "test-tube")]
use cosmwasm_std::{to_json_binary, Addr, Coin, Uint128};
use cw20::MinterResponse;

use dexter::asset::{Asset, AssetInfo};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use std::path::PathBuf;

use dexter::vault::{
    ConfigResponse, ExecuteMsg, FeeInfo, InstantiateMsg, NativeAssetPrecisionInfo, PauseInfo,
    PoolCreationFee, PoolInfoResponse, PoolType, PoolTypeConfig, QueryMsg,
};
use dexter_stable_pool::state::StablePoolParams;
use persistence_test_tube::{Account, Module, PersistenceTestApp, SigningAccount, Wasm};

fn get_wasm_bytes(contract_name: &str) -> Vec<u8> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let wasm_path = manifest_dir
        .join("../../artifacts")
        .join(format!("{}.wasm", contract_name.replace("-", "_")));
    std::fs::read(wasm_path).unwrap()
}

pub fn mock_app(init_coins: Vec<Coin>) -> (PersistenceTestApp, SigningAccount) {
    let app = PersistenceTestApp::new();
    let signer = app
        .init_account(&init_coins)
        .expect("Default account initialization failed");

    (app, signer)
}

pub fn store_vault_code(app: &PersistenceTestApp, signer: &SigningAccount) -> u64 {
    let wasm_bytes = get_wasm_bytes("dexter_vault");
    Wasm::new(app)
        .store_code(&wasm_bytes, None, signer)
        .unwrap()
        .data
        .code_id
}

pub fn store_token_code(app: &PersistenceTestApp, signer: &SigningAccount) -> u64 {
    let wasm_bytes = get_wasm_bytes("dexter_lp_token");
    Wasm::new(app)
        .store_code(&wasm_bytes, None, signer)
        .unwrap()
        .data
        .code_id
}

pub fn store_stable5_pool_code(app: &PersistenceTestApp, signer: &SigningAccount) -> u64 {
    let wasm_bytes = get_wasm_bytes("dexter_stable_pool");
    Wasm::new(app)
        .store_code(&wasm_bytes, None, signer)
        .unwrap()
        .data
        .code_id
}

pub fn store_weighted_pool_code(app: &PersistenceTestApp, signer: &SigningAccount) -> u64 {
    let wasm_bytes = get_wasm_bytes("dexter_weighted_pool");
    Wasm::new(app)
        .store_code(&wasm_bytes, None, signer)
        .unwrap()
        .data
        .code_id
}

// Initialize a vault with StableSwap, Weighted pools
pub fn instantiate_contract(app: &PersistenceTestApp, signer: &SigningAccount) -> String {
    let wasm = Wasm::new(app);
    let weighted_pool_code_id = store_weighted_pool_code(app, signer);
    let stable5_pool_code_id = store_stable5_pool_code(app, signer);

    let vault_code_id = store_vault_code(app, signer);
    let token_code_id = store_token_code(app, signer);

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
        owner: signer.address(),
        auto_stake_impl: dexter::vault::AutoStakeImpl::None,
        pool_creation_fee: PoolCreationFee::default(),
    };

    let vault_instance = wasm
        .instantiate(
            vault_code_id,
            &vault_init_msg,
            None,
            Some("vault"),
            &[],
            signer,
        )
        .unwrap()
        .data
        .address;

    vault_instance
}

pub fn store_multistaking_code(app: &PersistenceTestApp, signer: &SigningAccount) -> u64 {
    let wasm_bytes = get_wasm_bytes("dexter_multi_staking");
    Wasm::new(app)
        .store_code(&wasm_bytes, None, signer)
        .unwrap()
        .data
        .code_id
}

pub fn initialize_multistaking_contract(
    app: &PersistenceTestApp,
    signer: &SigningAccount,
) -> String {
    let wasm = Wasm::new(app);
    let multistaking_code_id = store_multistaking_code(app, signer);

    let keeper = app.init_account(&[]).unwrap();

    let multistaking_init_msg = dexter::multi_staking::InstantiateMsg {
        owner: Addr::unchecked(signer.address()),
        keeper_addr: Addr::unchecked(keeper.address()),
        unbond_config: dexter::multi_staking::UnbondConfig {
            instant_unbond_config: dexter::multi_staking::InstantUnbondConfig::Enabled {
                min_fee: 200u64,
                max_fee: 500u64,
                fee_tier_interval: 86400u64,
            },
            unlock_period: 86400u64,
        },
    };

    let multistaking_instance = wasm
        .instantiate(
            multistaking_code_id,
            &multistaking_init_msg,
            None,
            Some("multistaking"),
            &[],
            signer,
        )
        .unwrap()
        .data
        .address;

    multistaking_instance
}

pub fn initialize_3_tokens(
    app: &PersistenceTestApp,
    signer: &SigningAccount,
) -> (String, String, String) {
    let wasm = Wasm::new(app);
    let token_code_id = store_token_code(app, signer);

    let token_instance0 = wasm
        .instantiate(
            token_code_id,
            &TokenInstantiateMsg {
                name: "x_token".to_string(),
                symbol: "X-Tok".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: signer.address(),
                    cap: None,
                }),
                marketing: None,
            },
            None,
            Some("x_token"),
            &[],
            signer,
        )
        .unwrap()
        .data
        .address;
    let token_instance2 = wasm
        .instantiate(
            token_code_id,
            &TokenInstantiateMsg {
                name: "y_token".to_string(),
                symbol: "y-Tok".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: signer.address(),
                    cap: None,
                }),
                marketing: None,
            },
            None,
            Some("y_token"),
            &[],
            signer,
        )
        .unwrap()
        .data
        .address;
    let token_instance3 = wasm
        .instantiate(
            token_code_id,
            &TokenInstantiateMsg {
                name: "z_token".to_string(),
                symbol: "z-Tok".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: signer.address(),
                    cap: None,
                }),
                marketing: None,
            },
            None,
            Some("z_token"),
            &[],
            signer,
        )
        .unwrap()
        .data
        .address;
    (token_instance0, token_instance2, token_instance3)
}

// Mints some Tokens to "to" recipient
pub fn mint_some_tokens(
    app: &PersistenceTestApp,
    signer: &SigningAccount,
    token_instance: &str,
    amount: Uint128,
    to: String,
) {
    let wasm = Wasm::new(app);
    let msg = cw20::Cw20ExecuteMsg::Mint {
        recipient: to,
        amount,
    };
    wasm.execute(token_instance, &msg, &[], signer).unwrap();
}

// increase token allowance
pub fn increase_token_allowance(
    app: &PersistenceTestApp,
    signer: &SigningAccount,
    token_instance: &str,
    spender: String,
    amount: Uint128,
) {
    let wasm = Wasm::new(app);
    let msg = cw20::Cw20ExecuteMsg::IncreaseAllowance {
        spender,
        amount,
        expires: None,
    };
    wasm.execute(token_instance, &msg, &[], signer).unwrap();
}

pub fn dummy_pool_creation_msg(asset_infos: &[AssetInfo]) -> ExecuteMsg {
    ExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Weighted {},
        asset_infos: asset_infos.to_vec(),
        native_asset_precisions: vec![],
        init_params: Some(
            to_json_binary(&dexter_weighted_pool::state::WeightedParams {
                weights: asset_infos
                    .iter()
                    .map(|w| Asset {
                        info: w.clone(),
                        amount: Uint128::from(1u128),
                    })
                    .collect(),
                exit_fee: None,
            })
            .unwrap(),
        ),
        fee_info: None,
    }
}

pub fn initialize_stable_5_pool_2_asset(
    app: &PersistenceTestApp,
    signer: &SigningAccount,
    vault_instance: &str,
    token_instance0: String,
    denom0: String,
) -> (String, String, Uint128) {
    let wasm = Wasm::new(app);

    // Get the current pool count to determine the next pool ID
    let config: ConfigResponse = wasm.query(vault_instance, &QueryMsg::Config {}).unwrap();
    let next_pool_id = config.next_pool_id;

    let create_pool_msg = ExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::StableSwap {},
        asset_infos: vec![
            AssetInfo::Token {
                contract_addr: Addr::unchecked(token_instance0),
            },
            AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
        ],
        native_asset_precisions: vec![NativeAssetPrecisionInfo {
            denom: denom0,
            precision: 6,
        }],
        init_params: Some(
            to_json_binary(&StablePoolParams {
                amp: 10,
                supports_scaling_factors_update: false,
                scaling_factors: vec![],
                scaling_factor_manager: None,
            })
            .unwrap(),
        ),
        fee_info: None,
    };

    let _res = wasm
        .execute(vault_instance, &create_pool_msg, &[], signer)
        .unwrap();

    // Query the pool info directly using the next_pool_id
    let res: PoolInfoResponse = wasm
        .query(
            vault_instance,
            &QueryMsg::GetPoolById {
                pool_id: next_pool_id,
            },
        )
        .unwrap();

    (
        res.pool_addr.to_string(),
        res.lp_token_addr.to_string(),
        res.pool_id,
    )
}

pub fn initialize_stable_5_pool(
    app: &PersistenceTestApp,
    signer: &SigningAccount,
    vault_instance: &str,
    token_instance0: String,
    token_instance1: String,
    token_instance2: String,
    denom0: String,
    denom1: String,
) -> (String, String, Uint128) {
    let wasm = Wasm::new(app);
    let create_pool_msg = ExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::StableSwap {},
        asset_infos: vec![
            AssetInfo::Token {
                contract_addr: Addr::unchecked(token_instance0),
            },
            AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked(token_instance1),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked(token_instance2),
            },
            AssetInfo::NativeToken {
                denom: denom1.clone(),
            },
        ],
        native_asset_precisions: vec![
            NativeAssetPrecisionInfo {
                denom: denom0,
                precision: 6,
            },
            NativeAssetPrecisionInfo {
                denom: denom1,
                precision: 6,
            },
        ],
        init_params: Some(
            to_json_binary(&StablePoolParams {
                amp: 10,
                supports_scaling_factors_update: false,
                scaling_factors: vec![],
                scaling_factor_manager: None,
            })
            .unwrap(),
        ),
        fee_info: None,
    };

    // Get the current pool count to determine the next pool ID
    let config: ConfigResponse = wasm.query(vault_instance, &QueryMsg::Config {}).unwrap();
    let next_pool_id = config.next_pool_id;

    let _res = wasm
        .execute(vault_instance, &create_pool_msg, &[], signer)
        .unwrap();

    // Query the pool info directly using the next_pool_id
    let res: PoolInfoResponse = wasm
        .query(
            vault_instance,
            &QueryMsg::GetPoolById {
                pool_id: next_pool_id,
            },
        )
        .unwrap();

    (
        res.pool_addr.to_string(),
        res.lp_token_addr.to_string(),
        res.pool_id,
    )
}

pub fn initialize_weighted_pool(
    app: &PersistenceTestApp,
    signer: &SigningAccount,
    vault_instance: &str,
    token_instance0: String,
    token_instance1: String,
    token_instance2: String,
    denom0: String,
    denom1: String,
) -> (String, String, Uint128) {
    let wasm = Wasm::new(app);

    let mut asset_infos = vec![
        AssetInfo::NativeToken {
            denom: denom0.clone(),
        },
        AssetInfo::NativeToken {
            denom: denom1.clone(),
        },
        AssetInfo::Token {
            contract_addr: Addr::unchecked(token_instance1),
        },
        AssetInfo::Token {
            contract_addr: Addr::unchecked(token_instance0),
        },
        AssetInfo::Token {
            contract_addr: Addr::unchecked(token_instance2),
        },
    ];
    asset_infos.sort();

    // Get the current pool count to determine the next pool ID
    let config: ConfigResponse = wasm.query(vault_instance, &QueryMsg::Config {}).unwrap();
    let next_pool_id = config.next_pool_id;

    let create_pool_msg = ExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Weighted {},
        asset_infos: asset_infos.clone(),
        native_asset_precisions: vec![
            NativeAssetPrecisionInfo {
                denom: denom0,
                precision: 6,
            },
            NativeAssetPrecisionInfo {
                denom: denom1,
                precision: 6,
            },
        ],
        init_params: Some(
            to_json_binary(&dexter_weighted_pool::state::WeightedParams {
                weights: asset_infos
                    .iter()
                    .map(|w| Asset {
                        info: w.clone(),
                        amount: Uint128::from(1u128),
                    })
                    .collect(),
                exit_fee: None,
            })
            .unwrap(),
        ),
        fee_info: None,
    };

    let _res = wasm
        .execute(vault_instance, &create_pool_msg, &[], signer)
        .unwrap();

    // Query the pool info directly using the next_pool_id
    let pool_info: PoolInfoResponse = wasm
        .query(
            vault_instance,
            &QueryMsg::GetPoolById {
                pool_id: next_pool_id,
            },
        )
        .unwrap();

    (
        pool_info.pool_addr.to_string(),
        pool_info.lp_token_addr.to_string(),
        pool_info.pool_id,
    )
}

// Function to update vault config with keeper address
pub fn set_keeper_contract_in_config(
    app: &PersistenceTestApp,
    signer: &SigningAccount,
    vault_addr: &str,
    keeper_addr: &str,
) {
    let wasm = Wasm::new(app);
    let msg = ExecuteMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: None,
        pool_creation_fee: None,
        auto_stake_impl: Some(dexter::vault::AutoStakeImpl::Multistaking {
            contract_addr: Addr::unchecked(keeper_addr.to_string()),
        }),
        paused: None,
    };
    wasm.execute(vault_addr, &msg, &[], signer).unwrap();
}

pub fn query_vault_config(app: &PersistenceTestApp, vault_addr: &str) -> ConfigResponse {
    let wasm = Wasm::new(app);
    wasm.query(vault_addr, &QueryMsg::Config {}).unwrap()
}
