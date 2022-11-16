use cosmwasm_std::{Addr, testing::mock_env, Timestamp, Coin, Uint128, to_binary};
use cw_multi_test::{App, Executor, ContractWrapper};
use dexter::multi_staking::{InstantiateMsg, ExecuteMsg, Cw20HookMsg, QueryMsg, UnclaimedReward};
use cw20::{Cw20ExecuteMsg, MinterResponse};

const EPOCH_START: u64 = 1_000_000_000;

fn mock_app(admin: Addr, coins: Vec<Coin>) -> App {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(EPOCH_START);

    let mut app = App::new(|router, _, storage| {
        // initialization  moved to App construction
        router.bank.init_balance(storage, &admin, coins).unwrap();
    });
    app.set_block(env.block);
    app
}

fn instantiate_multi_staking_contract(
    app: &mut App, 
    code_id: u64,
    admin: Addr
) -> Addr {
    let instantiateMsg = InstantiateMsg {};

    let multi_staking_instance = app
        .instantiate_contract(
            code_id,
            admin.to_owned(),
            &instantiateMsg,
            &[],
            "multi_staking",
            None,
        )
        .unwrap();

    return multi_staking_instance;
}

fn store_multi_staking_contract(
    app: &mut App
) -> u64 {
    let multi_staking_contract = Box::new(
        ContractWrapper::new_with_empty(
            dexter_multi_staking::contract::execute,
            dexter_multi_staking::contract::instantiate,
            dexter_multi_staking::contract::query
        )
    );
    let code_id = app.store_code(multi_staking_contract);
    return code_id;
}

fn store_cw20_contract(
    app: &mut App
) -> u64 {
    let cw20_contract = Box::new(
        ContractWrapper::new_with_empty(
            cw20_base::contract::execute,
            cw20_base::contract::instantiate,
            cw20_base::contract::query
        )
    );
    let code_id = app.store_code(cw20_contract);
    return code_id;
}

fn store_lp_token_contract(
    app: &mut App
) -> u64 {
    let lp_token_contract = Box::new(
        ContractWrapper::new_with_empty(
            lp_token::contract::execute,
            lp_token::contract::instantiate,
            lp_token::contract::query
        )
    );
    let code_id = app.store_code(lp_token_contract);
    return code_id;
}

fn create_lp_token(
    app: &mut App,
    code_id: u64,
    sender: Addr,
    token_name: String,
) -> Addr {
    let lp_token_instantiate_msg = dexter::lp_token::InstantiateMsg {
        name: token_name,
        symbol: "abcde".to_string(),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: sender.to_string(),
            cap: None,
        }),
        marketing: None,
    };

    let lp_token_instance = app
        .instantiate_contract(
            code_id,
            sender.clone(),
            &lp_token_instantiate_msg,
            &[],
            "lp_token",
            Some(sender.to_string()),
        )
        .unwrap();

    return lp_token_instance;
}

fn setup(app: &mut App, admin_addr: Addr) -> (Addr, Addr) {
    let multi_staking_code_id = store_multi_staking_contract(app);
    let multi_staking_instance = instantiate_multi_staking_contract(
        app,
        multi_staking_code_id,
        admin_addr.clone()
    );


    let cw20_code_id = store_cw20_contract(app);
    let lp_token_code_id = store_lp_token_contract(app);

    let lp_token_addr = create_lp_token(
        app,
        lp_token_code_id,
        admin_addr.clone(),
        "Dummy LP Token".to_string()
    );

    // Allow LP token in the multi staking contract
    app.execute_contract(
        admin_addr.clone(),
        multi_staking_instance.clone(), 
        &ExecuteMsg::AllowLpToken {
            lp_token: lp_token_addr.clone(),
        },
         &vec![]
    ).unwrap();

    return (multi_staking_instance, lp_token_addr);
}

#[test]
fn test_fail_create_reward_with_less_amount() {
    let admin = String::from("admin");
    let coins = vec![
        Coin::new(1000_000_000, "uxprt"),
        Coin::new(1000_000_000, "uatom"),
    ];
    let admin_addr = Addr::unchecked(admin.clone());
    let mut app = mock_app(admin_addr.clone(), coins);

    let (multi_staking_instance, lp_token_addr) = setup(&mut app, admin_addr.clone());

    // Create a new reward schedule
    let response = app.execute_contract(
        admin_addr.clone(), 
        multi_staking_instance.clone(), 
        &ExecuteMsg::AddRewardFactory { 
            lp_token: lp_token_addr.clone(), 
            denom: "uxprt".to_string(), 
            amount: Uint128::from(100_000_000 as u64), 
            start_block_time: 1_000_001_000, 
            end_block_time: 1_000_002_000 
        },
        &vec![Coin::new(80_000_000, "uxprt")]
    );
    
    assert!(response.is_err());
    // Check the error is amount insufficied fundsinsufficient funds
    assert_eq!(response.unwrap_err().root_cause().to_string(), "Not enough asset for reward was sent");
}


#[test]
fn test_verify_extra_amount_is_sent_back() {
    let admin = String::from("admin");
    let coins = vec![
        Coin::new(1000_000_000, "uxprt"),
        Coin::new(1000_000_000, "uatom"),
    ];
    let admin_addr = Addr::unchecked(admin.clone());
    let mut app = mock_app(admin_addr.clone(), coins);

    let (multi_staking_instance, lp_token_addr) = setup(&mut app, admin_addr.clone());

    // Create a new reward schedule
    let response = app.execute_contract(
        admin_addr.clone(), 
        multi_staking_instance.clone(), 
        &ExecuteMsg::AddRewardFactory { 
            lp_token: lp_token_addr.clone(), 
            denom: "uxprt".to_string(), 
            amount: Uint128::from(100_000_000 as u64), 
            start_block_time: 1_000_001_000, 
            end_block_time: 1_000_002_000 
        },
        &vec![Coin::new(500_000_000, "uxprt")]
    );

    assert!(response.is_ok());

    // query bank module for admin balance
    let balances =  app.wrap().query_all_balances(admin_addr).unwrap();
    let balance_uxprt = balances.iter().find(|b| b.denom == "uxprt").unwrap();
    assert_eq!(balance_uxprt.amount, Uint128::from(900_000_000 as u64));

}


#[test]
fn test_staking() {
    let admin = String::from("admin");
    let user = String::from("user");

    let coins = vec![
        Coin::new(1000_000_000, "uxprt"),
        Coin::new(1000_000_000, "uatom"),
    ];

    let admin_addr = Addr::unchecked(admin.clone());
    let user_addr = Addr::unchecked(user.clone());

    let mut app = mock_app(admin_addr.clone(), coins);

    let (multi_staking_instance, lp_token_addr) = setup(&mut app, admin_addr.clone());

    // Create a new reward schedule
    app.execute_contract(
        admin_addr.clone(), 
        multi_staking_instance.clone(), 
        &ExecuteMsg::AddRewardFactory { 
            lp_token: lp_token_addr.clone(), 
            denom: "uxprt".to_string(), 
            amount: Uint128::from(100_000_000 as u64), 
            start_block_time: 1_000_001_000, 
            end_block_time: 1_000_002_000 
        },
        &vec![Coin::new(100_000_000, "uxprt")]
    ).unwrap();

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_000);
        b.height = b.height + 100;
    });

    // Mint some LP tokens
    app.execute_contract(
        admin_addr.clone(), 
        lp_token_addr.clone(), 
        &Cw20ExecuteMsg::Mint { 
            recipient: user_addr.to_string(),
            amount: Uint128::from(100_000_000 as u64) 
        },
        &vec![]
    ).unwrap();

    app.execute_contract(
        user_addr.clone(), 
        lp_token_addr.clone(), 
        &Cw20ExecuteMsg::Send { 
                contract: multi_staking_instance.to_string(), 
                amount: Uint128::from(100_000 as u64),
                msg: to_binary(&Cw20HookMsg::Bond {}).unwrap()
            },
        &vec![]
    ).unwrap();

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_500);
        b.height = b.height + 100;
    });

    // Unbond half of the amoutt at 50% of the reward schedule
    app.execute_contract(
        user_addr.clone(), 
        multi_staking_instance.clone(),
        &ExecuteMsg::Unbond { lp_token: lp_token_addr.clone(), amount: Uint128::from(50_000 as u64) },
        &vec![],
    ).unwrap();

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_002_001);
        b.height = b.height + 100;
    });

    app.execute_contract(
        user_addr.clone(), 
        multi_staking_instance.clone(),
        &ExecuteMsg::Unbond { lp_token: lp_token_addr.clone(), amount: Uint128::from(50_000 as u64) },
        &vec![],
    ).unwrap();
    
    let query_msg = QueryMsg::UnclaimedRewards { lp_token: lp_token_addr.clone(), user: user_addr.clone() };
    let response: Vec<UnclaimedReward> = app.wrap().query_wasm_smart(multi_staking_instance.clone(), &query_msg).unwrap();

    println!("Response: {:?}", response);
    assert_eq!(response.len(), 1);
    let unclaimed_reward = response.get(0).unwrap();
    assert_eq!(unclaimed_reward.amount, Uint128::from(100_000_000 as u64));
    assert_eq!(unclaimed_reward.asset, dexter::asset::AssetInfo::NativeToken { denom: "uxprt".to_string() });


    // Withdraw the rewards
    app.execute_contract(
        user_addr.clone(), 
        multi_staking_instance.clone(),
        &ExecuteMsg::Withdraw { lp_token: lp_token_addr.clone() },
        &vec![],
    ).unwrap();

    // query bank module for user balance
    let balances =  app.wrap().query_all_balances(user_addr).unwrap();
    let balance_uxprt = balances.iter().find(|b| b.denom == "uxprt").unwrap();
    assert_eq!(balance_uxprt.amount, Uint128::from(100_000_000 as u64));
}