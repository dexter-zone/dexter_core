use crate::contract::{ContractResult, CONTRACT_NAME};

use crate::state::POOL_CREATION_REQUESTS;

use const_format::concatcp;

use cosmwasm_std::{
    to_binary, CosmosMsg, DepsMut, Env, Event, MessageInfo, Response, Uint128, WasmMsg,
};

use dexter::helper::EventExt;
use dexter::querier::query_vault_config;

pub fn execute_resume_join_pool(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    pool_creation_request_id: u64,
) -> ContractResult<Response> {
    let pool_creation_request =
        POOL_CREATION_REQUESTS.load(deps.storage, pool_creation_request_id)?;

    // find the pool id from the vault by querying the vault for the next pool id
    let vault_config =
        query_vault_config(&deps.querier, pool_creation_request.vault_addr.to_string())?;
    let mut messages: Vec<CosmosMsg> = vec![];

    let pool_id = vault_config
        .next_pool_id
        .checked_sub(Uint128::from(1u128))?;

    // check if the pool creation request has a bootstrapping amount
    if let Some(bootstrapping_amount) = pool_creation_request.bootstrapping_amount {
        // now we can just join the pool
        let join_pool_msg = dexter::vault::ExecuteMsg::JoinPool {
            pool_id,
            recipient: Some(pool_creation_request.bootstrapping_liquidity_owner),
            assets: Some(bootstrapping_amount),
            min_lp_to_receive: None,
            auto_stake: None,
        };

        // add the message to the list of messages
        messages.push(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: pool_creation_request.vault_addr.to_string(),
                msg: to_binary(&join_pool_msg)?,
                funds: vec![],
            })
            .into(),
        );
    }

    // // check if the pool creation request has reward schedules
    // if let Some(reward_schedules) = pool_creation_request.reward_schedules {
    //     for reward_schedule in reward_schedules {
    //         let add_reward_schedule_msg = dexter::multi_staking::ExecuteMsg::ProposeRewardSchedule {
    //             pool_id,
    //             start_time: reward_schedule.start_time,
    //             end_time: reward_schedule.end_time,
    //             epoch_amount: reward_schedule.amount,
    //         };

    //         // add the message to the list of messages
    //         messages.push(
    //             CosmosMsg::Wasm(WasmMsg::Execute {
    //                 contract_addr: pool_creation_request.vault_addr.to_string(),
    //                 msg: to_binary(&add_reward_schedule_msg)?,
    //                 funds: vec![],
    //             })
    //             .into(),
    //         );
    //     }
    // }

    // // add the message to the list of messages
    // let mut messages: Vec<CosmosMsg> = vec![];
    // messages.push(
    //     CosmosMsg::Wasm(WasmMsg::Execute {
    //         contract_addr: vault_addr.to_string(),
    //         msg: to_binary(&join_pool_msg)?,
    //         funds: vec![],
    //     })
    //     .into(),
    // );

    let event = Event::from_info(concatcp!(CONTRACT_NAME, "::resume_join_pool"), &info);

    let res = Response::new().add_messages(messages).add_event(event);

    Ok(res)
}
