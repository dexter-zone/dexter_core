use cosmwasm_std::{to_binary, Addr, Coin, CosmosMsg, Uint128, WasmMsg};
use dexter::{
    asset::{Asset, AssetInfo},
    governance_admin::{
        ExecuteMsg as GovExecuteMsg, FundsCategory, GovAdminProposalRequestType,
        GovernanceProposalDescription, QueryMsg as GovQueryMsg, RefundReason, RefundResponse,
        RewardScheduleCreationRequest, RewardScheduleCreationRequestsState,
        RewardSchedulesCreationRequestStatus, UserDeposit, UserTotalDeposit,
    },
    multi_staking::{QueryMsg, RewardSchedule, RewardScheduleResponse},
    vault::{
        ExecuteMsg as VaultExecuteMsg, FeeInfo, NativeAssetPrecisionInfo, PoolInfo,
        PoolInfoResponse, QueryMsg as VaultQueryMsg,
    }, constants::GOV_MODULE_ADDRESS,
};

use persistence_std::types::{
    cosmos::gov::v1::{MsgSubmitProposal, VoteOption},
    cosmwasm::wasm::v1::{MsgExecuteContract, MsgExecuteContractResponse},
};
use persistence_test_tube::{
    Account, ExecuteResponse, Gov, Module, PersistenceTestApp, RunnerError, SigningAccount, Wasm,
};
use utils::GovAdminTestSetup;
use weighted_pool::state::WeightedParams;

mod utils;

#[test]
fn test_reward_schedules() {
    let gov_admin = utils::setup_test_contracts();
    let suite = RewardScheduleTestSuite::new(&gov_admin);

    suite.run_all();
}

struct RewardScheduleTestSuite<'a> {
    gov_admin: &'a GovAdminTestSetup,
    persistence: &'a PersistenceTestApp,
    user: &'a SigningAccount,
    validator: SigningAccount,
    pool_info: PoolInfo,
    proposal_description: GovernanceProposalDescription,
    valid_request: RewardScheduleCreationRequest,
    valid_funds: Vec<Coin>,
}

impl<'a> RewardScheduleTestSuite<'a> {
    fn new(gov_admin: &'a GovAdminTestSetup) -> Self {
        // Create a pool
        let asset_infos = vec![
            AssetInfo::native_token("uxprt".to_string()),
            AssetInfo::token(gov_admin.cw20_token_1.clone()),
        ];
        let pool_info = create_pool(gov_admin, asset_infos);

        let persistence = &gov_admin.persistence_test_app;
        let current_block_time = persistence.get_block_time_seconds() as u64;
        let validator = gov_admin
            .persistence_test_app
            .get_first_validator_signing_account()
            .unwrap();

        let valid_request = RewardScheduleCreationRequest {
            lp_token_addr: Some(pool_info.lp_token_addr.clone()),
            title: "Reward Schedule".to_string(),
            asset: AssetInfo::native_token("uxprt".to_string()),
            amount: Uint128::from(1000000u128),
            start_block_time: current_block_time + 7 * 24 * 60 * 60,
            end_block_time: current_block_time + 14 * 24 * 60 * 60,
        };

        RewardScheduleTestSuite {
            gov_admin,
            persistence: &gov_admin.persistence_test_app,
            user: &gov_admin.accs[0],
            validator,
            pool_info,
            proposal_description: GovernanceProposalDescription {
                title: "Create reward schedule".to_string(),
                metadata: "Create reward schedule".to_string(),
                summary: "Create reward schedule".to_string(),
            },
            valid_request,
            // min proposal deposit + reward amount
            valid_funds: vec![Coin::new(11000000, "uxprt")],
        }
    }

    fn run_all(&self) {
        println!("test: Query for reward schedule deposits");
        self.query_reward_schedule_creation_funds();

        println!("test: Empty reward schedule");
        self.test_empty_reward_schedules();

        println!("test: LP token not allowed");
        self.test_lp_token_not_allowed();

        println!("allowing LP token");
        self.allow_lp_token();

        println!("test: Start time is withing voting period");
        self.test_start_time_within_voting_period();

        println!("test: End time is before start time");
        self.test_end_time_before_start_time();

        println!("test: No LP token provided");
        self.test_no_lp_token_provided();

        println!("test: Insufficient funds");
        self.test_insufficient_fund();

        println!("test: Valid input & rejected proposal");
        self.test_valid_input_rejected_proposal();

        println!("test: Valid input & passed proposal");
        self.test_valid_input_passed_proposal();

        // TODO(ajeet): add a test case - more than required funds deposited
    }

    fn query_reward_schedule_creation_funds(&self) {
        let wasm = Wasm::new(self.persistence);
        let res = wasm.query(
            &self.gov_admin.gov_admin_instance.to_string(),
            &GovQueryMsg::FundsForRewardScheduleCreation {
                requests: vec![self.valid_request.clone()],
            },
        );

        assert!(res.is_ok());
        let funds: UserTotalDeposit = res.unwrap();
        assert_eq!(
            funds,
            UserTotalDeposit {
                deposit_breakdown: vec![
                    UserDeposit {
                        category: FundsCategory::ProposalDeposit,
                        assets: vec![Asset::new(
                            AssetInfo::native_token("uxprt".to_string()),
                            Uint128::from(10000000u128),
                        )],
                    },
                    UserDeposit {
                        category: FundsCategory::RewardScheduleAmount,
                        assets: vec![Asset::new(
                            AssetInfo::native_token("uxprt".to_string()),
                            Uint128::from(1000000u128),
                        )],
                    }
                ],
                total_deposit: vec![Asset::new(
                    AssetInfo::native_token("uxprt".to_string()),
                    Uint128::from(11000000u128),
                )],
            }
        );
    }

    fn test_empty_reward_schedules(&self) {
        let wasm = Wasm::new(self.persistence);
        let result = wasm.execute(
            &self.gov_admin.gov_admin_instance.to_string(),
            &GovExecuteMsg::CreateRewardSchedulesProposal {
                proposal_description: self.proposal_description.clone(),
                multistaking_contract_addr: self.gov_admin.multi_staking_instance.to_string(),
                reward_schedule_creation_requests: vec![],
            },
            &self.valid_funds,
            self.user,
        );

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: Must provide at least one reward schedule: execute wasm contract failed");
    }

    fn test_lp_token_not_allowed(&self) {
        let wasm = Wasm::new(self.persistence);
        let result = wasm.execute(
            &self.gov_admin.gov_admin_instance.to_string(),
            &GovExecuteMsg::CreateRewardSchedulesProposal {
                proposal_description: self.proposal_description.clone(),
                multistaking_contract_addr: self.gov_admin.multi_staking_instance.to_string(),
                reward_schedule_creation_requests: vec![self.valid_request.clone()],
            },
            &self.valid_funds,
            &self.user,
        );

        assert!(result.is_err());
        let error = result.unwrap_err();
        let expected_error = format!(
            "execute error: failed to execute message; message index: 0: LP Token: {} not allowed for reward schedule creation yet: execute wasm contract failed",
            self.pool_info.lp_token_addr,
        );
        assert_eq!(error.to_string(), expected_error);
    }

    fn test_start_time_within_voting_period(&self) {
        let wasm = Wasm::new(self.persistence);
        let current_block_time = self.persistence.get_block_time_seconds() as u64;

        let result = wasm.execute(
            &self.gov_admin.gov_admin_instance.to_string(),
            &GovExecuteMsg::CreateRewardSchedulesProposal {
                proposal_description: self.proposal_description.clone(),
                multistaking_contract_addr: self.gov_admin.multi_staking_instance.to_string(),
                reward_schedule_creation_requests: vec![RewardScheduleCreationRequest {
                    start_block_time: current_block_time + 60 * 60,
                    ..self.valid_request.clone()
                }],
            },
            &self.valid_funds,
            &self.user,
        );

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: Invalid reward schedule start block time: execute wasm contract failed");
    }

    fn test_end_time_before_start_time(&self) {
        let wasm = Wasm::new(self.persistence);
        let current_block_time = self.persistence.get_block_time_seconds() as u64;
        let result = wasm.execute(
            &self.gov_admin.gov_admin_instance.to_string(),
            &GovExecuteMsg::CreateRewardSchedulesProposal {
                proposal_description: self.proposal_description.clone(),
                multistaking_contract_addr: self.gov_admin.multi_staking_instance.to_string(),
                reward_schedule_creation_requests: vec![RewardScheduleCreationRequest {
                    end_block_time: current_block_time + 2 * 24 * 60 * 60,
                    ..self.valid_request.clone()
                }],
            },
            &self.valid_funds,
            &self.user,
        );

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: End block time must be after start block time: execute wasm contract failed");
    }

    fn test_no_lp_token_provided(&self) {
        let wasm = Wasm::new(self.persistence);
        let result = wasm.execute(
            &self.gov_admin.gov_admin_instance.to_string(),
            &GovExecuteMsg::CreateRewardSchedulesProposal {
                proposal_description: self.proposal_description.clone(),
                multistaking_contract_addr: self.gov_admin.multi_staking_instance.to_string(),
                reward_schedule_creation_requests: vec![RewardScheduleCreationRequest {
                    lp_token_addr: None,
                    ..self.valid_request.clone()
                }],
            },
            &self.valid_funds,
            &self.user,
        );

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: LP Token is expected in the reward schedule creation request but it is None: execute wasm contract failed");
    }

    fn test_insufficient_fund(&self) {
        let wasm = Wasm::new(self.persistence);
        let result = wasm.execute(
            &self.gov_admin.gov_admin_instance.to_string(),
            &GovExecuteMsg::CreateRewardSchedulesProposal {
                proposal_description: self.proposal_description.clone(),
                multistaking_contract_addr: self.gov_admin.multi_staking_instance.to_string(),
                reward_schedule_creation_requests: vec![self.valid_request.clone()],
            },
            &vec![Coin::new(10000000, "uxprt")],
            &self.user,
        );

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "execute error: failed to execute message; message index: 0: Insufficient funds sent for pool creation for uxprt - Amount Sent: 10000000 - Needed Amount: 11000000: execute wasm contract failed");
    }

    fn test_valid_input_rejected_proposal(&self) {
        let wasm = Wasm::new(self.persistence);
        let result = wasm.execute(
            &self.gov_admin.gov_admin_instance.to_string(),
            &GovExecuteMsg::CreateRewardSchedulesProposal {
                proposal_description: self.proposal_description.clone(),
                multistaking_contract_addr: self.gov_admin.multi_staking_instance.to_string(),
                reward_schedule_creation_requests: vec![self.valid_request.clone()],
            },
            &self.valid_funds,
            &self.user,
        );

        assert!(result.is_ok());
        let events = result.unwrap().events;
        let proposal_id: u64 = utils::find_event_attr(&events, "submit_proposal", "proposal_id")
            .parse()
            .unwrap();

        utils::vote_on_proposal(&self.gov_admin, proposal_id, VoteOption::No);

        let reward_schedule_request_id: u64 = utils::find_event_attr(
            &events,
            "wasm-dexter-governance-admin::create_reward_schedule_proposal",
            "reward_schedules_creation_request_id",
        )
        .parse()
        .unwrap();

        // check request status is ProposalCreated
        assert_eq!(
            self.query_request_status(reward_schedule_request_id),
            RewardSchedulesCreationRequestStatus::ProposalCreated { proposal_id }
        );

        // check rewards not created
        let reward_schedules = self.query_reward_schedules();
        assert_eq!(reward_schedules.len(), 0);

        let refundable_funds = self.query_refundable_funds(reward_schedule_request_id);

        // check refundable reason is ProposalRejectedFullRefund
        assert_eq!(
            refundable_funds.refund_reason,
            RefundReason::ProposalRejectedFullRefund
        );

        // check refundable funds includes total deposited funds
        assert_eq!(
            refundable_funds.refund_amount,
            vec![Asset::new(
                AssetInfo::native_token("uxprt".to_string()),
                self.valid_funds[0].amount,
            )]
        );

        // claim refunds
        let bal_before_refund = utils::query_balance(
            &self.gov_admin,
            self.user.address().to_string(),
            "uxprt".to_string(),
        );

        self.claim_refund(reward_schedule_request_id).unwrap();

        let bal_after_refund = utils::query_balance(
            &self.gov_admin,
            self.user.address().to_string(),
            "uxprt".to_string(),
        );

        // check balance includes the claim amount
        assert_eq!(
            bal_after_refund,
            bal_before_refund + self.valid_funds[0].amount
        );

        // check request status is RequestFailedAndRefunded
        assert_eq!(
            self.query_request_status(reward_schedule_request_id),
            RewardSchedulesCreationRequestStatus::RequestFailedAndRefunded {
                proposal_id,
                refund_block_height: self.persistence.get_block_height() as u64,
            }
        );

        // try to claim again
        let res = self.claim_refund(reward_schedule_request_id);
        assert!(res.is_err());
        let error = res.unwrap_err();
        let expected_err = format!(
            "execute error: failed to execute message; message index: 0: Funds already claimed for this request at block height: {}: execute wasm contract failed",
            self.persistence.get_block_height() - 1,
        );
        assert_eq!(error.to_string(), expected_err);
    }

    fn test_valid_input_passed_proposal(&self) {
        let wasm = Wasm::new(self.persistence);
        let result = wasm.execute(
            &self.gov_admin.gov_admin_instance.to_string(),
            &GovExecuteMsg::CreateRewardSchedulesProposal {
                proposal_description: self.proposal_description.clone(),
                multistaking_contract_addr: self.gov_admin.multi_staking_instance.to_string(),
                reward_schedule_creation_requests: vec![self.valid_request.clone()],
            },
            &self.valid_funds,
            &self.user,
        );

        assert!(result.is_ok());
        let events = result.unwrap().events;
        let proposal_id: u64 = utils::find_event_attr(&events, "submit_proposal", "proposal_id")
            .parse()
            .unwrap();

        utils::vote_on_proposal(&self.gov_admin, proposal_id, VoteOption::Yes);

        let reward_schedule_request_id: u64 = utils::find_event_attr(
            &events,
            "wasm-dexter-governance-admin::create_reward_schedule_proposal",
            "reward_schedules_creation_request_id",
        )
        .parse()
        .unwrap();

        // check request status is RewardSchedulesCreated
        assert_eq!(
            self.query_request_status(reward_schedule_request_id),
            RewardSchedulesCreationRequestStatus::RewardSchedulesCreated {
                proposal_id: Some(proposal_id)
            }
        );

        // check reward is created
        let reward_schedules = self.query_reward_schedules();
        assert_eq!(reward_schedules.len(), 1);
        assert_eq!(
            reward_schedules[0],
            RewardScheduleResponse {
                id: 1u64,
                reward_schedule: RewardSchedule {
                    title: self.valid_request.title.clone(),
                    amount: self.valid_request.amount,
                    start_block_time: self.valid_request.start_block_time,
                    end_block_time: self.valid_request.end_block_time,
                    creator: Addr::unchecked(self.user.address().to_string()),
                    asset: self.valid_request.asset.clone(),
                    staking_lp_token: self.pool_info.lp_token_addr.clone(),
                }
            }
        );

        let refundable_funds = self.query_refundable_funds(reward_schedule_request_id);

        // check refundable reason is ProposalPassedDepositRefund
        assert_eq!(
            refundable_funds.refund_reason,
            RefundReason::ProposalPassedDepositRefund
        );

        // check refundable funds includes proposal deposit
        assert_eq!(
            refundable_funds.refund_amount,
            vec![Asset::new(
                AssetInfo::native_token("uxprt".to_string()),
                Uint128::from(10000000u128),
            )]
        );

        // claim refunds
        let bal_before_refund = utils::query_balance(
            &self.gov_admin,
            self.user.address().to_string(),
            "uxprt".to_string(),
        );

        self.claim_refund(reward_schedule_request_id).unwrap();

        let bal_after_refund = utils::query_balance(
            &self.gov_admin,
            self.user.address().to_string(),
            "uxprt".to_string(),
        );

        // check balance includes the claim amount
        assert_eq!(
            bal_after_refund,
            bal_before_refund + Uint128::from(10000000u128)
        );

        // check request status is RequestSuccessfulAndDepositRefunded
        assert_eq!(
            self.query_request_status(reward_schedule_request_id),
            RewardSchedulesCreationRequestStatus::RequestSuccessfulAndDepositRefunded {
                proposal_id,
                refund_block_height: self.persistence.get_block_height() as u64,
            }
        );
    }

    fn allow_lp_token(&self) {
        let gov = Gov::new(self.persistence);

        let allow_lp_token_msg = dexter::multi_staking::ExecuteMsg::AllowLpToken {
            lp_token: self.pool_info.lp_token_addr.clone(),
        };

        let gov_exec_msg = GovExecuteMsg::ExecuteMsgs {
            msgs: vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: self.gov_admin.multi_staking_instance.to_string(),
                msg: to_binary(&allow_lp_token_msg).unwrap(),
                funds: vec![],
            })],
        };
        let wasm_msg = MsgExecuteContract {
            sender: GOV_MODULE_ADDRESS.to_owned(),
            contract: self.gov_admin.gov_admin_instance.to_string(),
            msg: to_binary(&gov_exec_msg).unwrap().0,
            funds: vec![],
        };

        let msg_submit_proposal = MsgSubmitProposal {
            messages: vec![wasm_msg.to_any()],
            initial_deposit: vec![persistence_std::types::cosmos::base::v1beta1::Coin {
                denom: "uxprt".to_string(),
                amount: Uint128::new(1000000000).to_string(),
            }],
            proposer: self.user.address().to_string(),
            metadata: "Allow LP token".to_string(),
            title: "Allow LP token".to_string(),
            summary: "EMPTY".to_string(),
        };

        let proposal_id = gov
            .submit_proposal(msg_submit_proposal, &self.user)
            .unwrap()
            .data
            .proposal_id;
        utils::vote_on_proposal(self.gov_admin, proposal_id, VoteOption::Yes);
    }

    fn query_request_status(
        &self,
        reward_schedule_request_id: u64,
    ) -> RewardSchedulesCreationRequestStatus {
        let wasm = Wasm::new(self.persistence);
        let request: RewardScheduleCreationRequestsState = wasm
            .query(
                &self.gov_admin.gov_admin_instance.to_string(),
                &GovQueryMsg::RewardScheduleRequest {
                    reward_schedule_request_id,
                },
            )
            .unwrap();
        return request.status;
    }

    fn query_refundable_funds(&self, reward_schedule_request_id: u64) -> RefundResponse {
        let wasm = Wasm::new(self.persistence);
        return wasm
            .query(
                &self.gov_admin.gov_admin_instance.to_string(),
                &GovQueryMsg::RefundableFunds {
                    request_type: GovAdminProposalRequestType::RewardSchedulesCreationRequest {
                        request_id: reward_schedule_request_id,
                    },
                },
            )
            .unwrap();
    }

    fn query_reward_schedules(&self) -> Vec<RewardScheduleResponse> {
        let wasm = Wasm::new(self.persistence);
        return wasm
            .query(
                &self.gov_admin.multi_staking_instance.to_string(),
                &QueryMsg::RewardSchedules {
                    lp_token: self.pool_info.lp_token_addr.clone(),
                    asset: AssetInfo::native_token("uxprt".to_string()),
                },
            )
            .unwrap();
    }

    fn claim_refund(
        &self,
        reward_schedule_request_id: u64,
    ) -> Result<ExecuteResponse<MsgExecuteContractResponse>, RunnerError> {
        let wasm = Wasm::new(self.persistence);
        return wasm.execute(
            &self.gov_admin.gov_admin_instance.to_string(),
            &GovExecuteMsg::ClaimRefund {
                request_type: GovAdminProposalRequestType::RewardSchedulesCreationRequest {
                    request_id: reward_schedule_request_id,
                },
            },
            &vec![],
            // running this with validator to avoid calculating gas fee
            &self.validator,
        );
    }
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
    let res = wasm
        .execute(
            &gov_admin_test_setup.vault_instance.to_string(),
            &create_pool_msg,
            &vec![],
            &user,
        )
        .unwrap();

    let pool_id = utils::find_event_attr(
        &res.events,
        "wasm-dexter-weighted-pool::instantiate",
        "pool_id",
    );
    let pool_info: PoolInfoResponse = wasm
        .query(
            &gov_admin_test_setup.vault_instance.to_string(),
            &VaultQueryMsg::GetPoolById {
                pool_id: pool_id.parse::<Uint128>().unwrap(),
            },
        )
        .unwrap();

    return pool_info;
}
