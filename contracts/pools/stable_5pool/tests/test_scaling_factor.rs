use crate::utils::{
    add_liquidity_to_pool, instantiate_contracts_scaling_factor, mock_app,
    perform_and_test_swap_give_in, perform_and_test_swap_give_out,
    instantiate_contract_generic, validate_culumative_prices,
};
use cosmwasm_std::{Addr, Coin, Decimal, Uint128, to_binary, Decimal256};
use cw_multi_test::Executor;
use dexter::asset::{Asset, AssetInfo, AssetExchangeRate};
use dexter::pool::{AfterJoinResponse, QueryMsg as PoolQueryMsg, AfterExitResponse};
use dexter::vault::{ExecuteMsg, PoolInfoResponse, QueryMsg, Cw20HookMsg, FeeInfo};
use cw20::Cw20ExecuteMsg;
use stable5pool::state::AssetScalingFactor;

pub mod utils;

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
        instantiate_contracts_scaling_factor(&mut app, &owner, vec![("uatom".to_string(), 6), ("ustkatom".to_string(), 6)]);

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
                slippage_tolerance: None,
            },
        )
        .unwrap();

    assert_eq!(join_pool_query_res.provided_assets[0], assets_msg[0]);
    assert_eq!(join_pool_query_res.provided_assets[1], assets_msg[1]);
    assert_eq!(
        join_pool_query_res.new_shares,
        Uint128::new(200_000_000_000)
    );

    let msg = ExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: None,
        lp_to_mint: None,
        auto_stake: None,
        slippage_tolerance: None,
        assets: Some(assets_msg.clone()),
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
                slippage_tolerance: None,
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
    assert_eq!(join_pool_query_res.new_shares, Uint128::new(2_020_377_540));

    let msg = ExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: None,
        lp_to_mint: None,
        auto_stake: None,
        slippage_tolerance: None,
        assets: Some(imbalanced_assets_msg),
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
                assets_out: None,
                burn_amount: Some(200_000_000_000u128.into()),
            },
        )
        .unwrap();

    assert_eq!(
        exit_pool_query_res.assets_out[0],
        Asset {
            info: AssetInfo::NativeToken {
                denom: "uatom".to_string(),
            },
            amount: Uint128::new(99_989_910_096u128),
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
        assets: None,
        burn_amount: Some(2_000_000_000u128.into()),
        recipient: None,
    };

    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_addr.to_string(),
        amount: Uint128::new(2_000_000_000u128),
        msg: to_binary(&exit_pool_hook_msg).unwrap(),
    };

    // Execute the exit pool message
    app.execute_contract(
        alice_address.clone(),
        lp_token_addr.clone(),
        &exit_msg,
        &[],
    ).unwrap();
    
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
        instantiate_contracts_scaling_factor(&mut app, &owner, vec![("uatom".to_string(), 6), ("ustkatom".to_string(), 6)]);

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
        Uint128::from(10_173_468u128),
        Uint128::from(1u128),
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
        Uint128::from(1_017_336_485u128),
        Uint128::from(10_453u128),
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
            vec![("uatom".to_string(), 6), ("ustkatom".to_string(), 9)]
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
        Uint128::from(10_173_468u128),
        Uint128::from(1u128),
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
        Uint128::from(1_017_336_485u128),
        Uint128::from(10_453u128),
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

    let (vault_addr, pool_addr, _lp_token_addr, _current_block_time) =
        instantiate_contract_generic(
            &mut app,
            &owner,
            fee_info,
            vec![st_atom_asset.clone(), stk_atom_asset.clone()],
            vec![(st_atom_asset.denom().unwrap(), 6), (stk_atom_asset.denom().unwrap(), 9)],
            scaling_factors,
            100
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
    add_liquidity_to_pool(
        &mut app,
        &owner,
        &alice_address,
        vault_addr.clone(),
        pool_id,
        assets_msg.clone(),
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
                rate: Uint128::from(1_020_833_322_000u64),
            },
            AssetExchangeRate {
                offer_info: stk_atom_asset.clone(),
                ask_info: st_atom_asset.clone(),
                rate: Uint128::from(979_591_000u64),
            }
        ]
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
        Uint128::from(9_766_529u128),
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
                rate: Uint128::from(1_327_087_498_500u64),
            },
            AssetExchangeRate {
                offer_info: stk_atom_asset.clone(),
                ask_info: st_atom_asset.clone(),
                rate: Uint128::from(1_273_464_300u64),
            }
        ]
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
        Uint128::from(976_643_025u128),
        Uint128::from(10_034u128),
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
                rate: Uint128::from(1_122_916_674_700u64),
            },
            AssetExchangeRate {
                offer_info: stk_atom_asset.clone(),
                ask_info: st_atom_asset.clone(),
                rate: Uint128::from(1_077_550_100u64),
            }
        ]
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
        Uint128::from(1_023_936_467_627u128),
        Uint128::from(30_685u128),
        Asset {
            info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::from(3_071_809_402u128),
        },
    );
}