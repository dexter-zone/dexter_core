use std::collections::HashMap;

use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{attr, to_binary, Addr, Coin, Decimal256, Timestamp, Uint128, Decimal};
use cw20::MinterResponse;
use cw_multi_test::{App, ContractWrapper, Executor};

use dexter::asset::{Asset, AssetInfo, AssetExchangeRate};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{ConfigResponse, FeeStructs, QueryMsg, SwapResponse, CumulativePricesResponse};
use dexter::vault::{
    ExecuteMsg as VaultExecuteMsg, FeeInfo, InstantiateMsg as VaultInstantiateMsg, PauseInfo,
    PoolCreationFee, PoolInfo, PoolType, PoolTypeConfig, QueryMsg as VaultQueryMsg, SwapType, SingleSwapRequest,
};

use cw20::Cw20ExecuteMsg;

use itertools::Itertools;
use stable5pool::state::{AssetScalingFactor, MathConfig, StablePoolParams};

pub const EPOCH_START: u64 = 1_000_000;

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

pub fn store_stable_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(ContractWrapper::new_with_empty(
        stable5pool::contract::execute,
        stable5pool::contract::instantiate,
        stable5pool::contract::query,
    ));
    app.store_code(pool_contract)
}

pub fn store_token_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        lp_token::contract::execute,
        lp_token::contract::instantiate,
        lp_token::contract::query,
    ));
    app.store_code(token_contract)
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
    let res = app
        .execute_contract(owner.clone(), token_instance.clone(), &msg, &[])
        .unwrap();
    assert_eq!(res.events[1].attributes[1], attr("action", "mint"));
    assert_eq!(res.events[1].attributes[2], attr("to", to));
    assert_eq!(res.events[1].attributes[3], attr("amount", amount));
}

pub fn instantiate_contract_generic(
    app: &mut App,
    owner: &Addr,
    fee_info: FeeInfo,
    asset_infos: Vec<AssetInfo>,
    native_asset_precisions: Vec<(String, u8)>,
    scaling_factors: Vec<AssetScalingFactor>,
    amp: u64,
) -> (Addr, Addr, Addr, u128) {
    let stable5pool_code_id = store_stable_pool_code(app);
    let vault_code_id = store_vault_code(app);
    let token_code_id = store_token_code(app);

    let pool_configs = vec![PoolTypeConfig {
        code_id: stable5pool_code_id,
        pool_type: PoolType::Stable5Pool {},
        default_fee_info: fee_info.clone(),
        allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
        paused: PauseInfo::default(),
    }];

    let vault_init_msg = VaultInstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: Some(token_code_id),
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        pool_creation_fee: PoolCreationFee::default(),
        auto_stake_impl: dexter::vault::AutoStakeImpl::None,
    };

    // Initialize Vault contract instance
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

    let msg = VaultExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Stable5Pool {},
        asset_infos: asset_infos.to_vec(),
        native_asset_precisions: native_asset_precisions.clone(),
        init_params: Some(
            to_binary(&StablePoolParams {
                amp,
                scaling_factors,
                supports_scaling_factors_update: false,
                scaling_factor_manager: None,
            })
            .unwrap(),
        ),
        fee_info: None,
    };
    let res = app
        .execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

    assert_eq!(
        res.events[1].attributes[2],
        attr("pool_type", "stable-5-pool")
    );
    let pool_res: PoolInfo = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &VaultQueryMsg::GetPoolById {
                pool_id: Uint128::from(1u128),
            },
        )
        .unwrap();

    assert_eq!(Uint128::from(1u128), pool_res.pool_id);
    assert_eq!(PoolType::Stable5Pool {}, pool_res.pool_type);

    let current_block = app.block_info();

    let assets = asset_infos
        .iter()
        .map(|a| Asset {
            info: a.clone(),
            amount: Uint128::zero(),
        })
        .collect_vec();

    let pool_config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(pool_res.pool_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    assert_eq!(assets, pool_config_res.assets);
    assert_eq!(
        FeeStructs {
            total_fee_bps: fee_info.total_fee_bps
        },
        pool_config_res.fee_info
    );

    assert_eq!(PoolType::Stable5Pool {}, pool_config_res.pool_type);
    assert_eq!(
        current_block.time.seconds(),
        pool_config_res.block_time_last
    );

    // Find max of native_asset_precisions
    let mut max_precision = 0u8;
    for (_, precision) in native_asset_precisions {
        if precision > max_precision {
            max_precision = precision;
        }
    }

    let math_config_binary = to_binary(&MathConfig {
        init_amp: amp * 100,
        init_amp_time: EPOCH_START,
        next_amp: amp * 100,
        next_amp_time: EPOCH_START,
        greatest_precision: max_precision,
    }).unwrap();

    assert_eq!(
        Some(math_config_binary),
        pool_config_res.math_params
    );

    let pool_id_res: Uint128 = app
        .wrap()
        .query_wasm_smart(pool_res.pool_addr.clone(), &QueryMsg::PoolId {})
        .unwrap();
    assert_eq!(Uint128::from(1u128), pool_id_res);

    return (
        vault_instance,
        pool_res.pool_addr,
        pool_res.lp_token_addr,
        current_block.time.seconds() as u128,
    );
}

pub fn instantiate_contracts_scaling_factor(
    app: &mut App,
    owner: &Addr,
    native_asset_precisions: Vec<(String, u8)>,
) -> (Addr, Addr, Addr, u128) {
    let asset_infos = vec![
        AssetInfo::NativeToken {
            denom: "uatom".to_string(),
        },
        AssetInfo::NativeToken {
            denom: "ustkatom".to_string(),
        },
    ];

    let scaling_factors = vec![
        AssetScalingFactor {
            asset_info: AssetInfo::NativeToken {
                denom: "uatom".to_string(),
            },
            scaling_factor: Decimal256::one(),
        },
        AssetScalingFactor {
            asset_info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            scaling_factor: Decimal256::from_ratio(98u128, 100u128),
        },
    ];

    let fee_info = FeeInfo {
        total_fee_bps: 30,
        protocol_fee_percent: 20,
    };

    let (vault_addr, pool_addr, lp_token, current_block_time) =
        instantiate_contract_generic(app, owner, fee_info, asset_infos, native_asset_precisions, scaling_factors, 100);

    return (vault_addr, pool_addr, lp_token, current_block_time);
}

pub fn instantiate_contracts_instance(
    app: &mut App,
    owner: &Addr,
) -> (Addr, Addr, Addr, Addr, Addr, u128) {
    let token_code_id = store_token_code(app);

    // Create Token X
    let init_msg = TokenInstantiateMsg {
        name: "x_token".to_string(),
        symbol: "X-Tok".to_string(),
        decimals: 6,
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
        decimals: 6,
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
        AssetInfo::NativeToken {
            denom: "axlusd".to_string(),
        },
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
        AssetInfo::Token {
            contract_addr: token_instance1.clone(),
        },
    ];

    let native_asset_precisions = vec![("axlusd".to_string(), 6)];

    let fee_info = FeeInfo {
        total_fee_bps: 300,
        protocol_fee_percent: 64,
    };

    let (vault_instance, pool_addr, lp_token_addr, current_block_time) =
        instantiate_contract_generic(app, owner, fee_info, asset_infos, native_asset_precisions, vec![], 10);

    return (
        vault_instance,
        pool_addr,
        lp_token_addr,
        token_instance0,
        token_instance1,
        current_block_time,
    );
}


pub fn add_liquidity_to_pool(
    app: &mut App,
    owner: &Addr,
    user: &Addr,
    vault_addr: Addr,
    pool_id: Uint128,
    assets_with_bootstrapping_amount: Vec<Asset>
) {
    
    // Find CW20 assets from the bootstrapping amount and mint token to the user
    let cw20_assets = assets_with_bootstrapping_amount
        .iter()
        .filter(|a| !a.info.is_native_token())
        .map(|a| a.info.clone())
        .collect_vec();

    // Step 1: Mint CW20 tokens to the user
    for asset in &cw20_assets {
        let mint_msg = Cw20ExecuteMsg::Mint {
            recipient: user.to_string(),
            amount: Uint128::from(1_000_000_000_000_000_000u128),
        };
        let contract_address = asset.to_string();
        app.execute_contract(
            owner.clone(),
            Addr::unchecked(contract_address),
            &mint_msg,
            &[],
        )
        .unwrap();
    }

    // Step 2: Add allowance for the pool to spend the user's tokens
    for asset in &cw20_assets {
        let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_addr.to_string(),
            amount: Uint128::from(1_000_000_000_000_000_000u128),
            expires: None,
        };
        let contract_address = asset.to_string();
        app.execute_contract(
            user.clone(),
            Addr::unchecked(contract_address),
            &allowance_msg,
            &[],
        )
        .unwrap();
    }

    // Step 3: Create coins vec for native tokens to be sent for joining pool
    let native_token = assets_with_bootstrapping_amount
        .iter()
        .filter(|a| a.info.is_native_token())
        .collect_vec();

    let mut coins = vec![];
    for asset in native_token {
        let denom = asset.info.to_string();
        coins.push(Coin {
            denom,
            amount: asset.amount,
        });
    }

    // Step 4: Execute join pool
    let msg = VaultExecuteMsg::JoinPool {
        pool_id,
        recipient: None,
        lp_to_mint: None,
        auto_stake: None,
        slippage_tolerance: None,
        assets: Some(assets_with_bootstrapping_amount),
    };

    app
        .execute_contract(
            user.clone(),
            vault_addr.clone(),
            &msg,
            &coins,
        )
        .unwrap();

}

pub fn perform_and_test_swap_give_in(
    app: &mut App,
    _owner: &Addr,
    user: &Addr,
    vault_addr: Addr,
    pool_addr: Addr,
    pool_id: Uint128,
    asset_in: Asset,
    asset_out: AssetInfo,
    max_spread: Option<Decimal>,
    expected_asset_out: Uint128,
    expected_spread: Uint128,
    expected_fee: Asset
) {
    let swap_query_msg = QueryMsg::OnSwap { 
        swap_type: SwapType::GiveIn {}, 
        offer_asset: asset_in.info.clone(), 
        ask_asset: asset_out.clone(),
        amount: asset_in.amount,
        max_spread,
        belief_price: None
    };

    let swap_query_res: SwapResponse = app
        .wrap()
        .query_wasm_smart(pool_addr.clone(), &swap_query_msg)
        .unwrap();

    assert_eq!(swap_query_res.trade_params.amount_out, expected_asset_out);
    assert_eq!(swap_query_res.trade_params.spread, expected_spread);
    assert_eq!(swap_query_res.trade_params.amount_in, asset_in.amount);
    assert_eq!(swap_query_res.fee, Some(expected_fee));

    // If asset in is a CW20 approve the vault to spend the token
    if !asset_in.info.is_native_token() {
        let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_addr.to_string(),
            amount: Uint128::from(asset_in.amount),
            expires: None,
        };
        let contract_address = asset_in.info.to_string();
        app.execute_contract(
            user.clone(),
            Addr::unchecked(contract_address),
            &allowance_msg,
            &[],
        )
        .unwrap();
    }

    let coins = if asset_in.info.is_native_token() {
        vec![Coin {
            denom: asset_in.info.to_string(),
            amount: asset_in.amount,
        }]
    } else {
        vec![]
    };

    let swap_msg = VaultExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(pool_id),
            swap_type: SwapType::GiveIn {},
            asset_in: asset_in.info,
            asset_out,
            amount: asset_in.amount,
            max_spread,
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };

    app
        .execute_contract(
            user.clone(),
            vault_addr.clone(),
            &swap_msg,
            &coins,
        )
        .unwrap();
}

pub fn perform_and_test_swap_give_out(
    app: &mut App,
    _owner: &Addr,
    user: &Addr,
    vault_addr: Addr,
    pool_addr: Addr,
    pool_id: Uint128,
    asset_out: Asset,
    asset_in: AssetInfo,
    max_spread: Option<Decimal>,
    expected_asset_in: Uint128,
    expected_spread: Uint128,
    expected_fee: Asset
) {

    let swap_query_msg = QueryMsg::OnSwap { 
        swap_type: SwapType::GiveOut {}, 
        offer_asset: asset_in.clone(), 
        ask_asset: asset_out.info.clone(),
        amount: asset_out.amount,
        max_spread,
        belief_price: None
    };

    let swap_query_res: SwapResponse = app
        .wrap()
        .query_wasm_smart(pool_addr.clone(), &swap_query_msg)
        .unwrap();

    assert_eq!(swap_query_res.trade_params.amount_out, asset_out.amount);
    assert_eq!(swap_query_res.trade_params.amount_in, expected_asset_in);
    assert_eq!(swap_query_res.fee, Some(expected_fee));
    assert_eq!(swap_query_res.trade_params.spread, expected_spread);

    // If asset in is a CW20 approve the vault to spend the token
    if !asset_in.is_native_token() {
        let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_addr.to_string(),
            amount: Uint128::from(expected_asset_in),
            expires: None,
        };
        let contract_address = asset_in.to_string();
        app.execute_contract(
            user.clone(),
            Addr::unchecked(contract_address),
            &allowance_msg,
            &[],
        )
        .unwrap();
    }

    let coins = if asset_in.is_native_token() {
        vec![Coin {
            denom: asset_in.to_string(),
            amount: expected_asset_in,
        }]
    } else {
        vec![]
    };

    let swap_msg = VaultExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(pool_id),
            swap_type: SwapType::GiveOut {},
            asset_in,
            asset_out: asset_out.info,
            amount: asset_out.amount,
            max_spread,
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };

    app
        .execute_contract(
            user.clone(),
            vault_addr.clone(),
            &swap_msg,
            &coins,
        )
        .unwrap();
}

pub fn validate_culumative_prices(
    app: &mut App,
    pool_addr: &Addr,
    expected_prices: Vec<AssetExchangeRate>
) {

    let cumulative_price_query = QueryMsg::CumulativePrices{};
    let cumulative_price_response: CumulativePricesResponse = app
        .wrap()
        .query_wasm_smart(pool_addr.clone(), &cumulative_price_query)
        .unwrap();
    
    // Expect price map
    let mut expected_price_map: HashMap<(AssetInfo, AssetInfo), Uint128> = HashMap::new();
    for expected_price in expected_prices {
        expected_price_map.insert((expected_price.offer_info, expected_price.ask_info), expected_price.rate);
    }

    for exchange_info in cumulative_price_response.exchange_infos {
        let key = (exchange_info.offer_info, exchange_info.ask_info);
        let expected_price = expected_price_map.get(&key).unwrap();
        assert_eq!(exchange_info.rate, *expected_price);
    }
}