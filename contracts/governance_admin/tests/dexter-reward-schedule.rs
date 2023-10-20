use cosmwasm_std::{to_binary, Uint128, Addr, Coin, Event, CosmosMsg, WasmMsg};
use dexter::{
    vault::{
        FeeInfo,
        ExecuteMsg as VaultExecuteMsg,
        NativeAssetPrecisionInfo,
        QueryMsg as VaultQueryMsg, PoolInfoResponse, PoolInfo,
    }, 
    asset::{AssetInfo, Asset},
    governance_admin::{GovernanceProposalDescription, RewardScheduleCreationRequest, ExecuteMsg as GovExecuteMsg},
};
use dexter_governance_admin::contract::GOV_MODULE_ADDRESS;
use persistence_std::types::{cosmwasm::wasm::v1::MsgExecuteContract, cosmos::gov::v1::{QueryProposalRequest, ProposalStatus, MsgSubmitProposal, VoteOption}};
use persistence_test_tube::{Wasm, Module, Account, Gov};
use utils::GovAdminTestSetup;
use weighted_pool::state::WeightedParams;

mod utils;

#[test]
fn test_reward_schedule() {
    let gov_admin_test_setup = utils::setup_test_contracts();
    println!("gov_admin_test_setup: {}", gov_admin_test_setup);

    let user = &gov_admin_test_setup.accs[0];
    let persistence_test_app = &gov_admin_test_setup.persistence_test_app;
    let wasm = Wasm::new(persistence_test_app);
    let current_block_time = persistence_test_app.get_block_time_seconds() as u64;

    // create pool
    let asset_infos = vec![
        AssetInfo::native_token("uxprt".to_string()),
        AssetInfo::token(gov_admin_test_setup.cw20_token_1.clone()),
    ];
    let pool_info = create_pool(&gov_admin_test_setup, asset_infos);

    // Request create reward schedule
    let proposal_description =  GovernanceProposalDescription {
        title: "Create reward schedule".to_string(),
        metadata: "Create reward schedule".to_string(),
        summary: "Create reward schedule".to_string(),
    };

    let valid_request = RewardScheduleCreationRequest {
        lp_token_addr: Some(pool_info.lp_token_addr.clone()),
        title: "Reward Schedule".to_string(),
        asset: AssetInfo::native_token("uxprt".to_string()),
        amount: Uint128::from(1000000u128),
        start_block_time: current_block_time + 7*24*60*60,
        end_block_time: current_block_time + 14*24*60*60,
    };

    // min proposal deposit + reward amount
    let valid_funds = vec![Coin::new(11000000, "uxprt")];

    // Case1: Empty reward schedules
    let result = wasm.execute(
        &gov_admin_test_setup.gov_admin_instance.to_string(),
        &GovExecuteMsg::CreateRewardSchedulesProposal {
            proposal_description: proposal_description.clone(),
            multistaking_contract_addr: gov_admin_test_setup.multi_staking_instance.to_string(),
            reward_schedule_creation_requests: vec![],
        },
        &valid_funds,
        &user,
    );

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: Must provide at least one reward schedule: execute wasm contract failed");

    // Case2: LP token not allowed
    let result = wasm.execute(
        &gov_admin_test_setup.gov_admin_instance.to_string(),
        &GovExecuteMsg::CreateRewardSchedulesProposal {
            proposal_description: proposal_description.clone(),
            multistaking_contract_addr: gov_admin_test_setup.multi_staking_instance.to_string(),
            reward_schedule_creation_requests: vec![valid_request.clone()],
        },
        &valid_funds,
        &user,
    );

    assert!(result.is_err());
    let error = result.unwrap_err();
    let expected_error = format!("execute error: failed to execute message; message index: 0: Generic error: LP token {} is not allowed for reward distribution: execute wasm contract failed", pool_info.lp_token_addr);
    assert_eq!(error.to_string(), expected_error);

    // Allow LP token
    allow_lp_token(&gov_admin_test_setup, pool_info.lp_token_addr.clone());

    // Case3: start time is within voting period
    let result = wasm.execute(
        &gov_admin_test_setup.gov_admin_instance.to_string(),
        &GovExecuteMsg::CreateRewardSchedulesProposal {
            proposal_description: proposal_description.clone(),
            multistaking_contract_addr: gov_admin_test_setup.multi_staking_instance.to_string(),
            reward_schedule_creation_requests: vec![RewardScheduleCreationRequest {
                start_block_time: current_block_time + 60 * 60,
                ..valid_request.clone()
            }],
        },
        &valid_funds,
        &user,
    );

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: Invalid reward schedule start block time: execute wasm contract failed");

    // Case4: end time is before start time
    let result = wasm.execute(
        &gov_admin_test_setup.gov_admin_instance.to_string(),
        &GovExecuteMsg::CreateRewardSchedulesProposal {
            proposal_description: proposal_description.clone(),
            multistaking_contract_addr: gov_admin_test_setup.multi_staking_instance.to_string(),
            reward_schedule_creation_requests: vec![RewardScheduleCreationRequest {
                end_block_time: current_block_time + 2 * 24 * 60 * 60,
                ..valid_request.clone()
            }],
        },
        &valid_funds,
        &user,
    );

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: End block time must be after start block time: execute wasm contract failed");

    // Case5: incorrect lp token
    let result = wasm.execute(
        &gov_admin_test_setup.gov_admin_instance.to_string(),
        &GovExecuteMsg::CreateRewardSchedulesProposal {
            proposal_description: proposal_description.clone(),
            multistaking_contract_addr: gov_admin_test_setup.multi_staking_instance.to_string(),
            reward_schedule_creation_requests: vec![RewardScheduleCreationRequest {
                lp_token_addr: None,
                ..valid_request.clone()
            }],
        },
        &valid_funds,
        &user,
    );

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: Generic error: LP token address is required for reward schedule creation request: execute wasm contract failed");

    // Case6: insufficient funds
    let result = wasm.execute(
        &gov_admin_test_setup.gov_admin_instance.to_string(),
        &GovExecuteMsg::CreateRewardSchedulesProposal {
            proposal_description: proposal_description.clone(),
            multistaking_contract_addr: gov_admin_test_setup.multi_staking_instance.to_string(),
            reward_schedule_creation_requests: vec![valid_request.clone()],
        },
        &vec![Coin::new(10000000, "uxprt")],
        &user,
    );

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: Insufficient funds sent for pool creation for uxprt - Amount Sent: 10000000 - Needed Amount: 11000000: execute wasm contract failed");

    // Case7: valid inputs & rejected proposal
    let result = wasm.execute(
        &gov_admin_test_setup.gov_admin_instance.to_string(),
        &GovExecuteMsg::CreateRewardSchedulesProposal {
            proposal_description: proposal_description.clone(),
            multistaking_contract_addr: gov_admin_test_setup.multi_staking_instance.to_string(),
            reward_schedule_creation_requests: vec![valid_request.clone()],
        },
        &valid_funds,
        &user,
    );

    assert!(result.is_ok());
    let events = result.unwrap().events;
    let proposal_id = find_event_attr(&events, "submit_proposal", "proposal_id");

    vote_on_proposal(&gov_admin_test_setup, proposal_id.parse().unwrap(), VoteOption::No);

    // TODO(ajeet): check reward schedule should not be created

    // Case8: valid inputs & passed proposal
    let result = wasm.execute(
        &gov_admin_test_setup.gov_admin_instance.to_string(),
        &GovExecuteMsg::CreateRewardSchedulesProposal {
            proposal_description: proposal_description.clone(),
            multistaking_contract_addr: gov_admin_test_setup.multi_staking_instance.to_string(),
            reward_schedule_creation_requests: vec![valid_request.clone()],
        },
        &valid_funds,
        &user,
    );

    assert!(result.is_ok());
    let events = result.unwrap().events;
    let proposal_id = find_event_attr(&events, "submit_proposal", "proposal_id");

    vote_on_proposal(&gov_admin_test_setup, proposal_id.parse().unwrap(), VoteOption::Yes);

    // TODO(ajeet): check reward schedule is created
}

fn create_pool(gov_admin_test_setup: &GovAdminTestSetup, asset_infos: Vec<AssetInfo>) -> PoolInfo {
    let create_pool_msg = VaultExecuteMsg::CreatePoolInstance {
        pool_type: dexter::vault::PoolType::Weighted {},
        fee_info: Some(FeeInfo {
            total_fee_bps: 30,
            protocol_fee_percent: 30,
        }),
        native_asset_precisions: vec![NativeAssetPrecisionInfo {
            denom: "uxprt".to_string(),
            precision: 6,
        }],
        init_params: Some(
            to_binary(&WeightedParams {
                weights: asset_infos
                    .iter()
                    .map(|i| Asset::new(i.clone(), Uint128::from(1u128)))
                    .collect(),
                exit_fee: None,
            })
            .unwrap(),
        ),
        asset_infos,
    };

    let wasm = Wasm::new(&gov_admin_test_setup.persistence_test_app);
    let user = &gov_admin_test_setup.accs[0];
    let res = wasm.execute(
        &gov_admin_test_setup.vault_instance.to_string(),
        &create_pool_msg, 
        &vec![], 
        &user,
    ).unwrap();

    let pool_id = find_event_attr(&res.events, "wasm-dexter-weighted-pool::instantiate", "pool_id");
    let pool_info: PoolInfoResponse = wasm.query(
        &gov_admin_test_setup.vault_instance.to_string(),
        &VaultQueryMsg::GetPoolById { pool_id: pool_id.parse::<Uint128>().unwrap() },
    ).unwrap();

    return pool_info;
}

fn allow_lp_token(gov_admin_test_setup: &GovAdminTestSetup, lp_token_addr: Addr) {
    let gov = Gov::new(&gov_admin_test_setup.persistence_test_app);
    let user = &gov_admin_test_setup.accs[0];

    let allow_lp_token_msg = dexter::multi_staking::ExecuteMsg::AllowLpToken {
        lp_token: lp_token_addr,
    };

    let gov_exec_msg = GovExecuteMsg::ExecuteMsgs {
        msgs: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: gov_admin_test_setup.multi_staking_instance.to_string(),
            msg: to_binary(&allow_lp_token_msg).unwrap(),
            funds: vec![],
        })]
    };
    let wasm_msg = MsgExecuteContract {
        sender: GOV_MODULE_ADDRESS.to_owned(),
        contract: gov_admin_test_setup.gov_admin_instance.to_string(),
        msg: to_binary(&gov_exec_msg).unwrap().0,
        funds: vec![],
    };

    let msg_submit_proposal = MsgSubmitProposal {
        messages: vec![wasm_msg.to_any()],
        initial_deposit: vec![persistence_std::types::cosmos::base::v1beta1::Coin {
            denom: "uxprt".to_string(),
            amount: Uint128::new(1000000000).to_string(),
        }],
        proposer: user.address().to_string(),
        metadata: "Allow LP token".to_string(),
        title: "Allow LP token".to_string(),
        summary: "EMPTY".to_string(),
    };
    
    let proposal_id = gov.submit_proposal(msg_submit_proposal, &user).unwrap().data.proposal_id;
    vote_on_proposal(gov_admin_test_setup, proposal_id, VoteOption::Yes);
}

fn vote_on_proposal(
    gov_admin_test_setup: &GovAdminTestSetup,
    proposal_id: u64,
    vote_option: VoteOption
) {
    let validator = gov_admin_test_setup.persistence_test_app.get_first_validator_signing_account().unwrap();

    // vote on the proposal
    let vote_msg = persistence_std::types::cosmos::gov::v1::MsgVote {
        proposal_id: proposal_id.clone(),
        voter: validator.address(),
        option: vote_option.into(),
        metadata: "".to_string()
    };

    let gov = Gov::new(&gov_admin_test_setup.persistence_test_app);
    gov.vote(vote_msg, &validator).unwrap();

    // make time fast forward for the proposal to pass
    let proposal = gov.query_proposal(&QueryProposalRequest{proposal_id}).unwrap().proposal.unwrap();

    // find the proposal voting end time and increase chain time to that
    let proposal_end_time = proposal.voting_end_time.unwrap();
    let proposal_start_time = proposal.voting_start_time.unwrap();

    let difference_seconds = proposal_end_time.seconds - proposal_start_time.seconds;
    gov_admin_test_setup.persistence_test_app.increase_time(difference_seconds as u64);

    // query proposal again
    let proposal = gov.query_proposal(&QueryProposalRequest{proposal_id}).unwrap().proposal.unwrap();

    // assert that the proposal has passed or rejected based on the vote
    match vote_option { 
        VoteOption::Yes => assert_eq!(proposal.status, ProposalStatus::Passed as i32),
        VoteOption::No => assert_eq!(proposal.status, ProposalStatus::Rejected as i32),
        _ => panic!("Invalid vote option")
    }
}

fn find_event_attr(events: &Vec<Event>, event: &str, attr: &str) -> String {
    return events.iter()
        .find(|e| e.ty == event).unwrap()
        .attributes.iter()
        .find(|a| a.key == attr).unwrap()
        .value.clone()
}