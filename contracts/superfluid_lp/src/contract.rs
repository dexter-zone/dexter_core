use std::collections::HashMap;

use crate::error::ContractError;
use crate::state::{LOCKED_TOKENS, CONFIG, OWNERSHIP_PROPOSAL};
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo,
    Response, Uint128, WasmMsg, StdError, Coin,
};
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;
use dexter::asset::{Asset, AssetInfo};
use dexter::helper::{build_send_native_asset_msg, build_transfer_cw20_from_user_msg, build_transfer_token_to_user_msg, propose_new_owner, claim_ownership, drop_ownership_proposal};
use dexter::superfluid_lp::{ExecuteMsg, InstantiateMsg, QueryMsg, LockInfo, Config};
use dexter::vault::ExecuteMsg as VaultExecuteMsg;

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter-router";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x      Instantiate Contract : Execute function     x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        base_lock_period: msg.base_lock_period,
        vault_addr: msg.vault_addr,
        owner: msg.owner,
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::LockedLstForUser { user, asset } => {
            let locked_tokens: Vec<LockInfo> = LOCKED_TOKENS
                .may_load(_deps.storage, (&user, &asset.info.to_string()))?
                .unwrap_or_default();

            // sum all the locked tokens
            let mut total_locked_amount = Uint128::zero();

            for lock in locked_tokens {
                total_locked_amount = total_locked_amount + lock.amount;
            }

            Ok(to_json_binary(&total_locked_amount)?)
        },

        QueryMsg::TotalAmountLocked { user, asset_info } => {
            let locked_tokens: Vec<LockInfo> = LOCKED_TOKENS
                .may_load(_deps.storage, (&user, &asset_info.to_string()))?
                .unwrap_or_default();

            // sum all the locked tokens
            let mut total_locked_amount = Uint128::zero();

            for lock in locked_tokens {
                total_locked_amount = total_locked_amount + lock.amount;
            }

            Ok(to_json_binary(&total_locked_amount)?)
        },

        QueryMsg::UnlockedAmount { user, asset_info } => {
            let locked_tokens: Vec<LockInfo> = LOCKED_TOKENS
                .may_load(_deps.storage, (&user, &asset_info.to_string()))?
                .unwrap_or_default();

            // sum all the locked tokens
            let mut total_locked_amount = Uint128::zero();

            for lock in locked_tokens {
                if lock.unlock_time <= env.block.time.seconds() {
                    total_locked_amount = total_locked_amount + lock.amount;
                }
            }

            Ok(to_json_binary(&total_locked_amount)?)
        },

        QueryMsg::TokenLocks { user, asset_info } => {
            let locked_tokens: Vec<LockInfo> = LOCKED_TOKENS
                .may_load(_deps.storage, (&user, &asset_info.to_string()))?
                .unwrap_or_default();

            Ok(to_json_binary(&locked_tokens)?)
        }
    
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::LockLstAssetForUser { asset, user} => {

            let user = info.sender.clone();
            let mut locked_tokens: Vec<LockInfo> = LOCKED_TOKENS
                .may_load(deps.storage, (&user, &asset.info.to_string()))?
                .unwrap_or_default();
       
            // confirm that this asset was sent along with the message. We only support native assets.
            match &asset.info {
                AssetInfo::NativeToken { denom } => {
                    for coin in info.funds.iter() {
                        if coin.denom == *denom {
                            // validate that the amount sent is exactly equal to the amount that is expected to be locked.
                            if coin.amount != asset.amount {
                                return Err(ContractError::InvalidAmount);
                            }
                        }
                    }
                }
                AssetInfo::Token { contract_addr: _ } => {
                    return Err(ContractError::UnsupportedAssetType);
                }
            }

            // add another lock to the list of locks for the user.
            let lock_info = LockInfo {
                amount: asset.amount,
                unlock_time: env.block.time.seconds() + 86400 * 7,
            };

            locked_tokens.push(lock_info);

            LOCKED_TOKENS.save(
                deps.storage,
                (&user, &asset.info.to_string()),
                &locked_tokens,
            )?;

            Ok(Response::default())
        }

        ExecuteMsg::JoinPoolAndBondUsingLockedLst {
            pool_id,
            total_assets,
            min_lp_to_receive,
        } => {
            let config = CONFIG.load(deps.storage)?;

            join_pool_and_bond_using_locked_lst(
                deps,
                env,
                info,
                config.vault_addr.to_string(),
                pool_id,
                total_assets,
                min_lp_to_receive,
            )
        }
        ExecuteMsg::DirectlyUnlockBaseLst { asset } => {
            // check for the locks that have finished their lock period and unlock them.
            let user = info.sender.clone();
            let locked_tokens: Vec<LockInfo> = LOCKED_TOKENS
                .may_load(deps.storage, (&user, &asset.info.to_string()))?
                .unwrap_or_default();

            let mut new_locked_tokens: Vec<LockInfo> = vec![];

            let mut total_amount_to_unlock = Uint128::zero();
            for lock in locked_tokens {
                if lock.unlock_time <= env.block.time.seconds() {
                    // unlock the tokens
                    total_amount_to_unlock = total_amount_to_unlock + lock.amount;
                } else {
                    new_locked_tokens.push(lock);
                }
            }

            LOCKED_TOKENS.save(
                deps.storage,
                (&user, &asset.info.to_string()),
                &new_locked_tokens,
            )?;

            // send the unlocked tokens back to the user.
            let msg = build_transfer_token_to_user_msg(
                asset.info.clone(),
                user,
                total_amount_to_unlock,
            )?;

            Ok(Response::default().add_message(msg))
        },
        ExecuteMsg::UpdateConfig { base_lock_period, vault_addr } => {
            // validate that the message sender is the owner of the contract.
            if info.sender != CONFIG.load(deps.storage)?.owner {
                return Err(ContractError::Unauthorized {});
            }

            let mut config = CONFIG.load(deps.storage)?;

            if let Some(base_lock_period) = base_lock_period {
                config.base_lock_period = base_lock_period;
            }

            if let Some(vault_addr) = vault_addr {
                config.vault_addr = vault_addr;
            }

            CONFIG.save(deps.storage, &config)?;
            Ok(Response::default())
        },
        ExecuteMsg::ProposeNewOwner { owner, expires_in } => {
            let config = CONFIG.load(deps.storage)?;
            propose_new_owner(
                deps,
                info,
                env,
                owner.to_string(),
                expires_in,
                config.owner,
                OWNERSHIP_PROPOSAL,
                CONTRACT_NAME
            )
            .map_err(|e| e.into())
        },
        ExecuteMsg::DropOwnershipProposal {} => {
            let config = CONFIG.load(deps.storage)?;

            drop_ownership_proposal(deps, info, config.owner, OWNERSHIP_PROPOSAL, CONTRACT_NAME)
                .map_err(|e| e.into())
        }
        ExecuteMsg::ClaimOwnership {} => {
            claim_ownership(deps, info, env, OWNERSHIP_PROPOSAL, |deps, new_owner| {
                CONFIG.update::<_, StdError>(deps.storage, |mut v| {
                    v.owner = new_owner;
                    Ok(v)
                })?;

                Ok(())
            }, CONTRACT_NAME)
            .map_err(|e| e.into())
        }
    }
}

fn join_pool_and_bond_using_locked_lst(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    vault_addr: String,
    pool_id: Uint128,
    total_assets: Vec<Asset>,
    min_lp_to_receive: Option<Uint128>,
) -> Result<Response, ContractError> {
    let mut response = Response::new();
    let mut msgs = vec![];

    let funds = info.funds;
    let funds_map: HashMap<String, Uint128> = funds
        .into_iter()
        .map(|asset| (asset.denom, asset.amount))
        .collect();

    let mut coins = vec![];

    // check if the user has enough balance to join the pool.
    for asset in &total_assets {
        let total_amount_to_spend = asset.amount;

        match &asset.info {
            AssetInfo::NativeToken { denom } => {
                let amount_sent = funds_map.get(denom).cloned().unwrap_or(Uint128::zero());
                
                if amount_sent == total_amount_to_spend {
                    // do nothing
                }
                // if amount sent is already bigger than the amount to spend, then we don't need to unlock any tokens for the user.
                // we can return any extra tokens back to the user.
                else if amount_sent > total_amount_to_spend {
                    let extra_amount = amount_sent.checked_sub(total_amount_to_spend).unwrap();
                    let msg =
                        build_send_native_asset_msg(info.sender.clone(), &denom, extra_amount)?;
                    msgs.push(msg);
                }
                // else check if we need to unlock some amount for the user.
                else {
                    let amount_to_unlock = total_amount_to_spend.checked_sub(amount_sent).unwrap();
                    let token_locks = LOCKED_TOKENS
                        .may_load(deps.storage, (&info.sender, &asset.info.to_string()))?
                        .unwrap_or_default();

                    // sum all the locked tokens and check if the user has enough balance to unlock.
                    let total_locked_amount = token_locks
                        .iter()
                        .fold(Uint128::zero(), |acc, lock| acc + lock.amount);

                    let total_spendable_amount = amount_sent + total_locked_amount;

                    // we can spend upto the total spendable amount for the user.
                    if total_amount_to_spend > total_spendable_amount {
                        return Err(ContractError::InsufficientBalance {
                            denom: denom.clone(),
                            available_balance: total_spendable_amount,
                            required_balance: total_amount_to_spend,
                        });
                    }

                    // use from the newest to the oldest lock to unlock the tokens as much as needed
                    let mut amount_to_unlock = amount_to_unlock;

                    // sort the locks in reverse order of unlock time.
                    let mut token_locks = token_locks.clone();
                    token_locks.sort_by(|a, b| b.unlock_time.cmp(&a.unlock_time));

                    let mut new_token_locks: Vec<LockInfo> = vec![];

                    for lock in token_locks.iter().rev() {
                        if amount_to_unlock == Uint128::zero() {
                            new_token_locks.push(lock.clone());
                        }

                        if lock.amount > amount_to_unlock {
                            let remaining_amount = lock.amount.checked_sub(amount_to_unlock).unwrap();
                            let new_lock = LockInfo {
                                amount: remaining_amount,
                                unlock_time: lock.unlock_time,
                            };

                            new_token_locks.push(new_lock);

                            amount_to_unlock = Uint128::zero();
                        } else {
                            amount_to_unlock = amount_to_unlock.checked_sub(lock.amount).unwrap();
                            
                        }
                    }

                    // update the locked token amount for the user
                    LOCKED_TOKENS.save(
                        deps.storage,
                        (&info.sender, &asset.info.to_string()),
                        &new_token_locks,
                    )?;
                }

                // add to coins vec
                let coin = Coin {
                    denom: denom.clone(),
                    amount: total_amount_to_spend,
                };

                coins.push(coin);
            }
            AssetInfo::Token { contract_addr } => {
                // create a message to send the tokens from the user to the contract.
                let msg = build_transfer_cw20_from_user_msg(
                    contract_addr.to_string(),
                    info.sender.to_string(),
                    env.contract.address.to_string(),
                    asset.amount,
                )?;

                msgs.push(msg);

                // Add another message to allow spending of the tokens from the current contract to the vault.
                let msg = Cw20ExecuteMsg::DecreaseAllowance {
                    spender: vault_addr.clone(),
                    amount: asset.amount,
                    expires: None,
                };

                let wasm_msg: CosmosMsg<Empty> = CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_json_binary(&msg)?,
                    funds: vec![],
                });

                msgs.push(wasm_msg);
            }
        }

    }
    // create a message to join the pool.
    let join_pool_msg = VaultExecuteMsg::JoinPool {
        pool_id,
        recipient: Some(info.sender.to_string()),
        assets: Some(total_assets.clone()),
        min_lp_to_receive,
        auto_stake: Some(true),
    };

    let wasm_msg: CosmosMsg<Empty> = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: vault_addr.to_string(),
        msg: to_json_binary(&join_pool_msg)?,
        funds: coins
    });


    msgs.push(wasm_msg);

    response = response.add_messages(msgs);
    return Ok(response);

}