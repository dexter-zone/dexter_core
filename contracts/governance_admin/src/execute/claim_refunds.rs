use cosmwasm_std::{DepsMut, Env, MessageInfo, Response};
use dexter::{
    governance_admin::{
        GovAdminProposalRequestType, PoolCreationRequestStatus, RewardSchedulesCreationRequestStatus,
    },
    helper::build_transfer_token_to_user_msg,
};

use crate::{
    contract::ContractResult,
    query::query_refundable_funds::query_refundable_funds,
    state::{POOL_CREATION_REQUEST_DATA, REWARD_SCHEDULE_REQUESTS},
};

pub fn execute_claim_refunds(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    request_type: GovAdminProposalRequestType,
) -> ContractResult<Response> {
    let refund_response = query_refundable_funds(deps.as_ref(), &request_type)?;

    // now, let's return the funds back to the user
    let mut messages = vec![];

    // return the user funds back
    for asset in &refund_response.refund_amount {
        let msg = build_transfer_token_to_user_msg(
            asset.info.clone(),
            refund_response.refund_receiver.clone(),
            asset.amount,
        )?;

        messages.push(msg);
    }

    match &request_type {
        GovAdminProposalRequestType::PoolCreationRequest { request_id } => {
            match refund_response.refund_reason {
                dexter::governance_admin::RefundReason::ProposalPassedDepositRefund => {
                    let mut pool_creation_request_context =
                        POOL_CREATION_REQUEST_DATA.load(deps.storage, *request_id)?;

                    let _status = pool_creation_request_context.status;

                    pool_creation_request_context.status =
                        PoolCreationRequestStatus::RequestSuccessfulAndDepositRefunded {
                            proposal_id: pool_creation_request_context
                                .status
                                .proposal_id()
                                .unwrap(),
                            refund_block_height: env.block.height,
                        };

                    POOL_CREATION_REQUEST_DATA.save(
                        deps.storage,
                        *request_id,
                        &pool_creation_request_context,
                    )?;
                }
                dexter::governance_admin::RefundReason::ProposalRejectedFullRefund
                | dexter::governance_admin::RefundReason::ProposalFailedFullRefund => {
                    let mut pool_creation_request_context =
                        POOL_CREATION_REQUEST_DATA.load(deps.storage, *request_id)?;

                    let _status = pool_creation_request_context.status;

                    pool_creation_request_context.status =
                        PoolCreationRequestStatus::RequestFailedAndRefunded {
                            proposal_id: pool_creation_request_context
                                .status
                                .proposal_id()
                                .unwrap(),
                            refund_block_height: env.block.height,
                        };

                    POOL_CREATION_REQUEST_DATA.save(
                        deps.storage,
                        *request_id,
                        &pool_creation_request_context,
                    )?;
                }
            }
        }
        GovAdminProposalRequestType::RewardSchedulesCreationRequest { request_id } => {
            match refund_response.refund_reason {
                dexter::governance_admin::RefundReason::ProposalPassedDepositRefund => {
                    let mut reward_schedule_request_state =
                        REWARD_SCHEDULE_REQUESTS.load(deps.storage, *request_id)?;

                    let _status = reward_schedule_request_state.status;

                    reward_schedule_request_state.status =
                        RewardSchedulesCreationRequestStatus::RequestSuccessfulAndDepositRefunded {
                            proposal_id: reward_schedule_request_state
                                .status
                                .proposal_id()
                                .unwrap(),
                            refund_block_height: env.block.height,
                        };

                    REWARD_SCHEDULE_REQUESTS.save(
                        deps.storage,
                        *request_id,
                        &reward_schedule_request_state,
                    )?;
                }
                dexter::governance_admin::RefundReason::ProposalRejectedFullRefund
                | dexter::governance_admin::RefundReason::ProposalFailedFullRefund => {
                    let mut reward_schedule_request_state =
                        REWARD_SCHEDULE_REQUESTS.load(deps.storage, *request_id)?;

                    let _status = reward_schedule_request_state.status;

                    reward_schedule_request_state.status =
                        RewardSchedulesCreationRequestStatus::RequestFailedAndRefunded {
                            proposal_id: reward_schedule_request_state
                                .status
                                .proposal_id()
                                .unwrap(),
                            refund_block_height: env.block.height,
                        };

                    REWARD_SCHEDULE_REQUESTS.save(
                        deps.storage,
                        *request_id,
                        &reward_schedule_request_state,
                    )?;
                }
            }
        }
    }

    Ok(Response::new().add_messages(messages))
}
