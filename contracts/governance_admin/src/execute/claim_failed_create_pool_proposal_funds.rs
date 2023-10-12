use cosmwasm_std::{DepsMut, Env, MessageInfo, Response, StdError};
use dexter::helper::build_transfer_token_to_user_msg;
use persistence_std::types::cosmos::gov::v1::ProposalStatus;

use crate::{
    contract::ContractResult,
    error::ContractError,
    state::{PoolCreationRequestStatus, POOL_CREATION_REQUEST_DATA},
    utils::query_gov_proposal_by_id,
};

pub fn execute_claim_failed_create_pool_proposal_funds(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    pool_creation_request_id: u64,
) -> ContractResult<Response> {
    // find the proposal id for the pool creation request id and check the status
    let mut pool_creation_request_context =
        POOL_CREATION_REQUEST_DATA.load(deps.storage, pool_creation_request_id)?;

    let proposal_id =
        pool_creation_request_context
            .status
            .proposal_id()
            .ok_or(ContractError::Std(StdError::generic_err(format!(
                "Proposal id not found for pool creation request id {}",
                pool_creation_request_id
            ))))?;

    // validate that the funds are not claimed back already
    let status = pool_creation_request_context.status;
    if let PoolCreationRequestStatus::RequestFailedAndRefunded {
        proposal_id: _,
        refund_block_height,
    } = status
    {
        return Err(ContractError::Std(StdError::generic_err(format!(
            "Funds are already claimed back for this pool creation request in block {refund_block_height}",
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

    // TODO: If passed then, we ned to refund the funds back to the user the propsal deposit amount


    // now, let's return the funds back to the user
    let mut messages = vec![];

    // return the user funds back
    for asset in &pool_creation_request_context.total_funds_acquired_from_user {
        let msg = build_transfer_token_to_user_msg(
            asset.info.clone(),
            pool_creation_request_context.request_sender.clone(),
            asset.amount,
        )?;

        messages.push(msg);
    }

    // update the context
    pool_creation_request_context.status = PoolCreationRequestStatus::RequestFailedAndRefunded {
        proposal_id: proposal_id.clone(),
        refund_block_height: env.block.height,
    };

    POOL_CREATION_REQUEST_DATA.save(
        deps.storage,
        pool_creation_request_id,
        &pool_creation_request_context,
    )?;

    Ok(Response::default())
}
