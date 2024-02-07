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
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, Event, MessageInfo, Response, StdError,
    StdResult,
};
use cw2::{get_contract_version, set_contract_version};

use dexter::governance_admin::{ExecuteMsg, InstantiateMsg, QueryMsg};
use dexter::helper::EventExt;

/// Contract name that is used for migration.
pub const CONTRACT_NAME: &str = "dexter-governance-admin";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CONTRACT_VERSION_V1: &str = "1.0.0";

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

            let event = Event::from_info(concatcp!(CONTRACT_NAME, "::execute_msgs"), &info);
            let res = Response::new()
                .add_event(event)
                .add_messages(msgs);
            
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
        } => to_json_binary(&POOL_CREATION_REQUEST_DATA.load(deps.storage, pool_creation_request_id)?),
        QueryMsg::RewardScheduleRequest {
            reward_schedule_request_id,
        } => to_json_binary(&REWARD_SCHEDULE_REQUESTS.load(deps.storage, reward_schedule_request_id)?),
        QueryMsg::FundsForPoolCreation { request } => {
            let user_total_deposit = query_funds_for_pool_creation_request(deps, &request)
                .map_err(|e| StdError::generic_err(e.to_string()))?;
            to_json_binary(&user_total_deposit)
        }
        QueryMsg::FundsForRewardScheduleCreation { requests } => {
            let user_total_deposit = query_funds_for_reward_schedule_creation(deps, &requests)
                .map_err(|e| StdError::generic_err(e.to_string()))?;

            to_json_binary(&user_total_deposit)
        }
        QueryMsg::RefundableFunds { request_type } => {
            let funds = query_refundable_funds(deps, &request_type)
                .map_err(|e| StdError::generic_err(e.to_string()))?;

            to_json_binary(&funds)
        }
    }
}

#[cw_serde]
pub enum MigrateMsg {
    V1_1 {}
}

// migrate handler
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {  
    match msg {
        MigrateMsg::V1_1 {} => {
            let contract_version = get_contract_version(deps.storage)?;

            // validate contract name
            if contract_version.contract != CONTRACT_NAME {
                return Err(ContractError::InvalidContractUpgrade {
                    expected_name: CONTRACT_NAME.to_string(),
                    actual_name: contract_version.contract,
                });
            }

            if contract_version.version != CONTRACT_VERSION_V1 {
                return Err(ContractError::InvalidContractVersionForUpgrade {
                    upgrade_version: CONTRACT_VERSION.to_string(),
                    expected: CONTRACT_VERSION_V1.to_string(),
                    actual: contract_version.version,
                });
            }

            set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

            let event = Event::new(concatcp!(CONTRACT_NAME, "::migrate"))
                .add_attribute("from", CONTRACT_VERSION_V1)
                .add_attribute("to", CONTRACT_VERSION);

            Ok(Response::new().add_event(event))
        }
    }
}
