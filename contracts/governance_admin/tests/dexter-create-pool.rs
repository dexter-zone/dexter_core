use cosmwasm_std::{to_binary, Addr, Coin, Uint128};
use dexter::{
    asset::{Asset, AssetInfo},
    governance_admin::{GovernanceProposalDescription, PoolCreationRequest},
    vault::{FeeInfo, NativeAssetPrecisionInfo, PoolInfoResponse}, multi_staking::{RewardSchedule, RewardScheduleResponse},
};
use persistence_std::types::cosmos::gov::{self, v1::{QueryParamsRequest, QueryProposalRequest, ProposalStatus}};
use persistence_test_tube::{Account, Module, Wasm, Gov};
use weighted_pool::state::WeightedParams;

mod utils;

#[test]
fn test_basic_functions() {
    let governance_params_query = QueryParamsRequest {
        params_type: String::from("deposit"),
    };

    println!("{:?}", governance_params_query);
}


#[test]
fn test_create_pool() {
    let vault_creator: Addr = Addr::unchecked("vault_creator".to_string());
    let keeper_owner: Addr = Addr::unchecked("keeper_owner".to_string());
    let _alice_address: Addr = Addr::unchecked("alice".to_string());

    let gov_admin_test_setup = utils::setup_test_contracts();

    panic!("Can't proceed");
    let persistence_test_app = &gov_admin_test_setup.persistence_test_app;
    let wasm = Wasm::new(&gov_admin_test_setup.persistence_test_app);

    let validator_signing_account = persistence_test_app.get_first_validator_signing_account().unwrap();
    let admin = &gov_admin_test_setup.accs[0];

    // mint CW20 tokens to the user
    let mint_msg = cw20_base::msg::ExecuteMsg::Mint {
        recipient: admin.address().to_string(),
        amount: Uint128::from(10000000000u128),
    };

    let _ = wasm.execute(
        &gov_admin_test_setup.cw20_token_1.to_string(),
        &mint_msg,
        &vec![],
        &admin,
    ).unwrap();

    let asset_info = vec![
        AssetInfo::native_token("uxprt".to_string()),
        AssetInfo::token(gov_admin_test_setup.cw20_token_1.clone()),
    ];

    let bootstrapping_amount = vec![
        Asset::new(asset_info[0].clone(), Uint128::from(1000000u128)),
        Asset::new(asset_info[1].clone(), Uint128::from(1000000u128)),
    ];

    // find the current block time in the chain
    let current_block_time = persistence_test_app.get_block_time_seconds() as u64;

    let reward_schedules = vec![
        dexter::governance_admin::RewardScheduleCreationRequest {
            lp_token_addr: None,
            title: "Reward Schedule 1".to_string(),
            asset: AssetInfo::native_token("uxprt".to_string()),
            amount: Uint128::from(1000000u128),
            start_block_time: current_block_time + 3*24*60*60,
            end_block_time: current_block_time + 10*24*60*60,
        }
    ];

    // create a pool using governance admin
    let valid_pool_creation_request = PoolCreationRequest {
        vault_addr: gov_admin_test_setup.vault_instance.to_string(),
        pool_type: dexter::vault::PoolType::Weighted {},
        fee_info: Some(FeeInfo {
            total_fee_bps: 30,
            protocol_fee_percent: 30,
        }),
        native_asset_precisions: vec![NativeAssetPrecisionInfo {
            denom: "uxprt".to_string(),
            precision: 6,
        }],
        asset_info: vec![
            AssetInfo::native_token("uxprt".to_string()),
            AssetInfo::token(gov_admin_test_setup.cw20_token_1.clone()),
        ],
        init_params: Some(
            to_binary(&WeightedParams {
                weights: asset_info
                    .iter()
                    .map(|i| Asset::new(i.clone(), Uint128::from(1u128)))
                    .collect(),
                exit_fee: None,
            })
            .unwrap(),
        ),
        bootstrapping_liquidity_owner: admin.address().to_string(),
        bootstrapping_amount: Some(bootstrapping_amount),
        reward_schedules: Some(reward_schedules),
    };

    let proposal_description =  GovernanceProposalDescription {
        title: "Create Pool".to_string(),
        metadata: "Create Pool".to_string(),
        summary: "Create Pool".to_string(),
    };

    let valid_create_pool_msg = dexter::governance_admin::ExecuteMsg::CreatePoolCreationProposal {
        proposal_description: proposal_description.clone(),
        pool_creation_request: valid_pool_creation_request.clone(),
    };

    // Failure Scenario 1: Try to bootstrap a pool with less assets than the actual number of assets in the pool
    let incorrect_bootstrapping_amount = vec![
        Asset::new(asset_info[0].clone(), Uint128::from(1000000u128)),
    ];

    let failure_case_pool_creation_request = PoolCreationRequest {
        bootstrapping_amount: Some(incorrect_bootstrapping_amount),
        ..valid_pool_creation_request.clone()
    };

    let failure_case_create_pool_msg = dexter::governance_admin::ExecuteMsg::CreatePoolCreationProposal {
        proposal_description: proposal_description.clone(),
        pool_creation_request: failure_case_pool_creation_request.clone()
    };
    

    // send funds 
    let total_funds_to_send = vec![
        // proposal deposit + bootstrapping amount
        Coin::new(110000000, "uxprt")
    ];

    let result = wasm
        .execute(
            &gov_admin_test_setup.gov_admin_instance.to_string(),
            &failure_case_create_pool_msg,
            &total_funds_to_send,
            &gov_admin_test_setup.accs[0],
        );

    println!("result: {:?}", result);

    // assert error
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: Insufficient funds sent for pool creation for uxprt - Amount Sent: 110000000 - Needed Amount: 12000000: execute wasm contract failed");

    
    // Failure Scenario 2: Try to create a proposal with sufficient native funds but without approving CW20 tokens
    let result = wasm
        .execute(
            &gov_admin_test_setup.gov_admin_instance.to_string(),
            &valid_create_pool_msg,
            &total_funds_to_send,
            &gov_admin_test_setup.accs[0],
        );

    println!("result: {:?}", result);
    // assert error
    assert!(result.is_err());
    let error = result.unwrap_err();

    assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: Insufficient spend limit for token persistence1mf6ptkssddfmxvhdx0ech0k03ktp6kf9yk59renau2gvht3nq2gqtu9smg - Current approval: 0 - Needed Approval: 1000000: execute wasm contract failed");

     // approve spending of CW20 tokens
     let approve_msg = cw20_base::msg::ExecuteMsg::IncreaseAllowance {
        spender: gov_admin_test_setup.gov_admin_instance.to_string(),
        amount: Uint128::from(1000000u128),
        expires: None
    };

    let _ = wasm.execute(
        &gov_admin_test_setup.cw20_token_1.to_string(),
        &approve_msg,
        &vec![],
        &gov_admin_test_setup.accs[0],
    ).unwrap();

    // Failure Scenario 3: Try to create a proposal with insufficient native funds
    let result = wasm
        .execute(
            &gov_admin_test_setup.gov_admin_instance.to_string(),
            &valid_create_pool_msg,
            &vec![],
            &gov_admin_test_setup.accs[0],
        );

    println!("result: {:?}", result);
    // assert error
    assert!(result.is_err());
    let error = result.unwrap_err();

    assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: Insufficient funds sent for pool creation for uxprt - Amount Sent: 0 - Needed Amount: 12000000: execute wasm contract failed");
   

    // let events = wasm
    //     .execute(
    //         &gov_admin_test_setup.gov_admin_instance.to_string(),
    //         &create_pool_msg,
    //         &total_funds_to_send,
    //         admin,
    //     )
    //     .unwrap()
    //     .events;

    // println!("{}", serde_json_wasm::to_string(&events).unwrap());
    // // find the proposal id from events
    // let proposal_id = events
    //     .iter()
    //     .find(|e| e.ty == "submit_proposal")
    //     .unwrap()
    //     .attributes
    //     .iter()
    //     .find(|a| a.key == "proposal_id")
    //     .unwrap()
    //     .value
    //     .clone();

    // println!("proposal_id: {}", proposal_id);

    // // vote on the proposal
    // let vote_msg = persistence_std::types::cosmos::gov::v1::MsgVote {
    //     proposal_id: proposal_id.parse().unwrap(),
    //     voter: validator_signing_account.address(),
    //     option: persistence_std::types::cosmos::gov::v1::VoteOption::Yes.into(),
    //     metadata: "".to_string()
    // };

    // let gov = Gov::new(&gov_admin_test_setup.persistence_test_app);
    // gov.vote(vote_msg, &validator_signing_account).unwrap();

    // // wait for the proposal to pass
    // let proposal = gov.query_proposal(&QueryProposalRequest {
    //     proposal_id: proposal_id.parse().unwrap(),
    // }).unwrap().proposal.unwrap();

    // // find the proposal voting end time and increase chain time to that
    // let proposal_end_time = proposal.voting_end_time.unwrap();
    // let proposal_start_time = proposal.voting_start_time.unwrap();

    // let difference_seconds = proposal_end_time.seconds - proposal_start_time.seconds;

    // gov_admin_test_setup.persistence_test_app.increase_time(difference_seconds as u64);

    // // query proposal again
    // let proposal = gov.query_proposal(&QueryProposalRequest {
    //     proposal_id: proposal_id.parse().unwrap(),
    // }).unwrap().proposal.unwrap();

    // println!("proposal: {:?}", proposal);

    // assert that the proposal has passed
    // assert_eq!(proposal.status, ProposalStatus::Passed as i32);

    // HACK: send execute resume create pool manually to find issue
    // let resume_create_pool_msg = dexter::governance_admin::ExecuteMsg::ResumeCreatePool {
    //     pool_creation_request_id: 1,
    // };

    // let result = wasm.execute(
    //     &gov_admin_test_setup.gov_admin_instance.to_string(),
    //     &resume_create_pool_msg,
    //     &vec![],
    //     &gov_admin_test_setup.accs[0],
    // ).unwrap();

    // println!("result: {:?}", result);


    // validate that the pool has been created successfully
    // query the vault contract to find the pool id

    let get_pool_by_id_query = dexter::vault::QueryMsg::GetPoolById { 
        // first pool so it must be 1
        pool_id: Uint128::from(1u128),
    };

    let pool_info: PoolInfoResponse = wasm.query(
        &gov_admin_test_setup.vault_instance.to_string(),
        &get_pool_by_id_query,
    ).unwrap();

    println!("pool_info: {:?}", pool_info);

    // validate if reward schedule is created by querying the multistaking contract
    let get_reward_schedules_query = dexter::multi_staking::QueryMsg::RewardSchedules {
        lp_token: pool_info.lp_token_addr,
        asset: AssetInfo::native_token("uxprt".to_string()),
    };

    let reward_schedules: Vec<RewardScheduleResponse> = wasm.query(
        &gov_admin_test_setup.multi_staking_instance.to_string(),
        &get_reward_schedules_query,
    ).unwrap();

    println!("reward_schedules: {:?}", reward_schedules);

    // print
}
