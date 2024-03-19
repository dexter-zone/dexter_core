use std::collections::HashMap;

use crate::utils::{
    add_liquidity_to_pool, create_cw20_asset, instantiate_contract_generic,
    instantiate_contracts_scaling_factor, mock_app, perform_and_test_add_liquidity,
    perform_and_test_exit_pool, perform_and_test_imbalanced_exit, perform_and_test_swap_give_in,
    perform_and_test_swap_give_out, store_token_code, validate_culumative_prices,
    log_pool_info
};
use cosmwasm_std::{to_json_binary, Addr, Coin, Decimal, Decimal256, Uint128};
use cw20::Cw20ExecuteMsg;
use cw_multi_test::Executor;
use dexter::asset::{Asset, AssetExchangeRate, AssetInfo};
use dexter::pool::{AfterExitResponse, AfterJoinResponse, ExitType, QueryMsg as PoolQueryMsg};
use dexter::vault::{Cw20HookMsg, ExecuteMsg, FeeInfo, PoolInfoResponse, QueryMsg};
use itertools::Itertools;
use dexter::vault;

use dexter_stable_pool::state::AssetScalingFactor;

pub mod utils;

#[macro_export]
macro_rules! uint128_with_precision {
    ($value:expr, $precision:expr) => {
        Uint128::from($value)
            .checked_mul(Uint128::from(10u64).pow($precision as u32))
            .unwrap()
    };
}

#[test]
fn test_join_and_exit_pool() {
    let owner: Addr = Addr::unchecked("owner".to_string());
    let alice_address: Addr = Addr::unchecked("alice".to_string());

    let mut app = mock_app(
        owner.clone(),
        vec![
            Coin {
                denom: "ustkatom".to_string(),
                amount: Uint128::new(100_000_000_000_000u128),
            },
            Coin {
                denom: "uatom".to_string(),
                amount: Uint128::new(100_000_000_000_000u128),
            },
        ],
    );

    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[
            Coin {
                denom: "ustkatom".to_string(),
                amount: Uint128::new(10_000_000_000_000u128),
            },
            Coin {
                denom: "uatom".to_string(),
                amount: Uint128::new(10_000_000_000_000u128),
            },
        ],
    )
    .unwrap();

    let (vault_addr, pool_addr, lp_token_addr, _current_block_time) =
        instantiate_contracts_scaling_factor(
            &mut app,
            &owner,
            vec![("uatom".to_string(), 6), ("ustkatom".to_string(), 6)],
        );

    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "uatom".to_string(),
            },
            amount: Uint128::new(100_000_000_000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::new(98_000_000_000u128),
        },
    ];

    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &PoolQueryMsg::OnJoinPool {
                assets_in: Some(assets_msg.clone()),
                mint_amount: None,
            },
        )
        .unwrap();

    assert_eq!(join_pool_query_res.provided_assets[0], assets_msg[0]);
    assert_eq!(join_pool_query_res.provided_assets[1], assets_msg[1]);
    assert_eq!(
        join_pool_query_res.new_shares,
        uint128_with_precision!(200000u64, 18)
    );

    let msg = ExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: None,
        auto_stake: None,
        assets: Some(assets_msg.clone()),
        min_lp_to_receive: None,
    };

    // Execute the join pool message
    app.execute_contract(
        alice_address.clone(),
        vault_addr.clone(),
        &msg,
        &[
            Coin {
                denom: "ustkatom".to_string(),
                amount: Uint128::new(98_000_000_000u128),
            },
            Coin {
                denom: "uatom".to_string(),
                amount: Uint128::new(100_000_000_000u128),
            },
        ],
    )
    .unwrap();

    // Query the vault and get the pool balances
    let query_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            vault_addr.clone(),
            &QueryMsg::GetPoolById {
                pool_id: Uint128::from(1u128),
            },
        )
        .unwrap();

    assert_eq!(query_res.assets[0], assets_msg[0]);
    assert_eq!(query_res.assets[1], assets_msg[1]);

    // Add imbalanced liquidity to the pool. Since assets have different scaling factors, adding same amount
    // of each asset will result in imbalanced liquidity addition where equally shared amount of each asset will be added and rest
    // will be added to the asset as single asset liquidity.
    let imbalanced_assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "uatom".to_string(),
            },
            amount: Uint128::new(1_000_000_000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::new(1_000_000_000u128),
        },
    ];

    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &PoolQueryMsg::OnJoinPool {
                assets_in: Some(imbalanced_assets_msg.clone()),
                mint_amount: None,
            },
        )
        .unwrap();

    assert_eq!(
        join_pool_query_res.provided_assets[0],
        imbalanced_assets_msg[0]
    );
    assert_eq!(
        join_pool_query_res.provided_assets[1],
        imbalanced_assets_msg[1]
    );
    assert_eq!(join_pool_query_res.new_shares, Uint128::new(20_203_77_540_814_273_400_000));

    let msg = ExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: None,
        auto_stake: None,
        assets: Some(imbalanced_assets_msg),
        min_lp_to_receive: None,
    };

    // Execute the join pool message
    app.execute_contract(
        alice_address.clone(),
        vault_addr.clone(),
        &msg,
        &[
            Coin {
                denom: "ustkatom".to_string(),
                amount: Uint128::new(1_000_000_000u128),
            },
            Coin {
                denom: "uatom".to_string(),
                amount: Uint128::new(1_000_000_000u128),
            },
        ],
    )
    .unwrap();

    // Exit the pool now
    let exit_pool_query_res: AfterExitResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &PoolQueryMsg::OnExitPool {
                exit_type: ExitType::ExactLpBurn(uint128_with_precision!(200_000u64, Decimal256::DECIMAL_PLACES)),
            },
        )
        .unwrap();

    assert_eq!(
        exit_pool_query_res.assets_out[0],
        Asset {
            info: AssetInfo::NativeToken {
                denom: "uatom".to_string(),
            },
            amount: Uint128::new(99_989_910_095u128),
        }
    );

    assert_eq!(
        exit_pool_query_res.assets_out[1],
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::new(98_009_911_876u128),
        }
    );

    let exit_pool_hook_msg = Cw20HookMsg::ExitPool {
        pool_id: Uint128::from(1u128),
        exit_type: vault::ExitType::ExactLpBurn {
            lp_to_burn: 2_000_000_000u128.into(),
            min_assets_out: None,
        },
        recipient: None,
    };

    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_addr.to_string(),
        amount: Uint128::new(2_000_000_000u128),
        msg: to_json_binary(&exit_pool_hook_msg).unwrap(),
    };

    // Execute the exit pool message
    app.execute_contract(alice_address.clone(), lp_token_addr.clone(), &exit_msg, &[])
        .unwrap();
}

#[test]
fn test_swap() {
    let owner: Addr = Addr::unchecked("owner".to_string());
    let alice_address: Addr = Addr::unchecked("alice".to_string());

    let mut app = mock_app(
        owner.clone(),
        vec![
            Coin {
                denom: "ustkatom".to_string(),
                amount: Uint128::new(100_000_000_000_000u128),
            },
            Coin {
                denom: "uatom".to_string(),
                amount: Uint128::new(100_000_000_000_000u128),
            },
        ],
    );

    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[
            Coin {
                denom: "ustkatom".to_string(),
                amount: Uint128::new(10_000_000_000_000u128),
            },
            Coin {
                denom: "uatom".to_string(),
                amount: Uint128::new(10_000_000_000_000u128),
            },
        ],
    )
    .unwrap();

    let (vault_addr, pool_addr, _lp_token_addr, _current_block_time) =
        instantiate_contracts_scaling_factor(
            &mut app,
            &owner,
            vec![("uatom".to_string(), 6), ("ustkatom".to_string(), 6)],
        );

    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "uatom".to_string(),
            },
            amount: Uint128::new(1_000_000_000_000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::new(980_000_000_000u128),
        },
    ];

    let pool_id = Uint128::from(1u128);
    add_liquidity_to_pool(
        &mut app,
        &owner,
        &alice_address,
        vault_addr.clone(),
        pool_id,
        pool_addr.clone(),
        assets_msg.clone(),
    );

    // Peform swap and test
    perform_and_test_swap_give_in(
        &mut app,
        &owner,
        &alice_address.clone(),
        vault_addr.clone(),
        pool_addr.clone(),
        pool_id,
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::new(10_000_000u128),
        },
        AssetInfo::NativeToken {
            denom: "uatom".to_string(),
        },
        Some(Decimal::from_ratio(20u64, 100u64)),
        Uint128::from(10_173_469u128),
        Uint128::from(0u128),
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::from(30_000u128),
        },
    );

    // Peform another swap of a large amount
    perform_and_test_swap_give_in(
        &mut app,
        &owner,
        &alice_address.clone(),
        vault_addr.clone(),
        pool_addr.clone(),
        pool_id,
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::new(1_000_000_000u128),
        },
        AssetInfo::NativeToken {
            denom: "uatom".to_string(),
        },
        Some(Decimal::from_ratio(20u64, 100u64)),
        Uint128::from(1_017_336_487u128),
        Uint128::from(10_451u128),
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::from(3_000_000u128),
        },
    );

    // Perform a give out swap
    perform_and_test_swap_give_out(
        &mut app,
        &owner,
        &alice_address.clone(),
        vault_addr.clone(),
        pool_addr.clone(),
        pool_id,
        Asset {
            info: AssetInfo::NativeToken {
                denom: "uatom".to_string(),
            },
            amount: Uint128::new(1_000_000_000u128),
        },
        AssetInfo::NativeToken {
            denom: "ustkatom".to_string(),
        },
        Some(Decimal::from_ratio(20u64, 100u64)),
        Uint128::from(982_978_603u128),
        Uint128::from(30_273u128),
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::from(2_948_935u128),
        },
    );
}

#[test]
fn test_swap_different_precision() {
    let owner: Addr = Addr::unchecked("owner".to_string());
    let alice_address: Addr = Addr::unchecked("alice".to_string());

    // For this test, we consider ustakatom to have 9 decimal places and uatom to have 6 decimal places

    let mut app = mock_app(
        owner.clone(),
        vec![
            Coin {
                denom: "ustkatom".to_string(),
                amount: Uint128::new(100_000_000_000_000_000u128),
            },
            Coin {
                denom: "uatom".to_string(),
                amount: Uint128::new(100_000_000_000_000u128),
            },
        ],
    );

    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[
            Coin {
                denom: "ustkatom".to_string(),
                amount: Uint128::new(10_000_000_000_000_000u128),
            },
            Coin {
                denom: "uatom".to_string(),
                amount: Uint128::new(10_000_000_000_000u128),
            },
        ],
    )
    .unwrap();

    let (vault_addr, pool_addr, _lp_token_addr, _current_block_time) =
        instantiate_contracts_scaling_factor(
            &mut app,
            &owner,
            vec![("uatom".to_string(), 6), ("ustkatom".to_string(), 9)],
        );

    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "uatom".to_string(),
            },
            amount: Uint128::new(1_000_000_000_000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::new(980_000_000_000_000u128),
        },
    ];

    let pool_id = Uint128::from(1u128);
    add_liquidity_to_pool(
        &mut app,
        &owner,
        &alice_address,
        vault_addr.clone(),
        pool_id,
        pool_addr.clone(),
        assets_msg.clone(),
    );

    // Peform swap and test
    perform_and_test_swap_give_in(
        &mut app,
        &owner,
        &alice_address.clone(),
        vault_addr.clone(),
        pool_addr.clone(),
        pool_id,
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::new(10_000_000_000u128),
        },
        AssetInfo::NativeToken {
            denom: "uatom".to_string(),
        },
        Some(Decimal::from_ratio(20u64, 100u64)),
        Uint128::from(10_173_469u128),
        Uint128::from(0u128),
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::from(30_000_000u128),
        },
    );

    // Peform another swap of a large amount
    perform_and_test_swap_give_in(
        &mut app,
        &owner,
        &alice_address.clone(),
        vault_addr.clone(),
        pool_addr.clone(),
        pool_id,
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::new(1_000_000_000_000u128),
        },
        AssetInfo::NativeToken {
            denom: "uatom".to_string(),
        },
        Some(Decimal::from_ratio(20u64, 100u64)),
        Uint128::from(1_017_336_487u128),
        Uint128::from(10_451u128),
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::from(3_000_000_000u128),
        },
    );

    // Perform a give out swap
    perform_and_test_swap_give_out(
        &mut app,
        &owner,
        &alice_address.clone(),
        vault_addr.clone(),
        pool_addr.clone(),
        pool_id,
        Asset {
            info: AssetInfo::NativeToken {
                denom: "uatom".to_string(),
            },
            amount: Uint128::new(1_000_000_000u128),
        },
        AssetInfo::NativeToken {
            denom: "ustkatom".to_string(),
        },
        Some(Decimal::from_ratio(20u64, 100u64)),
        Uint128::from(982_978_603_388u128),
        Uint128::from(30_273u128),
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::from(2_948_935_810u128),
        },
    );
}

#[test]
fn test_swap_different_lsd_assets() {
    let owner: Addr = Addr::unchecked("owner".to_string());
    let alice_address: Addr = Addr::unchecked("alice".to_string());

    // For this test, we consider ustakatom to have 9 decimal places and uatom to have 6 decimal places

    let stk_atom_asset = AssetInfo::NativeToken {
        denom: "ustkatom".to_string(),
    };

    let st_atom_asset = AssetInfo::NativeToken {
        denom: "ustatom".to_string(),
    };

    let mut app = mock_app(
        owner.clone(),
        vec![
            Coin {
                denom: stk_atom_asset.denom().unwrap(),
                amount: Uint128::new(100_000_000_000_000_000u128),
            },
            Coin {
                denom: st_atom_asset.denom().unwrap(),
                amount: Uint128::new(100_000_000_000_000u128),
            },
        ],
    );

    // Set Alice's balances
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &[
            Coin {
                denom: stk_atom_asset.denom().unwrap(),
                amount: Uint128::new(10_000_000_000_000_000u128),
            },
            Coin {
                denom: st_atom_asset.denom().unwrap(),
                amount: Uint128::new(10_000_000_000_000u128),
            },
        ],
    )
    .unwrap();

    let fee_info = FeeInfo {
        total_fee_bps: 30,
        protocol_fee_percent: 20,
    };

    let scaling_factors = vec![
        AssetScalingFactor {
            asset_info: st_atom_asset.clone(),
            scaling_factor: Decimal256::from_ratio(96u128, 100u128),
        },
        AssetScalingFactor {
            asset_info: stk_atom_asset.clone(),
            scaling_factor: Decimal256::from_ratio(98u128, 100u128),
        },
    ];

    let (vault_addr, pool_addr, lp_token_addr, _current_block_time) = instantiate_contract_generic(
        &mut app,
        &owner,
        fee_info,
        vec![st_atom_asset.clone(), stk_atom_asset.clone()],
        vec![
            (st_atom_asset.denom().unwrap(), 6),
            (stk_atom_asset.denom().unwrap(), 9),
        ],
        scaling_factors,
        100,
    );

    let assets_msg = vec![
        Asset {
            info: st_atom_asset.clone(),
            amount: Uint128::new(960_000_000_000u128),
        },
        Asset {
            info: stk_atom_asset.clone(),
            amount: Uint128::new(980_000_000_000_000u128),
        },
    ];

    let pool_id = Uint128::from(1u128);
    perform_and_test_add_liquidity(
        &mut app,
        &owner,
        &alice_address,
        vault_addr.clone(),
        lp_token_addr.clone(),
        pool_addr.clone(),
        pool_id,
        assets_msg.clone(),
        uint128_with_precision!(2_000_000u128, 18),
    );

    // increase block time
    app.update_block(|b| {
        b.time = b.time.plus_seconds(1000);
    });

    validate_culumative_prices(
        &mut app,
        &pool_addr,
        vec![
            AssetExchangeRate {
                offer_info: st_atom_asset.clone(),
                ask_info: stk_atom_asset.clone(),
                rate: Uint128::from(1_020_833_232_000u64),
            },
            AssetExchangeRate {
                offer_info: stk_atom_asset.clone(),
                ask_info: st_atom_asset.clone(),
                rate: Uint128::from(979_591_000u64),
            },
        ],
    );

    // Peform swap and test
    perform_and_test_swap_give_in(
        &mut app,
        &owner,
        &alice_address.clone(),
        vault_addr.clone(),
        pool_addr.clone(),
        pool_id,
        Asset {
            info: stk_atom_asset.clone(),
            amount: Uint128::new(10_000_000_000u128),
        },
        st_atom_asset.clone(),
        Some(Decimal::from_ratio(20u64, 100u64)),
        Uint128::from(9_766_530u128),
        Uint128::from(0u128),
        Asset {
            info: stk_atom_asset.clone(),
            amount: Uint128::from(30_000_000u128),
        },
    );

    // increase block time
    app.update_block(|b| {
        b.time = b.time.plus_seconds(100);
    });

    validate_culumative_prices(
        &mut app,
        &pool_addr,
        vec![
            AssetExchangeRate {
                offer_info: st_atom_asset.clone(),
                ask_info: stk_atom_asset.clone(),
                rate: Uint128::from(1_122_916_575_800u64),
            },
            AssetExchangeRate {
                offer_info: stk_atom_asset.clone(),
                ask_info: st_atom_asset.clone(),
                rate: Uint128::from(1_077_550_100u64),
            },
        ],
    );

    // Peform another swap of a large amount
    perform_and_test_swap_give_in(
        &mut app,
        &owner,
        &alice_address.clone(),
        vault_addr.clone(),
        pool_addr.clone(),
        pool_id,
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::new(1_000_000_000_000u128),
        },
        AssetInfo::NativeToken {
            denom: "ustatom".to_string(),
        },
        Some(Decimal::from_ratio(20u64, 100u64)),
        Uint128::from(976_643_026u128),
        Uint128::from(10_033u128),
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::from(3_000_000_000u128),
        },
    );

    // increase block time
    app.update_block(|b| {
        b.time = b.time.plus_seconds(200);
    });

    validate_culumative_prices(
        &mut app,
        &pool_addr,
        vec![
            AssetExchangeRate {
                offer_info: st_atom_asset.clone(),
                ask_info: stk_atom_asset.clone(),
                rate: Uint128::from(1_327_087_381_400u64),
            },
            AssetExchangeRate {
                offer_info: stk_atom_asset.clone(),
                ask_info: st_atom_asset.clone(),
                rate: Uint128::from(1_273_464_300u64),
            },
        ],
    );

    // Perform a give out swap
    perform_and_test_swap_give_out(
        &mut app,
        &owner,
        &alice_address.clone(),
        vault_addr.clone(),
        pool_addr.clone(),
        pool_id,
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustatom".to_string(),
            },
            amount: Uint128::new(1_000_000_000u128),
        },
        AssetInfo::NativeToken {
            denom: "ustkatom".to_string(),
        },
        Some(Decimal::from_ratio(20u64, 100u64)),
        Uint128::from(1_023_936_467_628u128),
        Uint128::from(30_685u128),
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::from(3_071_809_402u128),
        },
    );
}

#[test]
fn test_5_asset_lsd_pool_with_different_precisions() {
    let owner: Addr = Addr::unchecked("owner".to_string());
    let alice_address: Addr = Addr::unchecked("alice".to_string());

    // For this test, we consider ustakatom to have 9 decimal places and uatom to have 6 decimal places
    let atom_asset = AssetInfo::native_token("uatom".to_string());
    let statom_asset = AssetInfo::native_token("ustatom".to_string());
    let stkatom_asset = AssetInfo::native_token("ustkatom".to_string());
    let qatom_asset = AssetInfo::native_token("uqatom".to_string());

    let mut asset_decimals = HashMap::new();
    asset_decimals.insert(atom_asset.clone(), 6u8);
    asset_decimals.insert(statom_asset.clone(), 6u8);
    asset_decimals.insert(stkatom_asset.clone(), 9u8);
    asset_decimals.insert(qatom_asset.clone(), 12u8);

    // Native asset decimals
    let native_asset_decimals = vec![
        (
            statom_asset.denom().unwrap(),
            asset_decimals.get(&statom_asset).unwrap().clone(),
        ),
        (
            stkatom_asset.denom().unwrap(),
            asset_decimals.get(&stkatom_asset).unwrap().clone(),
        ),
        (
            atom_asset.denom().unwrap(),
            asset_decimals.get(&atom_asset).unwrap().clone(),
        ),
        (
            qatom_asset.denom().unwrap(),
            asset_decimals.get(&qatom_asset).unwrap().clone(),
        ),
    ];

    let native_assets = vec![
        stkatom_asset.clone(),
        statom_asset.clone(),
        atom_asset.clone(),
        qatom_asset.clone(),
    ];

    let initial_mint_balance = Uint128::new(1_000_000_000u128);
    let coins = native_assets
        .iter()
        .map(|info| {
            let denom = info.denom().unwrap();
            let decimals = asset_decimals.get(&info).unwrap().clone();
            Coin {
                denom,
                amount: uint128_with_precision!(initial_mint_balance, decimals),
            }
        })
        .collect_vec();

    let mut app = mock_app(owner.clone(), coins);

    // Transfer some tokens to alice
    let alice_balance = Uint128::new(2_000_000u128);
    let coins = native_assets
        .iter()
        .map(|info| {
            let denom = info.denom().unwrap();
            let decimals = asset_decimals.get(&info).unwrap().clone();
            Coin {
                denom,
                amount: uint128_with_precision!(alice_balance, decimals),
            }
        })
        .collect_vec();

    app.send_tokens(owner.clone(), alice_address.clone(), &coins)
        .unwrap();

    let cw20_code_id = store_token_code(&mut app);
    // Instnatiate a CW20 contract representing the wrapped atom
    let wrapped_atom_addr = create_cw20_asset(
        &mut app,
        &owner,
        cw20_code_id,
        "Wrapped Atom".to_string(),
        "WATOM".to_string(),
        6,
    );
    let wrapped_atom_asset = AssetInfo::token(wrapped_atom_addr);
    asset_decimals.insert(wrapped_atom_asset.clone(), 6u8);

    // Scaling factors
    let scaling_factors = vec![
        AssetScalingFactor::new(atom_asset.clone(), Decimal256::from_ratio(1u64, 1u64)),
        AssetScalingFactor::new(statom_asset.clone(), Decimal256::from_ratio(96u64, 100u64)),
        AssetScalingFactor::new(stkatom_asset.clone(), Decimal256::from_ratio(98u64, 100u64)),
        AssetScalingFactor::new(qatom_asset.clone(), Decimal256::from_ratio(99u64, 100u64)),
        AssetScalingFactor::new(
            wrapped_atom_asset.clone(),
            Decimal256::from_ratio(1u64, 1u64),
        ),
    ];

    let fee_info = FeeInfo {
        total_fee_bps: 30,
        protocol_fee_percent: 20,
    };

    let asset_infos = vec![
        wrapped_atom_asset.clone(),
        atom_asset.clone(),
        qatom_asset.clone(),
        statom_asset.clone(),
        stkatom_asset.clone(),
    ];

    let (vault_addr, pool_addr, lp_token_addr, _current_block_time) = instantiate_contract_generic(
        &mut app,
        &owner,
        fee_info,
        asset_infos.clone(),
        native_asset_decimals,
        scaling_factors,
        100,
    );

    let pool_bootstrapping_amount_0_scale = Uint128::from(1_000_000u128);

    let assets_msg = asset_infos
        .iter()
        .map(|asset| {
            let info = asset.clone();
            let decimals = asset_decimals.get(&info).unwrap().clone();
            Asset::new(
                info,
                uint128_with_precision!(pool_bootstrapping_amount_0_scale, decimals),
            )
        })
        .collect_vec();

    let pool_id = Uint128::from(1u128);
    perform_and_test_add_liquidity(
        &mut app,
        &owner,
        &alice_address,
        vault_addr.clone(),
        lp_token_addr.clone(),
        pool_addr.clone(),
        pool_id,
        assets_msg.clone(),
        Uint128::from(5_072_169_964_407_537_367_258_707u128),
    );

    // Swap 1 ustatom for uatom
    perform_and_test_swap_give_in(
        &mut app,
        &owner,
        &alice_address.clone(),
        vault_addr.clone(),
        pool_addr.clone(),
        pool_id,
        Asset {
            info: statom_asset.clone(),
            amount: uint128_with_precision!(
                Uint128::from(1u64),
                asset_decimals.get(&statom_asset).unwrap().clone()
            ),
        },
        atom_asset.clone(),
        Some(Decimal::from_ratio(20u64, 100u64)),
        Uint128::from(1_038_125u128),
        Uint128::from(416u128),
        Asset {
            info: statom_asset.clone(),
            amount: Uint128::from(3_000u128),
        },
    );

    // test a larger amount in both scaled assets
    perform_and_test_swap_give_in(
        &mut app,
        &owner,
        &alice_address.clone(),
        vault_addr.clone(),
        pool_addr.clone(),
        pool_id,
        Asset {
            info: statom_asset.clone(),
            amount: uint128_with_precision!(
                Uint128::from(10_000u64),
                asset_decimals.get(&statom_asset).unwrap().clone()
            ),
        },
        qatom_asset.clone(),
        Some(Decimal::from_ratio(20u64, 100u64)),
        Uint128::from(10_277_441_665_935_813u128),
        Uint128::from(4_120_834_064_186u128),
        Asset {
            info: statom_asset.clone(),
            amount: Uint128::from(30_000_000u128),
        },
    );

    // test a give out swap with a large amount
    perform_and_test_swap_give_out(
        &mut app,
        &owner,
        &alice_address.clone(),
        vault_addr.clone(),
        pool_addr.clone(),
        pool_id,
        Asset {
            info: stkatom_asset.clone(),
            amount: uint128_with_precision!(
                Uint128::from(10_000u64),
                asset_decimals.get(&stkatom_asset).unwrap().clone()
            ),
        },
        wrapped_atom_asset.clone(),
        Some(Decimal::from_ratio(20u64, 100u64)),
        Uint128::from(10_233_757_152u128),
        Uint128::from(0u128),
        Asset {
            info: wrapped_atom_asset.clone(),
            amount: Uint128::from(30_701_271u128),
        },
    );

    log_pool_info(&mut app, &pool_addr);

    // Test out pool exit
    perform_and_test_exit_pool(
        &mut app,
        &alice_address.clone(),
        vault_addr.clone(),
        pool_addr.clone(),
        pool_id,
        lp_token_addr.clone(),
        uint128_with_precision!(Uint128::from(1u64), 18),
        vec![
            Asset::new(wrapped_atom_asset.clone(), Uint128::from(199_170u128)),
            Asset::new(atom_asset.clone(), Uint128::from(197_154u128)),
            Asset::new(qatom_asset.clone(), Uint128::from(195_128_034_998u128)),
            Asset::new(statom_asset.clone(), Uint128::from(199_124u128)),
            Asset::new(stkatom_asset.clone(), Uint128::from(195_182_733u128)),
        ],
        Some(vec![]),
    );

    // Test imbalanced exit
    perform_and_test_imbalanced_exit(
        &mut app,
        &alice_address.clone(),
        vault_addr.clone(),
        pool_addr.clone(),
        pool_id,
        lp_token_addr.clone(),
        vec![Asset::new(atom_asset.clone(), Uint128::from(100_000_000_000u128))],
        Uint128::from(100_206_787_474_015_742_838_214u128),
        Some(vec![
            // Asset::new(wrapped_atom_asset.clone(), Uint128::from(18_684_894u128)),
            Asset::new(wrapped_atom_asset.clone(), Uint128::from(18_682_739u128)),
            // Asset::new(atom_asset.clone(), Uint128::from(75_254_292u128)),
            Asset::new(atom_asset.clone(), Uint128::from(75_256_425u128)),
            // Asset::new(qatom_asset.clone(), Uint128::from(18_305_638_299_226u128)),
            Asset::new(qatom_asset.clone(), Uint128::from(18_303_527_329_581u128)),
            // Asset::new(statom_asset.clone(), Uint128::from(18_680_591u128)),
            Asset::new(statom_asset.clone(), Uint128::from(18_678_437u128)),
            // Asset::new(stkatom_asset.clone(), Uint128::from(18_310_769_784u128)),
            Asset::new(stkatom_asset.clone(), Uint128::from(18_308_658_223u128)),
        ]),
    );
}
