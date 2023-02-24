pub mod utils;

use cosmwasm_std::{to_binary, Addr, Coin, Uint128};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};
use cw_multi_test::Executor;
use dexter::asset::{Asset, AssetInfo};

use dexter::vault::{Cw20HookMsg, ExecuteMsg};

use crate::utils::{
    initialize_3_tokens, initialize_stable_5_pool,
    initialize_weighted_pool, instantiate_contract, mint_some_tokens,
    mock_app, set_keeper_contract_in_config
};

#[test]
fn test_exit_pool() {
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
    let (_, stable5_lp_token_addr, stable5_pool_id) = initialize_stable_5_pool(
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
    let (_, weighted_lp_token_addr, weighted_pool_id) = initialize_weighted_pool(
        &mut app,
        &Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        token_instance1.clone(),
        token_instance2.clone(),
        token_instance3.clone(),
        denom0.clone(),
        denom1.clone(),
    );

    // Update config to set keeper contract address
    set_keeper_contract_in_config(&mut app, owner.clone(), vault_instance.clone());

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

    // -------x---------- Stable-5-swap-POOL -::- WITHDRAW LIQUIDITY -------x---------
    // -------x---------- -------x---------- -------x---------- -------x--------------

    // When you withdraw only 1 token from a stable-5-pool
    // VAULT -::- Exit Pool -::- Execution Function
    // Stable-5-Pool : Imbalanced Withdraw
    // Stable-5-Pool : Initial D : 5000
    // Stable-5-Pool : Withdraw D : 4899.612056677904874438
    // For token0, fee is charged on 79.922411335580974887 amount, which is difference b/w 979.922411335580974887 (ideal_balance) and 900 (new_balance). Fee charged:0.749272606271071639
    // For contract1, fee is charged on 20.077588664419025113 amount, which is difference b/w 979.922411335580974887 (ideal_balance) and 1000 (new_balance). Fee charged:0.18822739372892836
    // For contract2, fee is charged on 20.077588664419025113 amount, which is difference b/w 979.922411335580974887 (ideal_balance) and 1000 (new_balance). Fee charged:0.18822739372892836
    // For contract3, fee is charged on 20.077588664419025113 amount, which is difference b/w 979.922411335580974887 (ideal_balance) and 1000 (new_balance). Fee charged:0.18822739372892836
    // For token1, fee is charged on 20.077588664419025113 amount, which is difference b/w 979.922411335580974887 (ideal_balance) and 1000 (new_balance). Fee charged:0.18822739372892836
    // Stable-5-Pool : After Fee D : 4898.105276502220535553
    // Stable-5-Pool : Total Share : 5000000000
    // Stable-5-Pool : Burn Amount : 101894724
    // act_burn_amount: 101894724
    // Transfering total "0" "contract1" to the User. Total Fee : "188227" (protocol_fee="92231", dev_fee="28234" LP fee="67762"). Liquidity withdrawn = "120465" "contract1"
    // Transfering total "0" "contract2" to the User. Total Fee : "188227" (protocol_fee="92231", dev_fee="28234" LP fee="67762"). Liquidity withdrawn = "120465" "contract2"
    // Transfering total "0" "contract3" to the User. Total Fee : "188227" (protocol_fee="92231", dev_fee="28234" LP fee="67762"). Liquidity withdrawn = "120465" "contract3"
    // Transfering total "100000000" "token0" to the User. Total Fee : "749272" (protocol_fee="367143", dev_fee="112390" LP fee="269739"). Liquidity withdrawn = "100479533" "token0"
    // Transfering total "0" "token1" to the User. Total Fee : "188227" (protocol_fee="92231", dev_fee="28234" LP fee="67762"). Liquidity withdrawn = "120465" "token1"
    // test test_exit_pool ... ok
    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(500_000000u128),
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(stable5_pool_id),
            recipient: None,
            assets: Some(vec![Asset {
                info: AssetInfo::NativeToken {
                    denom: denom0.clone(),
                },
                amount: Uint128::from(100_000000u128),
            }]),
            burn_amount: Some(Uint128::from(500_000000u128)),
        })
        .unwrap(),
    };
    app.execute_contract(owner.clone(), stable5_lp_token_addr.clone(), &exit_msg, &[])
        .unwrap();

    // When you withdraw multiple tokens from a stable-5-pool
    // VAULT -::- Exit Pool -::- Execution Function
    // Stable-5-Pool : Imbalanced Withdraw
    // Stable-5-Pool : Initial D : 4898.647724421098219454
    // Stable-5-Pool : Withdraw D : 4508.309241082299087282
    // For token0, fee is charged on 53.32359569956993967 amount, which is difference b/w 827.84406269956993967 (ideal_balance) and 774.520467 (new_balance). Fee charged:0.499908709683468184
    // For token1, fee is charged on 74.673306424484064835 amount, which is difference b/w 920.206228575515935165 (ideal_balance) and 994.879535 (new_balance). Fee charged:0.700062247729538107
    // For contract2, fee is charged on 177.326693575515935165 amount, which is difference b/w 920.206228575515935165 (ideal_balance) and 742.879535 (new_balance). Fee charged:1.662437752270461892
    // For contract1, fee is charged on 79.673306424484064835 amount, which is difference b/w 920.206228575515935165 (ideal_balance) and 999.879535 (new_balance). Fee charged:0.746937247729538107
    // For contract3, fee is charged on 79.673306424484064835 amount, which is difference b/w 920.206228575515935165 (ideal_balance) and 999.879535 (new_balance). Fee charged:0.746937247729538107
    // Stable-5-Pool : After Fee D : 4503.934926829800769923
    // Stable-5-Pool : Total Share : 4898105276
    // Stable-5-Pool : Burn Amount : 394669090
    // act_burn_amount: 394669090
    // Transfering total "0" "contract1" to the User. Total Fee : "746937" (protocol_fee="365999", dev_fee="112040" LP fee="268898"). Liquidity withdrawn = "478039" "contract1"
    // Transfering total "257000000" "contract2" to the User. Total Fee : "1662437" (protocol_fee="814594", dev_fee="249365" LP fee="598478"). Liquidity withdrawn = "258063959" "contract2"
    // Transfering total "0" "contract3" to the User. Total Fee : "746937" (protocol_fee="365999", dev_fee="112040" LP fee="268898"). Liquidity withdrawn = "478039" "contract3"
    // Transfering total "125000000" "token0" to the User. Total Fee : "499908" (protocol_fee="244954", dev_fee="74986" LP fee="179968"). Liquidity withdrawn = "125319940" "token0"
    // Transfering total "5000000" "token1" to the User. Total Fee : "700062" (protocol_fee="343030", dev_fee="105009" LP fee="252023"). Liquidity withdrawn = "5448039" "token1"

    let cur_user_lp_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &stable5_lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();

    let vault_token1_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    let vault_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    let vault_token3_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance3.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    let keeper_token1_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    let keeper_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();

    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(500_000000u128),
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(stable5_pool_id),
            recipient: None,
            assets: Some(vec![
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: denom0.clone(),
                    },
                    amount: Uint128::from(125_000000u128),
                },
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: denom1.clone(),
                    },
                    amount: Uint128::from(5_000000u128),
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: token_instance2.clone(),
                    },
                    amount: Uint128::from(257_000000u128),
                },
            ]),
            burn_amount: Some(Uint128::from(500_000000u128)),
        })
        .unwrap(),
    };
    app.execute_contract(owner.clone(), stable5_lp_token_addr.clone(), &exit_msg, &[])
        .unwrap();

    // Checks -
    // - Tokens transferred to the Vault.
    // - Fee transferred correctly
    // - LP tokens burnt & returned correctly
    let new_user_lp_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &stable5_lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(394669090u128),
        cur_user_lp_balance.balance - new_user_lp_balance.balance
    );

    let new_vault_token1_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(478039u128),
        vault_token1_balance.balance - new_vault_token1_balance.balance
    );

    let new_vault_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(258063959u128),
        vault_token2_balance.balance - new_vault_token2_balance.balance
    );
    let new_vault_token3_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance3.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(478039u128),
        vault_token3_balance.balance - new_vault_token3_balance.balance
    );

    // FEE CHECKS
    let new_keeper_token1_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(478039u128),
        new_keeper_token1_balance.balance - keeper_token1_balance.balance
    );

    let new_keeper_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(1063959u128),
        new_keeper_token2_balance.balance - keeper_token2_balance.balance
    );

    // When its normal withdraw from a stable-5-pool. No fee charged
    // VAULT -::- Exit Pool -::- Execution Function
    // act_burn_amount: 50000000
    // Transfering total "11095988" "contract1" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "11095988" "contract1"
    // Transfering total "8236106" "contract2" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "8236106" "contract2"
    // Transfering total "11095988" "contract3" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "11095988" "contract3"
    // Transfering total "8595664" "token0" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "8595664" "token0"
    // Transfering total "11040808" "token1" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "11040808" "token1"
    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(50_000000u128),
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(stable5_pool_id),
            recipient: None,
            assets: None,
            burn_amount: Some(Uint128::from(50_000000u128)),
        })
        .unwrap(),
    };
    app.execute_contract(owner.clone(), stable5_lp_token_addr.clone(), &exit_msg, &[])
        .unwrap();

    // -------x---------- Weighted POOL -::- WITHDRAW LIQUIDITY -------x---------
    // -------x---------- -------x---------- -------x---------- -------x---------

    // No Fee charged by weighted pool
    // VAULT -::- Exit Pool -::- Execution Function
    // Transfering total "49500000" "contract1" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "49500000" "contract1"
    // Transfering total "49500000" "contract2" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "49500000" "contract2"
    // Transfering total "49500000" "contract3" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "49500000" "contract3"
    // Transfering total "49500000" "token0" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "49500000" "token0"
    // Transfering total "49500000" "token1" to the User. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity withdrawn = "49500000" "token1"
    let exit_msg = Cw20ExecuteMsg::Send {
        contract: vault_instance.clone().to_string(),
        amount: Uint128::from(5000_000u128),
        msg: to_binary(&Cw20HookMsg::ExitPool {
            pool_id: Uint128::from(weighted_pool_id),
            recipient: None,
            assets: None,
            burn_amount: Some(Uint128::from(5000_000u128)),
        })
        .unwrap(),
    };
    app.execute_contract(
        owner.clone(),
        weighted_lp_token_addr.clone(),
        &exit_msg,
        &[],
    )
    .unwrap();
}
