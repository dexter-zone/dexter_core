#[cfg(not(feature = "library"))]
use crate::error::ContractError;
use crate::execute::claim_refunds::execute_claim_refunds;

use crate::execute::create_pool_creation_proposal::execute_create_pool_creation_proposal;
use crate::execute::create_reward_schedule_proposal::execute_create_reward_schedule_creation_proposal;
use crate::execute::post_proposal_creation_callback::execute_post_governance_proposal_creation_callback;
use crate::execute::resume_create_pool::execute_resume_create_pool;
use crate::execute::resume_join_pool::execute_resume_join_pool;
use crate::execute::resume_reward_schedule_creation::execute_resume_reward_schedule_creation;
use crate::query::query_pool_creation_funds::query_funds_for_pool_creation_request;
use crate::query::query_refundable_funds::query_refundable_funds;
use crate::query::query_reward_schedule_creation_funds::query_funds_for_reward_schedule_creation;
use crate::state::{POOL_CREATION_REQUEST_DATA, REWARD_SCHEDULE_REQUESTS};
use crate::utils::validate_sender::{
    validatate_goverance_module_or_self_sender, validate_goverance_module_sender,
    validate_self_sender,
};

use const_format::concatcp;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, Event, MessageInfo, Response, StdError,
    StdResult,
};
use cw2::set_contract_version;

use dexter::governance_admin::{ExecuteMsg, InstantiateMsg, QueryMsg};
use dexter::helper::EventExt;

/// Contract name that is used for migration.
pub const CONTRACT_NAME: &str = "dexter-governance-admin";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub type ContractResult<T> = Result<T, ContractError>;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new().add_event(Event::from_info(
        concatcp!(CONTRACT_NAME, "::instantiate"),
        &info,
    )))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::ExecuteMsgs { msgs } => {
            // validatate that the governance module is sending the message
            validate_goverance_module_sender(&info)?;

            let mut res = Response::new();
            let mut event = Event::from_info(concatcp!(CONTRACT_NAME, "::execute_msgs"), &info);
            // log if this part of a transaction or not
            event = match env.transaction {
                None => event.add_attribute("tx", "none"),
                Some(tx) => event.add_attribute("tx", tx.index.to_string()),
            };
            res = res.add_messages(msgs).add_event(event);
            Ok(res)
        }

        ExecuteMsg::CreatePoolCreationProposal {
            proposal_description,
            pool_creation_request,
        } => execute_create_pool_creation_proposal(
            deps,
            env,
            info,
            proposal_description,
            pool_creation_request,
        ),
        ExecuteMsg::PostGovernanceProposalCreationCallback { gov_proposal_type } => {
            validate_self_sender(&info, env.clone())?;
            execute_post_governance_proposal_creation_callback(deps, env, info, gov_proposal_type)
        }
        ExecuteMsg::ResumeCreatePool {
            pool_creation_request_id,
        } => {
            validate_goverance_module_sender(&info)?;
            execute_resume_create_pool(deps, env, info, pool_creation_request_id)
        }
        ExecuteMsg::ResumeJoinPool {
            pool_creation_request_id,
        } => {
            validate_self_sender(&info, env.clone())?;
            execute_resume_join_pool(deps, env, info, pool_creation_request_id)
        }

        ExecuteMsg::CreateRewardSchedulesProposal {
            proposal_description,
            multistaking_contract_addr,
            reward_schedule_creation_requests,
        } => {
            let multi_staking_addr = deps.api.addr_validate(&multistaking_contract_addr)?;
            execute_create_reward_schedule_creation_proposal(
                deps,
                env,
                info,
                proposal_description,
                multi_staking_addr,
                reward_schedule_creation_requests,
            )
        }
        ExecuteMsg::ResumeCreateRewardSchedules {
            reward_schedules_creation_request_id,
        } => {
            validatate_goverance_module_or_self_sender(&info, env)?;
            execute_resume_reward_schedule_creation(deps, info, reward_schedules_creation_request_id)
        }

        ExecuteMsg::ClaimRefund { request_type } => {
            execute_claim_refunds(deps, env, info, request_type)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::PoolCreationRequest {
            pool_creation_request_id,
        } => to_binary(&POOL_CREATION_REQUEST_DATA.load(deps.storage, pool_creation_request_id)?),
        QueryMsg::RewardScheduleRequest {
            reward_schedule_request_id,
        } => to_binary(&REWARD_SCHEDULE_REQUESTS.load(deps.storage, reward_schedule_request_id)?),
        QueryMsg::FundsForPoolCreation { request } => {
            let user_total_deposit = query_funds_for_pool_creation_request(deps, &request)
                .map_err(|e| StdError::generic_err(e.to_string()))?;
            to_binary(&user_total_deposit)
        }
        QueryMsg::FundsForRewardScheduleCreation { requests } => {
            let user_total_deposit = query_funds_for_reward_schedule_creation(deps, &requests)
                .map_err(|e| StdError::generic_err(e.to_string()))?;

            to_binary(&user_total_deposit)
        }
        QueryMsg::RefundableFunds { request_type } => {
            let funds = query_refundable_funds(deps, &request_type)
                .map_err(|e| StdError::generic_err(e.to_string()))?;

            to_binary(&funds)
        }
    }
}

#[cw_serde]
pub struct MigrateMsg {}

// migrate handler
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    return Ok(Response::default());
}
