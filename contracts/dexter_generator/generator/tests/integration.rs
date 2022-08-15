use cosmwasm_std::testing::{mock_env};
use cosmwasm_std::{attr, Addr,Decimal, Coin, Uint128, Timestamp, Uint64};
use cw20::MinterResponse;
use cw_multi_test::{App, BasicApp, ContractWrapper, Executor};
use dexter::asset::{Asset, AssetInfo};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::{
    vault::{ConfigResponse as VaultConfigResponse, QueryMsg as VaultQueryMsg, PoolConfig , InstantiateMsg as VaultInstantiateMsg, PoolType, FeeInfo},
    generator::{
        ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, PendingTokenResponse,
        PoolInfoResponse, PoolLengthResponse, QueryMsg, RewardInfoResponse, ExecuteOnReply, Config
    },
    generator_proxy::{
        Cw20HookMsg as ProxyCw20HookMsg, ExecuteMsg as ProxyExecuteMsg, QueryMsg as ProxyQueryMsg,
    },
    vesting::ExecuteMsg as VestingExecuteMsg,
};

const EPOCH_START: u64 = 1_000_000;


fn mock_app(owner: String, coins: Vec<Coin>) -> App {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(EPOCH_START);

    let mut app = App::new(|router, _, storage| {
        // initialization  moved to App construction
        router.bank.init_balance(storage, &Addr::unchecked(owner.clone()), coins).unwrap();
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

fn store_generator_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_generator::contract::execute,
        dexter_generator::contract::instantiate,
        dexter_generator::contract::query,
    ));
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
    let pool_contract = Box::new(
        ContractWrapper::new_with_empty(
            xyk_pool::contract::execute,
            xyk_pool::contract::instantiate,
            xyk_pool::contract::query,
        )
        .with_reply_empty(xyk_pool::contract::reply),
    );
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
        guardian: None,
        dex_token: None,
        tokens_per_block: Uint128::zero(),
        start_block: Uint64::from(current_block.height) ,
        unbonding_period: 86400u64
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



// #[test]
// fn test_set_tokens_per_block() { 

// }


// #[test]
// fn test_set_allowed_reward_proxies() { 

// }

// #[test]
// fn test_send_orphan_proxy_reward() { 

// }

// #[test]
// fn test_update_allowed_provies() { 

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
    let mut app = mock_app(owner.clone(), vec![Coin { denom: "xprt".to_string(), amount: Uint128::new(100_000_000_000u128)}]);
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
    app.execute_contract(Addr::unchecked("owner"), generator_instance.clone(), &msg, &[])
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
    app.execute_contract(Addr::unchecked("owner"), generator_instance.clone(), &msg, &[])
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
    let res: ConfigResponse = app.wrap().query_wasm_smart(&generator_instance, &msg).unwrap();

    assert_eq!(res.owner, new_owner)
}


// Tests the following -
//  ExecuteMsg::UpdateConfig
#[test]
fn test_update_config() { 
    let owner = "owner".to_string();
    let mut app = mock_app(owner.clone(), vec![Coin { denom: "xprt".to_string(), amount: Uint128::new(100_000_000_000u128)}]);
    let (generator_instance, vault_instance) = instantiate_contracts(&mut app, Addr::unchecked(owner.clone()));

    let msg = QueryMsg::Config {};
    let after_init_config_res: ConfigResponse =
        app.wrap().query_wasm_smart(&generator_instance, &msg).unwrap();

    assert_eq!(owner, after_init_config_res.owner);
    assert_eq!(vault_instance, after_init_config_res.vault);
    assert_eq!(None, after_init_config_res.dex_token);
    assert_eq!(None, after_init_config_res.vesting_contract);
    assert_eq!(None, after_init_config_res.guardian);      
    assert_eq!(None, after_init_config_res.checkpoint_generator_limit);        

    //// -----x----- Success :: update config -----x----- ////

    let msg = ExecuteMsg::UpdateConfig {
        dex_token: Some("dex_token".to_string()),
        vesting_contract: Some("vesting_contract".to_string()),
        guardian: Some("guardian".to_string()),
        checkpoint_generator_limit: Some(10u32),
        unbonding_period: Some(86400u64)
    };

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    let msg = QueryMsg::Config {};
    let config_res: ConfigResponse = app.wrap().query_wasm_smart(&generator_instance, &msg).unwrap();

    assert_eq!(owner, config_res.owner);
    assert_eq!(vault_instance, config_res.vault);
    assert_eq!(Addr::unchecked("dex_token".to_string()), config_res.dex_token.unwrap());
    assert_eq!(Addr::unchecked("vesting_contract".to_string()), config_res.vesting_contract.unwrap());
    assert_eq!(Addr::unchecked("guardian".to_string()), config_res.guardian.unwrap());      
    assert_eq!(Some(10u32), config_res.checkpoint_generator_limit);        

    //// -----x----- Error :: Permission Checks -----x----- ////

    let msg = ExecuteMsg::UpdateConfig {
        dex_token: Some("dex_token".to_string()),
        vesting_contract: Some("vesting_contract".to_string()),
        guardian: Some("guardian".to_string()),
        checkpoint_generator_limit: Some(10u32),
        unbonding_period: Some(86400u64)
    };

    let err_res = app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &msg,
        &[],
    )
    .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Dex token already set");


    let err_res = app.execute_contract(
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
        guardian: Some("guardian".to_string()),
        checkpoint_generator_limit: Some(10u32),
        unbonding_period: Some(86400u64)
    };

    let err_res = app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &msg,
        &[],
    )
    .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Vesting contract already set");


}


// Tests the following -
//  ExecuteMsg::SetupPools
//  ExecuteMsg::DeactivatePool
#[test]
fn test_setup_pool_deactivate_pool() { 
    let owner = "owner".to_string();
    let mut app = mock_app(owner.clone(), vec![Coin { denom: "xprt".to_string(), amount: Uint128::new(100_000_000_000u128)}]);
    let (generator_instance, _) = instantiate_contracts(&mut app, Addr::unchecked(owner.clone()));


    //// -----x----- Error :: Permission Check -----x----- ////

    let msg = ExecuteMsg::SetupPools {
        pools: vec![ ("lp_token1".to_string(),Uint128::from(100u128)),  ("lp_token2".to_string(),Uint128::from(200u128)) ] ,
    };

    let err_res = app.execute_contract(
        Addr::unchecked("not_owner".to_string().clone()),
        generator_instance.clone(),
        &msg,
        &[],
    )
    .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Unauthorized");

    //// -----x----- Error :: Duplicate of pool Check -----x----- ////

    let msg = ExecuteMsg::SetupPools {
        pools: vec![ ("lp_token1".to_string(),Uint128::from(100u128)),  ("lp_token1".to_string(),Uint128::from(200u128)) ] , 
    };

    let err_res = app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &msg,
        &[],
    )
    .unwrap_err();
    assert_eq!(err_res.root_cause().to_string(), "Duplicate of pool");

    //// -----x----- Success :: Setup 2 pools -----x----- ////
    let lp_token1 = create_token(&mut app, &owner.clone(), &"lp_token1".to_string(), &"lpt".to_string() );
    let lp_token2 = create_token(&mut app, &owner.clone(), &"lp_token2".to_string(), &"lpt".to_string() );

    let pools = vec![ (lp_token1.clone().to_string(),Uint128::from(100u128)),  (lp_token2.to_string(),Uint128::from(200u128)) ] ;
    let msg = ExecuteMsg::SetupPools {
        pools: pools.clone() , 
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
    let config_res: ConfigResponse = app.wrap().query_wasm_smart(&generator_instance, &QueryMsg::Config {}).unwrap();
    assert_eq!(vec![ (lp_token1.clone(),Uint128::from(100u128)),  (lp_token2.clone(),Uint128::from(200u128)) ] , config_res.active_pools);
    assert_eq!(Uint128::from(300u128), config_res.total_alloc_point);

    // Query::ActivePoolLength Check
    let pool_length_res: PoolLengthResponse = app.wrap().query_wasm_smart(&generator_instance, &QueryMsg::ActivePoolLength {}).unwrap();
    assert_eq!( 2, pool_length_res.length);

    // Query::PoolLength Check
    let pool_length_res: PoolLengthResponse = app.wrap().query_wasm_smart(&generator_instance, &QueryMsg::PoolLength {}).unwrap();
    assert_eq!( 2, pool_length_res.length);

    let current_block = app.block_info();
    // Query::PoolInfo Check
    let empty_proxy: Vec<(Addr, Decimal)> = vec![];
    let empty_proxy_orphan: Vec<(Addr, Uint128)> = vec![];
    let pool_info_res: PoolInfoResponse = app.wrap().query_wasm_smart(&generator_instance, &QueryMsg::PoolInfo { lp_token: lp_token2.clone().to_string() }).unwrap();
    assert_eq!( Uint128::from(200u128), pool_info_res.alloc_point);
    assert_eq!( Uint128::from(0u128), pool_info_res.dex_tokens_per_block);
    assert_eq!( current_block.height, pool_info_res.last_reward_block);
    assert_eq!( current_block.height, pool_info_res.current_block);
    assert_eq!(Decimal::zero(), pool_info_res.global_reward_index);
    assert_eq!( Uint128::from(0u128), pool_info_res.pending_dex_rewards);
    assert_eq!( None, pool_info_res.reward_proxy);
    assert_eq!( None, pool_info_res.pending_proxy_rewards);
    assert_eq!( empty_proxy, pool_info_res.accumulated_proxy_rewards_per_share);
    assert_eq!( Uint128::from(0u128), pool_info_res.proxy_reward_balance_before_update);
    assert_eq!( empty_proxy_orphan, pool_info_res.orphan_proxy_rewards);
    assert_eq!( Uint128::from(0u128), pool_info_res.lp_supply);

    // Deactivate 1 pool
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &ExecuteMsg::DeactivatePool { lp_token: lp_token1.clone().to_string() },
        &[],
    )
    .unwrap();

    // Query::Config Check
    let config_res: ConfigResponse = app.wrap().query_wasm_smart(&generator_instance, &QueryMsg::Config {}).unwrap();
    assert_eq!(vec![ (lp_token1.clone(),Uint128::from(0u128)) ,  (lp_token2.clone(),Uint128::from(200u128)) ] , config_res.active_pools);
    assert_eq!(Uint128::from(200u128), config_res.total_alloc_point);

    // Query::ActivePoolLength Check
    let pool_length_res: PoolLengthResponse = app.wrap().query_wasm_smart(&generator_instance, &QueryMsg::ActivePoolLength {}).unwrap();
    assert_eq!( 2, pool_length_res.length);

    // Query::PoolLength Check
    let pool_length_res: PoolLengthResponse = app.wrap().query_wasm_smart(&generator_instance, &QueryMsg::PoolLength {}).unwrap();
    assert_eq!( 2, pool_length_res.length);

    // Setup 1 new pool and remove existing pool
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &ExecuteMsg::SetupPools {
            pools: vec![(lp_token1.to_string(),Uint128::from(0u128)),  (lp_token2.to_string(),Uint128::from(200u128)), ("lp_token3".to_string(),Uint128::from(300u128))] , 
        },
        &[],
    )
    .unwrap();


    // Query::Config Check
    let config_res: ConfigResponse = app.wrap().query_wasm_smart(&generator_instance, &QueryMsg::Config {}).unwrap();
    assert_eq!(vec![ (lp_token1.clone(),Uint128::from(0u128)),  (lp_token2.clone(),Uint128::from(200u128)), (Addr::unchecked("lp_token3".to_string()) ,Uint128::from(300u128))  ] , config_res.active_pools);
    assert_eq!(Uint128::from(500u128), config_res.total_alloc_point);

    // Query::ActivePoolLength Check
    let pool_length_res: PoolLengthResponse = app.wrap().query_wasm_smart(&generator_instance, &QueryMsg::ActivePoolLength {}).unwrap();
    assert_eq!( 3, pool_length_res.length);

    // Query::PoolLength Check
    let pool_length_res: PoolLengthResponse = app.wrap().query_wasm_smart(&generator_instance, &QueryMsg::PoolLength {}).unwrap();
    assert_eq!( 3, pool_length_res.length);


}