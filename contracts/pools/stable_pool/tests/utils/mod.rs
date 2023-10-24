use std::collections::HashMap;

use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{attr, to_binary, Addr, Coin, Decimal, Decimal256, Timestamp, Uint128};
use cw20::{BalanceResponse, Cw20QueryMsg, MinterResponse};
use cw_multi_test::{App, ContractWrapper, Executor};

use dexter::asset::{Asset, AssetExchangeRate, AssetInfo};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{
    AfterExitResponse, AfterJoinResponse, ConfigResponse, CumulativePricesResponse, ExitType,
    FeeStructs, QueryMsg, SwapResponse,
};
use dexter::vault::{
    Cw20HookMsg, ExecuteMsg as VaultExecuteMsg, FeeInfo, InstantiateMsg as VaultInstantiateMsg,
    NativeAssetPrecisionInfo, PauseInfo, PoolCreationFee, PoolInfo, PoolType, PoolTypeConfig,
    QueryMsg as VaultQueryMsg, SingleSwapRequest, SwapType,
};

use cw20::Cw20ExecuteMsg;

use dexter::pool::ExitType::ExactLpBurn;
use dexter::vault;
use itertools::Itertools;
use stable_pool::state::{AssetScalingFactor, MathConfig, StablePoolParams};

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
        stable_pool::contract::execute,
        stable_pool::contract::instantiate,
        stable_pool::contract::query,
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
        pool_type: PoolType::StableSwap {},
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

    let native_asset_precisions = native_asset_precisions
        .into_iter()
        .map(|(k, v)| NativeAssetPrecisionInfo {
            denom: k,
            precision: v,
        })
        .collect_vec();

    let msg = VaultExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::StableSwap {},
        asset_infos: asset_infos.to_vec(),
        native_asset_precisions: native_asset_precisions.clone(),
        init_params: Some(
            to_binary(&StablePoolParams {
                amp,
                scaling_factors,
                supports_scaling_factors_update: false,
                scaling_factor_manager: None,
                max_allowed_spread: Decimal::from_ratio(50u128, 100u128),
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
        attr("pool_type", "stable-swap")
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
    assert_eq!(PoolType::StableSwap {}, pool_res.pool_type);

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

    assert_eq!(PoolType::StableSwap {}, pool_config_res.pool_type);
    assert_eq!(
        current_block.time.seconds(),
        pool_config_res.block_time_last
    );

    // Find max of native_asset_precisions
    let mut max_precision = 0u8;
    for precision in native_asset_precisions.iter() {
        if precision.precision > max_precision {
            max_precision = precision.precision;
        }
    }

    let math_config_binary = to_binary(&MathConfig {
        init_amp: amp * 100,
        init_amp_time: EPOCH_START,
        next_amp: amp * 100,
        next_amp_time: EPOCH_START,
        greatest_precision: max_precision,
    })
    .unwrap();

    assert_eq!(Some(math_config_binary), pool_config_res.math_params);

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

    let (vault_addr, pool_addr, lp_token, current_block_time) = instantiate_contract_generic(
        app,
        owner,
        fee_info,
        asset_infos,
        native_asset_precisions,
        scaling_factors,
        50,
    );

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
        instantiate_contract_generic(
            app,
            owner,
            fee_info,
            asset_infos,
            native_asset_precisions,
            vec![],
            10,
        );

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
    pool_addr: Addr,
    amount_to_add: Vec<Asset>,
) -> Uint128 {
    // Find CW20 assets from the bootstrapping amount and mint token to the user
    let cw20_assets = amount_to_add
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
    let native_token = amount_to_add
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

    // Step 4: Do the query to get to join pool once
    let query_msg = QueryMsg::OnJoinPool {
        assets_in: Some(amount_to_add.clone()),
        mint_amount: None,
    };

    let res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(pool_addr.as_str(), &query_msg)
        .unwrap();

    // Step 4: Execute join pool
    let msg = VaultExecuteMsg::JoinPool {
        pool_id,
        recipient: None,
        auto_stake: None,
        assets: Some(amount_to_add),
        min_lp_to_receive: None,
    };

    app.execute_contract(user.clone(), vault_addr.clone(), &msg, &coins)
        .unwrap();

    res.new_shares
}

pub fn query_cw20_balance(app: &mut App, user: &Addr, contract_addr: Addr) -> Uint128 {
    let query_msg = Cw20QueryMsg::Balance {
        address: user.to_string(),
    };
    let res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(contract_addr.as_str(), &query_msg)
        .unwrap();

    res.balance
}

pub fn query_bank_balance(app: &mut App, user: &Addr, denom: String) -> Uint128 {
    let res: Coin = app.wrap().query_balance(user.clone(), denom).unwrap();

    res.amount
}

pub fn perform_and_test_add_liquidity(
    app: &mut App,
    owner: &Addr,
    user: &Addr,
    vault_addr: Addr,
    lp_token_addr: Addr,
    pool_addr: Addr,
    pool_id: Uint128,
    amount_to_add: Vec<Asset>,
    expected_lp_token_amount: Uint128,
) {
    let lp_token_before = query_cw20_balance(app, user, lp_token_addr.clone());

    let new_shares_from_query = add_liquidity_to_pool(
        app,
        owner,
        user,
        vault_addr.clone(),
        pool_id,
        pool_addr,
        amount_to_add,
    );

    let lp_token_after = query_cw20_balance(app, user, lp_token_addr.clone());

    assert_eq!(
        new_shares_from_query, expected_lp_token_amount,
        "Unexpected LP token amount from query"
    );

    assert_eq!(
        lp_token_after,
        lp_token_before + expected_lp_token_amount,
        "Unexpected LP token amount after adding liquidity"
    );
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
    expected_fee: Asset,
) {
    let swap_query_msg = QueryMsg::OnSwap {
        swap_type: SwapType::GiveIn {},
        offer_asset: asset_in.info.clone(),
        ask_asset: asset_out.clone(),
        amount: asset_in.amount,
        max_spread,
        belief_price: None,
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

    app.execute_contract(user.clone(), vault_addr.clone(), &swap_msg, &coins)
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
    expected_fee: Asset,
) {
    let swap_query_msg = QueryMsg::OnSwap {
        swap_type: SwapType::GiveOut {},
        offer_asset: asset_in.clone(),
        ask_asset: asset_out.info.clone(),
        amount: asset_out.amount,
        max_spread,
        belief_price: None,
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

    app.execute_contract(user.clone(), vault_addr.clone(), &swap_msg, &coins)
        .unwrap();
}

pub fn validate_culumative_prices(
    app: &mut App,
    pool_addr: &Addr,
    expected_prices: Vec<AssetExchangeRate>,
) {
    let cumulative_price_query = QueryMsg::CumulativePrices {};
    let cumulative_price_response: CumulativePricesResponse = app
        .wrap()
        .query_wasm_smart(pool_addr.clone(), &cumulative_price_query)
        .unwrap();

    // Expect price map
    let mut expected_price_map: HashMap<(AssetInfo, AssetInfo), Uint128> = HashMap::new();
    for expected_price in expected_prices {
        expected_price_map.insert(
            (expected_price.offer_info, expected_price.ask_info),
            expected_price.rate,
        );
    }

    for exchange_info in cumulative_price_response.exchange_infos {
        let key = (exchange_info.offer_info, exchange_info.ask_info);
        let expected_price = expected_price_map.get(&key).unwrap();
        assert_eq!(exchange_info.rate, *expected_price);
    }
}

pub fn create_cw20_asset(
    app: &mut App,
    owner: &Addr,
    token_code_id: u64,
    name: String,
    symbol: String,
    decimals: u8,
) -> Addr {
    // Create Token X
    let init_msg = TokenInstantiateMsg {
        name: name.clone(),
        symbol,
        decimals,
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
            name,
            None,
        )
        .unwrap();

    return token_instance0;
}

pub fn perform_and_test_exit_pool(
    app: &mut App,
    user: &Addr,
    vault_addr: Addr,
    pool_addr: Addr,
    pool_id: Uint128,
    lp_token_addr: Addr,
    burn_amount: Uint128,
    expected_asset_out: Vec<Asset>,
    expected_fee: Option<Vec<Asset>>,
) {
    let exit_query_msg = QueryMsg::OnExitPool {
        exit_type: ExactLpBurn(burn_amount),
    };

    let exit_query_res: AfterExitResponse = app
        .wrap()
        .query_wasm_smart(pool_addr.clone(), &exit_query_msg)
        .unwrap();

    assert_eq!(exit_query_res.assets_out, expected_asset_out);
    assert_eq!(exit_query_res.fee, expected_fee);

    let exit_pool_hook_msg = Cw20HookMsg::ExitPool {
        pool_id,
        exit_type: vault::ExitType::ExactLpBurn {
            lp_to_burn: burn_amount,
            min_assets_out: None,
        },
        recipient: None,
    };

    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_addr.to_string(),
        amount: burn_amount,
        msg: to_binary(&exit_pool_hook_msg).unwrap(),
    };

    // Execute the exit pool message
    app.execute_contract(user.clone(), lp_token_addr.clone(), &exit_msg, &[])
        .unwrap();
}

pub fn perform_and_test_imbalanced_exit(
    app: &mut App,
    user: &Addr,
    vault_addr: Addr,
    pool_addr: Addr,
    pool_id: Uint128,
    lp_token_addr: Addr,
    assets_out: Vec<Asset>,
    expected_burn_amount: Uint128,
    expected_fee: Option<Vec<Asset>>,
) {
    let exit_query_msg = QueryMsg::OnExitPool {
        exit_type: ExitType::ExactAssetsOut(assets_out.clone()),
    };

    let exit_query_res: AfterExitResponse = app
        .wrap()
        .query_wasm_smart(pool_addr.clone(), &exit_query_msg)
        .unwrap();

    let assets_out_actual_map = assets_out
        .iter()
        .map(|asset| (asset.info.clone(), asset.amount))
        .collect::<HashMap<AssetInfo, Uint128>>();

    for asset in assets_out.iter() {
        let actual_amount = assets_out_actual_map.get(&asset.info).unwrap();
        assert_eq!(asset.amount, *actual_amount);
    }

    let sorted_fee = exit_query_res.fee.map(|mut a| {
        a.sort_by_key(|i| i.info.clone());
        a
    });

    assert_eq!(sorted_fee, expected_fee);
    assert_eq!(exit_query_res.burn_shares, expected_burn_amount);

    let exit_pool_hook_msg = Cw20HookMsg::ExitPool {
        pool_id,
        exit_type: vault::ExitType::ExactAssetsOut {
            assets_out: assets_out.clone(),
            max_lp_to_burn: Some(expected_burn_amount),
        },
        recipient: None,
    };

    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_addr.to_string(),
        amount: expected_burn_amount,
        msg: to_binary(&exit_pool_hook_msg).unwrap(),
    };

    // Execute the exit pool message
    app.execute_contract(user.clone(), lp_token_addr.clone(), &exit_msg, &[])
        .unwrap();
}

pub fn log_pool_info(app: &mut App, pool_addr: &Addr) {
    let pool_info_query = QueryMsg::Config {};
    let pool_info_response: ConfigResponse = app
        .wrap()
        .query_wasm_smart(pool_addr.clone(), &pool_info_query)
        .unwrap();

    println!("Pool Info: {:?}", pool_info_response);
}
