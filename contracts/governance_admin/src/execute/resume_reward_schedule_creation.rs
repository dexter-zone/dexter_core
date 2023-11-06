use const_format::concatcp;
use cosmwasm_std::{
    to_json_binary, Coin, CosmosMsg, DepsMut, Event, MessageInfo, Response, StdError,
};
use dexter::{governance_admin::RewardSchedulesCreationRequestStatus, helper::EventExt};

use crate::{
    add_wasm_execute_msg,
    contract::{ContractResult, CONTRACT_NAME},
    error::ContractError,
    state::REWARD_SCHEDULE_REQUESTS,
};

pub fn execute_resume_reward_schedule_creation(
    deps: DepsMut,
    info: MessageInfo,
    reward_schedule_creation_request_id: u64,
) -> ContractResult<Response> {
    let mut msgs: Vec<CosmosMsg> = vec![];

    // find the reward schedule creation request
    let mut reward_schedule_creation_request =
        REWARD_SCHEDULE_REQUESTS.load(deps.storage, reward_schedule_creation_request_id)?;

    let mut event = Event::from_info(
        concatcp!(CONTRACT_NAME, "::resume_reward_schedule_creation"),
        &info,
    );

    // mark the request as done
    match reward_schedule_creation_request.status {
        RewardSchedulesCreationRequestStatus::NonProposalRewardSchedule => {
            reward_schedule_creation_request.status =
                RewardSchedulesCreationRequestStatus::RewardSchedulesCreated { proposal_id: None };
            event = event.add_attribute("proposal_id", "-1".to_string());
        }
        RewardSchedulesCreationRequestStatus::ProposalCreated { proposal_id } => {
            reward_schedule_creation_request.status =
                RewardSchedulesCreationRequestStatus::RewardSchedulesCreated {
                    proposal_id: Some(proposal_id),
                };
            event = event.add_attribute("proposal_id", "proposal_id".to_string());
        }
        _ => {
            return Err(ContractError::InvalidRewardScheduleRequestStatus);
        }
    }

    REWARD_SCHEDULE_REQUESTS.save(
        deps.storage,
        reward_schedule_creation_request_id,
        &reward_schedule_creation_request,
    )?;

    // create the reward schedules
    for request in reward_schedule_creation_request.reward_schedule_creation_requests.clone() {
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
                    msg: to_json_binary(&msg_create_reward_schedule)?,
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

    event = event
        .add_attribute(
            "reward_schedule_creation_request_id",
            reward_schedule_creation_request_id.to_string(),
        )
        .add_attribute(
            "reward_schedule_creation_requests",
            serde_json_wasm::to_string(
                &reward_schedule_creation_request.reward_schedule_creation_requests,
            )
            .unwrap(),
        );
    let response = Response::new().add_messages(msgs).add_event(event);

    Ok(response)
}
