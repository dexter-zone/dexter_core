use cosmwasm_std::{Coin, Addr, Timestamp};
use cw_multi_test::Executor;
use dexter::multi_staking::{ExecuteMsg, QueryMsg};

use crate::utils::{setup, mock_app};

pub mod utils;

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

    // Test: try to create ownership proposal from unauthorized address
    let unauthorized_addr = Addr::unchecked("unauthorized".to_string());
    let response = app.execute_contract(
        unauthorized_addr.clone(), 
        multi_staking_instance.clone(), 
        &ExecuteMsg::ProposeNewOwner { 
            owner: new_admin_addr.clone(),
            expires_in: 1000,
        },
        &vec![]
    );
    
    assert!(response.is_err());
    // Check error is unauthorized
    assert_eq!(response.unwrap_err().root_cause().to_string(), "Generic error: Unauthorized");

    // Create owner update proposal
    app.execute_contract(
        admin_addr.clone(), 
        multi_staking_instance.clone(), 
        &ExecuteMsg::ProposeNewOwner {
            owner: new_admin_addr.clone(),
            expires_in: 1000,
        },
        &vec![]
    ).unwrap();

    // Test: Try to claim ownership from unauthorized address
    let response = app.execute_contract(
        unauthorized_addr.clone(), 
        multi_staking_instance.clone(), 
        &ExecuteMsg::ClaimOwnership {},
        &vec![]
    );

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().root_cause().to_string(), "Generic error: Unauthorized");

    // Test: Try to claim ownership after proposal expires
    // update block
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_001);
        b.height = b.height + 100;
    });

    // try claiming ownership
    let response =  app.execute_contract(
        new_admin_addr.clone(), 
        multi_staking_instance.clone(), 
        &ExecuteMsg::ClaimOwnership {},
        &vec![]
    );

    // response should be error
    assert!(response.is_err());
    // Error should be time expired
    assert_eq!(response.unwrap_err().root_cause().to_string(), "Generic error: Ownership proposal expired");

    // Test: Try to claim ownership before proposal expires
    // Create new owner update proposal
    app.execute_contract(
        admin_addr.clone(), 
        multi_staking_instance.clone(), 
        &ExecuteMsg::ProposeNewOwner {
            owner: new_admin_addr.clone(),
            expires_in: 1000,
        },
        &vec![]
    ).unwrap();

    // update block
    app.update_block(|b| {
        b.time = Timestamp::from_seconds(1_000_001_500);
        b.height = b.height + 100;
    });

    // try claiming ownership
    let response =  app.execute_contract(
        new_admin_addr.clone(), 
        multi_staking_instance.clone(), 
        &ExecuteMsg::ClaimOwnership {},
        &vec![]
    );

    // response should be ok
    assert!(response.is_ok());

    // Check if admin is updated
    let admin: Addr = app.wrap().query_wasm_smart(
        multi_staking_instance.clone(), 
        &QueryMsg::Owner {}
    ).unwrap();

    assert_eq!(admin, new_admin_addr);


    // Test: Try to claim ownership after proposal drop
    // Create new owner update proposal
    app.execute_contract(
        new_admin_addr.clone(), 
        multi_staking_instance.clone(), 
        &ExecuteMsg::ProposeNewOwner {
            owner: admin_addr.clone(),
            expires_in: 1000,
        },
        &vec![]
    ).unwrap();

    // drop the proposal
    app.execute_contract(
        new_admin_addr.clone(), 
        multi_staking_instance.clone(), 
        &ExecuteMsg::DropOwnershipProposal {},
        &vec![]
    ).unwrap();

    // try to claim ownership
    let response =  app.execute_contract(
        admin_addr.clone(), 
        multi_staking_instance.clone(), 
        &ExecuteMsg::ClaimOwnership {},
        &vec![]
    );

    // response should be error
    assert!(response.is_err());
    // Error should be proposal not found
    assert_eq!(response.unwrap_err().root_cause().to_string(), "Generic error: Ownership proposal not found");
}
