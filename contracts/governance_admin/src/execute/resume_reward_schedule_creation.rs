use cosmwasm_std::{to_binary, Coin, CosmosMsg, DepsMut, Response, StdError};
use dexter::governance_admin::RewardSchedulesCreationRequestStatus;

use crate::{
    add_wasm_execute_msg, contract::ContractResult, error::ContractError,
    state::REWARD_SCHEDULE_REQUESTS,
};

pub fn execute_resume_reward_schedule_creation(
    deps: DepsMut,
    reward_schedule_creation_request_id: u64,
) -> ContractResult<Response> {
    let mut msgs: Vec<CosmosMsg> = vec![];

    // find the reward schedule creation request
    let mut reward_schedule_creation_request =
        REWARD_SCHEDULE_REQUESTS.load(deps.storage, reward_schedule_creation_request_id)?;

    // mark the request as done
    match reward_schedule_creation_request.status {
        RewardSchedulesCreationRequestStatus::NonProposalRewardSchedule => {
            reward_schedule_creation_request.status =
                RewardSchedulesCreationRequestStatus::RewardSchedulesCreated { proposal_id: None };
        }
        RewardSchedulesCreationRequestStatus::ProposalCreated { proposal_id } => {
            reward_schedule_creation_request.status =
                RewardSchedulesCreationRequestStatus::RewardSchedulesCreated {
                    proposal_id: Some(proposal_id),
                };
        }
        _ => {
            return Err(ContractError::Std(StdError::generic_err(format!(
                "invalid reward schedule creation request status"
            ))));
        }
    }

    REWARD_SCHEDULE_REQUESTS.save(
        deps.storage,
        reward_schedule_creation_request_id,
        &reward_schedule_creation_request,
    )?;

    // create the reward schedules
    for request in reward_schedule_creation_request.reward_schedule_creation_requests {
        match request.asset {
            dexter::asset::AssetInfo::Token { contract_addr } => {
                // send a CW20 hook msg to the multistaking contract
                let msg_create_reward_schedule =
                    dexter::multi_staking::Cw20HookMsg::CreateRewardSchedule {
                        lp_token: request.lp_token_addr.ok_or(ContractError::LpTokenNull)?,
                        title: request.title,
                        actual_creator: Some(
                            reward_schedule_creation_request.request_sender.clone(),
                        ),
                        start_block_time: request.start_block_time,
                        end_block_time: request.end_block_time,
                    };

                let cw20_transfer_msg_lp_token = cw20::Cw20ExecuteMsg::Send {
                    contract: reward_schedule_creation_request
                        .multistaking_contract_addr
                        .to_string(),
                    amount: request.amount,
                    msg: to_binary(&msg_create_reward_schedule)?,
                };

                add_wasm_execute_msg!(
                    msgs,
                    contract_addr.to_string(),
                    cw20_transfer_msg_lp_token,
                    vec![]
                );
            }
            dexter::asset::AssetInfo::NativeToken { denom } => {
                let msg_create_reward_schedule =
                    dexter::multi_staking::ExecuteMsg::CreateRewardSchedule {
                        lp_token: request.lp_token_addr.ok_or(ContractError::LpTokenNull)?,
                        title: request.title,
                        actual_creator: Some(
                            reward_schedule_creation_request.request_sender.clone(),
                        ),
                        start_block_time: request.start_block_time,
                        end_block_time: request.end_block_time,
                    };

                add_wasm_execute_msg!(
                    msgs,
                    reward_schedule_creation_request.multistaking_contract_addr,
                    msg_create_reward_schedule,
                    vec![Coin {
                        denom,
                        amount: request.amount,
                    }]
                );
            }
        }
    }

    let mut response = Response::new();
    response = response.add_messages(msgs);

    Ok(response)
}
