use cosmwasm_std::{Coin, Addr, Uint128};
use cw_multi_test::Executor;
use dexter::multi_staking::{ExecuteMsg, QueryMsg};
use crate::utils::{setup, mock_app};

mod utils;

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
        &ExecuteMsg::AddRewardSchedule { 
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
        &ExecuteMsg::AddRewardSchedule { 
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