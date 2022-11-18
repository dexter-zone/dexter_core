use cosmwasm_std::{Addr, testing::mock_env, Timestamp, Coin, Uint128, to_binary};
use cw_multi_test::{App, Executor, ContractWrapper};
use dexter::{multi_staking::{InstantiateMsg, ExecuteMsg, Cw20HookMsg, QueryMsg, UnclaimedReward}, asset::AssetInfo};
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
    let instantiate_msg = InstantiateMsg {
        admin: admin.clone(),
    };

    let multi_staking_instance = app
        .instantiate_contract(
            code_id,
            admin.to_owned(),
            &instantiate_msg,
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

#[allow(dead_code)]
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

    // let cw20_code_id = store_cw20_contract(app);
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


fn create_reward_schedule(
    app: &mut App,
    admin_addr: &Addr,
    multistaking_contract: &Addr,
    lp_token: &Addr,
    reward_asset: AssetInfo,
    amount: Uint128,
    start_block_time: u64,
    end_block_time: u64,
) {

    match reward_asset {
        AssetInfo::NativeToken { denom } => {
            app.execute_contract(
                admin_addr.clone(), 
                multistaking_contract.clone(), 
                &ExecuteMsg::AddRewardFactory { 
                        lp_token: lp_token.clone(), 
                        denom: denom.clone(), 
                        amount: amount.clone(), 
                        start_block_time, 
                        end_block_time 
                },
                &vec![Coin::new(amount.u128(), denom.as_str())]
            ).unwrap();
        },
        AssetInfo::Token { contract_addr } => {
            app.execute_contract(
                admin_addr.clone(), 
                contract_addr.clone(), 
                &Cw20ExecuteMsg::Send { 
                        contract: multistaking_contract.to_string(),
                        amount,
                        msg: to_binary(&Cw20HookMsg::AddRewardFactory {
                            lp_token: lp_token.clone(),
                            start_block_time,
                            end_block_time,
                        }).unwrap()
                    },
                &vec![]
            ).unwrap();
        }
    }
}

fn mint_lp_tokens_to_addr(
    app: &mut App,
    admin_addr: &Addr,
    lp_token_addr: &Addr,
    recipient_addr: &Addr,
    amount: Uint128,
) {
    app.execute_contract(
        admin_addr.clone(),
        lp_token_addr.clone(),
        &Cw20ExecuteMsg::Mint {
            recipient: recipient_addr.to_string(),
            amount,
        },
        &vec![],
    )
    .unwrap();
}

fn bond_lp_tokens(
    app: &mut App,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
    sender: &Addr,
    amount: Uint128,
) {
    app.execute_contract(
        sender.clone(),
        lp_token_addr.clone(),
        &Cw20ExecuteMsg::Send {
            contract: multistaking_contract.to_string(),
            amount,
            msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
        },
        &vec![],
    )
    .unwrap();
}

fn unbond_lp_tokens(
    app: &mut App,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
    sender: &Addr,
    amount: Uint128,
) {
    app.execute_contract(
        sender.clone(), 
        multistaking_contract.clone(),
        &ExecuteMsg::Unbond { lp_token: lp_token_addr.clone(), amount },
        &vec![],
    ).unwrap();
}

fn query_unclaimed_rewards(
    app: &mut App,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
    user_addr: &Addr,
) -> Vec<UnclaimedReward> {
    app
        .wrap()
        .query_wasm_smart(
            multistaking_contract.clone(),
            &QueryMsg::UnclaimedRewards {
                lp_token: lp_token_addr.clone(),
                user: user_addr.clone(),
                block_time: None
            },
        )
        .unwrap()
}

fn withdraw_unclaimed_rewards(
    app: &mut App,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
    user_addr: &Addr,
) {
    app.execute_contract(
        user_addr.clone(),
        multistaking_contract.clone(),
        &ExecuteMsg::Withdraw {
            lp_token: lp_token_addr.clone(),
        },
        &vec![],
    )
    .unwrap();
}

// test if only admin is able to allow new lp tokens for reward
#[test]
fn test_allow_lp_token() {
    let admin = String::from("admin");
    let coins = vec![
        Coin::new(1000_000_000, "uxprt"),
        Coin::new(1000_000_000, "uatom"),
    ];
    let admin_addr = Addr::unchecked(admin.clone());
    let mut app = mock_app(admin_addr.clone(), coins);

    let (multi_staking_instance, lp_token_addr) = setup(&mut app, admin_addr.clone());

    // New LP token
    let new_lp_token_addr = Addr::unchecked("new_lp_token".to_string());

    // Create a new reward schedule
    let unauthorized_addr = Addr::unchecked("unauthorized".to_string());
    let response = app.execute_contract(
        unauthorized_addr.clone(), 
        multi_staking_instance.clone(), 
        &ExecuteMsg::AllowLpToken { 
            lp_token: new_lp_token_addr.clone(), 
        },
        &vec![]
    );
    
    assert!(response.is_err());
    // Check the error is amount insufficied fundsinsufficient funds
    assert_eq!(response.unwrap_err().root_cause().to_string(), "Generic error: Only admin can allow lp token for reward");

    // Allow lp token for reward
   let response =  app.execute_contract(
        admin_addr.clone(), 
        multi_staking_instance.clone(), 
        &ExecuteMsg::AllowLpToken { 
            lp_token: new_lp_token_addr.clone(),
        },
        &vec![]
    );

    // response should be ok
    response.unwrap();
    // assert_eq!(response.is_ok(), true);

    // Check if lp token is allowed for reward
    let allowed_lp_tokens: Vec<Addr> = app.wrap().query_wasm_smart(
        multi_staking_instance.clone(), 
        &QueryMsg::AllowedLPTokensForReward {}
    ).unwrap();

    assert_eq!(allowed_lp_tokens.len(), 2);
    assert_eq!(allowed_lp_tokens[0], lp_token_addr);
    assert_eq!(allowed_lp_tokens[1], new_lp_token_addr);
}

#[test]
fn test_update_admin() {
    let admin = String::from("admin");
    let coins = vec![
        Coin::new(1000_000_000, "uxprt"),
        Coin::new(1000_000_000, "uatom"),
    ];
    let admin_addr = Addr::unchecked(admin.clone());
    let mut app = mock_app(admin_addr.clone(), coins);

    let (multi_staking_instance, _lp_token_addr) = setup(&mut app, admin_addr.clone());

    // New admin
    let new_admin_addr = Addr::unchecked("new_admin".to_string());

    // Create a new reward schedule
    let unauthorized_addr = Addr::unchecked("unauthorized".to_string());
    let response = app.execute_contract(
        unauthorized_addr.clone(), 
        multi_staking_instance.clone(), 
        &ExecuteMsg::UpdateAdmin { 
            new_admin: new_admin_addr.clone(), 
        },
        &vec![]
    );
    
    assert!(response.is_err());
    // Check the error is amount insufficied fundsinsufficient funds
    assert_eq!(response.unwrap_err().root_cause().to_string(), "Generic error: Only admin can update admin");

    // Update admin
   let response =  app.execute_contract(
        admin_addr.clone(), 
        multi_staking_instance.clone(), 
        &ExecuteMsg::UpdateAdmin { 
            new_admin: new_admin_addr.clone(),
        },
        &vec![]
    );

    // response should be ok
    response.unwrap();
    // assert_eq!(response.is_ok(), true);

    // Check if admin is updated
    let admin: Addr = app.wrap().query_wasm_smart(
        multi_staking_instance.clone(), 
        &QueryMsg::Admin {}
    ).unwrap();

    assert_eq!(admin, new_admin_addr);
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

    create_reward_schedule(&mut app, 
        &admin_addr, 
        &multi_staking_instance, 
        &lp_token_addr, 
        AssetInfo::NativeToken { denom: "uxprt".to_string() },
        Uint128::from( 100_000_000 as u64),
        1000_001_000,
        1000_002_000
    );

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_000);
        b.height = b.height + 100;
    });

    // Mint some LP tokens
    mint_lp_tokens_to_addr(&mut app, &admin_addr, &lp_token_addr, &user_addr, Uint128::from(100_000_000 as u64));

    bond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_addr, Uint128::from(100_000_000 as u64));

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_500);
        b.height = b.height + 100;
    });

    // Unbond half of the amoutt at 50% of the reward schedule
    unbond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_addr, Uint128::from(50_000_000 as u64));

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_002_001);
        b.height = b.height + 100;
    });

    unbond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_addr, Uint128::from(50_000_000 as u64));
    
    let query_msg = QueryMsg::UnclaimedRewards { lp_token: lp_token_addr.clone(), user: user_addr.clone(), block_time: None };
    let response: Vec<UnclaimedReward> = app.wrap().query_wasm_smart(multi_staking_instance.clone(), &query_msg).unwrap();

    assert_eq!(response.len(), 1);
    let unclaimed_reward = response.get(0).unwrap();
    assert_eq!(unclaimed_reward.amount, Uint128::from(100_000_000 as u64));
    assert_eq!(unclaimed_reward.asset, dexter::asset::AssetInfo::NativeToken { denom: "uxprt".to_string() });


    // Withdraw the rewards
    withdraw_unclaimed_rewards(&mut app, &multi_staking_instance, &lp_token_addr, &user_addr);

    // query bank module for user balance
    let balances =  app.wrap().query_all_balances(user_addr).unwrap();
    let balance_uxprt = balances.iter().find(|b| b.denom == "uxprt").unwrap();
    assert_eq!(balance_uxprt.amount, Uint128::from(100_000_000 as u64));
}


#[test]
fn test_multi_asset_multi_reward_schedules() {
    let admin = String::from("admin");
    let user_1 = String::from("user_1");

    let coins = vec![
        Coin::new(1000_000_000, "uxprt"),
        Coin::new(1000_000_000, "uatom"),
    ];

    let admin_addr = Addr::unchecked(admin.clone());
    let user_1_addr = Addr::unchecked(user_1.clone());

    let mut app = mock_app(admin_addr.clone(), coins);

    let (multi_staking_instance, lp_token_addr) = setup(&mut app, admin_addr.clone());

    create_reward_schedule(&mut app, 
        &admin_addr, 
        &multi_staking_instance, 
        &lp_token_addr, 
        AssetInfo::NativeToken { denom: "uxprt".to_string() },
        Uint128::from(100_000_000 as u64),
        1000_001_000,
        1000_002_000
    );

    create_reward_schedule(&mut app, 
        &admin_addr, 
        &multi_staking_instance, 
        &lp_token_addr, 
        AssetInfo::NativeToken { denom: "uxprt".to_string() },
        Uint128::from(150_000_000 as u64),
        1000_001_500,
        1000_002_000
    );

    create_reward_schedule(&mut app, 
        &admin_addr, 
        &multi_staking_instance, 
        &lp_token_addr, 
        AssetInfo::NativeToken { denom: "uatom".to_string() },
        Uint128::from(200_000_000 as u64),
        1000_001_200,
        1000_002_000
    );

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_000);
        b.height = b.height + 100;
    });

    // Mint some LP tokens
    mint_lp_tokens_to_addr(&mut app, &admin_addr, &lp_token_addr, &user_1_addr, Uint128::from(200_000 as u64));

    bond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr, Uint128::from(100_000 as u64));

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_500);
        b.height = b.height + 100;
    });

    // Unbond half of the amoutt at 50% of the reward schedule
    unbond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr, Uint128::from(50_000 as u64));

     
    let unclaimed_rewards_user_1 = query_unclaimed_rewards(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr);    
    assert_eq!(unclaimed_rewards_user_1.len(), 2);

    for unclaimed_reward in unclaimed_rewards_user_1 {
        match unclaimed_reward.asset {
            AssetInfo::NativeToken { denom } => {
                if denom == "uxprt" {
                    assert_eq!(unclaimed_reward.amount, Uint128::from(50_000_000 as u64));
                } else if denom == "uatom" {
                    assert_eq!(unclaimed_reward.amount, Uint128::from(75_000_000 as u64));
                } else {
                    panic!("Unexpected denom");
                }
            },
            _ => panic!("Unexpected asset type"),
        }
    }

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_002_001);
        b.height = b.height + 100;
    });

    unbond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr, Uint128::from(50_000 as u64));

    let unclaimed_rewards_user_1 = query_unclaimed_rewards(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr);    
    assert_eq!(unclaimed_rewards_user_1.len(), 2);

    // validate unclaimed rewards
    for unclaimed_reward in unclaimed_rewards_user_1 {
        match unclaimed_reward.asset {
            AssetInfo::NativeToken { denom } => {
                if denom == "uxprt" {
                    assert_eq!(unclaimed_reward.amount, Uint128::from(250_000_000 as u64));
                } else if denom == "uatom" {
                    assert_eq!(unclaimed_reward.amount, Uint128::from(200_000_000 as u64));
                } else {
                    panic!("Unexpected denom");
                }
            },
            _ => panic!("Unexpected asset type"),
        }
    }

    // withdraw the rewards
    withdraw_unclaimed_rewards(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr);
    // validate the withdrawn amount
    let balances =  app.wrap().query_all_balances(user_1_addr.clone()).unwrap();
    let uxprt_balance = balances.iter().find(|b| b.denom == "uxprt").unwrap();
    let uatom_balance = balances.iter().find(|b| b.denom == "uatom").unwrap();

    assert_eq!(uxprt_balance.amount, Uint128::from(250_000_000 as u64));
    assert_eq!(uatom_balance.amount, Uint128::from(200_000_000 as u64));

}

#[test]
fn test_multi_user_multi_reward_schedule() {
    let admin = String::from("admin");
    let user_1 = String::from("user_1");
    let user_2 = String::from("user_2");

    let coins = vec![
        Coin::new(1000_000_000, "uxprt"),
        Coin::new(1000_000_000, "uatom"),
    ];

    let admin_addr = Addr::unchecked(admin.clone());
    let user_1_addr = Addr::unchecked(user_1.clone());
    let user_2_addr = Addr::unchecked(user_2.clone());

    let mut app = mock_app(admin_addr.clone(), coins);

    let (multi_staking_instance, lp_token_addr) = setup(&mut app, admin_addr.clone());

    create_reward_schedule(&mut app, 
        &admin_addr, 
        &multi_staking_instance, 
        &lp_token_addr, 
        AssetInfo::NativeToken { denom: "uxprt".to_string() },
        Uint128::from(100_000_000 as u64),
        1000_001_000,
        1000_002_000
    );

    create_reward_schedule(&mut app, 
        &admin_addr, 
        &multi_staking_instance, 
        &lp_token_addr, 
        AssetInfo::NativeToken { denom: "uxprt".to_string() },
        Uint128::from(100_000_000 as u64),
        1000_001_500,
        1000_002_000
    );

    create_reward_schedule(&mut app, 
        &admin_addr, 
        &multi_staking_instance, 
        &lp_token_addr, 
        AssetInfo::NativeToken { denom: "uatom".to_string() },
        Uint128::from(200_000_000 as u64),
        1000_001_200,
        1000_002_000
    );

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_000);
        b.height = b.height + 100;
    });

    // Mint some LP tokens
    mint_lp_tokens_to_addr(&mut app, &admin_addr, &lp_token_addr, &user_1_addr, Uint128::from(200_000 as u64));
    mint_lp_tokens_to_addr(&mut app, &admin_addr, &lp_token_addr, &user_2_addr, Uint128::from(1_000_000 as u64));

    bond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr, Uint128::from(100_000 as u64));

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_200);
        b.height = b.height + 100;
    });

    bond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_2_addr, Uint128::from(1_000_000 as u64));

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_500);
        b.height = b.height + 100;
    });

    // Unbond half of the amoutt at 50% of the reward schedule
    unbond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr, Uint128::from(50_000 as u64));

    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_002_001);
        b.height = b.height + 100;
    });

    unbond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr, Uint128::from(50_000 as u64));
    unbond_lp_tokens(&mut app, &multi_staking_instance, &lp_token_addr, &user_2_addr, Uint128::from(1_000_000 as u64));
    

    let unclaimed_rewards_user_1 = query_unclaimed_rewards(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr);
    let unclaimed_rewards_user_2 = query_unclaimed_rewards(&mut app, &multi_staking_instance, &lp_token_addr, &user_2_addr);
    
    // validator unclaimed rewards 
    assert_eq!(unclaimed_rewards_user_1.len(), 2);
    assert_eq!(unclaimed_rewards_user_2.len(), 2);

    for unclaimed_reward in unclaimed_rewards_user_1 {
        if let AssetInfo::NativeToken { denom } = unclaimed_reward.asset {
            match denom.as_str() {
                "uxprt" => assert_eq!(unclaimed_reward.amount, Uint128::from(29_870_129 as u64)),
                "uatom" => assert_eq!(unclaimed_reward.amount, Uint128::from(12_770_562 as u64)),
                _ => panic!("Unexpected denom"),
            }
        } else {
            panic!("Unexpected asset type")
        }
    }

    for unclaimed_reward in unclaimed_rewards_user_2 {
        if let AssetInfo::NativeToken { denom } = unclaimed_reward.asset {
            match denom.as_str() {
                "uxprt" => assert_eq!(unclaimed_reward.amount, Uint128::from(170_129_870 as u64)),
                "uatom" => assert_eq!(unclaimed_reward.amount, Uint128::from(187_229_437 as u64)),
                _ => panic!("Unexpected denom"),
            }
        } else {
            panic!("Unexpected asset type")
        }
    }

    // withdraw rewards
    withdraw_unclaimed_rewards(&mut app, &multi_staking_instance, &lp_token_addr, &user_1_addr);
    
    let user_1_balance = app.wrap().query_all_balances(user_1_addr.clone()).unwrap();
    
    let user1_uxprt_balance = user_1_balance.iter().find(|b| b.denom == "uxprt").unwrap();
    let user1_uatom_balance = user_1_balance.iter().find(|b| b.denom == "uatom").unwrap();
    
    assert_eq!(user1_uxprt_balance.amount, Uint128::from(29_870_129 as u64));
    assert_eq!(user1_uatom_balance.amount, Uint128::from(12_770_562 as u64));
    
    withdraw_unclaimed_rewards(&mut app, &multi_staking_instance, &lp_token_addr, &user_2_addr);
    
    let user_2_balance = app.wrap().query_all_balances(user_2_addr.clone()).unwrap();
    
    let user2_uxprt_balance = user_2_balance.iter().find(|b| b.denom == "uxprt").unwrap();
    let user2_uatom_balance = user_2_balance.iter().find(|b| b.denom == "uatom").unwrap();

    assert_eq!(user2_uxprt_balance.amount, Uint128::from(170_129_870 as u64));
    assert_eq!(user2_uatom_balance.amount, Uint128::from(187_229_437 as u64));

}