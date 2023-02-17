use crate::utils::{instantiate_contracts_scaling_factor, mock_app};
use cosmwasm_std::{Addr, Coin, Uint128, Decimal};
use cw_multi_test::Executor;
use dexter::asset::{Asset, AssetInfo};
use dexter::pool::{AfterJoinResponse, QueryMsg as PoolQueryMsg, SwapResponse};
use dexter::vault::{ExecuteMsg, QueryMsg, PoolInfoResponse, SingleSwapRequest, SwapType};

pub mod utils;

#[test]
fn test_join_pool() {
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
        instantiate_contracts_scaling_factor(&mut app, &owner);

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
    assert_eq!(join_pool_query_res.new_shares, Uint128::new(200_000_000_000));

    let msg = ExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: None,
        lp_to_mint: None,
        auto_stake: None,
        slippage_tolerance: None,
        assets: Some(assets_msg.clone()),
    };

    // Execute the join pool message
    app
        .execute_contract(
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
    assert_eq!(join_pool_query_res.new_shares, Uint128::new(2_020_101_947));

    let msg = ExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: None,
        lp_to_mint: None,
        auto_stake: None,
        slippage_tolerance: None,
        assets: Some(imbalanced_assets_msg),
    };

    // Execute the join pool message
    app
        .execute_contract(
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
        instantiate_contracts_scaling_factor(&mut app, &owner);

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

    let msg = ExecuteMsg::JoinPool {
        pool_id: Uint128::from(1u128),
        recipient: None,
        lp_to_mint: None,
        auto_stake: None,
        slippage_tolerance: None,
        assets: Some(assets_msg.clone()),
    };

    // Execute the join pool message
    app
        .execute_contract(
            alice_address.clone(),
            vault_addr.clone(),
            &msg,
            &[
                Coin {
                    denom: "uatom".to_string(),
                    amount: Uint128::new(1_000_000_000_000u128),
                },
                Coin {
                    denom: "ustkatom".to_string(),
                    amount: Uint128::new(980_000_000_000u128),
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

    let swap_query_msg = PoolQueryMsg::OnSwap { 
        swap_type: SwapType::GiveIn {}, 
        offer_asset: AssetInfo::NativeToken {
            denom: "ustkatom".to_string(),
        }, 
        ask_asset: AssetInfo::NativeToken {
            denom: "uatom".to_string(),
        },
        amount: Uint128::from(10_000_000u128),
        max_spread: None,
        belief_price: None
    };

    let swap_query_res: SwapResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &swap_query_msg,
        )
        .unwrap();

    println!("Swap query response: {:?}", swap_query_res);
    assert_eq!(swap_query_res.trade_params.amount_out, Uint128::from(9_897_951u128));
    assert_eq!(swap_query_res.trade_params.spread, Uint128::from(8u128));
    assert_eq!(swap_query_res.trade_params.amount_in, Uint128::from(10_000_000u128));
    assert_eq!(swap_query_res.fee, Some(
        Asset { 
            info: AssetInfo::NativeToken { 
                denom: "ustkatom".to_string() 
            },
            amount: Uint128::from(300000u128) 
        }));
    

    // Test swap
    let swap_msg = ExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(1u128),
            swap_type: SwapType::GiveIn {},
            asset_in: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            asset_out: AssetInfo::NativeToken {
                denom: "uatom".to_string(),
            },
            amount: Uint128::from(10_000_000u128),
            max_spread: Some(Decimal::from_ratio(20u128, 100u128)),
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };

    app
        .execute_contract(
            alice_address.clone(),
            vault_addr.clone(),
            &swap_msg,
            &[Coin {
                denom: "ustkatom".to_string(),
                amount: Uint128::new(10_000_000u128),
            }],
        )
        .unwrap(); 
}