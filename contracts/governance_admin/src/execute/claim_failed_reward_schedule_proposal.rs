use cosmwasm_std::{DepsMut, Env, MessageInfo, Response, StdError};
use dexter::{helper::build_transfer_token_to_user_msg, governance_admin::RewardSchedulesCreationRequestStatus};
use persistence_std::types::cosmos::gov::v1::ProposalStatus;

use crate::{
    contract::ContractResult,
    error::ContractError,
    state::REWARD_SCHEDULE_REQUESTS,
    utils::query_gov_proposal_by_id,
};

pub fn execute_claim_failed_reward_schedule_proposal_funds(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    reward_schedules_creation_request_id: u64,
) -> ContractResult<Response> {
    // find the proposal id for the pool creation request id and check the status
    let mut reward_schedule_creation_requests_state =
        REWARD_SCHEDULE_REQUESTS.load(deps.storage, reward_schedules_creation_request_id)?;

    let proposal_id =
        reward_schedule_creation_requests_state
            .status
            .proposal_id()
            .ok_or(ContractError::Std(StdError::generic_err(format!(
                "Refund claim can only happen for a reward schedule request linked to a proposal id"
            ))))?;

    // validate that the funds are not claimed back already
    let status = reward_schedule_creation_requests_state.status;
    if let RewardSchedulesCreationRequestStatus::RequestFailedAndRefunded {
        proposal_id: _,
        refund_block_height,
    } = status
    {
        return Err(ContractError::Std(StdError::generic_err(format!(
            "Funds are already claimed back for this reward schedules request in block {refund_block_height}",
        ))));
    }

    // query the proposal from chain
    let proposal = query_gov_proposal_by_id(&deps.querier, proposal_id)?;

    // validate that proposal status must be either REJECTED, FAILED
    let proposal_status = proposal.status;
    if proposal_status != ProposalStatus::Rejected as i32
        || proposal_status != ProposalStatus::Failed as i32
    {
        return Err(ContractError::Std(StdError::generic_err(format!(
            "Proposal status must be either REJECTED or FAILED to claim funds back"
        ))));
    }

    // now, let's return the funds back to the user
    let mut messages = vec![];

    // return the user funds back
    for asset in &reward_schedule_creation_requests_state.total_funds_acquired_from_user {
        let msg = build_transfer_token_to_user_msg(
            asset.info.clone(),
            reward_schedule_creation_requests_state.request_sender.clone(),
            asset.amount,
        )?;

        messages.push(msg);
    }

    // update the context
    reward_schedule_creation_requests_state.status = RewardSchedulesCreationRequestStatus::RequestFailedAndRefunded {
        proposal_id: proposal_id.clone(),
        refund_block_height: env.block.height,
    };

    REWARD_SCHEDULE_REQUESTS.save(
        deps.storage,
        reward_schedules_creation_request_id,
        &reward_schedule_creation_requests_state,
    )?;

    Ok(Response::default())
}
