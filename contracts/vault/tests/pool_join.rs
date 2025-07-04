pub mod utils;

use cosmwasm_std::{Addr, Coin, Uint128};
use cw20::{BalanceResponse, Cw20QueryMsg};
use cw_multi_test::Executor;
use dexter::asset::{Asset, AssetInfo};

use dexter::pool::{
    AfterJoinResponse, ConfigResponse as Pool_ConfigResponse, QueryMsg as PoolQueryMsg,
};
use dexter::vault::{ExecuteMsg, PauseInfo, PoolInfoResponse, PoolType, QueryMsg};
use dexter::vault::PoolType::{StableSwap, Weighted};

use crate::utils::{
    increase_token_allowance, initialize_3_tokens,
    initialize_multistaking_contract,
    initialize_stable_5_pool, initialize_weighted_pool,
    instantiate_contract, mint_some_tokens, mock_app,
    set_keeper_contract_in_config
};

#[test]
fn test_join_pool() {
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

    // Set keeper contract
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
    increase_token_allowance(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        vault_instance.clone().into_string(),
        Uint128::new(10000000_000000u128),
    );

    increase_token_allowance(
        &mut app,
        owner.clone(),
        token_instance2.clone(),
        vault_instance.clone().into_string(),
        Uint128::new(10000000_000000u128),
    );

    increase_token_allowance(
        &mut app,
        owner.clone(),
        token_instance3.clone(),
        vault_instance.clone().into_string(),
        Uint128::new(10000000_000000u128),
    );

    // check pools query before creating pools
    let pool_infos: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            &vault_instance,
            &QueryMsg::Pools {
                start_after: None,
                limit: None
            },
        )
        .unwrap();
    assert_eq!(pool_infos.len(), 0);

    // Create STABLE-5-POOL pool
    let (stable5_pool_addr, stable5_lp_token_addr, stable5_pool_id) = initialize_stable_5_pool(
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

    // check pools query after creating pools
    let pool_infos: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            &vault_instance,
            &QueryMsg::Pools {
                start_after: None,
                limit: None
            },
        )
        .unwrap();
    assert_eq!(pool_infos.len(), 2);
    assert_eq!(pool_infos[0].pool_type, StableSwap {});
    assert_eq!(pool_infos[1].pool_type, Weighted {});

    // check pools query with custom start & limit
    let pool_infos: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            &vault_instance,
            &QueryMsg::Pools {
                start_after: Some(Uint128::one()),
                limit: Some(1)
            },
        )
        .unwrap();
    assert_eq!(pool_infos.len(), 1);
    assert_eq!(pool_infos[0].pool_type, Weighted {});

    // check pools query for non-existent pools
    let pool_infos: Vec<PoolInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            &vault_instance,
            &QueryMsg::Pools {
                start_after: Some(Uint128::from(2u128)),
                limit: None
            },
        )
        .unwrap();
    assert_eq!(pool_infos.len(), 0);

    // -------x---------- STABLE-5-POOL -::- PROVIDE LIQUIDITY -------x----------
    // -------x---------- -------x---------- -------x---------- -------x----------

    // VAULT -::- Join Pool -::- Execution Function
    // --- StableSwap Pool:OnJoinPool Query : Begin ---
    // init_d: 0
    // deposit_d: 5000
    // Fee will be charged only during imbalanced provide i.e. if invariant D was changed
    // --- StableSwap Pool:OnJoinPool Query :: End ---
    // Following assets are to be transferred by the user to the Vault:
    // ::: "contract1" "1000000000"
    // ::: "contract2" "1000000000"
    // ::: "contract3" "1000000000"
    // ::: "token0" "1000000000"
    // ::: "token1" "1000000000"
    // LP tokens to be minted: "5000000000"
    // Transfering total "1000000000" "contract1" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "contract1"
    // Transfering total "1000000000" "contract2" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "contract2"
    // Transfering total "1000000000" "contract3" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "contract3"
    // Transfering total "1000000000" "token0" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "token0"
    // Transfering total "1000000000" "token1" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "token1"
    let mut assets_msg = vec![
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
    // Check Query Response
    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            stable5_pool_addr.clone(),
            &PoolQueryMsg::OnJoinPool {
                assets_in: Some(assets_msg.clone()),
                mint_amount: None,
            },
        )
        .unwrap();

    // pause deposits for all pools
    let msg = ExecuteMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: None,
        auto_stake_impl: None,
        pool_creation_fee: None,
        paused: Some(PauseInfo{deposit: true, swap: false, imbalanced_withdraw: false}),
        reward_schedule_validation_assets: None,
    };
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
        .unwrap();

    let stable5_pool_join_msg = ExecuteMsg::JoinPool {
        pool_id: Uint128::from(stable5_pool_id),
        recipient: None,
        min_lp_to_receive: None,
        auto_stake: None,
        assets: Some(assets_msg.clone()),
    };

    // try to provide liquidity to empty stable 5 pool => should fail with paused error
    assert_eq!("Deposits are paused", app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &stable5_pool_join_msg,
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
    ).unwrap_err().root_cause().to_string());

    // resume deposits for all pools
    let msg = ExecuteMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: None,
        auto_stake_impl: None,
        pool_creation_fee: None,
        paused: Some(PauseInfo{deposit: false, swap: false, imbalanced_withdraw: false}),
        reward_schedule_validation_assets: None,
    };
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
        .unwrap();

    // pause deposits specifically for stable 5 pool type
    let msg = ExecuteMsg::UpdatePoolTypeConfig {
        pool_type: PoolType::StableSwap {},
        allow_instantiation: None,
        new_fee_info: None,
        paused: Some(PauseInfo{deposit: true, swap: false, imbalanced_withdraw: false}),
    };
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
        .unwrap();

    // try to provide liquidity to empty stable 5 pool => should still fail with paused error
    assert_eq!("Deposits are paused", app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &stable5_pool_join_msg,
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
    ).unwrap_err().root_cause().to_string());

    // resume deposits specifically for stable 5 pool type
    let msg = ExecuteMsg::UpdatePoolTypeConfig {
        pool_type: PoolType::StableSwap {},
        allow_instantiation: None,
        new_fee_info: None,
        paused: Some(PauseInfo{deposit: false, swap: false, imbalanced_withdraw: false}),
    };
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
        .unwrap();

    // pause deposits specifically for stable 5 pool id
    let msg = ExecuteMsg::UpdatePoolConfig {
        pool_id: stable5_pool_id,
        fee_info: None,
        paused: Some(PauseInfo{deposit: true, swap: false, imbalanced_withdraw: false}),
    };
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
        .unwrap();

    // try to provide liquidity to empty stable 5 pool => should still fail with paused error
    assert_eq!("Deposits are paused", app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &stable5_pool_join_msg,
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
    ).unwrap_err().root_cause().to_string());

    // resume deposits specifically for stable 5 pool id
    let msg = ExecuteMsg::UpdatePoolConfig {
        pool_id: stable5_pool_id,
        fee_info: None,
        paused: Some(PauseInfo{deposit: false, swap: false, imbalanced_withdraw: false}),
    };
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        vault_instance.clone(),
        &msg,
        &[],
    )
        .unwrap();

    // Provide liquidity to empty stable 5 pool by asking more LP that feasible => should fail
    assert_eq!("MinReceiveError - return amount 5000000000000000000000 is less than minimum requested amount 5000000000000000000001", app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(stable5_pool_id),
            recipient: None,
            min_lp_to_receive: Some(join_pool_query_res.new_shares + Uint128::one()), // just ask for more LP to receive than what would actually be received
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
    ).unwrap_err().root_cause().to_string());

    // Provide liquidity to empty stable 5 pool => should work. No fee is charged
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(stable5_pool_id),
            recipient: None,
            min_lp_to_receive: Some(join_pool_query_res.new_shares), // just ask for exact LP to receive as what would actually be received
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

    // Pool Config
    let pool_config_res: Pool_ConfigResponse = app
        .wrap()
        .query_wasm_smart(stable5_pool_addr.clone(), &PoolQueryMsg::Config {})
        .unwrap();

    // Checks -
    // - Pool Liquidity balances updates correctly
    // - Tokens transferred to the Vault.
    // - Fee transferred correctly - 0 fee charged
    // - LP tokens minted & transferred correctly
    let mut cur_user_lp_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &stable5_lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(join_pool_query_res.new_shares, cur_user_lp_balance.balance);
    assert_eq!(join_pool_query_res.provided_assets, pool_config_res.assets);

    let vault_token1_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1000_000000u128), vault_token1_balance.balance);
    let mut vault_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1000_000000u128), vault_token2_balance.balance);
    let vault_token3_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance3.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1000_000000u128), vault_token3_balance.balance);

    // VAULT -::- Join Pool -::- Execution Function
    // --- StableSwap Pool:OnJoinPool Query : Begin ---
    // init_d: 5000
    // deposit_d: 11036.237493754238660601
    // Fee will be charged only during imbalanced provide i.e. if invariant D was changed
    // For contract1, fee is charged on 245.75250124915226788 amount, which is difference b/w 2207.24749875084773212 (ideal_balance) and 2453 (new_balance). Fee charged:2.303929699210802511
    // For contract2, fee is charged on 262.24749875084773212 amount, which is difference b/w 2207.24749875084773212 (ideal_balance) and 1945 (new_balance). Fee charged:2.458570300789197488
    // For contract3, fee is charged on 355.75250124915226788 amount, which is difference b/w 2207.24749875084773212 (ideal_balance) and 2563 (new_balance). Fee charged:3.335179699210802511
    // For token0, fee is charged on 73.24749875084773212 amount, which is difference b/w 2207.24749875084773212 (ideal_balance) and 2134 (new_balance). Fee charged:0.686695300789197488
    // For token1, fee is charged on 259.24749875084773212 amount, which is difference b/w 2207.24749875084773212 (ideal_balance) and 1948 (new_balance). Fee charged:2.430445300789197488
    // after_fee_d (Invariant computed for - total tokens provided as liquidity - total fee): 11025.030251953726515704
    // --- StableSwap Pool:OnJoinPool Query :: End ---
    // Following assets are to be transferred by the user to the Vault:
    // ::: "contract1" "1453000000"
    // ::: "contract2" "945000000"
    // ::: "contract3" "1563000000"
    // ::: "token0" "1134000000"
    // ::: "token1" "948000000"
    // LP tokens to be minted: "6025030251"
    // Transfering total "1453000000" "contract1" to the Vault. Total Fee : "2303929" (protocol_fee="1128925", dev_fee="345589" LP fee="829415"). Liquidity updated with "1451525486" "contract1"
    // Transfering total "945000000" "contract2" to the Vault. Total Fee : "2458570" (protocol_fee="1204699", dev_fee="368785" LP fee="885086"). Liquidity updated with "943426516" "contract2"
    // Transfering total "1563000000" "contract3" to the Vault. Total Fee : "3335179" (protocol_fee="1634237", dev_fee="500276" LP fee="1200666"). Liquidity updated with "1560865487" "contract3"
    // Transfering total "1134000000" "token0" to the Vault. Total Fee : "686695" (protocol_fee="336480", dev_fee="103004" LP fee="247211"). Liquidity updated with "1133560516" "token0"
    // Transfering total "948000000" "token1" to the Vault. Total Fee : "2430445" (protocol_fee="1190918", dev_fee="364566" LP fee="874961"). Liquidity updated with "946444516" "token1"
    assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            amount: Uint128::from(1134_000000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom1.clone(),
            },
            amount: Uint128::from(948_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(1453_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(945_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance3.clone(),
            },
            amount: Uint128::from(1563_000000u128),
        },
    ];

    // Check Query Response
    let join_pool_query_res: AfterJoinResponse = app
        .wrap()
        .query_wasm_smart(
            stable5_pool_addr.clone(),
            &PoolQueryMsg::OnJoinPool {
                assets_in: Some(assets_msg.clone()),
                mint_amount: None,
            },
        )
        .unwrap();

    // Provide imbalanced liquidity to stable 5 pool. Fee is charged
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
                amount: Uint128::new(1155_000000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(10000_000000u128),
            },
        ],
    )
    .unwrap();

    // Checks -
    // - Pool Liquidity balances updates correctly
    // - Tokens transferred to the Vault.
    // - Fee transferred correctly - 0 fee charged
    // - LP tokens minted & transferred correctly
    let mut new_user_lp_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &stable5_lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        join_pool_query_res.new_shares,
        new_user_lp_balance.balance - cur_user_lp_balance.balance
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
        Uint128::from(1451525486u128),
        new_vault_token1_balance.balance - vault_token1_balance.balance
    );

    let mut new_vault_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(943426516u128),
        new_vault_token2_balance.balance - vault_token2_balance.balance
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
        Uint128::from(1560865486u128),
        new_vault_token3_balance.balance - vault_token3_balance.balance
    );

    // FEE CHECKS
    let keeper_token1_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance1.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1474514u128), keeper_token1_balance.balance);
    let mut keeper_token2_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1573484u128), keeper_token2_balance.balance);
    let keeper_token3_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &token_instance3.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(2134514u128), keeper_token3_balance.balance);

    // Provide only 2 of 5 assets liquidity to stable 5 pool. Fee is charged
    // VAULT -::- Join Pool -::- Execution Function
    // --- StableSwap Pool:OnJoinPool Query : Begin ---
    // init_d: 11029.064881254479322174
    // deposit_d: 12287.337905948465585852
    // Fee will be charged only during imbalanced provide i.e. if invariant D was changed
    // For contract1, fee is charged on 279.687210257190258222 amount, which is difference b/w 2731.212696257190258222 (ideal_balance) and 2451.525486 (new_balance). Fee charged:2.62206759616115867
    // For contract2, fee is charged on 523.280281521050150215 amount, which is difference b/w 2165.146234478949849785 (ideal_balance) and 2688.426516 (new_balance). Fee charged:4.905752639259845158
    // For contract3, fee is charged on 292.161483938556503376 amount, which is difference b/w 2853.026970938556503376 (ideal_balance) and 2560.865487 (new_balance). Fee charged:2.739013911923967219
    // For token0, fee is charged on 270.588462146246137888 amount, which is difference b/w 2376.972053853753862112 (ideal_balance) and 2647.560516 (new_balance). Fee charged:2.536766832621057542
    // For token1, fee is charged on 222.064033072200704588 amount, which is difference b/w 2168.508549072200704588 (ideal_balance) and 1946.444516 (new_balance). Fee charged:2.081850310051881605
    // after_fee_d (Invariant computed for - total tokens provided as liquidity - total fee): 12272.475667670006139966
    // --- StableSwap Pool:OnJoinPool Query :: End ---
    // Following assets are to be transferred by the user to the Vault:
    // ::: "contract1" "0"
    // ::: "contract2" "745000000"
    // ::: "contract3" "0"
    // ::: "token0" "514000000"
    // ::: "token1" "0"
    // LP tokens to be minted: "1242955924"
    // Transfering total "0" "contract1" to the Vault. Total Fee : "2622067" (protocol_fee="1284812", dev_fee="393310" LP fee="943945"). Liquidity updated by subtracting  "1678122" "contract1"
    // Transfering total "745000000" "contract2" to the Vault. Total Fee : "4905752" (protocol_fee="2403818", dev_fee="735862" LP fee="1766072"). Liquidity updated with "741860320" "contract2"
    // Transfering total "0" "contract3" to the Vault. Total Fee : "2739013" (protocol_fee="1342116", dev_fee="410851" LP fee="986046"). Liquidity updated by subtracting  "1752967" "contract3"
    // Transfering total "514000000" "token0" to the Vault. Total Fee : "2536766" (protocol_fee="1243015", dev_fee="380514" LP fee="913237"). Liquidity updated with "512376471" "token0"
    // Transfering total "0" "token1" to the Vault. Total Fee : "2081850" (protocol_fee="1020106", dev_fee="312277" LP fee="749467"). Liquidity updated by subtracting  "1332383" "token1"
    assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            amount: Uint128::from(514_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(745_000000u128),
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
        &[Coin {
            denom: denom0.clone(),
            amount: Uint128::new(1155_000000u128),
        }],
    )
    .unwrap();

    // -------x---------- WEIGHTED-POOL -::- PROVIDE LIQUIDITY -------x----------
    // -------x---------- -------x---------- -------x---------- -------x----------

    // --- Liquidity provided to empty pool - No fee is charged ---
    // VAULT -::- Join Pool -::- Execution Function
    // --- WeightedPool:OnJoinPool Query : Begin ---
    // Lp shares to mint (exact-ratio-join): 100000000
    // Following assets are to be transferred by the user to the Vault:
    // ::: "contract1" "1000000000"
    // ::: "contract2" "1000000000"
    // ::: "contract3" "1000000000"
    // ::: "token0" "1000000000"
    // ::: "token1" "1000000000"
    // LP tokens to be minted: "100000000"
    // Transfering total "1000000000" "contract1" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "contract1"
    // Transfering total "1000000000" "contract2" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "contract2"
    // Transfering total "1000000000" "contract3" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "contract3"
    // Transfering total "1000000000" "token0" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "token0"
    // Transfering total "1000000000" "token1" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "1000000000" "token1"
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

    // --- Liquidity added with all tokens in imbalanced manner ---
    // VAULT -::- Join Pool -::- Execution Function
    // --- WeightedPool:OnJoinPool Query : Begin ---
    // Lp shares to mint (exact-ratio-join): 94500000
    // We need to charge fee during single asset join for :  "contract1"
    // "contract1" - Tokens in = "508000000" Tokens in (after fee) = "495808000" Fee charged = "12192000"
    // contract1 new_num_shares_from_single: 9036549 | in_asset : 508000000, fee_charged: 12192000
    // We need to charge fee during single asset join for :  "contract3"
    // "contract3" - Tokens in = "618000000" Tokens in (after fee) = "603168000" Fee charged = "14832000"
    // contract3 new_num_shares_from_single: 11297986 | in_asset : 618000000, fee_charged: 14832000
    // We need to charge fee during single asset join for :  "token0"
    // "token0" - Tokens in = "189000000" Tokens in (after fee) = "184464000" Fee charged = "4536000"
    // token0 new_num_shares_from_single: 3928648 | in_asset : 189000000, fee_charged: 4536000
    // We need to charge fee during single asset join for :  "token1"
    // "token1" - Tokens in = "3000000" Tokens in (after fee) = "2928000" Fee charged = "72000"
    // token1 new_num_shares_from_single: 65825 | in_asset : 3000000, fee_charged: 72000
    // --- WeightedPool:OnJoinPool Query :: End ---
    // Following assets are to be transferred by the user to the Vault:
    // ::: "contract1" "1453000000"
    // ::: "contract2" "945000000"
    // ::: "contract3" "1563000000"
    // ::: "token0" "1134000000"
    // ::: "token1" "948000000"
    // LP tokens to be minted: "118829008"
    // Transfering total "1453000000" "contract1" to the Vault. Total Fee : "12192000" (protocol_fee="5974080", dev_fee="1828800" LP fee="4389120"). Liquidity updated with "1445197120" "contract1"
    // Transfering total "945000000" "contract2" to the Vault. Total Fee : "0" (protocol_fee="0", dev_fee="0" LP fee="0"). Liquidity updated with "945000000" "contract2"
    // Transfering total "1563000000" "contract3" to the Vault. Total Fee : "14832000" (protocol_fee="7267680", dev_fee="2224800" LP fee="5339520"). Liquidity updated with "1553507520" "contract3"
    // Transfering total "1134000000" "token0" to the Vault. Total Fee : "4536000" (protocol_fee="2222640", dev_fee="680400" LP fee="1632960"). Liquidity updated with "1131096960" "token0"
    // Transfering total "948000000" "token1" to the Vault. Total Fee : "72000" (protocol_fee="35280", dev_fee="10800" LP fee="25920"). Liquidity updated with "947953920" "token1"
    let assets_msg = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom0.clone(),
            },
            amount: Uint128::from(1134_000000u128),
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: denom1.clone(),
            },
            amount: Uint128::from(948_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::from(1453_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance2.clone(),
            },
            amount: Uint128::from(945_000000u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance3.clone(),
            },
            amount: Uint128::from(1563_000000u128),
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
                amount: Uint128::new(1500_000000u128),
            },
            Coin {
                denom: denom1.clone(),
                amount: Uint128::new(1000_000000u128),
            },
        ],
    )
    .unwrap();

    // --- Liquidity added with a single token ---
    // VAULT -::- Join Pool -::- Execution Function
    // --- WeightedPool:OnJoinPool Query : Begin ---
    // ---- Single asset join
    // We need to charge fee during single asset join for :  "contract2"
    // "contract2" - Tokens in = "945000000" Tokens in (after fee) = "922320000" Fee charged = "22680000"
    // Following assets are to be transferred by the user to the Vault:
    // ::: "contract1" "0"
    // ::: "contract2" "945000000"
    // ::: "contract3" "0"
    // ::: "token0" "0"
    // ::: "token1" "0"
    // LP tokens to be minted: "17662854"
    // Transfering total "945000000" "contract2" to the Vault. Total Fee : "22680000" (protocol_fee="11113200", dev_fee="3402000" LP fee="8164800"). Liquidity updated with "930484800" "contract2"

    cur_user_lp_balance = app
        .wrap()
        .query_wasm_smart(
            &weighted_lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();

    vault_token2_balance = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    keeper_token2_balance = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: "fee_collector".to_string(),
            },
        )
        .unwrap();

    let assets_msg = vec![Asset {
        info: AssetInfo::Token {
            contract_addr: token_instance2.clone(),
        },
        amount: Uint128::from(945_000000u128),
    }];

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
        &[],
    )
    .unwrap();

    // Checks -
    // - Tokens transferred to the Vault.
    // - Fee transferred correctly - 0 fee charged
    // - LP tokens minted & transferred correctly
    new_user_lp_balance = app
        .wrap()
        .query_wasm_smart(
            &weighted_lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();

    assert_eq!(
        Uint128::from(17662855075318413770u128),
        new_user_lp_balance.balance - cur_user_lp_balance.balance
    );

    new_vault_token2_balance = app
        .wrap()
        .query_wasm_smart(
            &token_instance2.clone(),
            &Cw20QueryMsg::Balance {
                address: vault_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(930484800u128),
        new_vault_token2_balance.balance - vault_token2_balance.balance
    );

    // // FEE CHECKS
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
        Uint128::from(14515200u128),
        new_keeper_token2_balance.balance - keeper_token2_balance.balance
    );
}

/// This test is for testing the following:
/// 1. Create a new pool
/// 2. Provide liquidity to the pool with auto-stake enabled
#[test]
fn test_join_auto_stake() {
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
    increase_token_allowance(
        &mut app,
        owner.clone(),
        token_instance1.clone(),
        vault_instance.clone().into_string(),
        Uint128::new(10000000_000000u128),
    );

    increase_token_allowance(
        &mut app,
        owner.clone(),
        token_instance2.clone(),
        vault_instance.clone().into_string(),
        Uint128::new(10000000_000000u128),
    );

    increase_token_allowance(
        &mut app,
        owner.clone(),
        token_instance3.clone(),
        vault_instance.clone().into_string(),
        Uint128::new(10000000_000000u128),
    );

    // Create a WEIGHTED pool
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

    // -------x---------- WEIGHTED-POOL -::- PROVIDE LIQUIDITY -------x---------
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

    // Check if LP tokens are minted to user when auto-stake is disabled
    let new_user_lp_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &weighted_lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();

    assert_eq!(new_user_lp_balance.balance, Uint128::from(100_000_000_000_000_000_000u128));

    // setup multistaking contract
    let multistaking_contract_address = initialize_multistaking_contract(
        &mut app,
        &Addr::unchecked(owner.clone())
    );

    // Update vault config to set multistaking
    let config_update_msg = ExecuteMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: None,
        pool_creation_fee: None,
        auto_stake_impl: Some(
            dexter::vault::AutoStakeImpl::Multistaking {
                contract_addr: multistaking_contract_address.clone(),
            }
        ),
        paused: None,
        reward_schedule_validation_assets: None,
    };

    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &config_update_msg,
        &[],
    ).unwrap();

    // Allow LP tokens to be staked in multistaking contract
    let allow_lp_token_msg = dexter::multi_staking::ExecuteMsg::AllowLpToken {
        lp_token: weighted_lp_token_addr.clone(),
    };

    app.execute_contract(
        owner.clone(),
        multistaking_contract_address.clone(),
        &allow_lp_token_msg,
        &[],
    ).unwrap();

    // Check if LP tokens are minted and staked to multistaking when auto-stake is enabled
    app.execute_contract(
        owner.clone(),
        vault_instance.clone(),
        &ExecuteMsg::JoinPool {
            pool_id: Uint128::from(weighted_pool_id),
            recipient: None,
            min_lp_to_receive: None,
            auto_stake: Some(true),
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
    ).unwrap();

    // fetch user staked tokens in multistaking
    let bonded_amount: Uint128 = app
        .wrap()
        .query_wasm_smart(
            &multistaking_contract_address.clone(),
            &dexter::multi_staking::QueryMsg::BondedLpTokens {
                lp_token: weighted_lp_token_addr.clone(),
                user: owner.clone(),
            },
        )
        .unwrap();

    assert_eq!(bonded_amount, Uint128::from(100_000_000_000_000_000_000u128));

    // Check user LP balance is still same
    let new_user_lp_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &weighted_lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: owner.clone().to_string(),
            },
        )
        .unwrap();

    // This means auto-stake didn't make any changes to user's LP balance but staked it in multistaking
    assert_eq!(new_user_lp_balance.balance, Uint128::from(100_000_000_000_000_000_000u128));
}
