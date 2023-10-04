#[cfg(not(feature = "library"))]
use crate::error::ContractError;
use crate::execute::create_pool_creation_proposal::execute_create_pool_creation_proposal;
use crate::execute::create_reward_schedule_proposal::execute_create_reward_schedule_creation_proposal;
use crate::execute::post_pool_creation_callback::execute_post_governance_proposal_creation_callback;
use crate::execute::resume_create_pool::execute_resume_create_pool;
use crate::execute::resume_join_pool::execute_resume_join_pool;
use crate::state::{POOL_CREATION_REQUESTS, POOL_CREATION_REQUEST_PROPOSAL_ID, REWARD_SCHEDULE_REQUESTS, REWARD_SCHEDULE_REQUEST_PROPOSAL_ID};

use const_format::concatcp;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    entry_point, Binary, Deps, DepsMut, Env, Event, MessageInfo, Response, StdError, StdResult, to_binary, Uint128,
};
use cw2::set_contract_version;

use dexter::governance_admin::{ExecuteMsg, InstantiateMsg, QueryMsg};
use dexter::helper::EventExt;

/// Contract name that is used for migration.
pub const CONTRACT_NAME: &str = "dexter-governance-admin";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// this address is derived using: https://gist.github.com/xlab/490d0e7937a8ccdbf805acb00f5dd9a1
pub const GOV_MODULE_ADDRESS: &str = "persistence10d07y265gmmuvt4z0w9aw880jnsr700j5w4kch";

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

pub fn validatate_goverance_module_sender(info: &MessageInfo) -> StdResult<()> {
    if info.sender != GOV_MODULE_ADDRESS {
        return Err(StdError::generic_err("unauthorized"));
    } else {
        return Ok(());
    }
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
            validatate_goverance_module_sender(&info)?;

            // validate that all funds were sent along with the message. Ideally this contract should not hold any funds.
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
        ExecuteMsg::PostGovernanceProposalCreationCallback {
            gov_proposal_type,
        } => execute_post_governance_proposal_creation_callback(
            deps,
            env,
            info,
            gov_proposal_type,
        ),
        ExecuteMsg::ResumeCreatePool {
            pool_creation_request_id,
        } => execute_resume_create_pool(deps, env, info, pool_creation_request_id),

        ExecuteMsg::ResumeJoinPool {
            pool_creation_request_id,
        } => execute_resume_join_pool(deps, env, info, pool_creation_request_id),

        ExecuteMsg::CreateRewardSchedulesProposal { proposal_description, multistaking_contract_addr, reward_schedule_creation_requests } => {
            let multi_staking_addr = deps.api.addr_validate(&multistaking_contract_addr)?;
            execute_create_reward_schedule_creation_proposal(deps, env, info, proposal_description, multi_staking_addr, reward_schedule_creation_requests)
        },
        ExecuteMsg::ResumeCreateRewardSchedules { reward_schedules_creation_request_id } => {
            todo!()
        },
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::PoolCreationRequest { pool_creation_request_id } => {
            to_binary(&POOL_CREATION_REQUESTS.load(deps.storage, pool_creation_request_id)?)
        }
        QueryMsg::PoolCreationRequestProposalId { pool_creation_request_id } => {
            to_binary(&POOL_CREATION_REQUEST_PROPOSAL_ID.load(deps.storage, pool_creation_request_id)?)
        }
        QueryMsg::RewardScheduleRequest { reward_schedule_request_id } => {
            to_binary(&REWARD_SCHEDULE_REQUESTS.load(deps.storage, reward_schedule_request_id)?)
        }
        QueryMsg::RewardScheduleRequestProposalId { reward_schedule_request_id } => {
            to_binary(&REWARD_SCHEDULE_REQUEST_PROPOSAL_ID.load(deps.storage, reward_schedule_request_id)?)
        }

        QueryMsg::RefundableFunds { pool_creation_request_id } => {
            todo!()
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
