use cosmwasm_std::{Addr, Coin, Uint128};
use cw_multi_test::Executor;
use dexter::{
    asset::{Asset, AssetInfo},
    vault::FeeInfo,
};

use crate::utils::assert_cw20_balance;

mod utils;

/// Test the exit pool execute message within the keeper contract
/// It should follow the following steps:
/// 1. Create a new dexter keeper and vault. Register keeper in the vault and vault in the keeper.
/// 2. Create a weighted pool of uxprt and uatom.
/// 3. Add liquidity to the pool.
/// 4. Send some liquidity tokens to the keeper contract.
/// 5. Perform an exit from the pool.
/// 6. Check that the keeper contract has the correct amount of tokens.
/// 6. Perform a swap of an asset now present in keeper for a different asset
/// 7. Validate that the swap was successful and the keeper has the correct amount of expected input/output tokens
#[test]
fn test_exit_and_swap() {
    let owner: Addr = Addr::unchecked("owner".to_string());
    let keeper_owner: Addr = Addr::unchecked("keeper_owner".to_string());
    let alice_address: Addr = Addr::unchecked("alice".to_string());

    let mut app = utils::mock_app(
        owner.clone(),
        vec![
            Coin {
                denom: "uxprt".to_string(),
                amount: uint128_with_precision!(100_000u128, 6),
            },
            Coin {
                denom: "uatom".to_string(),
                amount: uint128_with_precision!(100_000u128, 6),
            },
        ],
    );

    let fee_info = FeeInfo {
        total_fee_bps: 30,
        protocol_fee_percent: 20,
    };

    let asset_infos = vec![
        AssetInfo::native_token("uatom".to_string()),
        AssetInfo::native_token("uxprt".to_string()),
    ];

    let native_asset_precisions = vec![("uatom".to_string(), 6u8), ("uxprt".to_string(), 6u8)];

    let (vault_addr, keeper_addr, pool_id, _pool_addr, lp_token_addr) =
        utils::instantiate_contracts(
            &mut app,
            &owner,
            &keeper_owner,
            fee_info,
            asset_infos,
            native_asset_precisions,
        );

    // send some funds to alice
    app.send_tokens(
        owner.clone(),
        alice_address.clone(),
        &vec![
            Coin {
                denom: "uatom".to_string(),
                amount: uint128_with_precision!(100u128, 6),
            },
            Coin {
                denom: "uxprt".to_string(),
                amount: uint128_with_precision!(100u128, 6),
            },
        ],
    )
    .unwrap();

    // Join pool from a user to add liquidity
    let join_msg = dexter::vault::ExecuteMsg::JoinPool {
        pool_id,
        recipient: None,
        assets: Some(vec![
            Asset::new_native("uatom".to_string(), uint128_with_precision!(50u128, 6)),
            Asset::new_native("uxprt".to_string(), uint128_with_precision!(50u128, 6)),
        ]),
        min_lp_to_receive: None,
        auto_stake: None,
    };

    // send the message
    app.execute_contract(
        alice_address.clone(),
        vault_addr.clone(),
        &join_msg,
        &[
            Coin {
                denom: "uatom".to_string(),
                amount: uint128_with_precision!(50u128, 6),
            },
            Coin {
                denom: "uxprt".to_string(),
                amount: uint128_with_precision!(50u128, 6),
            },
        ],
    )
    .unwrap();

    // verify is LP token balance is correct for alice. LP Tokens are CW20
    assert_cw20_balance(
        &app,
        &lp_token_addr,
        &alice_address,
        uint128_with_precision!(100u128, 18),
    );

    // send some LP tokens from Alice to keeper contract
    let send_msg = cw20::Cw20ExecuteMsg::Transfer {
        recipient: keeper_addr.clone().into(),
        amount: uint128_with_precision!(20u128, 18),
    };

    app.execute_contract(alice_address.clone(), lp_token_addr.clone(), &send_msg, &[])
        .unwrap();

    // verify is LP token balance is correct for keeper. LP Tokens are CW20
    assert_cw20_balance(
        &app,
        &lp_token_addr,
        &keeper_addr,
        uint128_with_precision!(20u128, 18),
    );

    // Exit pool from keeper
    let exit_msg = dexter::keeper::ExecuteMsg::ExitLPTokens {
        lp_token_address: lp_token_addr.to_string(),
        amount: uint128_with_precision!(10u128, 18),
        min_assets_received: None,
    };

    // send the message
    app.execute_contract(keeper_owner.clone(), keeper_addr.clone(), &exit_msg, &[])
        .unwrap();

    // verify is LP token balance is correct for keeper
    assert_cw20_balance(
        &app,
        &lp_token_addr,
        &keeper_addr,
        uint128_with_precision!(10u128, 18),
    );

    // verify is LP token balance is correct for alice
    assert_cw20_balance(
        &app,
        &lp_token_addr,
        &alice_address,
        uint128_with_precision!(80u128, 18),
    );

    // validate if the exit was successful and relevant tokens were transferred to the keeper
    let uatom_balance = app
        .wrap()
        .query_balance(keeper_addr.clone(), "uatom".to_string())
        .unwrap();

    let uxprt_balance = app
        .wrap()
        .query_balance(keeper_addr.clone(), "uxprt".to_string())
        .unwrap();

    assert_eq!(uatom_balance.amount, uint128_with_precision!(5u128, 6));
    assert_eq!(uxprt_balance.amount, uint128_with_precision!(5u128, 6));

    // try to swap uatom for uxprt
    let swap_msg = dexter::keeper::ExecuteMsg::SwapAsset {
        offer_asset: Asset::new_native("uatom".to_string(), uint128_with_precision!(5u128, 6)),
        ask_asset_info: AssetInfo::native_token("uxprt".to_string()),
        min_ask_amount: None,
        pool_id,
    };

    // send the message
    app.execute_contract(keeper_owner.clone(), keeper_addr.clone(), &swap_msg, &[])
        .unwrap();

    // validate if the swap was successful and relevant tokens were transferred to the keeper
    let uatom_balance = app
        .wrap()
        .query_balance(keeper_addr.clone(), "uatom".to_string())
        .unwrap();

    let uxprt_balance = app
        .wrap()
        .query_balance(keeper_addr.clone(), "uxprt".to_string())
        .unwrap();

    assert_eq!(uatom_balance.amount, Uint128::from(3000u64));
    assert_eq!(uxprt_balance.amount, Uint128::from(9487846u64));
}
