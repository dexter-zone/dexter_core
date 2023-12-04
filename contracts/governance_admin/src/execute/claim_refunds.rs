use const_format::concatcp;
use cosmwasm_std::{DepsMut, Env, Event, MessageInfo, Response};
use dexter::{
    governance_admin::{
        GovAdminProposalRequestType, PoolCreationRequestStatus,
        RewardSchedulesCreationRequestStatus,
    },
    helper::{build_transfer_token_to_user_msg, EventExt},
};

use crate::{
    contract::{ContractResult, CONTRACT_NAME},
    query::query_refundable_funds::query_refundable_funds,
    state::{POOL_CREATION_REQUEST_DATA, REWARD_SCHEDULE_REQUESTS}, error::ContractError,
};

/// Claim refunds for the given request type
/// Any address can submit this request but the final claim will go back to the user who submitted the request initially
/// This is to allow bots to claim the refunds on behalf of the user
/// Claim refunds can be done only if the proposal is in the following states:
/// 1. Proposal is rejected and no refund is done
/// 2. Proposal is passed but the creation of the pool failed i.e. Proposal failed, and no refund is done
/// 3. Proposal is passed and creation of the pool was successful but the Governance Proposal deposit is not yet refunded to the user
pub fn execute_claim_refunds(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    request_type: GovAdminProposalRequestType,
) -> ContractResult<Response> {
    // validate that no funds are sent
    if info.funds.len() > 0 {
        return Err(ContractError::NoFundsExpected {});
    }

    let refund_response = query_refundable_funds(deps.as_ref(), &request_type)?;
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

    let refund_block_height = env.block.height;
    let mut event = Event::from_info(concatcp!(CONTRACT_NAME, "::claim_refund"), &info);

    match &request_type {
        GovAdminProposalRequestType::PoolCreationRequest { request_id } => {
            let mut pool_creation_request_context = POOL_CREATION_REQUEST_DATA.load(deps.storage, *request_id)?;
            let proposal_id = pool_creation_request_context.status.proposal_id().ok_or(
                ContractError::ProposalIdNotSet {
                    request_type: GovAdminProposalRequestType::PoolCreationRequest {
                        request_id: *request_id,
                    },
                },
            )?;

            let status = match refund_response.refund_reason {
                dexter::governance_admin::RefundReason::ProposalPassedDepositRefund => {
                    PoolCreationRequestStatus::RequestSuccessfulAndDepositRefunded {
                            proposal_id,
                            refund_block_height,
                    }
                }
                dexter::governance_admin::RefundReason::ProposalRejectedFullRefund
                | dexter::governance_admin::RefundReason::ProposalVetoedRefundExceptDeposit
                | dexter::governance_admin::RefundReason::ProposalFailedFullRefund => {
                    PoolCreationRequestStatus::RequestFailedAndRefunded {
                            proposal_id,
                            refund_block_height
                    }
                }
            };

            pool_creation_request_context.status = status;
            POOL_CREATION_REQUEST_DATA.save(
                deps.storage,
                *request_id,
                &pool_creation_request_context,
            )?;

            event = event
                .add_attribute("proposal_id", proposal_id.to_string())
                .add_attribute("request_id", request_id.to_string());
        }
        GovAdminProposalRequestType::RewardSchedulesCreationRequest { request_id } => {
            let mut reward_schedule_request_state = REWARD_SCHEDULE_REQUESTS.load(deps.storage, *request_id)?;
            let proposal_id = reward_schedule_request_state.status.proposal_id().ok_or(
                ContractError::ProposalIdNotSet {
                    request_type: GovAdminProposalRequestType::RewardSchedulesCreationRequest {
                        request_id: *request_id,
                    },
                },
            )?;

            let status = match refund_response.refund_reason {
                dexter::governance_admin::RefundReason::ProposalPassedDepositRefund => {
                    RewardSchedulesCreationRequestStatus::RequestSuccessfulAndDepositRefunded {
                            proposal_id,
                            refund_block_height,
                    }
                }
                dexter::governance_admin::RefundReason::ProposalRejectedFullRefund
                | dexter::governance_admin::RefundReason::ProposalVetoedRefundExceptDeposit
                | dexter::governance_admin::RefundReason::ProposalFailedFullRefund => {
                    RewardSchedulesCreationRequestStatus::RequestFailedAndRefunded {
                            proposal_id,
                            refund_block_height,
                    }
                }
            };

            reward_schedule_request_state.status = status;
            REWARD_SCHEDULE_REQUESTS.save(
                deps.storage,
                *request_id,
                &reward_schedule_request_state,
            )?;

            event = event
                .add_attribute("proposal_id", proposal_id.to_string())
                .add_attribute("request_id", request_id.to_string());
        }
    }

    event = event
        .add_attribute("request_type", serde_json_wasm::to_string(&request_type).unwrap())
        .add_attribute("refund_receiver", refund_response.refund_receiver)
        .add_attribute("refund_amount", serde_json_wasm::to_string(&refund_response.refund_amount).unwrap())
        .add_attribute("detailed_refund_amount", serde_json_wasm::to_string(&refund_response.detailed_refund_amount).unwrap())
        .add_attribute("refund_reason", refund_response.refund_reason.to_string());

    Ok(Response::new().add_event(event).add_messages(messages))
}
