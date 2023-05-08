use crate::{contract::{update_staking_rewards, CONTRACT_NAME, ContractResult}, state::USER_LP_TOKEN_LOCKS, utils::{calculate_unlock_fee, find_lock_difference}};
use const_format::concatcp;
use cosmwasm_std::{
    Addr, DepsMut, Env, Event,
    MessageInfo, Response, Uint128,
};

use dexter::{
    asset::AssetInfo,
    helper::{
        build_transfer_token_to_user_msg, build_transfer_cw20_token_msg,
    },
    multi_staking::{
        Config, TokenLock,
    },
};


use dexter::helper::EventExt;

use crate::{
    error::ContractError,
    state::{
        CONFIG, LP_GLOBAL_STATE, USER_BONDED_LP_TOKENS,
    },
};

// Instant unlock is a extension of instant unbonding feature which allows to insantly unbond tokens
/// which are in a locked state post normal unbonding.
/// This is useful when a user mistakenly unbonded the tokens instead of instant unbonding or if a black swan event
/// occurs and the user has the LP tokens in a locked state after unbonding.
/// Penalty fee is same as instant unbonding.
pub fn instant_unlock(
    deps: DepsMut,
    env: &Env,
    info: &MessageInfo,
    lp_token: &Addr,
    token_locks: Vec<TokenLock>,
) -> ContractResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    let user = info.sender.clone();
    let locks = USER_LP_TOKEN_LOCKS
        .may_load(deps.storage, (&lp_token, &user))?
        .unwrap_or_default();
    
    let (final_locks_after_unlocking, valid_locks_to_be_unlocked) = find_lock_difference(locks.clone(), token_locks.clone());

    let total_amount = valid_locks_to_be_unlocked
        .iter()
        .fold(Uint128::zero(), |acc, lock| acc + lock.amount);

    let mut total_amount_to_be_unlocked = Uint128::zero();
    let mut total_fee_charged = Uint128::zero();

    let current_block_time = env.block.time.seconds();
    for lock in valid_locks_to_be_unlocked.iter() {
        let (_, unlock_fee) = calculate_unlock_fee(lock, current_block_time, &config);
        total_amount_to_be_unlocked += lock.amount.checked_sub(unlock_fee)?;
        total_fee_charged += unlock_fee;
    }

    USER_LP_TOKEN_LOCKS.save(deps.storage, (&lp_token, &user), &final_locks_after_unlocking)?;

    let fee_recipient = config.keeper.unwrap_or(config.owner);
    
    let mut response = Response::new().add_event(
        Event::from_sender(concatcp!(CONTRACT_NAME, "::instant_unlock"), user.clone())
        .add_attribute("lp_token", lp_token.clone())
        .add_attribute("amount", total_amount)
        .add_attribute("fee", total_fee_charged)
        .add_attribute("fee_recipient", fee_recipient.clone()),
    );

    // transfer amount to user
    response = response.add_message(build_transfer_cw20_token_msg(
        user.clone(),
        lp_token.to_string(),
        total_amount_to_be_unlocked,
    )?);

    // transfer fee to keeper if set else to the contract owner
    response = response.add_message(build_transfer_cw20_token_msg(
        fee_recipient,
        lp_token.to_string(),
        total_fee_charged,
    )?);
    
    Ok(response)
}