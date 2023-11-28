use std::collections::HashMap;

use crate::error::ContractError;
use crate::state::LOCKED_TOKENS;
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo,
    Response, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;
use dexter::asset::{Asset, AssetInfo};
use dexter::helper::{build_send_native_asset_msg, build_transfer_cw20_from_user_msg};
use dexter::superfluid_lp::{ExecuteMsg, InstantiateMsg, QueryMsg};
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
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::LockedLstForUser { user, asset } => {
            let locked_tokens: Uint128 = LOCKED_TOKENS
                .may_load(_deps.storage, (&user, &asset.info.to_string()))?
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
        ExecuteMsg::LockLstAssetForUser { asset, user } => {
            // validate that the sender is the PSTAKE issuance module on the Persistence chain i.e. lscosmos module.
            if info.sender != "persistence15uvj9phxl275x2yggyp2q4kalvhaw85syqnacq" {
                return Err(ContractError::Unauthorized);
            }

            let locked_tokens: Uint128 = LOCKED_TOKENS
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

            // We can do this by doing a bank module query for the balance
            let locked_tokens = locked_tokens.checked_add(asset.amount).unwrap();
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
            let vault_addr = "vault".to_string();
            join_pool_and_bond_using_locked_lst(
                deps,
                env,
                info,
                vault_addr,
                pool_id,
                total_assets,
                min_lp_to_receive,
            )
        }
        ExecuteMsg::DirectlyUnlockBaseLst { asset: _ } => {
            Err(ContractError::NotImplemented)
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

    // check if the user has enough balance to join the pool.
    for asset in &total_assets {
        let total_amount_to_spend = asset.amount;

        match &asset.info {
            AssetInfo::NativeToken { denom } => {
                let amount_sent = funds_map.get(denom).cloned().unwrap_or(Uint128::zero());
                // if amount sent is already bigger than the amount to spend, then we don't need to unlock any tokens for the user.
                // we can return any extra tokens back to the user.

                if amount_sent >= total_amount_to_spend {
                    let extra_amount = amount_sent.checked_sub(total_amount_to_spend).unwrap();
                    let msg =
                        build_send_native_asset_msg(info.sender.clone(), &denom, extra_amount)?;
                    msgs.push(msg);
                }
                // else check if we need to unlock some amount for the user.
                else {
                    let amount_to_unlock = total_amount_to_spend.checked_sub(amount_sent).unwrap();
                    let locked_amount = LOCKED_TOKENS
                        .may_load(deps.storage, (&info.sender, &asset.info.to_string()))?
                        .unwrap_or_default();

                    let total_spendable_amount = amount_sent + locked_amount;

                    // we can spend upto the total spendable amount for the user.
                    if total_amount_to_spend > total_spendable_amount {
                        return Err(ContractError::InsufficientBalance {
                            denom: denom.clone(),
                            available_balance: total_spendable_amount,
                            required_balance: total_amount_to_spend,
                        });
                    }

                    // update the locked token amount for the user
                    LOCKED_TOKENS.save(
                        deps.storage,
                        (&info.sender, &asset.info.to_string()),
                        &(locked_amount - amount_to_unlock),
                    )?;
                }
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
            funds: vec![],
        });

        msgs.push(wasm_msg);
    }

    response = response.add_messages(msgs);
    return Ok(response);
}