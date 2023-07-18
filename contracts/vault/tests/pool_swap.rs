pub mod utils;

use cosmwasm_std::{Addr, Coin, Decimal, Timestamp, Uint128};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};
use cw_multi_test::Executor;
use dexter::asset::{Asset, AssetInfo};

use dexter::vault::{ExecuteMsg, PauseInfo, PoolType, SingleSwapRequest, SwapType, SudoMsg};

use crate::utils::{
    initialize_3_tokens, initialize_stable_5_pool,
    initialize_weighted_pool, instantiate_contract, mint_some_tokens,
    mock_app, set_keeper_contract_in_config
};

#[test]
fn test_swap() {
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
    set_keeper_contract_in_config(&mut app, owner.clone(), vault_instance.clone());

    let (token_instance1, token_instance2, token_instance3) =
        initialize_3_tokens(&mut app, &owner.clone());

    // Mint Tokens
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        Uint128::new(10000000_000000u128),
        owner.clone().to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance2.clone(),
        Uint128::new(10000000_000000u128),
        owner.clone().to_string(),
    );
    mint_some_tokens(
        &mut app,
        owner.clone(),
        token_instance3.clone(),
        Uint128::new(10000000_000000u128),
        owner.clone().to_string(),
    );

    // Increase Allowances
    app.execute_contract(
        owner.clone(),
        token_instance1.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: vault_instance.clone().to_string(),
            amount: Uint128::from(1000000_000000u128),
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
            amount: Uint128::from(1000000_000000u128),
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
            amount: Uint128::from(1000000_000000u128),
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

    // pause swaps for all pools
    let msg = SudoMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: None,
        auto_stake_impl: None,
        pool_creation_fee: None,
        paused: Some(PauseInfo{deposit: false, swap: true, imbalanced_withdraw: false}),
    };
    app.wasm_sudo(
        vault_instance.clone(),
        &msg,
    )
        .unwrap();

    // Provide liquidity to empty stable 5 pool. No fee is charged
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance3.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
    ];
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(stable5_pool_id),
            recipient: None,
            min_lp_to_receive: None,
            auto_stake: None,
            assets: Some(assets_msg.clone()),
        },
        &[
            Coin {
                denom: denom0.clone(),
                amount: Uint128::new(1000_000000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(1000_000000u128),
            },
        ],
    )
    .unwrap();

    // Liquidity provided to empty Weighted pool - No fee is charged
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance3.clone(),
            },
            amount: Uint128::from(1000_000000u128),
        },
    ];
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(weighted_pool_id),
            recipient: None,
            min_lp_to_receive: None,
            auto_stake: None,
            assets: Some(assets_msg.clone()),
        },
        &[
            Coin {
                denom: denom0.clone(),
                amount: Uint128::new(1000_000000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(1000_000000u128),
            },
        ],
    )
    .unwrap();

    let _current_block = app.block_info();
    app.update_block(|b| {
        b.height += 10;
        b.time = Timestamp::from_seconds(_current_block.time.seconds() + 90)
    });

    // -------x---------- Stable-5-swap-POOL -::- SWAP TOKENS -------x---------
    // -------x---------- -------x---------- -------x---------- -------x--------------
    // Execute Swap :: GiveIn Type
    // VAULT -::- Swap -::- Execution Function
    // Offer asset: token1 Ask asset: contract2 Swap Type: "give-in" Amount: 252000000
    // --- StableSwap Pool:OnSwap Query :: Start ---
    // SwapType::GiveIn
    // In compute_swap() fn, we calculate the new ask pool balance which is 753939768 and calculate the return amount (cur_pool_balance - new_pool_balance) which is 246060232
    // fee yet to be charged: 7381806, hence return amount (actual return amount - total_fee) = 238678426
    // VAULT -::- Swap -::- Pool Swap Transition Query Response returned - amount_in:252000000 amount_out:238678426 spread:5939768. Response: success
    // Fee: 7381806 contract2
    // Protocol Fee: 3617084 Dev Fee: 1107270
    // Ask Asset ::: Pool Liquidity being updated. Current pool balance: 1000000000. Ask Asset Amount: 238678426
    // Ask Asset ::: Pool Liquidity after subtracting the ask asset amount to be transferred 761321574
    // Fee Asset ::: Pool Liquidity being updated. Protocol and dev fee to be subtracted. Current pool liquidity 761321574
    // Fee Asset ::: Pool Liquidity after being updated: 756597220
    // Offer Asset ::: Pool Liquidity being updated. Current pool balance: 1000000000. Offer Asset Amount: 252000000
    // Offer Asset ::: Pool Liquidity after adding offer asset amount provided 1252000000
    let swap_msg = ExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(stable5_pool_id),
            swap_type: SwapType::GiveIn {},
            asset_in: AssetInfo::NativeToken {
                denom: denom1.to_string(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(252_000000u128),
            max_spread: Some(Decimal::percent(50)),
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };

    // swap should fail because of the pause
    assert_eq!("Swaps are paused", app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: denom1.to_string(),
            amount: Uint128::new(252_000000u128),
        }],
    )
    .unwrap_err().root_cause().to_string());

    // resume swaps for all pools
    let msg = SudoMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: None,
        auto_stake_impl: None,
        pool_creation_fee: None,
        paused: Some(PauseInfo{deposit: false, swap: false, imbalanced_withdraw: false}),
    };
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
        .unwrap();

    // pause swaps specifically for stable 5 pool type
    let msg = SudoMsg::UpdatePoolTypeConfig {
        pool_type: PoolType::StableSwap {},
        allow_instantiation: None,
        new_fee_info: None,
        paused: Some(PauseInfo{deposit: false, swap: true, imbalanced_withdraw: false}),
    };
    app.wasm_sudo(
        vault_instance.clone(),
        &msg,
    )
        .unwrap();

    // swap should still fail because of the pause
    assert_eq!("Swaps are paused", app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: denom1.to_string(),
            amount: Uint128::new(252_000000u128),
        }],
    )
        .unwrap_err().root_cause().to_string());

    // resume swaps specifically for stable 5 pool type
    let msg = SudoMsg::UpdatePoolTypeConfig {
        pool_type: PoolType::StableSwap {},
        allow_instantiation: None,
        new_fee_info: None,
        paused: Some(PauseInfo{deposit: false, swap: false, imbalanced_withdraw: false}),
    };
    app.wasm_sudo(
        vault_instance.clone(),
        &msg,
    )
        .unwrap();

    // pause swaps specifically for stable 5 pool id
    let msg = SudoMsg::UpdatePoolConfig {
        pool_id: stable5_pool_id,
        fee_info: None,
        paused: Some(PauseInfo{deposit: false, swap: true, imbalanced_withdraw: false}),
    };
    app.wasm_sudo(
        vault_instance.clone(),
        &msg,
    )
        .unwrap();

    // swap should still fail because of the pause
    assert_eq!("Swaps are paused", app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: denom1.to_string(),
            amount: Uint128::new(252_000000u128),
        }],
    )
        .unwrap_err().root_cause().to_string());

    // resume swaps specifically for stable 5 pool id
    let msg = SudoMsg::UpdatePoolConfig {
        pool_id: stable5_pool_id,
        fee_info: None,
        paused: Some(PauseInfo{deposit: false, swap: false, imbalanced_withdraw: false}),
    };
    app.wasm_sudo(
        vault_instance.clone(),
        &msg
    )
        .unwrap();

    // swapping now should work
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: denom1.to_string(),
            amount: Uint128::new(252_000000u128),
        }],
    )
    .unwrap();

    let user_ask_token_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();
    let vault_ask_token_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    let keeper_ask_token_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    let keeper_offer_token_balance: Uint128 = app
        .wrap()
        .query_balance("fee_collector".to_string(), denom1.to_string())
        .unwrap().amount;

    // Execute Swap :: GiveOut Type
    // VAULT -::- Swap -::- Execution Function
    // Offer asset: token1 Ask asset: contract2 Swap Type: "give-out" Amount: 252000000
    // --- StableSwap Pool:OnSwap Query :: Start ---
    // SwapType::GiveOut
    // In compute_offer_amount() fn, we calculate the new ask offer pool balance which is 1537249235 based on updated ask_pool balance which includes ask_amount + total fee yet to be charged. ask_amount = 252000000, ask_amount_before_commission = 259.793814
    // offer amount = 285249235, total fee = 7793814
    // VAULT -::- Swap -::- Pool Swap Transition Query Response returned - amount_in:285249235 amount_out:252000000 spread:25455421. Response: success
    // Fee: 7793814 contract2
    // Protocol Fee: 3818968 Dev Fee: 1169072
    // Ask Asset ::: Pool Liquidity being updated. Current pool balance: 756597220. Ask Asset Amount: 252000000
    // Ask Asset ::: Pool Liquidity after subtracting the ask asset amount to be transferred 504597220
    // Fee Asset ::: Pool Liquidity being updated. Protocol and dev fee to be subtracted. Current pool liquidity 504597220
    // Fee Asset ::: Pool Liquidity after being updated: 499609180
    // Offer Asset ::: Pool Liquidity being updated. Current pool balance: 1252000000. Offer Asset Amount: 285249235
    // Offer Asset ::: Pool Liquidity after adding offer asset amount provided 1537249235
    let swap_msg = ExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(stable5_pool_id),
            swap_type: SwapType::GiveOut {},
            asset_in: AssetInfo::NativeToken {
                denom: denom1.to_string(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(252_000000u128),
            max_spread: Some(Decimal::percent(50)),
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: denom1.to_string(),
            amount: Uint128::new(292_000000u128),
        }],
    )
    .unwrap();

    // Checks if tokens are transferred correctly
    let new_user_ask_token_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();
    let new_vault_ask_token_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    let new_keeper_ask_token_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    let new_keeper_offer_token_balance: Uint128 = app
        .wrap()
        .query_balance("fee_collector".to_string(), denom1.to_string())
        .unwrap().amount;

    assert_eq!(
        Uint128::from(252000000u128),
        vault_ask_token_balance.balance - new_vault_ask_token_balance.balance
    );

    assert_eq!(
        Uint128::from(252000000u128),
        new_user_ask_token_balance.balance - user_ask_token_balance.balance
    );

    // Fee is not charged in ask token but in offer token
    assert_eq!(
        Uint128::from(0u128),
        new_keeper_ask_token_balance.balance - keeper_ask_token_balance.balance
    );
    assert_eq!(
        Uint128::from(5457256u128),
        new_keeper_offer_token_balance - keeper_offer_token_balance
    );

    // VAULT -::- Swap -::- Execution Function
    // Offer asset: token1 Ask asset: contract2 Swap Type: "give-out" Amount: 252000000
    // --- StableSwap Pool:OnSwap Query :: Start ---
    // SwapType::GiveOut
    // In compute_offer_amount() fn, we calculate the new ask offer pool balance which is 1537249235 based on updated ask_pool balance which includes ask_amount + total fee yet to be charged. ask_amount = 252000000, ask_amount_before_commission = 259.793814
    // offer amount = 285249235, total fee = 7793814
    // VAULT -::- Swap -::- Pool Swap Transition Query Response returned - amount_in:285249235 amount_out:252000000 spread:25455421. Response: success
    // Fee: 7793814 contract2
    // Protocol Fee: 3818968 Dev Fee: 1169072
    // Ask Asset ::: Pool Liquidity being updated. Current pool balance: 756597220. Ask Asset Amount: 252000000
    // Ask Asset ::: Pool Liquidity after subtracting the ask asset amount to be transferred 504597220
    // Fee Asset ::: Pool Liquidity being updated. Protocol and dev fee to be subtracted. Current pool liquidity 504597220
    // Fee Asset ::: Pool Liquidity after being updated: 499609180
    // Offer Asset ::: Pool Liquidity being updated. Current pool balance: 1252000000. Offer Asset Amount: 285249235
    // Offer Asset ::: Pool Liquidity after adding offer asset amount provided 1537249235

    // -------x---------- Weighted POOL -::- SWAP TOKENS -------x----------------
    // -------x---------- -------x---------- -------x---------- -------x---------

    // VAULT -::- Swap -::- Execution Function
    // Offer asset: token1 Ask asset: contract2 Swap Type: "give-in" Amount: 252000000
    // --- Weighted:OnSwap Query :: Start ---
    // SwapType::GiveIn
    // In compute_swap() fn in weighted pool, we solve for constant function variant with updated offer pool balance and calculate the return amount, which is 201277955
    // fee yet to be charged: 6038338, hence return amount (actual return amount - total_fee) = 195239617
    // VAULT -::- Swap -::- Pool Swap Transition Query Response returned - amount_in:252000000 amount_out:195239617 spread:0. Response: success
    // Fee: 6038338 contract2
    // Protocol Fee: 2958785 Dev Fee: 905750
    // Ask Asset ::: Pool Liquidity being updated. Current pool balance: 1000000000. Ask Asset Amount: 195239617
    // Ask Asset ::: Pool Liquidity after subtracting the ask asset amount to be transferred 804760383
    // Fee Asset ::: Pool Liquidity being updated. Protocol and dev fee to be subtracted. Current pool liquidity 804760383
    // Fee Asset ::: Pool Liquidity after being updated: 800895848
    // Offer Asset ::: Pool Liquidity being updated. Current pool balance: 1000000000. Offer Asset Amount: 252000000
    // Offer Asset ::: Pool Liquidity after adding offer asset amount provided 1252000000
    let swap_msg = ExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(weighted_pool_id),
            swap_type: SwapType::GiveIn {},
            asset_in: AssetInfo::NativeToken {
                denom: denom1.to_string(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(252_000000u128),
            max_spread: Some(Decimal::percent(50)),
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: denom1.to_string(),
            amount: Uint128::new(252_000000u128),
        }],
    )
    .unwrap();

    // VAULT -::- Swap -::- Execution Function
    // Offer asset: token1 Ask asset: contract2 Swap Type: "give-out" Amount: 252000000
    // --- Weighted:OnSwap Query :: Start ---
    // SwapType::GiveOut
    // In compute_offer_amount() fn, we calculate the new ask offer pool balance which is 541.102033567010309468 based on updated ask_pool balance which includes ask_amount + total fee yet to be charged. ask_amount = 252, ask_amount_before_commission = 259.793814432989690532
    // VAULT -::- Swap -::- Pool Swap Transition Query Response returned - amount_in:601110022 amount_out:252000000 spread:0. Response: success
    // Fee: 7793814 contract2
    // Protocol Fee: 3818968 Dev Fee: 1169072
    // Ask Asset ::: Pool Liquidity being updated. Current pool balance: 800895848. Ask Asset Amount: 252000000
    // Ask Asset ::: Pool Liquidity after subtracting the ask asset amount to be transferred 548895848
    // Fee Asset ::: Pool Liquidity being updated. Protocol and dev fee to be subtracted. Current pool liquidity 548895848
    // Fee Asset ::: Pool Liquidity after being updated: 543907808
    // Offer Asset ::: Pool Liquidity being updated. Current pool balance: 1252000000. Offer Asset Amount: 601110022
    // Offer Asset ::: Pool Liquidity after adding offer asset amount provided 1853110022
    let swap_msg = ExecuteMsg::Swap {
        swap_request: SingleSwapRequest {
            pool_id: Uint128::from(weighted_pool_id),
            swap_type: SwapType::GiveOut {},
            asset_in: AssetInfo::NativeToken {
                denom: denom1.to_string(),
            },
            asset_out: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(252_000000u128),
            max_spread: Some(Decimal::percent(50)),
            belief_price: None,
        },
        recipient: None,
        min_receive: None,
        max_spend: None,
    };
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &swap_msg,
        &[Coin {
            denom: denom1.to_string(),
            amount: Uint128::new(792_000000u128),
        }],
    )
    .unwrap();
}
