use cosmwasm_std::{to_binary, Addr, Coin, Uint128};
use dexter::{
    asset::{Asset, AssetInfo},
    governance_admin::{
        GovAdminProposalRequestType, GovernanceProposalDescription, PoolCreationRequest, QueryMsg,
        RefundResponse, UserDeposit, UserTotalDeposit,
    },
    multi_staking::{RewardSchedule, RewardScheduleResponse},
    vault::{FeeInfo, NativeAssetPrecisionInfo, PoolInfoResponse},
};
use persistence_std::types::cosmos::gov::v1::VoteOption;
use persistence_test_tube::{Account, Module, PersistenceTestApp, SigningAccount, Wasm};
use utils::GovAdminTestSetup;
use weighted_pool::state::WeightedParams;

mod utils;

fn create_pool_creation_proposal(
    wasm: &Wasm<PersistenceTestApp>,
    gov_admin_test_setup: &utils::GovAdminTestSetup,
    valid_create_pool_msg: dexter::governance_admin::ExecuteMsg,
    total_funds_to_send: Vec<Coin>,
    admin: &SigningAccount,
) -> u64 {
    let events = wasm
        .execute(
            &gov_admin_test_setup.gov_admin_instance.to_string(),
            &valid_create_pool_msg,
            &total_funds_to_send,
            admin,
        )
        .unwrap()
        .events;

    // find the proposal id from events
    let proposal_id = utils::find_event_attr(&events, "submit_proposal", "proposal_id");

    proposal_id.parse().unwrap()
}

struct CreatePoolTestSuite<'a> {
    test_setup: &'a GovAdminTestSetup,
    persistence: &'a PersistenceTestApp,
    user: &'a SigningAccount,
    validator: SigningAccount,
    proposal_description: GovernanceProposalDescription,
    // valid_reward_schedules: Vec<RewardScheduleCreationRequest>,
    asset_info: Vec<AssetInfo>,
    valid_request: PoolCreationRequest,
    valid_funds: Vec<Coin>,
}

impl<'a> CreatePoolTestSuite<'a> {
    fn new(test_setup: &'a GovAdminTestSetup) -> Self {
        let persistence_test_app = &test_setup.persistence_test_app;
        let admin = &test_setup.accs[0];

        let validator = test_setup
            .persistence_test_app
            .get_first_validator_signing_account()
            .unwrap();

        let current_block_time = persistence_test_app.get_block_time_seconds() as u64;
        let wasm = Wasm::new(persistence_test_app);

        let xprt_asset_info = AssetInfo::native_token("uxprt".to_string());
        let cw20_asset_info = AssetInfo::token(test_setup.cw20_token_1.clone());

        // mint CW20 tokens to the user
        let mint_msg = cw20_base::msg::ExecuteMsg::Mint {
            recipient: admin.address().to_string(),
            amount: Uint128::from(10000000000u128),
        };

        let _ = wasm
            .execute(
                &test_setup.cw20_token_1.to_string(),
                &mint_msg,
                &vec![],
                &admin,
            )
            .unwrap();

        let asset_info = vec![xprt_asset_info.clone(), cw20_asset_info.clone()];

        let bootstrapping_amount = vec![
            Asset::new(asset_info[0].clone(), Uint128::from(1000000u128)),
            Asset::new(asset_info[1].clone(), Uint128::from(1000000u128)),
        ];

        let valid_reward_schedules =
            vec![dexter::governance_admin::RewardScheduleCreationRequest {
                lp_token_addr: None,
                title: "Reward Schedule 1".to_string(),
                asset: xprt_asset_info.clone(),
                amount: Uint128::from(1000000u128),
                start_block_time: current_block_time + 3 * 24 * 60 * 60,
                end_block_time: current_block_time + 10 * 24 * 60 * 60,
            }];

        let valid_pool_creation_request = PoolCreationRequest {
            vault_addr: test_setup.vault_instance.to_string(),
            pool_type: dexter::vault::PoolType::Weighted {},
            fee_info: Some(FeeInfo {
                total_fee_bps: 30,
                protocol_fee_percent: 30,
            }),
            native_asset_precisions: vec![NativeAssetPrecisionInfo {
                denom: xprt_asset_info.denom().unwrap(),
                precision: 6,
            }],
            asset_info: asset_info.clone(),
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
            reward_schedules: Some(valid_reward_schedules.clone()),
        };

        let proposal_description = GovernanceProposalDescription {
            title: "Create Pool".to_string(),
            metadata: "Create Pool".to_string(),
            summary: "Create Pool".to_string(),
        };

        let total_funds_to_send = vec![
            // proposal deposit + bootstrapping amount
            Coin::new(12000000, xprt_asset_info.denom().unwrap()),
        ];

        CreatePoolTestSuite {
            test_setup,
            persistence: persistence_test_app,
            user: &test_setup.accs[0],
            validator,
            asset_info: asset_info.clone(),
            proposal_description,
            valid_request: valid_pool_creation_request,
            valid_funds: total_funds_to_send,
        }
    }

    fn run_all(&self) {
        println!("Test pool creation funds query");
        self.test_pool_creation_funds_query();

        println!("Running failure case: Create Pool with incorrect bootstrapping amount");
        self.test_failure_create_pool_with_incorrect_bootstrapping_amount();

        println!("Running failure case: Create Pool without CW20 approval");
        self.test_failure_without_cw20_approval();

        println!("Running failure case: Create Pool with insufficient native funds");
        self.test_failure_with_insufficient_native_funds();

        println!("Running failure case: Create Pool with invalid reward schedule start time");
        self.test_invalid_reward_schedule_start_time();

        println!("Running failure case: Create Pool with invalid reward schedule end time");
        self.test_invalid_reward_schedule_end_time();

        println!("Running success case: Create Pool with reward schedules");
        self.test_success_create_pool_with_reward_schedules();

        println!("Running success case: Refund amount post successful pool creation");
        self.test_valid_refund_amount_post_successful_pool_creation();

        println!("Running success case: Refund amount post rejected proposal");
        self.test_refund_for_rejected_proposal();
    }

    fn test_pool_creation_funds_query(&self) {
        let wasm = Wasm::new(self.persistence);

        let query_funds_msg = QueryMsg::FundsForPoolCreation {
            request: self.valid_request.clone(),
        };

        let funds_for_pool_creation: UserTotalDeposit = wasm
            .query(
                &self.test_setup.gov_admin_instance.to_string(),
                &query_funds_msg,
            )
            .unwrap();

        // assert that the funds are equal to the deposit amount
        assert_eq!(
            funds_for_pool_creation.total_deposit,
            vec![
                Asset::new(
                    AssetInfo::token(self.test_setup.cw20_token_1.clone()),
                    Uint128::from(1000000u128)
                ),
                Asset::new(
                    AssetInfo::native_token("uxprt".to_string()),
                    Uint128::from(12000000u128)
                ),
            ]
        );
    }

    fn test_failure_create_pool_with_incorrect_bootstrapping_amount(&self) {
        let incorrect_bootstrapping_amount = vec![Asset::new(
            self.asset_info[0].clone(),
            Uint128::from(1000000u128),
        )];

        let failure_case_pool_creation_request = PoolCreationRequest {
            bootstrapping_amount: Some(incorrect_bootstrapping_amount),
            ..self.valid_request.clone()
        };

        let failure_case_create_pool_msg =
            dexter::governance_admin::ExecuteMsg::CreatePoolCreationProposal {
                proposal_description: self.proposal_description.clone(),
                pool_creation_request: failure_case_pool_creation_request.clone(),
            };

        // send funds
        let total_funds_to_send = vec![
            // proposal deposit + bootstrapping amount
            Coin::new(12000000, "uxprt"),
        ];

        let wasm = Wasm::new(self.persistence);

        let result = wasm.execute(
            &self.test_setup.gov_admin_instance.to_string(),
            &failure_case_create_pool_msg,
            &total_funds_to_send,
            &self.user,
        );

        // assert error
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: Bootstrapping amount must exactly include all the assets in the pool: execute wasm contract failed");
    }

    fn test_failure_without_cw20_approval(&self) {
        let wasm = Wasm::new(self.persistence);

        let valid_create_pool_msg =
            dexter::governance_admin::ExecuteMsg::CreatePoolCreationProposal {
                proposal_description: self.proposal_description.clone(),
                pool_creation_request: self.valid_request.clone(),
            };

        let result = wasm.execute(
            &self.test_setup.gov_admin_instance.to_string(),
            &valid_create_pool_msg,
            &self.valid_funds,
            &self.user,
        );

        // assert error
        assert!(result.is_err());
        let error = result.unwrap_err();

        assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: Insufficient spend limit for token persistence1mf6ptkssddfmxvhdx0ech0k03ktp6kf9yk59renau2gvht3nq2gqtu9smg - Current approval: 0 - Needed Approval: 1000000: execute wasm contract failed");
    }

    fn test_failure_with_insufficient_native_funds(&self) {
        let wasm = Wasm::new(self.persistence);

        // approve spending of CW20 tokens
        let approve_msg = cw20_base::msg::ExecuteMsg::IncreaseAllowance {
            spender: self.test_setup.gov_admin_instance.to_string(),
            amount: Uint128::from(1000000u128),
            expires: None,
        };

        let _ = wasm
            .execute(
                &self.test_setup.cw20_token_1.to_string(),
                &approve_msg,
                &vec![],
                &self.user,
            )
            .unwrap();

        let valid_create_pool_msg =
            dexter::governance_admin::ExecuteMsg::CreatePoolCreationProposal {
                proposal_description: self.proposal_description.clone(),
                pool_creation_request: self.valid_request.clone(),
            };

        // Failure Scenario 3: Try to create a proposal with insufficient native funds
        let result = wasm.execute(
            &self.test_setup.gov_admin_instance.to_string(),
            &valid_create_pool_msg,
            &vec![],
            &self.user,
        );

        // assert error
        assert!(result.is_err());
        let error = result.unwrap_err();

        assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: Insufficient funds sent for pool creation for uxprt - Amount Sent: 0 - Needed Amount: 12000000: execute wasm contract failed");
    }

    fn test_invalid_reward_schedule_start_time(&self) {
        let wasm = Wasm::new(self.persistence);
        let current_block_time = self.persistence.get_block_time_seconds() as u64;
        let invalid_reward_schedules =
            vec![dexter::governance_admin::RewardScheduleCreationRequest {
                lp_token_addr: None,
                title: "Reward Schedule 1".to_string(),
                asset: AssetInfo::native_token("uxprt".to_string()),
                amount: Uint128::from(1000000u128),
                start_block_time: current_block_time + 2 * 24 * 60 * 60,
                end_block_time: current_block_time + 10 * 24 * 60 * 60,
            }];

        let failure_case_pool_creation_request = PoolCreationRequest {
            reward_schedules: Some(invalid_reward_schedules),
            ..self.valid_request.clone()
        };

        let failure_case_create_pool_msg =
            dexter::governance_admin::ExecuteMsg::CreatePoolCreationProposal {
                proposal_description: self.proposal_description.clone(),
                pool_creation_request: failure_case_pool_creation_request.clone(),
            };

        let result = wasm.execute(
            &self.test_setup.gov_admin_instance.to_string(),
            &failure_case_create_pool_msg,
            &self.valid_funds,
            &self.user,
        );

        // assert error
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: Invalid reward schedule start block time: execute wasm contract failed");
    }

    fn test_invalid_reward_schedule_end_time(&self) {
        let wasm = Wasm::new(self.persistence);
        let current_block_time = self.persistence.get_block_time_seconds() as u64;

        let invalid_reward_schedules =
            vec![dexter::governance_admin::RewardScheduleCreationRequest {
                lp_token_addr: None,
                title: "Reward Schedule 1".to_string(),
                asset: AssetInfo::native_token("uxprt".to_string()),
                amount: Uint128::from(1000000u128),
                start_block_time: current_block_time + 3 * 24 * 60 * 60,
                end_block_time: current_block_time + 2 * 24 * 60 * 60,
            }];

        let failure_case_pool_creation_request = PoolCreationRequest {
            reward_schedules: Some(invalid_reward_schedules),
            ..self.valid_request.clone()
        };

        let failure_case_create_pool_msg =
            dexter::governance_admin::ExecuteMsg::CreatePoolCreationProposal {
                proposal_description: self.proposal_description.clone(),
                pool_creation_request: failure_case_pool_creation_request.clone(),
            };

        let result = wasm.execute(
            &self.test_setup.gov_admin_instance.to_string(),
            &failure_case_create_pool_msg,
            &self.valid_funds,
            &self.user,
        );

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: End block time must be after start block time: execute wasm contract failed");
    }

    fn test_success_create_pool_with_reward_schedules(&self) {
        let wasm = Wasm::new(self.persistence);
        let valid_create_pool_msg =
            dexter::governance_admin::ExecuteMsg::CreatePoolCreationProposal {
                proposal_description: self.proposal_description.clone(),
                pool_creation_request: self.valid_request.clone(),
            };
        let proposal_id = create_pool_creation_proposal(
            &wasm,
            &self.test_setup,
            valid_create_pool_msg.clone(),
            self.valid_funds.clone(),
            self.user,
        );

        utils::vote_on_proposal(&self.test_setup, proposal_id, VoteOption::Yes);

        // validate that the pool has been created successfully
        // query the vault contract to find the pool id

        let get_pool_by_id_query = dexter::vault::QueryMsg::GetPoolById {
            // first pool so it must be 1
            pool_id: Uint128::from(1u128),
        };

        let pool_info: PoolInfoResponse = wasm
            .query(
                &self.test_setup.vault_instance.to_string(),
                &get_pool_by_id_query,
            )
            .unwrap();

        // validate if reward schedule is created by querying the multistaking contract
        let get_reward_schedules_query = dexter::multi_staking::QueryMsg::RewardSchedules {
            lp_token: pool_info.lp_token_addr.clone(),
            asset: AssetInfo::native_token("uxprt".to_string()),
        };

        let reward_schedules: Vec<RewardScheduleResponse> = wasm
            .query(
                &self.test_setup.multi_staking_instance.to_string(),
                &get_reward_schedules_query,
            )
            .unwrap();

        let valid_reward_schedules = self.valid_request.reward_schedules.clone().unwrap();
        // assert that the reward schedule is created and the count is 1
        assert_eq!(reward_schedules.len(), 1);
        assert_eq!(
            reward_schedules[0],
            RewardScheduleResponse {
                id: 1u64,
                reward_schedule: RewardSchedule {
                    title: valid_reward_schedules[0].title.clone(),
                    amount: valid_reward_schedules[0].amount,
                    start_block_time: valid_reward_schedules[0].start_block_time,
                    end_block_time: valid_reward_schedules[0].end_block_time,
                    creator: Addr::unchecked(self.user.address().to_string()),
                    asset: valid_reward_schedules[0].asset.clone(),
                    staking_lp_token: pool_info.lp_token_addr.clone(),
                }
            }
        );
    }

    fn test_valid_refund_amount_post_successful_pool_creation(&self) {
        let wasm = Wasm::new(self.persistence);
        let query_refund_msg = QueryMsg::RefundableFunds {
            request_type: GovAdminProposalRequestType::PoolCreationRequest { request_id: 1 },
        };

        let refundable_funds: RefundResponse = wasm
            .query(
                &self.test_setup.gov_admin_instance.to_string(),
                &query_refund_msg,
            )
            .unwrap();

        // assert that the refundable funds are equal to the deposit amount
        assert_eq!(
            refundable_funds.refund_amount,
            vec![Asset::new(
                AssetInfo::native_token("uxprt".to_string()),
                Uint128::from(10000000u128)
            )]
        );

        // Claim the refund
        let claim_refund_msg = dexter::governance_admin::ExecuteMsg::ClaimRefund {
            request_type: GovAdminProposalRequestType::PoolCreationRequest { request_id: 1 },
        };

        // get balance of the user before claiming the refund
        let user_balance_before_refund = utils::query_balance(
            self.test_setup,
            self.user.address().to_string(),
            "uxprt".to_string(),
        );

        let _ = wasm
            .execute(
                &self.test_setup.gov_admin_instance.to_string(),
                &claim_refund_msg,
                &vec![],
                // anyone can claim the refund for the user but it should go back to the user
                &self.validator,
            )
            .unwrap();

        // get balance of the user after claiming the refund
        let user_balance_after_refund = utils::query_balance(
            &self.test_setup,
            self.user.address().to_string(),
            "uxprt".to_string(),
        );

        // assert that the user has received the refund
        assert_eq!(
            user_balance_after_refund,
            user_balance_before_refund + Uint128::from(10000000u128)
        );

        // claiming funds again should fail
        let res = wasm.execute(
            &self.test_setup.gov_admin_instance.to_string(),
            &claim_refund_msg,
            &vec![],
            // anyone can claim the refund for the user but it should go back to the user
            &self.validator,
        );

        // assert error
        assert!(res.is_err());

        let error = res.unwrap_err();
        assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: Generic error: Funds are already claimed back for this pool creation request in block 29: execute wasm contract failed");
    }

    fn test_refund_for_rejected_proposal(&self) {
        let wasm = Wasm::new(self.persistence);
        let current_block_time = self.persistence.get_block_time_seconds() as u64;

        let xprt_asset = self.asset_info[0].clone();
        let cw20_asset = self.asset_info[1].clone();

        // test failure case. Try to claim the refund for a non-existent pool creation request
        let modified_create_pool_msg =
            dexter::governance_admin::ExecuteMsg::CreatePoolCreationProposal {
                proposal_description: self.proposal_description.clone(),
                pool_creation_request: PoolCreationRequest {
                    reward_schedules: Some(vec![
                        dexter::governance_admin::RewardScheduleCreationRequest {
                            lp_token_addr: None,
                            title: "Reward Schedule 1".to_string(),
                            asset: AssetInfo::native_token("uxprt".to_string()),
                            amount: Uint128::from(1000000u128),
                            start_block_time: current_block_time + 3 * 24 * 60 * 60,
                            end_block_time: current_block_time + 10 * 24 * 60 * 60,
                        },
                    ]),
                    ..self.valid_request.clone()
                },
            };

        // approve spending of CW20 tokens
        let approve_msg: cw20::Cw20ExecuteMsg = cw20_base::msg::ExecuteMsg::IncreaseAllowance {
            spender: self.test_setup.gov_admin_instance.to_string(),
            amount: Uint128::from(1000000u128),
            expires: None,
        };

        let _ = wasm
            .execute(
                &self.test_setup.cw20_token_1.to_string(),
                &approve_msg,
                &vec![],
                &self.user,
            )
            .unwrap();

        // test failure case, create a new pool but reject the proposal
        let proposal_id = create_pool_creation_proposal(
            &wasm,
            self.test_setup,
            modified_create_pool_msg.clone(),
            self.valid_funds.clone(),
            self.user,
        );

        // create a pool using governance admin
        utils::vote_on_proposal(&self.test_setup, proposal_id, VoteOption::No);

        // query for the refund of the deposit amount
        let query_refund_msg = QueryMsg::RefundableFunds {
            request_type: GovAdminProposalRequestType::PoolCreationRequest { request_id: 2 },
        };

        let refundable_funds: RefundResponse = wasm
            .query(
                &self.test_setup.gov_admin_instance.to_string(),
                &query_refund_msg,
            )
            .unwrap();

        // verify that the refundable funds are equal to the deposit amount + pool creation fee + reward schedule amount + pool bootstrapping amount
        assert_eq!(
            refundable_funds.refund_amount,
            vec![
                Asset::new(cw20_asset.clone(), Uint128::from(1000000u128)),
                Asset::new(xprt_asset.clone(), Uint128::from(12000000u128))
            ]
        );

        // verify detailed refund amount
        assert_eq!(
            refundable_funds.detailed_refund_amount,
            vec![
                UserDeposit {
                    category: dexter::governance_admin::FundsCategory::ProposalDeposit,
                    assets: vec![Asset::new(xprt_asset.clone(), Uint128::from(10000000u128))]
                },
                UserDeposit {
                    category: dexter::governance_admin::FundsCategory::PoolBootstrappingAmount,
                    assets: vec![
                        Asset::new(xprt_asset.clone(), Uint128::from(1000000u128)),
                        Asset::new(cw20_asset.clone(), Uint128::from(1000000u128))
                    ]
                },
                UserDeposit {
                    category: dexter::governance_admin::FundsCategory::RewardScheduleAmount,
                    assets: vec![Asset::new(xprt_asset.clone(), Uint128::from(1000000u128))]
                }
            ]
        );
    }
}

#[test]
fn run_create_pool_test_suite() {
    let gov_admin = utils::setup_test_contracts();
    let suite = CreatePoolTestSuite::new(&gov_admin);

    suite.run_all();
}
