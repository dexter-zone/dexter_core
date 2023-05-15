use crate::{contract::{CONTRACT_NAME, ContractResult}, state::USER_LP_TOKEN_LOCKS, utils::{calculate_unlock_fee, find_lock_difference}};
use const_format::concatcp;
use cosmwasm_std::{
    Addr, DepsMut, Env, Event,
    MessageInfo, Response, Uint128, CosmosMsg, WasmMsg, to_binary,
};

use cw20::Cw20ExecuteMsg;
use dexter::{
    helper::build_transfer_cw20_token_msg,
    multi_staking::{
        TokenLock,
    },
};


use dexter::helper::EventExt;
use crate::state::CONFIG;

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
        .add_attribute("fee_recipient", fee_recipient.clone())
        .add_attribute("locks", serde_json_wasm::to_string(&token_locks).unwrap())
    );


    println!("total_amount_to_be_unlocked: {}", total_amount_to_be_unlocked);
    println!("total_fee_charged: {}", total_fee_charged);

    println!("fee recipient: {}", fee_recipient);

    
    println!("total_amount_to_be_unlocked: {}", total_amount_to_be_unlocked);
    println!("total_fee_charged: {}", total_fee_charged);

    println!("fee recipient: {}", fee_recipient);

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

pub fn unlock(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_token: Addr,
) -> ContractResult<Response> {
    let locks = USER_LP_TOKEN_LOCKS
        .may_load(deps.storage, (&lp_token, &info.sender))?
        .unwrap_or_default();

    let unlocked_locks = locks
        .iter()
        .filter(|lock| lock.unlock_time <= env.block.time.seconds())
        .cloned()
        .collect::<Vec<TokenLock>>();

    let total_unlocked_amount = unlocked_locks
        .iter()
        .fold(Uint128::zero(), |acc, lock| acc + lock.amount);

    let updated_locks = locks
        .into_iter()
        .filter(|lock| lock.unlock_time > env.block.time.seconds())
        .collect::<Vec<TokenLock>>();

    
    let mut response = Response::new().add_event(
        Event::from_info(concatcp!(CONTRACT_NAME, "::unlock"), &info)
        .add_attribute("lp_token", lp_token.clone())
        .add_attribute("amount", total_unlocked_amount)
        .add_attribute("unlocked_locks", serde_json_wasm::to_string(&unlocked_locks).unwrap())
        .add_attribute("updated_locks", serde_json_wasm::to_string(&updated_locks).unwrap())
    );
    
    if total_unlocked_amount.is_zero() {
        return Ok(response);
    }
    
    USER_LP_TOKEN_LOCKS.save(deps.storage, (&lp_token, &info.sender), &updated_locks)?;
    response = response.add_message(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_token.to_string(),
        funds: vec![],
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: info.sender.to_string(),
            amount: total_unlocked_amount,
        })?,
    }));

    Ok(response)
}