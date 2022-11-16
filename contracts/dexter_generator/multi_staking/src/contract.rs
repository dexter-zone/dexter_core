#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, Addr, Decimal, DepsMut, Env, MessageInfo, Response, StdError, StdResult, Uint128, Deps, Binary, to_binary, CosmosMsg,
};

use dexter::{
    asset::AssetInfo,
    multi_staking::{AssetRewardState, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, UnclaimedReward}, helper::{build_transfer_token_to_user_msg},
};

// use crate::state::{Config, StakerInfo, State, CONFIG, STATE, USERS};

use cw20::{Cw20ReceiveMsg};

use crate::state::{
    AssetStakerInfo, Config, RewardSchedule, CONFIG, REWARD_SCHEDULES, REWARD_STATES, LP_ACTIVE_REWARD_ASSETS, ASSET_STAKER_INFO
};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> StdResult<Response> {
    CONFIG.save(
        deps.storage,
        &Config {
            allowed_lp_tokens: vec![]
        },
    )?;

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
        ExecuteMsg::AllowLpToken { lp_token } => allow_lp_token(deps, env, info, lp_token),
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::AddRewardFactory {
            lp_token,
            denom,
            amount,
            start_block_time,
            end_block_time,
        } => {
            // Verify that the asset for reward was sent with the message
            if info.funds.len() != 1 {
                return Err(StdError::generic_err("Only 1 asset can be sent with the message"));
            }

            let sender = info.sender.clone();
            let sent_asset = info.funds[0].clone();

            if sent_asset.denom == denom {
                let asset = AssetInfo::NativeToken { denom: denom.clone() };
                // verify that enough amount was sent
                if sent_asset.amount >= amount {
                    let mut response = add_reward_factory(deps, env, info, lp_token, asset.clone(), amount, start_block_time, end_block_time)?;

                    let extra_amount = sent_asset.amount.checked_sub(amount)?;
                    response =  build_transfer_token_to_user_msg(asset, sender, extra_amount)
                        .map(|msg| response.add_message(msg))?;

                    Ok(response)
                } else {
                    Err(StdError::generic_err("Not enough asset for reward was sent"))
                }
            } else {
                Err(StdError::generic_err("Asset for reward was not sent with the message"))
            }
        },
        ExecuteMsg::Unbond { lp_token, amount } => unbond(deps, env, info.sender, lp_token, amount),
        ExecuteMsg::Withdraw { lp_token } => withdraw(deps, env, &info.sender, lp_token),
    }
}

fn allow_lp_token(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    lp_token: Addr,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;
    config.allowed_lp_tokens.push(lp_token);
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::default())
}

pub fn add_reward_factory(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
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

    let mut reward_schedules =
        REWARD_SCHEDULES.may_load(deps.storage, (&lp_token, &asset.to_string()))?
            .unwrap_or_default();
    
    reward_schedules.push(reward_schedule);

    REWARD_SCHEDULES.save(
        deps.storage,
        (&lp_token, &asset.to_string()),
        &reward_schedules,
    )?;

    let mut lp_active_reward_assets = LP_ACTIVE_REWARD_ASSETS
        .may_load(deps.storage, &lp_token)?
        .unwrap_or_default();

    if !lp_active_reward_assets.contains(&asset) {
        lp_active_reward_assets.push(asset);
    }

    LP_ACTIVE_REWARD_ASSETS.save(deps.storage, &lp_token, &lp_active_reward_assets)?;
    
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
                    if !config.allowed_lp_tokens.contains(&lp_token) {
                        return Err(StdError::generic_err("LP Token not supported for staking"));
                    }
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
    _config: &Config,
    current_block_time: u64,
    state: &mut AssetRewardState,
    reward_schedules: Vec<RewardSchedule>,
) {

    println!("Reward schedules {:?}", reward_schedules);
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
            - std::cmp::max(start_time, state.last_distributed);

        

        let time = end_time - start_time;
        let distribution_amount_per_second: Decimal = Decimal::from_ratio(s.amount, time);
        distributed_amount += distribution_amount_per_second * Uint128::from(passed_time as u128);

        println!("passed time {}, distribution amount per second {}, distributed amount {}", passed_time, distribution_amount_per_second, distributed_amount);
    }

    println!("Distributed amount: {:?}", distributed_amount);
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
    state.total_bond_amount = state.total_bond_amount.checked_add(amount)?;
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
    let config = CONFIG.load(deps.storage)?;

    let lp_active_reward_assets = LP_ACTIVE_REWARD_ASSETS
        .may_load(deps.storage, &lp_token)?
        .unwrap_or_default();

    for asset in lp_active_reward_assets {
        let mut asset_staker_info = ASSET_STAKER_INFO
            .may_load(deps.storage, (&lp_token, &sender, &asset.to_string()))?
            .unwrap_or(AssetStakerInfo {
                asset: asset.clone(),
                bond_amount: Uint128::zero(),
                pending_reward: Uint128::zero(),
                reward_index: Decimal::zero(),
            });

        let mut asset_state =
            REWARD_STATES.may_load(deps.storage, &asset.to_string())?
            .unwrap_or(AssetRewardState {
                total_bond_amount: Uint128::zero(),
                reward_index: Decimal::zero(),
                last_distributed: env.block.time.seconds(),
            });
        
        let reward_schedules = REWARD_SCHEDULES.may_load(
            deps.storage,
            (&lp_token, &asset_staker_info.asset.to_string()),
        )?.unwrap_or_default();
        
        compute_reward(
            &config,
            env.block.time.seconds(),
            &mut asset_state,
            reward_schedules,
        );
        compute_staker_reward(&mut asset_state, &mut asset_staker_info)?;
        increase_bond_amount(&mut asset_state, &mut asset_staker_info, amount)?;

        println!("\n\nBlock time {}", env.block.time.seconds());
        println!("BOND: Asset state: {:?}", asset_state);
        println!("BOND: Asset staker info: {:?}", asset_staker_info);
        REWARD_STATES.save(
            deps.storage,
            &asset_staker_info.asset.to_string(),
            &asset_state,
        )?;

        ASSET_STAKER_INFO.save(
            deps.storage,
            (&lp_token, &sender, &asset_staker_info.asset.to_string()),
            &asset_staker_info,
        )?;
    }

    Ok(Response::default())
}

pub fn unbond(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    lp_token: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    let lp_active_reward_assets = LP_ACTIVE_REWARD_ASSETS
        .may_load(deps.storage, &lp_token)?
        .unwrap_or_default();

    for asset in lp_active_reward_assets {
        let mut asset_staker_info = ASSET_STAKER_INFO
            .may_load(deps.storage, (&lp_token, &sender, &asset.to_string()))?
            .unwrap_or(AssetStakerInfo {
                asset: asset.clone(),
                bond_amount: Uint128::zero(),
                pending_reward: Uint128::zero(),
                reward_index: Decimal::zero(),
            });

        let mut asset_state =
            REWARD_STATES.load(deps.storage, &asset.to_string())?;
        
        let reward_schedules = REWARD_SCHEDULES.may_load(
            deps.storage,
            (&lp_token, &asset_staker_info.asset.to_string()),
        )?.unwrap_or_default();
        
        compute_reward(
            &config,
            env.block.time.seconds(),
            &mut asset_state,
            reward_schedules,
        );
        compute_staker_reward(&mut asset_state, &mut asset_staker_info)?;
        decrease_bond_amount(&mut asset_state, &mut asset_staker_info, amount)?;

        println!("\n\nBlock time {}", env.block.time.seconds());
        println!("UNBOND: Asset state: {:?}", asset_state);
        println!("UNBOND: Asset staker info: {:?}", asset_staker_info);
        REWARD_STATES.save(
            deps.storage,
            &asset_staker_info.asset.to_string(),
            &asset_state,
        )?;

        ASSET_STAKER_INFO.save(
            deps.storage,
            (&lp_token, &sender, &asset_staker_info.asset.to_string()),
            &asset_staker_info,
        )?;
    }

    Ok(Response::default())
}

pub fn withdraw(deps: DepsMut, env: Env, sender: &Addr, lp_token: Addr) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    let lp_active_reward_assets = LP_ACTIVE_REWARD_ASSETS
        .may_load(deps.storage, &lp_token)?
        .unwrap_or_default();

    let mut transfer_msgs: Vec<CosmosMsg> = vec![];
    for asset in lp_active_reward_assets {
        let mut asset_staker_info = ASSET_STAKER_INFO
            .may_load(deps.storage, (&lp_token, &sender, &asset.to_string()))?
            .unwrap_or(AssetStakerInfo {
                asset: asset.clone(),
                bond_amount: Uint128::zero(),
                pending_reward: Uint128::zero(),
                reward_index: Decimal::zero(),
            });

        let mut asset_state =
            REWARD_STATES.load(deps.storage, &asset.to_string())?;
        
        let reward_schedules = REWARD_SCHEDULES.may_load(
            deps.storage,
            (&lp_token, &asset_staker_info.asset.to_string()),
        )?.unwrap_or_default();
        
        compute_reward(
            &config,
            env.block.time.seconds(),
            &mut asset_state,
            reward_schedules,
        );
        compute_staker_reward(&mut asset_state, &mut asset_staker_info)?;

        transfer_msgs.push(
            build_transfer_token_to_user_msg(asset, sender.clone(), asset_staker_info.pending_reward)?
        );
        asset_staker_info.pending_reward = Uint128::zero();

        REWARD_STATES.save(
            deps.storage,
            &asset_staker_info.asset.to_string(),
            &asset_state,
        )?;

        ASSET_STAKER_INFO.save(
            deps.storage,
            (&lp_token, &sender, &asset_staker_info.asset.to_string()),
            &asset_staker_info,
        )?;
    }


    Ok(Response::default().add_messages(transfer_msgs))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::UnclaimedRewards { lp_token, user } => {
            let assets_for_lp = LP_ACTIVE_REWARD_ASSETS
                .may_load(deps.storage, &lp_token)?
                .unwrap_or_default();
            
            
            let mut unclaimed_rewards: Vec<UnclaimedReward> = vec![];
            for asset in assets_for_lp {
                let asset_staker_info = ASSET_STAKER_INFO
                    .may_load(deps.storage, (&lp_token, &user, &asset.to_string()))?
                    .unwrap_or(AssetStakerInfo {
                        asset: asset.clone(),
                        bond_amount: Uint128::zero(),
                        pending_reward: Uint128::zero(),
                        reward_index: Decimal::zero(),
                    });

                unclaimed_rewards.push(UnclaimedReward {
                    asset: asset.clone(),
                    amount: asset_staker_info.pending_reward,
                });
            }
            to_binary(&unclaimed_rewards)
        }
    }
}
