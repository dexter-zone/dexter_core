
use const_format::concatcp;
use cosmwasm_std::{
    Addr, DepsMut, Env, Event,
    MessageInfo, Response,
};


use dexter::{
    asset::AssetInfo,
    multi_staking::RewardSchedule,
};


use dexter::asset::Asset;
use dexter::helper::{EventExt, build_transfer_cw20_from_user_msg};


use crate::contract::{ContractResult, CONTRACT_NAME};
use crate::state::LP_TOKEN_ASSET_REWARD_SCHEDULE;
use crate::{
    error::ContractError,
    state::{
        next_reward_schedule_id, CONFIG, LP_GLOBAL_STATE, REWARD_SCHEDULES,
    },
};

use super::{check_if_lp_token_allowed, NO_PKEY_ALLOWED_ADDR};


pub fn create_reward_schedule(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_token: Addr,
    title: String,
    start_block_time: u64,
    end_block_time: u64,
    creator: Addr,
    asset: Asset,
) -> ContractResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    check_if_lp_token_allowed(&config, &lp_token)?;

    // validate sender as the non-private key having module address so we can be sure no-one is able to execute this directly without governance
    if info.sender != NO_PKEY_ALLOWED_ADDR {
        return Err(ContractError::Unauthorized {});
    }

    // validate start and end block times
    if start_block_time < env.block.time.seconds() {
        return Err(ContractError::InvalidStartBlockTime {
            start_block_time,
            current_block_time: env.block.time.seconds(),
        });
    }

    if end_block_time < start_block_time {
        return Err(ContractError::InvalidEndBlockTime {
            end_block_time,
            start_block_time,
        });
    }

    let mut res = Response::new().add_attribute("action", "create_reward_schedule");

    match &asset.info {
        AssetInfo::NativeToken { .. } => {
            // validate if the amount is sent in the message
            if info.funds.len() != 1 {
                return Err(ContractError::InvalidNumberOfAssets {
                    correct_number: 1,
                    received_number: info.funds.len() as u8,
                });
            }

            let amount = info.funds[0].amount;
            if amount != asset.amount {
                return Err(ContractError::InvalidRewardScheduleAmount {
                    correct_amount: asset.amount,
                    received_amount: amount,
                });
            }
        }
        AssetInfo::Token { contract_addr } => {
            // transfer the token to contract before creating the reward schedule
            deps.api.addr_validate(contract_addr.as_str())?;
            let transfer_msg = build_transfer_cw20_from_user_msg(
                asset.info.contract_addr()?,
                creator.to_string(),
                env.contract.address.to_string(),
                asset.amount.clone(),
            )?;

            res = res.add_message(transfer_msg);
        }
    }



    let mut lp_global_state = LP_GLOBAL_STATE
        .may_load(deps.storage, &lp_token)?
        .unwrap_or_default();

    if !lp_global_state.active_reward_assets.contains(&asset.info) {
        lp_global_state.active_reward_assets.push(asset.info.clone());
    }

    LP_GLOBAL_STATE.save(deps.storage, &lp_token, &lp_global_state)?;


    let reward_schedule_id = next_reward_schedule_id(deps.storage)?;
    let reward_schedule = RewardSchedule {
        title,
        creator: creator.clone(),
        asset: asset.info.clone(),
        amount: asset.amount,
        staking_lp_token: lp_token.clone(),
        start_block_time,
        end_block_time,
    };

    REWARD_SCHEDULES.save(deps.storage, reward_schedule_id, &reward_schedule)?;

    let mut reward_schedules_ids = LP_TOKEN_ASSET_REWARD_SCHEDULE
                .may_load(
                    deps.storage,
                    (&lp_token, &asset.info.to_string()),
                )?
                .unwrap_or_default();

    reward_schedules_ids.push(reward_schedule_id);
    LP_TOKEN_ASSET_REWARD_SCHEDULE.save(
        deps.storage,
        (&lp_token, &asset.info.to_string()),
        &reward_schedules_ids,
    )?;

    Ok(res
        .add_event(
            Event::from_info(
                concatcp!(CONTRACT_NAME, "::create_reward_schedule"),
                &info,
            )
            .add_attribute("reward_schedule_id", reward_schedule_id.to_string())
            .add_attribute("lp_token", lp_token)
            .add_attribute("asset", serde_json_wasm::to_string(&asset).unwrap())
            .add_attribute("amount", asset.amount.to_string())
            .add_attribute("start_block_time", start_block_time.to_string())
            .add_attribute("end_block_time", end_block_time.to_string())
            .add_attribute("creator", creator),
        )
    )
}
