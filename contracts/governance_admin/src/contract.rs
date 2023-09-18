#[cfg(not(feature = "library"))]
use crate::error::ContractError;
use crate::execute::create_pool_creation_proposal::execute_create_pool_creation_proposal;
use crate::execute::post_pool_creation_callback::execute_post_governance_proposal_creation_callback;
use crate::execute::resume_create_pool::execute_resume_create_pool;
use crate::execute::resume_join_pool::execute_resume_join_pool;

use const_format::concatcp;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    entry_point, Binary, Deps, DepsMut, Env, Event, MessageInfo, Response, StdError, StdResult,
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
            title,
            description,
            pool_creation_request,
        } => execute_create_pool_creation_proposal(
            deps,
            env,
            info,
            title,
            description,
            pool_creation_request,
        ),
        ExecuteMsg::PostGovernanceProposalCreationCallback {
            pool_creation_request_id,
        } => execute_post_governance_proposal_creation_callback(
            deps,
            env,
            info,
            pool_creation_request_id,
        ),
        ExecuteMsg::ResumeCreatePool {
            pool_creation_request_id,
        } => execute_resume_create_pool(deps, env, info, pool_creation_request_id),

        ExecuteMsg::ResumeJoinPool {
            pool_creation_request_id,
        } => execute_resume_join_pool(deps, env, info, pool_creation_request_id),

        ExecuteMsg::CreateRewardSchedulesProposal { title, description, reward_schedules } => {
            todo!()
        },
        ExecuteMsg::PostRewardSchedulesProposalCreationCallback { reward_schedules_creation_request_id } => {
            todo!()
        },
        ExecuteMsg::ResumeCreateRewardSchedules { reward_schedules_creation_request_id } => {
            todo!()
        },
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    return Err(StdError::generic_err("unsupported query"));
}

#[cw_serde]
pub struct MigrateMsg {}

// migrate handler
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    return Ok(Response::default());
}
