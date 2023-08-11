use crate::{
    contract::{update_staking_rewards, ContractResult, CONTRACT_NAME},
    state::USER_LP_TOKEN_LOCKS,
};
use const_format::concatcp;
use cosmwasm_std::{Addr, DepsMut, Env, Event, MessageInfo, Response, Uint128};

use dexter::{
    asset::AssetInfo,
    helper::build_transfer_token_to_user_msg,
    multi_staking::{Config, TokenLock, MAX_USER_LP_TOKEN_LOCKS},
};

use dexter::helper::EventExt;

use crate::{
    error::ContractError,
    state::{CONFIG, LP_GLOBAL_STATE, USER_BONDED_LP_TOKENS},
};

/// Allows to instantly unbond LP tokens without waiting for the unlock period
/// This is a special case and should only be used in emergencies like a black swan event or a hack.
/// The user will pay a penalty fee in the form of a percentage of the unbonded amount which will be
/// sent to the keeper i.e. protocol treasury.
pub fn instant_unbond(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_token: Addr,
    amount: Uint128,
) -> ContractResult<Response> {
    let mut response = Response::new();

    if amount.is_zero() {
        return Err(ContractError::ZeroAmount);
    }

    let current_bond_amount = USER_BONDED_LP_TOKENS
        .may_load(deps.storage, (&lp_token, &info.sender))?
        .unwrap_or_default();

    let config: Config = CONFIG.load(deps.storage)?;
    let mut lp_global_state = LP_GLOBAL_STATE.load(deps.storage, &lp_token)?;

    let user_updated_bond_amount = current_bond_amount.checked_sub(amount).map_err(|_| {
        ContractError::CantUnbondMoreThanBonded {
            amount_to_unbond: amount,
            current_bond_amount,
        }
    })?;

    for asset in &lp_global_state.active_reward_assets {
        update_staking_rewards(
            asset,
            &lp_token,
            &info.sender,
            lp_global_state.total_bond_amount,
            current_bond_amount,
            env.block.time.seconds(),
            &mut deps,
            &mut response,
            None,
        )?;
    }

    // Decrease bond amount
    lp_global_state.total_bond_amount = lp_global_state.total_bond_amount.checked_sub(amount)?;
    LP_GLOBAL_STATE.save(deps.storage, &lp_token, &lp_global_state)?;

    USER_BONDED_LP_TOKENS.save(
        deps.storage,
        (&lp_token, &info.sender),
        &user_updated_bond_amount,
    )?;

    // whole instant unbond fee is sent to the keeper as protocol treasury
    let instant_unbond_fee = amount.multiply_ratio(config.instant_unbond_fee_bp, Uint128::from(10000u128));

    // Check if the keeper is available, if not, send the fee to the contract owner
    let fee_receiver = config.keeper;

    // Send the instant unbond fee to the keeper as protocol treasury
    let fee_msg = build_transfer_token_to_user_msg(
        AssetInfo::token(lp_token.clone()),
        fee_receiver,
        instant_unbond_fee,
    )?;
    response = response.add_message(fee_msg);

    // Send the unbonded amount to the user
    let unbond_msg = build_transfer_token_to_user_msg(
        AssetInfo::token(lp_token.clone()),
        info.sender.clone(),
        amount.checked_sub(instant_unbond_fee)?,
    )?;
    response = response.add_message(unbond_msg);

    let event = Event::from_info(concatcp!(CONTRACT_NAME, "::instant_unbond"), &info)
        .add_attribute("lp_token", lp_token)
        .add_attribute("amount", amount)
        .add_attribute("total_bond_amount", lp_global_state.total_bond_amount)
        .add_attribute("user_updated_bond_amount", user_updated_bond_amount)
        .add_attribute("instant_unbond_fee", instant_unbond_fee)
        .add_attribute("user_withdrawn_amount", amount.checked_sub(instant_unbond_fee)?);

    response = response.add_event(event);
    Ok(response)
}

/// Unbond LP tokens
pub fn unbond(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_token: Addr,
    amount: Option<Uint128>,
) -> ContractResult<Response> {
    // We don't have to check for LP token allowed here, because there's a scenario that we allowed bonding
    // for an asset earlier and then we remove the LP token from the list of allowed LP tokens. In this case
    // we still want to allow unbonding.
    let mut response = Response::new();

    let current_bond_amount = USER_BONDED_LP_TOKENS
        .may_load(deps.storage, (&lp_token, &info.sender))?
        .unwrap_or_default();

    // if user didn't explicitly mention any amount, unbond everything.
    let amount = amount.unwrap_or(current_bond_amount);
    if amount.is_zero() {
        return Err(ContractError::ZeroAmount);
    }

    let mut lp_global_state = LP_GLOBAL_STATE.load(deps.storage, &lp_token)?;
    for asset in &lp_global_state.active_reward_assets {
        update_staking_rewards(
            asset,
            &lp_token,
            &info.sender,
            lp_global_state.total_bond_amount,
            current_bond_amount,
            env.block.time.seconds(),
            &mut deps,
            &mut response,
            None,
        )?;
    }

    // Decrease bond amount
    lp_global_state.total_bond_amount = lp_global_state.total_bond_amount.checked_sub(amount)?;
    LP_GLOBAL_STATE.save(deps.storage, &lp_token, &lp_global_state)?;

    let user_updated_bond_amount = current_bond_amount.checked_sub(amount).map_err(|_| {
        ContractError::CantUnbondMoreThanBonded {
            amount_to_unbond: amount,
            current_bond_amount,
        }
    })?;

    USER_BONDED_LP_TOKENS.save(
        deps.storage,
        (&lp_token, &info.sender),
        &user_updated_bond_amount,
    )?;

    // Start unlocking clock for the user's LP Tokens
    let mut unlocks = USER_LP_TOKEN_LOCKS
        .may_load(deps.storage, (&lp_token, &info.sender))?
        .unwrap_or_default();

    if unlocks.len() == MAX_USER_LP_TOKEN_LOCKS {
        return Err(ContractError::CantAllowAnyMoreLpTokenUnbonds);
    }

    let config = CONFIG.load(deps.storage)?;

    let unlock_time = env.block.time.seconds() + config.unlock_period;
    unlocks.push(TokenLock {
        unlock_time,
        amount,
    });

    USER_LP_TOKEN_LOCKS.save(deps.storage, (&lp_token, &info.sender), &unlocks)?;

    let event = Event::from_info(concatcp!(CONTRACT_NAME, "::unbond"), &info)
        .add_attribute("lp_token", lp_token)
        .add_attribute("amount", amount)
        .add_attribute("total_bond_amount", lp_global_state.total_bond_amount)
        .add_attribute("user_updated_bond_amount", user_updated_bond_amount)
        .add_attribute("unlock_time", unlock_time.to_string());

    response = response.add_event(event);
    Ok(response)
}
