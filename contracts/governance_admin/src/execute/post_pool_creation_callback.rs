use crate::contract::{ContractResult, CONTRACT_NAME};
use crate::error::ContractError;

use crate::state::{POOL_CREATION_REQUEST_PROPOSAL_ID, REWARD_SCHEDULE_REQUEST_PROPOSAL_ID};
use crate::utils::query_latest_governance_proposal;

use const_format::concatcp;

use cosmwasm_std::{to_binary, Binary, DepsMut, Env, Event, MessageInfo, Response, StdError};

use dexter::governance_admin::GovAdminProposalType;
use dexter::helper::EventExt;

use persistence_std::types::cosmwasm::wasm::v1::MsgExecuteContract;

pub fn execute_post_governance_proposal_creation_callback(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_type: GovAdminProposalType
) -> ContractResult<Response> {
    // proposal has been successfully created at this point, we can query the governance module and find the proposal id
    // and store it in the state
    let latest_proposal = query_latest_governance_proposal(env.contract.address, &deps.querier)?;

    // validate the proposal content to make sure that pool creation request id matches.
    // this is more of a sanity check
    let proposal_content = latest_proposal.messages.first().unwrap();

    let execute_contract_proposal_content =
        MsgExecuteContract::try_from(Binary::from(proposal_content.value.as_slice()))?;

    let proposal_msg_bytes = match &proposal_type {
        GovAdminProposalType::PoolCreationRequest { request_id } => {
            let resume_create_pool_msg = dexter::governance_admin::ExecuteMsg::ResumeCreatePool {
                pool_creation_request_id: request_id.clone()
            };
            to_binary(&resume_create_pool_msg).unwrap()
        },
        GovAdminProposalType::RewardSchedulesCreationRequest { request_id } => {
            let resume_create_reward_schedule_msg = dexter::governance_admin::ExecuteMsg::ResumeCreateRewardSchedules {
                reward_schedules_creation_request_id: request_id.clone()
            };
            to_binary(&resume_create_reward_schedule_msg).unwrap()
        }
    };

    if execute_contract_proposal_content.msg != proposal_msg_bytes {
        return Err(ContractError::Std(StdError::generic_err(format!(
            "proposal content does not match. B1: {} B2: {}",
            String::from_utf8_lossy(&execute_contract_proposal_content.msg),
            String::from_utf8_lossy(&proposal_msg_bytes)
        ))));
    }

    let mut event = Event::from_info(
        concatcp!(
            CONTRACT_NAME,
            "::post_governance_proposal_creation_callback"
        ),
        &info,
    );

    match proposal_type {
        GovAdminProposalType::PoolCreationRequest { request_id } => {
             // store the proposal id in the state
            POOL_CREATION_REQUEST_PROPOSAL_ID.save(
                deps.storage,
                request_id,
                &latest_proposal.id,
            )?;

            event = event
                .add_attribute("pool_creation_request_id",request_id.to_string())
                .add_attribute("proposal_id",latest_proposal.id.to_string());
        },
        GovAdminProposalType::RewardSchedulesCreationRequest { request_id } => {
            // store the proposal id in the state
            REWARD_SCHEDULE_REQUEST_PROPOSAL_ID.save(
                deps.storage,
                request_id,
                &latest_proposal.id,
            )?;

            event = event
                .add_attribute("reward_schedules_creation_request_id",request_id.to_string())
                .add_attribute("proposal_id",latest_proposal.id.to_string());
        }
    }

    Ok(Response::default().add_event(event))
}