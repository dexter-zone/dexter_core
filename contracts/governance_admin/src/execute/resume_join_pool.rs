use crate::add_wasm_execute_msg;
use crate::contract::{ContractResult, CONTRACT_NAME};

use crate::error::ContractError;
use crate::state::{POOL_CREATION_REQUEST_DATA, REWARD_SCHEDULE_REQUESTS, next_reward_schedule_request_id};

use const_format::concatcp;

use cosmwasm_std::{
    to_binary, CosmosMsg, DepsMut, Env, Event, MessageInfo, Response, Uint128, Coin, StdError,
};
use cw20::Expiration;
use dexter::asset::AssetInfo;

use dexter::governance_admin::{RewardScheduleCreationRequest, RewardScheduleCreationRequestsState, RewardSchedulesCreationRequestStatus, PoolCreationRequestStatus};
use dexter::helper::EventExt;
use dexter::querier::query_vault_config;
use dexter::vault::AutoStakeImpl;

pub fn execute_resume_join_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_creation_request_id: u64,
) -> ContractResult<Response> {
    let mut pool_creation_request_context =
        POOL_CREATION_REQUEST_DATA.load(deps.storage, pool_creation_request_id)?;

    let pool_creation_request = &pool_creation_request_context.pool_creation_request;

    // find the pool id from the vault by querying the vault for the next pool id
    let vault_config =
        query_vault_config(&deps.querier, pool_creation_request.vault_addr.to_string())?;
    let mut messages: Vec<CosmosMsg> = vec![];

    let pool_id = vault_config
        .next_pool_id
        .checked_sub(Uint128::from(1u128))?;

    pool_creation_request_context.status = PoolCreationRequestStatus::PoolCreated { 
        proposal_id: pool_creation_request_context.status.proposal_id().unwrap(),
        pool_id: pool_id.clone(),
    };

    POOL_CREATION_REQUEST_DATA.save(
        deps.storage,
        pool_creation_request_id,
        &pool_creation_request_context,
    )?;

    let get_pool_details = dexter::vault::QueryMsg::GetPoolById {
        pool_id,
    };

    let pool_info_response: dexter::vault::PoolInfoResponse =
        deps.querier.query_wasm_smart(pool_creation_request.vault_addr.to_string(), &get_pool_details)?;

    let multistaking_address = if let AutoStakeImpl::Multistaking { contract_addr } = vault_config.auto_stake_impl {
        contract_addr
    } else {
        return Err(ContractError::Std(StdError::generic_err(format!(
            "Auto stake implementation is not multistaking"
        ))));
    };
 
    let lp_token = pool_info_response.lp_token_addr;

    // check if the pool creation request has a bootstrapping amount
    if let Some(bootstrapping_amount) = &pool_creation_request.bootstrapping_amount {

        let mut native_coins = vec![];

        // allow the vault to spend the CW20 token funds if there are any in the bootstrapping amount
        for asset in bootstrapping_amount {
            match &asset.info {
                AssetInfo::Token { contract_addr} => {
                    let msg = cw20::Cw20ExecuteMsg::IncreaseAllowance {
                        spender: pool_creation_request.vault_addr.to_string(),
                        amount: asset.amount,
                        expires: Some(Expiration::AtHeight(env.block.height + 1)),
                    };
                    add_wasm_execute_msg!(messages, contract_addr.to_string(), msg, vec![]);
                }
                AssetInfo::NativeToken { .. } => {
                    native_coins.push(Coin {
                        denom: asset.info.to_string(),
                        amount: asset.amount,
                    });
                }
            }
        }

        // now we can just join the pool
        let join_pool_msg = dexter::vault::ExecuteMsg::JoinPool {
            pool_id,
            recipient: Some(pool_creation_request.bootstrapping_liquidity_owner.clone()),
            assets: Some(bootstrapping_amount.clone()),
            min_lp_to_receive: None,
            auto_stake: None,
        };

        // add the message to the list of messages
        add_wasm_execute_msg!(messages, pool_creation_request.vault_addr.to_string(), join_pool_msg, native_coins);
    }


    // register the LP token in the multistaking contract 
    let register_lp_token_msg = dexter::multi_staking::ExecuteMsg::AllowLpToken {
        lp_token: lp_token.clone(),
    };

    // add the message to the list of messages
    add_wasm_execute_msg!(messages, multistaking_address.to_string(), register_lp_token_msg, vec![]);

    // create a reward schedule creation request if there are any reward schedules
    // store the reward schedule creation request
    let next_reward_schedules_creation_request_id = next_reward_schedule_request_id(deps.storage)?;

    if let Some(reward_schedules) = &pool_creation_request.reward_schedules {
        let mut updated_reward_schedules =  vec![];

        for reward_schedule in reward_schedules {
            let updated_reward_schedule = RewardScheduleCreationRequest {
                lp_token_addr: Some(lp_token.clone()),
                ..reward_schedule.clone()
            };

            updated_reward_schedules.push(updated_reward_schedule);
        }


        REWARD_SCHEDULE_REQUESTS.save(
            deps.storage,
            next_reward_schedules_creation_request_id,
            &RewardScheduleCreationRequestsState {
                    request_sender: pool_creation_request_context.request_sender,
                    status: RewardSchedulesCreationRequestStatus::NonProposalRewardSchedule,
                    multistaking_contract_addr:  multistaking_address,
                    reward_schedule_creation_requests: updated_reward_schedules.clone(),
                    user_deposits_detailed: vec![],
                    total_funds_acquired_from_user: vec![],
                },
        )?;

        // add a message to resume the reward schedule creation
        let resume_create_reward_schedules_msg = dexter::governance_admin::ExecuteMsg::ResumeCreateRewardSchedules {
            reward_schedules_creation_request_id: next_reward_schedules_creation_request_id,
        };

        // add the message to the list of messages
        add_wasm_execute_msg!(messages, env.contract.address.to_string(), resume_create_reward_schedules_msg, vec![]);
    }


    let mut event = Event::from_info(concatcp!(CONTRACT_NAME, "::resume_join_pool"), &info);
    event = event
        .add_attribute("pool_creation_request_id", pool_creation_request_id.to_string())
        .add_attribute("pool_id", pool_id.to_string());

    let res = Response::new().add_messages(messages).add_event(event);

    Ok(res)
}
