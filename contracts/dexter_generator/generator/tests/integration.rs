use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{attr, to_binary, Addr, Coin, Decimal, Timestamp, Uint128, Uint64};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse};
use cw_multi_test::{App, BasicApp, ContractWrapper, Executor};
use dexter::asset::{Asset, AssetInfo};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::{
    generator::{
        Config, ConfigResponse, Cw20HookMsg, ExecuteMsg, ExecuteOnReply, InstantiateMsg,
        MigrateMsg, PendingTokenResponse, PoolInfoResponse, PoolLengthResponse, QueryMsg,
        RewardInfoResponse, UnbondingInfo, UserInfoResponse,
    },
    generator_proxy::{
        Cw20HookMsg as ProxyCw20HookMsg, ExecuteMsg as ProxyExecuteMsg, QueryMsg as ProxyQueryMsg,
    },
    vault::{
        ConfigResponse as VaultConfigResponse, FeeInfo, InstantiateMsg as VaultInstantiateMsg,
        PoolConfig, PoolType, QueryMsg as VaultQueryMsg,
    },
    vesting::ExecuteMsg as VestingExecuteMsg,
};

const EPOCH_START: u64 = 1_000_000;

fn mock_app(owner: String, coins: Vec<Coin>) -> App {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(EPOCH_START);

    let mut app = App::new(|router, _, storage| {
        // initialization  moved to App construction
        router
            .bank
            .init_balance(storage, &Addr::unchecked(owner.clone()), coins)
            .unwrap();
    });
    app.set_block(env.block);
    app
}

fn store_token_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        lp_token::contract::execute,
        lp_token::contract::instantiate,
        lp_token::contract::query,
    ));
    app.store_code(token_contract)
}

fn store_vesting_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_vesting::contract::execute,
        dexter_vesting::contract::instantiate,
        dexter_vesting::contract::query,
    ));
    app.store_code(token_contract)
}

fn store_staking_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        anchor_staking::contract::execute,
        anchor_staking::contract::instantiate,
        anchor_staking::contract::query,
    ));
    app.store_code(token_contract)
}

fn store_proxy_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_generator_proxy::contract::execute,
        dexter_generator_proxy::contract::instantiate,
        dexter_generator_proxy::contract::query,
    ));
    app.store_code(token_contract)
}

fn store_generator_code(app: &mut App) -> u64 {
    let token_contract = Box::new(
        ContractWrapper::new_with_empty(
            dexter_generator::contract::execute,
            dexter_generator::contract::instantiate,
            dexter_generator::contract::query,
        )
        .with_reply_empty(dexter_generator::contract::reply),
    );
    app.store_code(token_contract)
}

fn store_vault_code(app: &mut App) -> u64 {
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

fn store_xyk_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(ContractWrapper::new_with_empty(
        xyk_pool::contract::execute,
        xyk_pool::contract::instantiate,
        xyk_pool::contract::query,
    ));
    app.store_code(pool_contract)
}

// Creates a token instance and returns its address
fn create_token(app: &mut App, owner: &str, name: &str, symbol: &str) -> Addr {
    let token_code_id = store_token_code(app);
    let token_instance = app
        .instantiate_contract(
            token_code_id,
            Addr::unchecked(owner.clone()),
            &TokenInstantiateMsg {
                name: name.to_string(),
                symbol: symbol.to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: owner.to_string(),
                    cap: None,
                }),
                marketing: None,
            },
            &[],
            symbol,
            None,
        )
        .unwrap();
    token_instance
}

// Mints some Tokens to "to" recipient
fn mint_some_tokens(app: &mut App, owner: Addr, token_instance: Addr, amount: Uint128, to: String) {
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

// Initializes the following -
// 1. Dexter Vault
// 2. Dexter Generator
fn instantiate_contracts(app: &mut App, owner: Addr) -> (Addr, Addr) {
    // Initialize Dexter::Vault Contract with XYK Pool and LP Token
    let vault_code_id = store_vault_code(app);
    let token_code_id = store_token_code(app);
    let xyk_pool_code_id = store_xyk_pool_code(app);

    let pool_configs = vec![PoolConfig {
        code_id: xyk_pool_code_id,
        pool_type: PoolType::Xyk {},
        fee_info: FeeInfo {
            total_fee_bps: 300u16,
            protocol_fee_percent: 49u16,
            dev_fee_percent: 15u16,
            developer_addr: None,
        },
        is_disabled: false,
        is_generator_disabled: false,
    }];
    let vault_init_msg = VaultInstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: token_code_id,
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        generator_address: None,
    };
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

    // Initialize Dexter::Generator Contract
    let current_block = app.block_info();
    let generator_code_id = store_generator_code(app);
    let generator_init_msg = InstantiateMsg {
        owner: owner.to_string(),
        vault: vault_instance.clone().to_string(),
        dex_token: None,
        tokens_per_block: Uint128::zero(),
        start_block: Uint64::from(current_block.height),
        unbonding_period: 86400u64,
    };

    let generator_instance = app
        .instantiate_contract(
            generator_code_id,
            owner.to_owned(),
            &generator_init_msg,
            &[],
            "generator",
            None,
        )
        .unwrap();

    (generator_instance, vault_instance)
}

// Initializes the following -
// 1. Dexter Vault
// 2. Dexter Generator
fn setup_proxy_with_staking(
    app: &mut App,
    owner: Addr,
    generator_addr: Addr,
) -> (Addr, Addr, Addr, Addr) {
    let staking_code_id = store_staking_code(app);
    let proxy_code_id = store_proxy_code(app);

    let reward_token = create_token(
        app,
        &owner.clone().to_string(),
        &"anc_token".to_string(),
        &"anc".to_string(),
    );
    let lp_token = create_token(
        app,
        &owner.clone().to_string(),
        &"anc_token".to_string(),
        &"anc".to_string(),
    );

    // Setup Staking Contract
    let staking_instance = app
        .instantiate_contract(
            staking_code_id,
            owner.to_owned(),
            &dexter::ref_staking::InstantiateMsg {
                anchor_token: reward_token.clone().to_string(),
                staking_token: lp_token.clone().to_string(),
                distribution_schedule: vec![(
                    EPOCH_START,
                    EPOCH_START + (86400 * 30),
                    Uint128::from(1000000000000u128),
                )],
            },
            &[],
            "staking",
            None,
        )
        .unwrap();
    // Mint reward tokens to staking contract
    mint_some_tokens(
        app,
        owner.clone(),
        reward_token.clone(),
        Uint128::new(1000000000000),
        staking_instance.to_string(),
    );

    // Setup Proxy Contract
    let proxy_instance = app
        .instantiate_contract(
            proxy_code_id,
            owner.to_owned(),
            &dexter::generator_proxy::InstantiateMsg {
                generator_contract_addr: generator_addr.clone().to_string(),
                pair_addr: "pair_addr".to_string(),
                lp_token_addr: lp_token.clone().to_string(),
                reward_contract_addr: staking_instance.clone().to_string(),
                reward_token: AssetInfo::Token {
                    contract_addr: reward_token.clone(),
                },
            },
            &[],
            "proxy",
            None,
        )
        .unwrap();

    (staking_instance, proxy_instance, lp_token, reward_token)
}

// #[test]
// fn test_set_tokens_per_block() {

// }

// #[test]
// fn test_send_orphan_proxy_reward() {

// }

// #[test]
// fn test_cw20_hook_deposit() {

// }

// #[test]
// fn test_cw20_hook_deposit_for() {

// }

// #[test]
// fn test_unstake() {

// }

// #[test]
// fn test_emergency_unstake() {

// }

// #[test]
// fn test_unlock() {

// }

// Tests the following -
//  ExecuteMsg::ProposeNewOwner
//  ExecuteMsg::DropOwnershipProposal
//  ExecuteMsg::ClaimOwnership
#[test]
fn test_update_owner() {
    let owner = "owner".to_string();
    let mut app = mock_app(
        owner.clone(),
        vec![Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(100_000_000_000u128),
        }],
    );
    let (generator_instance, _) = instantiate_contracts(&mut app, Addr::unchecked(owner.clone()));

    let new_owner = String::from("new_owner");

    // New owner
    let msg = ExecuteMsg::ProposeNewOwner {
        owner: new_owner.clone(),
        expires_in: 100, // seconds
    };

    // Unauthed check
    let err = app
        .execute_contract(
            Addr::unchecked("not_owner"),
            generator_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    // Claim before proposal
    let err = app
        .execute_contract(
            Addr::unchecked(new_owner.clone()),
            generator_instance.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Ownership proposal not found"
    );

    // Propose new owner
    app.execute_contract(
        Addr::unchecked("owner"),
        generator_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    // Claim from invalid addr
    let err = app
        .execute_contract(
            Addr::unchecked("invalid_addr"),
            generator_instance.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap_err();
    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    // Drop ownership proposal
    let err = app
        .execute_contract(
            Addr::unchecked(new_owner.clone()),
            generator_instance.clone(),
            &ExecuteMsg::DropOwnershipProposal {},
            &[],
        )
        .unwrap_err();
    // new_owner is not an owner yet
    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &ExecuteMsg::DropOwnershipProposal {},
        &[],
    )
    .unwrap();

    // Try to claim ownership
    let err = app
        .execute_contract(
            Addr::unchecked(new_owner.clone()),
            generator_instance.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Ownership proposal not found"
    );

    // Propose new owner again
    app.execute_contract(
        Addr::unchecked("owner"),
        generator_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();
    // Claim ownership
    app.execute_contract(
        Addr::unchecked(new_owner.clone()),
        generator_instance.clone(),
        &ExecuteMsg::ClaimOwnership {},
        &[],
    )
    .unwrap();

    // Let's query the contract state
    let msg = QueryMsg::Config {};
    let res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&generator_instance, &msg)
        .unwrap();

    assert_eq!(res.owner, new_owner)
}

// Tests the following -
//  ExecuteMsg::UpdateConfig
#[test]
fn test_update_config() {
    let owner = "owner".to_string();
    let mut app = mock_app(
        owner.clone(),
        vec![Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(100_000_000_000u128),
        }],
    );
    let (generator_instance, vault_instance) =
        instantiate_contracts(&mut app, Addr::unchecked(owner.clone()));

    let msg = QueryMsg::Config {};
    let after_init_config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&generator_instance, &msg)
        .unwrap();

    assert_eq!(owner, after_init_config_res.owner);
    assert_eq!(vault_instance, after_init_config_res.vault);
    assert_eq!(None, after_init_config_res.dex_token);
    assert_eq!(None, after_init_config_res.vesting_contract);
    assert_eq!(None, after_init_config_res.checkpoint_generator_limit);

    //// -----x----- Success :: update config -----x----- ////

    let msg = ExecuteMsg::UpdateConfig {
        dex_token: Some("dex_token".to_string()),
        vesting_contract: Some("vesting_contract".to_string()),
        checkpoint_generator_limit: Some(10u32),
        unbonding_period: Some(86400u64),
    };

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    let msg = QueryMsg::Config {};
    let config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&generator_instance, &msg)
        .unwrap();

    assert_eq!(owner, config_res.owner);
    assert_eq!(vault_instance, config_res.vault);
    assert_eq!(
        Addr::unchecked("dex_token".to_string()),
        config_res.dex_token.unwrap()
    );
    assert_eq!(
        Addr::unchecked("vesting_contract".to_string()),
        config_res.vesting_contract.unwrap()
    );
    assert_eq!(Some(10u32), config_res.checkpoint_generator_limit);

    //// -----x----- Error :: Permission Checks -----x----- ////

    let msg = ExecuteMsg::UpdateConfig {
        dex_token: Some("dex_token".to_string()),
        vesting_contract: Some("vesting_contract".to_string()),
        checkpoint_generator_limit: Some(10u32),
        unbonding_period: Some(86400u64),
    };

    let err_res = app
        .execute_contract(
            Addr::unchecked(owner.clone()),
            generator_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Dex token already set");

    let err_res = app
        .execute_contract(
            Addr::unchecked("not_owner".to_string().clone()),
            generator_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Unauthorized");

    let msg = ExecuteMsg::UpdateConfig {
        dex_token: None,
        vesting_contract: Some("vesting_contract".to_string()),
        checkpoint_generator_limit: Some(10u32),
        unbonding_period: Some(86400u64),
    };

    let err_res = app
        .execute_contract(
            Addr::unchecked(owner.clone()),
            generator_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err_res.root_cause().to_string(),
        "Vesting contract already set"
    );
}

// Tests the following -
//  ExecuteMsg::SetupPools
//  ExecuteMsg::DeactivatePool
#[test]
fn test_setup_pool_deactivate_pool() {
    let owner = "owner".to_string();
    let mut app = mock_app(
        owner.clone(),
        vec![Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(100_000_000_000u128),
        }],
    );
    let (generator_instance, _) = instantiate_contracts(&mut app, Addr::unchecked(owner.clone()));

    //// -----x----- Error :: Permission Check -----x----- ////

    let msg = ExecuteMsg::SetupPools {
        pools: vec![
            ("lp_token1".to_string(), Uint128::from(100u128)),
            ("lp_token2".to_string(), Uint128::from(200u128)),
        ],
    };

    let err_res = app
        .execute_contract(
            Addr::unchecked("not_owner".to_string().clone()),
            generator_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Unauthorized");

    //// -----x----- Error :: Duplicate of pool Check -----x----- ////

    let msg = ExecuteMsg::SetupPools {
        pools: vec![
            ("lp_token1".to_string(), Uint128::from(100u128)),
            ("lp_token1".to_string(), Uint128::from(200u128)),
        ],
    };

    let err_res = app
        .execute_contract(
            Addr::unchecked(owner.clone()),
            generator_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Duplicate of pool");

    //// -----x----- Success :: Setup 2 pools -----x----- ////
    let lp_token1 = create_token(
        &mut app,
        &owner.clone(),
        &"lp_token1".to_string(),
        &"lpt".to_string(),
    );
    let lp_token2 = create_token(
        &mut app,
        &owner.clone(),
        &"lp_token2".to_string(),
        &"lpt".to_string(),
    );

    let pools = vec![
        (lp_token1.clone().to_string(), Uint128::from(100u128)),
        (lp_token2.to_string(), Uint128::from(200u128)),
    ];
    let msg = ExecuteMsg::SetupPools {
        pools: pools.clone(),
    };

    // Setup 2 pools
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    // Query::Config Check
    let config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&generator_instance, &QueryMsg::Config {})
        .unwrap();
    assert_eq!(
        vec![
            (lp_token1.clone(), Uint128::from(100u128)),
            (lp_token2.clone(), Uint128::from(200u128))
        ],
        config_res.active_pools
    );
    assert_eq!(Uint128::from(300u128), config_res.total_alloc_point);

    // Query::ActivePoolLength Check
    let pool_length_res: PoolLengthResponse = app
        .wrap()
        .query_wasm_smart(&generator_instance, &QueryMsg::ActivePoolLength {})
        .unwrap();
    assert_eq!(2, pool_length_res.length);

    // Query::PoolLength Check
    let pool_length_res: PoolLengthResponse = app
        .wrap()
        .query_wasm_smart(&generator_instance, &QueryMsg::PoolLength {})
        .unwrap();
    assert_eq!(2, pool_length_res.length);

    let current_block = app.block_info();
    // Query::PoolInfo Check
    let pool_info_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &generator_instance,
            &QueryMsg::PoolInfo {
                lp_token: lp_token2.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(200u128), pool_info_res.alloc_point);
    assert_eq!(Uint128::from(0u128), pool_info_res.dex_tokens_per_block);
    assert_eq!(current_block.height, pool_info_res.last_reward_block);
    assert_eq!(current_block.height, pool_info_res.current_block);
    assert_eq!(Decimal::zero(), pool_info_res.global_reward_index);
    assert_eq!(Uint128::from(0u128), pool_info_res.pending_dex_rewards);
    assert_eq!(None, pool_info_res.reward_proxy);
    assert_eq!(None, pool_info_res.pending_proxy_rewards);
    assert_eq!(
        Decimal::zero(),
        pool_info_res.accumulated_proxy_rewards_per_share
    );
    assert_eq!(
        Uint128::from(0u128),
        pool_info_res.proxy_reward_balance_before_update
    );
    assert_eq!(Uint128::from(0u128), pool_info_res.orphan_proxy_rewards);
    assert_eq!(Uint128::from(0u128), pool_info_res.lp_supply);

    // Deactivate 1 pool
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &ExecuteMsg::DeactivatePool {
            lp_token: lp_token1.clone().to_string(),
        },
        &[],
    )
    .unwrap();

    // Query::Config Check
    let config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&generator_instance, &QueryMsg::Config {})
        .unwrap();
    assert_eq!(
        vec![
            (lp_token1.clone(), Uint128::from(0u128)),
            (lp_token2.clone(), Uint128::from(200u128))
        ],
        config_res.active_pools
    );
    assert_eq!(Uint128::from(200u128), config_res.total_alloc_point);

    // Query::ActivePoolLength Check
    let pool_length_res: PoolLengthResponse = app
        .wrap()
        .query_wasm_smart(&generator_instance, &QueryMsg::ActivePoolLength {})
        .unwrap();
    assert_eq!(2, pool_length_res.length);

    // Query::PoolLength Check
    let pool_length_res: PoolLengthResponse = app
        .wrap()
        .query_wasm_smart(&generator_instance, &QueryMsg::PoolLength {})
        .unwrap();
    assert_eq!(2, pool_length_res.length);

    // Setup 1 new pool and remove existing pool
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &ExecuteMsg::SetupPools {
            pools: vec![
                (lp_token1.to_string(), Uint128::from(0u128)),
                (lp_token2.to_string(), Uint128::from(200u128)),
                ("lp_token3".to_string(), Uint128::from(300u128)),
            ],
        },
        &[],
    )
    .unwrap();

    // Query::Config Check
    let config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&generator_instance, &QueryMsg::Config {})
        .unwrap();
    assert_eq!(
        vec![
            (lp_token1.clone(), Uint128::from(0u128)),
            (lp_token2.clone(), Uint128::from(200u128)),
            (
                Addr::unchecked("lp_token3".to_string()),
                Uint128::from(300u128)
            )
        ],
        config_res.active_pools
    );
    assert_eq!(Uint128::from(500u128), config_res.total_alloc_point);

    // Query::ActivePoolLength Check
    let pool_length_res: PoolLengthResponse = app
        .wrap()
        .query_wasm_smart(&generator_instance, &QueryMsg::ActivePoolLength {})
        .unwrap();
    assert_eq!(3, pool_length_res.length);

    // Query::PoolLength Check
    let pool_length_res: PoolLengthResponse = app
        .wrap()
        .query_wasm_smart(&generator_instance, &QueryMsg::PoolLength {})
        .unwrap();
    assert_eq!(3, pool_length_res.length);
}

// Tests the following -
//  ExecuteMsg::SetAllowedRewardProxies
//  ExecuteMsg::UpdateAllowedProxies
#[test]
fn test_set_update_allowed_proxies() {
    let owner = "owner".to_string();
    let mut app = mock_app(
        owner.clone(),
        vec![Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(100_000_000_000u128),
        }],
    );
    let (generator_instance, _) = instantiate_contracts(&mut app, Addr::unchecked(owner.clone()));

    //// -----x----- Error :: Permission Check -----x----- ////

    let msg = ExecuteMsg::SetAllowedRewardProxies {
        proxies: vec!["proxy1".to_string(), "proxy2".to_string()],
    };
    let err_res = app
        .execute_contract(
            Addr::unchecked("not_owner".to_string().clone()),
            generator_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Unauthorized");

    let msg = ExecuteMsg::UpdateAllowedProxies {
        add: Some(vec!["proxy1".to_string(), "proxy2".to_string()]),
        remove: None,
    };
    let err_res = app
        .execute_contract(
            Addr::unchecked("not_owner".to_string().clone()),
            generator_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Unauthorized");

    //// -----x----- Success :: -----x----- ////

    let msg = ExecuteMsg::SetAllowedRewardProxies {
        proxies: vec!["proxy1".to_string(), "proxy2".to_string()],
    };
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();
    // Query::Config Check
    let config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&generator_instance, &QueryMsg::Config {})
        .unwrap();
    assert_eq!(
        vec![
            Addr::unchecked("proxy1".to_string()),
            Addr::unchecked("proxy2".to_string())
        ],
        config_res.allowed_reward_proxies
    );

    let msg = ExecuteMsg::UpdateAllowedProxies {
        add: Some(vec!["proxy3".to_string(), "proxy4".to_string()]),
        remove: Some(vec!["proxy1".to_string()]),
    };
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();
    // Query::Config Check
    let config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&generator_instance, &QueryMsg::Config {})
        .unwrap();
    assert_eq!(
        vec![
            Addr::unchecked("proxy2".to_string()),
            Addr::unchecked("proxy3".to_string()),
            Addr::unchecked("proxy4".to_string())
        ],
        config_res.allowed_reward_proxies
    );
}

// We instantiate the staking and proxy contracts, setup rewards via the proxy contract, then add the token to the generator and test the deposit --> claim --> umbond --> withdraw lifecycle
// Tests the following -
//  ExecuteMsg::Deposit
//  ExecuteMsg::DepositFor
//  ExecuteMsg::Unstake
//  ExecuteMsg::EmergencyUnstake
//  ExecuteMsg::Unlock
#[test]
fn test_generator_with_no_dex_rewards() {
    let owner = "owner".to_string();
    let mut app = mock_app(
        owner.clone(),
        vec![Coin {
            denom: "xprt".to_string(),
            amount: Uint128::new(100_000_000_000u128),
        }],
    );
    let (generator_instance, _) = instantiate_contracts(&mut app, Addr::unchecked(owner.clone()));

    let (staking_instance, proxy_instance, lp_token, reward_token) = setup_proxy_with_staking(
        &mut app,
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
    );

    let msg = ExecuteMsg::SetupPools {
        pools: vec![
            ("lp_token1".to_string(), Uint128::from(100u128)),
            ("lp_token2".to_string(), Uint128::from(200u128)),
        ],
    };

    // Error Check ::: set proxy for lp token generator
    let err_res = app
        .execute_contract(
            Addr::unchecked("notowner".to_string().clone()),
            generator_instance.clone(),
            &ExecuteMsg::SetupProxyForPool {
                lp_token: lp_token.clone().to_string(),
                proxy_addr: proxy_instance.clone().to_string(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Unauthorized");

    // Error Check ::: set proxy for lp token generator
    let err_res = app
        .execute_contract(
            Addr::unchecked(owner.clone()),
            generator_instance.clone(),
            &ExecuteMsg::SetupProxyForPool {
                lp_token: lp_token.clone().to_string(),
                proxy_addr: proxy_instance.clone().to_string(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err_res.root_cause().to_string(),
        "Generator pool doesn't exist"
    );

    // setup pool with 0 alloc in generator
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &ExecuteMsg::SetupPools {
            pools: vec![(lp_token.to_string(), Uint128::from(0u128))],
        },
        &[],
    )
    .unwrap();

    // set proxy as allowed in generator
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &ExecuteMsg::SetAllowedRewardProxies {
            proxies: vec![proxy_instance.to_string()],
        },
        &[],
    )
    .unwrap();

    // set proxy for lp token generator
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &ExecuteMsg::SetupProxyForPool {
            lp_token: lp_token.clone().to_string(),
            proxy_addr: proxy_instance.clone().to_string(),
        },
        &[],
    )
    .unwrap();

    let current_block = app.block_info();
    // Query::PoolInfo Check
    let pool_info_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &generator_instance,
            &QueryMsg::PoolInfo {
                lp_token: lp_token.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(0u128), pool_info_res.alloc_point);
    assert_eq!(Uint128::from(0u128), pool_info_res.dex_tokens_per_block);
    assert_eq!(current_block.height, pool_info_res.last_reward_block);
    assert_eq!(current_block.height, pool_info_res.current_block);
    assert_eq!(Decimal::zero(), pool_info_res.global_reward_index);
    assert_eq!(Uint128::from(0u128), pool_info_res.pending_dex_rewards);
    assert_eq!(Some(proxy_instance.clone()), pool_info_res.reward_proxy);
    assert_eq!(None, pool_info_res.pending_proxy_rewards);
    assert_eq!(
        Decimal::zero(),
        pool_info_res.accumulated_proxy_rewards_per_share
    );
    assert_eq!(
        Uint128::from(0u128),
        pool_info_res.proxy_reward_balance_before_update
    );
    assert_eq!(Uint128::from(0u128), pool_info_res.orphan_proxy_rewards);
    assert_eq!(Uint128::from(0u128), pool_info_res.lp_supply);

    // Mint LP tokens to user so he can deposit them
    let user = "user".to_string();
    mint_some_tokens(
        &mut app,
        Addr::unchecked(owner.clone()),
        lp_token.clone(),
        Uint128::new(9_000_000),
        user.clone(),
    );

    // SUCCESS :::: ExecuteContract::Deposit
    app.execute_contract(
        Addr::unchecked(user.clone()),
        lp_token.clone(),
        &Cw20ExecuteMsg::Send {
            contract: generator_instance.clone().to_string(),
            amount: Uint128::new(1_000_000),
            msg: to_binary(&Cw20HookMsg::Deposit {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    app.update_block(|b| {
        b.time = b.time.plus_seconds(1000);
        b.height = b.height + 100;
    });
    let current_block = app.block_info();

    let pool_info_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &generator_instance,
            &QueryMsg::PoolInfo {
                lp_token: lp_token.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(current_block.height, pool_info_res.current_block);
    assert_eq!(Some(proxy_instance.clone()), pool_info_res.reward_proxy);
    assert_eq!(
        Some(Uint128::from(385802469u128)),
        pool_info_res.pending_proxy_rewards
    );
    assert_eq!(
        Decimal::zero(),
        pool_info_res.accumulated_proxy_rewards_per_share
    );
    assert_eq!(
        Uint128::from(0u128),
        pool_info_res.proxy_reward_balance_before_update
    );
    assert_eq!(Uint128::from(0u128), pool_info_res.orphan_proxy_rewards);
    assert_eq!(Uint128::from(1000000u128), pool_info_res.lp_supply);

    let pending_token_res: PendingTokenResponse = app
        .wrap()
        .query_wasm_smart(
            &generator_instance,
            &QueryMsg::PendingToken {
                lp_token: lp_token.clone().to_string(),
                user: user.clone(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(0u128), pending_token_res.pending);
    assert_eq!(
        Uint128::from(385802469u128),
        pending_token_res.pending_on_proxy.unwrap()
    );

    let prev_user_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &reward_token,
            &Cw20QueryMsg::Balance {
                address: user.clone(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(0u128), prev_user_balance.balance);

    // SUCCESS :::: ExecuteContract::Deposit
    app.execute_contract(
        Addr::unchecked(user.clone()),
        lp_token.clone(),
        &Cw20ExecuteMsg::Send {
            contract: generator_instance.clone().to_string(),
            amount: Uint128::new(1_0000),
            msg: to_binary(&&Cw20HookMsg::Deposit {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    app.update_block(|b| {
        b.time = b.time.plus_seconds(10);
        b.height = b.height + 1;
    });
    let pool_info_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &generator_instance,
            &QueryMsg::PoolInfo {
                lp_token: lp_token.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Some(Uint128::from(3858024u128)),
        pool_info_res.pending_proxy_rewards
    );
    assert_eq!(
        Decimal::zero(),
        pool_info_res.accumulated_proxy_rewards_per_share
    );
    assert_eq!(Uint128::from(1010000u128), pool_info_res.lp_supply);

    let pending_token_res: PendingTokenResponse = app
        .wrap()
        .query_wasm_smart(
            &generator_instance,
            &QueryMsg::PendingToken {
                lp_token: lp_token.clone().to_string(),
                user: user.clone(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(0u128), pending_token_res.pending);
    assert_eq!(
        Uint128::from(3858023u128),
        pending_token_res.pending_on_proxy.unwrap()
    );

    app.execute_contract(
        Addr::unchecked(user.clone()),
        generator_instance.clone(),
        &ExecuteMsg::ClaimRewards {
            lp_tokens: vec![lp_token.clone().to_string()],
        },
        &[],
    )
    .unwrap();

    // SUCCESS :::: ExecuteContract::Unstake
    app.execute_contract(
        Addr::unchecked(user.clone()),
        generator_instance.clone(),
        &ExecuteMsg::Unstake {
            lp_token: lp_token.clone().to_string(),
            amount: Uint128::new(1),
        },
        &[],
    )
    .unwrap();

    // let prev_user_balance: BalanceResponse = app.wrap().query_wasm_smart(&reward_token, &Cw20QueryMsg::Balance { address: user.clone()  }).unwrap();
    // assert_eq!( Uint128::from(0u128), prev_user_balance.balance);

    let user_info_re: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &generator_instance,
            &QueryMsg::UserInfo {
                lp_token: lp_token.clone().to_string(),
                user: user.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1009999u128), user_info_re.amount);
    assert_eq!(Uint128::from(0u128), user_info_re.reward_debt);
    assert_eq!(Uint128::from(0u128), user_info_re.reward_debt_proxy);
    assert_eq!(
        vec![UnbondingInfo {
            amount: Uint128::from(1u128),
            unlock_timstamp: 1087410u64
        }],
        user_info_re.unbonding_periods
    );

    app.update_block(|b| {
        b.time = b.time.plus_seconds(1087410);
        b.height = b.height + 1;
    });

    let prev_user_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_token,
            &Cw20QueryMsg::Balance {
                address: user.clone(),
            },
        )
        .unwrap();

    // SUCCESS :::: ExecuteContract::UnLock
    app.execute_contract(
        Addr::unchecked(user.clone()),
        generator_instance.clone(),
        &ExecuteMsg::Unlock {
            lp_token: lp_token.clone().to_string(),
        },
        &[],
    )
    .unwrap();

    // Check if the user has received the LP Token
    let new_user_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_token,
            &Cw20QueryMsg::Balance {
                address: user.clone(),
            },
        )
        .unwrap();
    assert_eq!(
        prev_user_balance.balance + Uint128::from(1u128),
        new_user_balance.balance
    );

    // SUCCESS :::: ExecuteContract::Unstake
    app.execute_contract(
        Addr::unchecked(user.clone()),
        generator_instance.clone(),
        &ExecuteMsg::Unstake {
            lp_token: lp_token.clone().to_string(),
            amount: Uint128::new(1),
        },
        &[],
    )
    .unwrap();

    let user_info_re: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &generator_instance,
            &QueryMsg::UserInfo {
                lp_token: lp_token.clone().to_string(),
                user: user.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1009998u128), user_info_re.amount);
    assert_eq!(Uint128::from(0u128), user_info_re.reward_debt);
    assert_eq!(Uint128::from(0u128), user_info_re.reward_debt_proxy);
    assert_eq!(
        vec![UnbondingInfo {
            amount: Uint128::from(1u128),
            unlock_timstamp: 2174820u64
        }],
        user_info_re.unbonding_periods
    );
}
