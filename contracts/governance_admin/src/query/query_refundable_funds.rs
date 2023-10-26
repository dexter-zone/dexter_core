use std::collections::HashMap;

use cosmwasm_std::{Deps, StdError};
use dexter::{
    asset::Asset,
    governance_admin::{
        FundsCategory, GovAdminProposalRequestType, PoolCreationRequestStatus, RefundReason,
        RefundResponse, RewardSchedulesCreationRequestStatus,
    },
};
use persistence_std::types::cosmos::gov::v1::ProposalStatus;

use crate::{
    contract::ContractResult,
    error::ContractError,
    state::{POOL_CREATION_REQUEST_DATA, REWARD_SCHEDULE_REQUESTS},
    utils::query_gov_proposal_by_id,
};

/// Query refundable funds for a given request type
/// Refundable funds are the funds that are not claimed back yet from the governance admin contract since it owns the all the deposits
/// Claim refunds can be done only in the following states:
/// 1. Proposal is rejected and no refund is done
/// 2. Proposal is passed but the creation of the pool failed i.e. Proposal failed, and no refund is done
/// 3. Proposal is passed and creation of the pool was successful but the Governance Proposal deposit is not yet refunded to the user
pub fn query_refundable_funds(
    deps: Deps,
    request_desciption: &GovAdminProposalRequestType,
) -> ContractResult<RefundResponse> {
    let (proposal_id, refund_receiver, user_total_deposits) = match request_desciption {
        GovAdminProposalRequestType::PoolCreationRequest { request_id } => {
            // query pool creation request
            let pool_creation_request_context =
                POOL_CREATION_REQUEST_DATA.load(deps.storage, *request_id)?;

            let proposal_id =
                pool_creation_request_context
                    .status
                    .proposal_id()
                    .ok_or(ContractError::Std(StdError::generic_err(format!(
                        "Proposal id not found for pool creation request id {}",
                        request_id
                    ))))?;

            // validate that the funds are not claimed back already
            let status = pool_creation_request_context.status;
            match status {
                PoolCreationRequestStatus::RequestFailedAndRefunded {
                    proposal_id: _,
                    refund_block_height,
                } => {
                    return Err(ContractError::Std(StdError::generic_err(format!(
                        "Funds are already claimed back for this pool creation request in block {refund_block_height}",
                    ))));
                }
                PoolCreationRequestStatus::RequestSuccessfulAndDepositRefunded {
                    proposal_id: _,
                    refund_block_height,
                } => {
                    return Err(ContractError::Std(StdError::generic_err(format!(
                        "Funds are already claimed back for this pool creation request in block {refund_block_height}",
                    ))));
                }
                _ => (),
            }

            (
                proposal_id,
                pool_creation_request_context.request_sender,
                pool_creation_request_context.user_deposits_detailed,
            )
        }
        GovAdminProposalRequestType::RewardSchedulesCreationRequest { request_id } => {
            // query reward schedule creation request

            let reward_schedule_request_state =
                REWARD_SCHEDULE_REQUESTS.load(deps.storage, *request_id)?;

            let proposal_id =
                reward_schedule_request_state
                    .status
                    .proposal_id()
                    .ok_or(ContractError::Std(StdError::generic_err(format!(
                        "Proposal id not found for reward schedule creation request id {}",
                        request_id
                    ))))?;

            // validate that the funds are not claimed back already
            let status = reward_schedule_request_state.status;

            if let RewardSchedulesCreationRequestStatus::RequestFailedAndRefunded {
                proposal_id: _,
                refund_block_height,
            } = status
            {
                return Err(ContractError::Std(StdError::generic_err(format!(
                    "Funds are already claimed back for this reward schedule creation request in block {refund_block_height}",
                ))));
            }

            (
                proposal_id,
                reward_schedule_request_state.request_sender,
                reward_schedule_request_state.user_deposits_detailed,
            )
        }
    };

    // query the proposal from chain
    let proposal = query_gov_proposal_by_id(&deps.querier, proposal_id)?;

    // validate that proposal status must be either REJECTED, FAILED
    let proposal_status = ProposalStatus::try_from(proposal.status).map_err(|_| {
        ContractError::CannotDecodeProposalStatus {
            status: proposal.status,
        }
    })?;

    let (final_refundable_deposits, refund_reason) = match proposal_status {
        ProposalStatus::Rejected => Ok((
            user_total_deposits,
            RefundReason::ProposalRejectedFullRefund,
        )),
        ProposalStatus::Failed => Ok((user_total_deposits, RefundReason::ProposalFailedFullRefund)),
        ProposalStatus::Passed => {
            // return only the proposal deposit amount back to the user
            let mut user_deposits = vec![];
            for user_deposit in user_total_deposits {
                if let FundsCategory::ProposalDeposit = user_deposit.category {
                    user_deposits.push(user_deposit);
                }
            }

            Ok((user_deposits, RefundReason::ProposalPassedDepositRefund))
        }
        _ => Err(ContractError::Std(StdError::generic_err(format!(
            "Proposal status must be either REJECTED or FAILED or PASSED to be refundable"
        )))),
    }?;

    let mut map_asset_refunds = HashMap::new();
    for user_deposit in &final_refundable_deposits {
        let assets = &user_deposit.assets;
        for asset in assets {
            let asset_info = asset.info.clone();
            let amount = asset.amount;
            let total_refund = map_asset_refunds.entry(asset_info).or_insert(0u128);
            *total_refund += amount.u128();
        }
    }

    let mut refund_amount = vec![];
    for (asset_info, amount) in map_asset_refunds {
        refund_amount.push(Asset {
            info: asset_info,
            amount: amount.into(),
        });
    }

    Ok(RefundResponse {
        refund_reason,
        refund_receiver,
        refund_amount,
        detailed_refund_amount: final_refundable_deposits,
    })
}
