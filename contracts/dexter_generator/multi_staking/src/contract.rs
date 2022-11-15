#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, Addr, Decimal, DepsMut, Env, MessageInfo, Response, StdError, StdResult, Uint128,
};

use dexter::{
    asset::AssetInfo,
    multi_staking::{AssetRewardState, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, State},
};

// use crate::state::{Config, StakerInfo, State, CONFIG, STATE, USERS};

use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use crate::state::{
    AssetStakerInfo, Config, RewardSchedule, CONFIG, REWARD_SCHEDULES, REWARD_STATES, STAKERS,
    STATE,
};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    // CONFIG.save(
    //     deps.storage,
    //     &Config {
    //         anchor_token: deps.api.addr_canonicalize(&msg.anchor_token)?,
    //         staking_token: deps.api.addr_canonicalize(&msg.staking_token)?,
    //         distribution_schedule: msg.distribution_schedule,
    //     },
    // )?;

    // STATE.save(
    //     deps.storage,
    //     &State {
    //         last_distributed: env.block.time.seconds(),
    //         total_bond_amount: Uint128::zero(),
    //         global_reward_index: Decimal::zero(),
    //     },
    // )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::AddRewardFactory {
            lp_token,
            asset,
            amount,
            start_block_time,
            end_block_time,
        } => add_reward_factory(
            deps,
            env,
            info,
            lp_token,
            asset,
            amount,
            start_block_time,
            end_block_time,
        ),
        ExecuteMsg::Unbond { lp_token, amount } => unbond(deps, env, info.sender, lp_token, amount),
        ExecuteMsg::Withdraw { lp_token } => withdraw(deps, env, info.sender, lp_token),
    }
}

pub fn add_reward_factory(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    lp_token: Addr,
    asset: AssetInfo,
    amount: Uint128,
    start_block_time: u64,
    end_block_time: u64,
) -> StdResult<Response> {
    let reward_schedule = RewardSchedule {
        asset: asset.clone(),
        amount,
        staking_lp_token: lp_token.clone(),
        start_block_time,
        end_block_time,
    };

    let factory_info = STATE.load(deps.storage)?;
    let mut reward_schedules =
        REWARD_SCHEDULES.load(deps.storage, (&lp_token, &asset.to_string()))?;
    reward_schedules.push(reward_schedule);

    REWARD_SCHEDULES.save(
        deps.storage,
        (&lp_token, &asset.to_string()),
        &reward_schedules,
    )?;

    Ok(Response::default())
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    match from_binary(&cw20_msg.msg) {
        Ok(msg) => {
            match msg {
                Cw20HookMsg::Bond {} => {
                    let token_address = deps.api.addr_validate(info.sender.as_str())?;
                    if !config.allowed_lp_tokens.contains(&token_address) {
                        return Err(StdError::generic_err("LP Token not supported for staking"));
                    }

                    let cw20_sender = deps.api.addr_validate(&cw20_msg.sender)?;
                    bond(deps, env, cw20_sender, token_address, cw20_msg.amount)
                }
                Cw20HookMsg::AddRewardFactory {
                    lp_token,
                    start_block_time,
                    end_block_time,
                } => {
                    let token_address = deps.api.addr_validate(info.sender.as_str())?;
                    add_reward_factory(
                        deps,
                        env,
                        info,
                        lp_token,
                        AssetInfo::Token {
                            contract_addr: token_address,
                        },
                        cw20_msg.amount,
                        start_block_time,
                        end_block_time,
                    )
                }
            }
        }
        Err(_) => Err(StdError::generic_err("data should be given")),
    }
}

pub fn compute_reward(
    config: &Config,
    current_block_time: u64,
    state: &mut AssetRewardState,
    reward_schedules: Vec<RewardSchedule>,
) {
    if state.total_bond_amount.is_zero() {
        state.last_distributed = current_block_time;
        return;
    }

    let mut distributed_amount: Uint128 = Uint128::zero();
    for s in reward_schedules.iter() {
        let start_time = s.start_block_time;
        let end_time = s.end_block_time;

        if start_time > current_block_time || end_time < state.last_distributed {
            continue;
        }

        // min(s.1, block_time) - max(s.0, last_distributed)
        let passed_time = std::cmp::min(end_time, current_block_time)
            - std::cmp::max(start_time, current_block_time);

        let time = end_time - start_time;
        let distribution_amount_per_second: Decimal = Decimal::from_ratio(s.amount, time);
        distributed_amount += distribution_amount_per_second * Uint128::from(passed_time as u128);
    }

    state.last_distributed = current_block_time;
    state.reward_index =
        state.reward_index + Decimal::from_ratio(distributed_amount, state.total_bond_amount);
}

pub fn compute_staker_reward(
    state: &AssetRewardState,
    staker_info: &mut AssetStakerInfo,
) -> StdResult<()> {
    let pending_reward = (staker_info.bond_amount * state.reward_index)
        .checked_sub(staker_info.bond_amount * staker_info.reward_index)?;

    staker_info.reward_index = state.reward_index;
    staker_info.pending_reward += pending_reward;
    Ok(())
}

pub fn increase_bond_amount(
    state: &mut AssetRewardState,
    staker_info: &mut AssetStakerInfo,
    amount: Uint128,
) -> StdResult<()> {
    staker_info.bond_amount = staker_info.bond_amount.checked_add(amount)?;
    state.total_bond_amount = staker_info.bond_amount.checked_add(amount)?;
    Ok(())
}

pub fn decrease_bond_amount(
    state: &mut AssetRewardState,
    staker_info: &mut AssetStakerInfo,
    amount: Uint128,
) -> StdResult<()> {
    staker_info.bond_amount = staker_info.bond_amount.checked_sub(amount)?;
    state.total_bond_amount = state.total_bond_amount.checked_sub(amount)?;
    Ok(())
}

pub fn bond(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    lp_token: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;
    let mut user_staking_info = STAKERS.load(deps.storage, (&sender, &lp_token))?;

    for asset_staker_info in &mut user_staking_info {
        let mut asset_state =
            REWARD_STATES.load(deps.storage, &asset_staker_info.asset.to_string())?;
        let reward_schedules = REWARD_SCHEDULES.load(
            deps.storage,
            (&lp_token, &asset_staker_info.asset.to_string()),
        )?;

        compute_reward(
            &config,
            env.block.time.seconds(),
            &mut asset_state,
            reward_schedules,
        );
        compute_staker_reward(&mut asset_state, asset_staker_info)?;

        increase_bond_amount(&mut asset_state, asset_staker_info, amount)?;

        REWARD_STATES.save(
            deps.storage,
            &asset_staker_info.asset.to_string(),
            &asset_state,
        )?;
    }

    STAKERS.save(deps.storage, (&sender, &lp_token), &user_staking_info);

    return Err(StdError::generic_err("data should be given"));
}

pub fn unbond(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    lp_token: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;

    let mut user_staking_info = STAKERS.load(deps.storage, (&sender, &lp_token))?;

    for asset_staker_info in &mut user_staking_info {
        let mut asset_state =
            REWARD_STATES.load(deps.storage, &asset_staker_info.asset.to_string())?;
        let reward_schedules = REWARD_SCHEDULES.load(
            deps.storage,
            (&lp_token, &asset_staker_info.asset.to_string()),
        )?;

        compute_reward(
            &config,
            env.block.time.seconds(),
            &mut asset_state,
            reward_schedules,
        );
        compute_staker_reward(&mut asset_state, asset_staker_info)?;

        decrease_bond_amount(&mut asset_state, asset_staker_info, amount)?;

        REWARD_STATES.save(
            deps.storage,
            &asset_staker_info.asset.to_string(),
            &asset_state,
        )?;
    }

    STAKERS.save(deps.storage, (&sender, &lp_token), &user_staking_info);

    return Err(StdError::generic_err("data should be given"));
}

pub fn withdraw(deps: DepsMut, env: Env, sender: Addr, lp_token: Addr) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    let mut user_staking_info = STAKERS.load(deps.storage, (&sender, &lp_token))?;

    let mut transfer_msgs: Vec<Cw20ExecuteMsg> = vec![];
    for asset_staker_info in &mut user_staking_info {
        let mut asset_state =
            REWARD_STATES.load(deps.storage, &asset_staker_info.asset.to_string())?;
        let reward_schedules = REWARD_SCHEDULES.load(
            deps.storage,
            (&lp_token, &asset_staker_info.asset.to_string()),
        )?;

        compute_reward(
            &config,
            env.block.time.seconds(),
            &mut asset_state,
            reward_schedules,
        );
        compute_staker_reward(&mut asset_state, asset_staker_info)?;

        asset_staker_info.pending_reward = Uint128::zero();
        transfer_msgs.push(
            Cw20ExecuteMsg::Transfer {
                recipient: sender.to_string(),
                amount: asset_staker_info.pending_reward,
            }
            .into(),
        );

        REWARD_STATES.save(
            deps.storage,
            &asset_staker_info.asset.to_string(),
            &asset_state,
        )?;
    }

    STAKERS.save(deps.storage, (&sender, &lp_token), &user_staking_info);

    Ok(Response::default())
}

// #[cfg_attr(not(feature = "library"), entry_point)]
// pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
//     // match msg {
//     //     QueryMsg::Config {} => to_binary(&query_config(deps)?),
//     //     QueryMsg::State { block_time } => to_binary(&query_state(deps, block_time)?),
//     //     QueryMsg::StakerInfo { staker, block_time } => {
//     //         to_binary(&query_staker_info(deps, staker, block_time)?)
//     //     }
//     // }
// }
