use std::collections::HashMap;
use std::vec;

use crate::error::ContractError;
use crate::state::{CONFIG, OWNERSHIP_PROPOSAL, LOCK_AMOUNT};
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo,
    Response, Uint128, WasmMsg, StdError, Coin,
};
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;
use dexter::asset::{Asset, AssetInfo};
use dexter::helper::{build_transfer_cw20_from_user_msg, propose_new_owner, claim_ownership, drop_ownership_proposal, build_transfer_token_to_user_msg};
use dexter::superfluid_lp::{ExecuteMsg, InstantiateMsg, QueryMsg, Config};
use dexter::vault::ExecuteMsg as VaultExecuteMsg;

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter-superfluid-lp";
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

    // validate vault address
    deps.api.addr_validate(&msg.vault_addr.to_string())?;

    // validate owner address
    deps.api.addr_validate(&msg.owner.to_string())?;

    let mut all_denoms = vec![];
    // validate that all assets are native tokens
    for allowed_asset in &msg.allowed_lockable_tokens {
        // validate that no token is a cw20 token.
        match &allowed_asset {
            AssetInfo::NativeToken { denom } => {
                // check no duplicate denoms
                if all_denoms.contains(denom) {
                   // reject duplicate denoms
                     return Err(ContractError::DuplicateDenom); 
                }
                all_denoms.push(denom.clone()); 
            }
            AssetInfo::Token { contract_addr: _ } => {
                // we don't support cw20 tokens for now.
                return Err(ContractError::UnsupportedAssetType)
            }
        }
    }

    let config = Config {
        vault_addr: msg.vault_addr,
        owner: msg.owner,
        allowed_lockable_tokens: msg.allowed_lockable_tokens,
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::TotalAmountLocked { user, asset_info } => {
            
            let locked_amount = LOCK_AMOUNT
                .may_load(_deps.storage, (&user, &asset_info.to_string()))?
                .unwrap_or_default();

            Ok(to_json_binary(&locked_amount)?)
        },

        QueryMsg::Config {} => {
            let config = CONFIG.load(_deps.storage)?;
            Ok(to_json_binary(&config)?)
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
        ExecuteMsg::LockLstAsset { asset} => {

            let user = info.sender.clone();

            // validate that the asset is allowed to be locked.
            let config = CONFIG.load(deps.storage)?;
            let mut allowed = false;
            for allowed_asset in config.allowed_lockable_tokens {
                if allowed_asset == asset.info {
                    allowed = true;
                    break;
                }
            }

            if !allowed {
                return Err(ContractError::AssetNotAllowedToBeLocked);
            }

            let mut locked_amount: Uint128 = LOCK_AMOUNT
                .may_load(deps.storage, (&user, &asset.info.to_string()))?
                .unwrap_or_default();
       
            // confirm that this asset was sent along with the message. We only support native assets.
            match &asset.info {
                AssetInfo::NativeToken { denom } => {
                    let amount =  cw_utils::must_pay(&info, denom).map_err(|e| ContractError::PaymentError(e))?;
                    if amount != asset.amount {
                        return Err(ContractError::InvalidAmount);
                    }
                }
                AssetInfo::Token { contract_addr: _ } => {
                    return Err(ContractError::UnsupportedAssetType);
                }
            }

             // add the amount to the locked amount
            locked_amount = locked_amount + asset.amount;

            // update locked amount
            LOCK_AMOUNT.save(deps.storage, (&user, &asset.info.to_string()), &locked_amount)?;
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

        ExecuteMsg::UpdateConfig { vault_addr } => {
            // validate that the message sender is the owner of the contract.
            if info.sender != CONFIG.load(deps.storage)?.owner {
                return Err(ContractError::Unauthorized {});
            }

            let mut config = CONFIG.load(deps.storage)?;

            if let Some(vault_addr) = vault_addr {
                // validate vault address
                deps.api.addr_validate(&vault_addr.to_string())?;
                config.vault_addr = vault_addr;
            }

            CONFIG.save(deps.storage, &config)?;
            Ok(Response::default())
        },

        ExecuteMsg::AddAllowedLockableToken { asset_info } => {
            // validate that the message sender is the owner of the contract.
            if info.sender != CONFIG.load(deps.storage)?.owner {
                return Err(ContractError::Unauthorized {});
            }

            // validate that the token is native
            match &asset_info {
                AssetInfo::NativeToken { denom: _ } => {}
                AssetInfo::Token { contract_addr: _ } => {
                    return Err(ContractError::UnsupportedAssetType);
                }
            }

            let mut config = CONFIG.load(deps.storage)?;

            // validate that the token is not already in the list of allowed lockable tokens.
            for allowed_asset in &config.allowed_lockable_tokens {
                if allowed_asset == &asset_info {
                    return Err(ContractError::AssetAlreadyAllowedToBeLocked);
                }
            }

            config.allowed_lockable_tokens.push(asset_info);

            CONFIG.save(deps.storage, &config)?;
            Ok(Response::default())
        }

        ExecuteMsg::RemoveAllowedLockableToken { asset_info } => {
            // validate that the message sender is the owner of the contract.
            if info.sender != CONFIG.load(deps.storage)?.owner {
                return Err(ContractError::Unauthorized {});
            }

            let mut config = CONFIG.load(deps.storage)?;

            // validate that the token is in the list of allowed lockable tokens.
            let mut found = false;
            for (i, allowed_asset) in config.allowed_lockable_tokens.iter().enumerate() {
                if allowed_asset == &asset_info {
                    found = true;
                    config.allowed_lockable_tokens.remove(i);
                    break;
                }
            }

            if !found {
                return Err(ContractError::AssetNotInAllowedList);
            }

            CONFIG.save(deps.storage, &config)?;
            Ok(Response::default())
        }

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
        },

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

    let mut unspent_sent_funds = funds_map.clone();

    let mut coins = vec![];

    // check if the user has enough balance to join the pool.
    for asset in &total_assets {
        let total_amount_to_spend = asset.amount;

        match &asset.info {
            AssetInfo::NativeToken { denom } => {
                let amount_sent = funds_map.get(denom).cloned().unwrap_or(Uint128::zero());
                
                if amount_sent == total_amount_to_spend {
                    // do nothing
                    unspent_sent_funds.remove(denom);
                }
                // if amount sent is already bigger than the amount to spend, then we don't need to unlock any tokens for the user.
                // we can return any extra tokens back to the user.
                else if amount_sent > total_amount_to_spend {
                    let extra_amount = amount_sent.checked_sub(total_amount_to_spend).unwrap();
                    unspent_sent_funds.insert(denom.clone(), extra_amount);
                }
                // else check if we need to unlock some amount for the user.
                else {

                    // remove from the unspent funds map.
                    unspent_sent_funds.remove(denom);

                    let amount_to_unlock = total_amount_to_spend.checked_sub(amount_sent).unwrap();

                    let mut locked_amount = LOCK_AMOUNT
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

                    locked_amount = locked_amount.checked_sub(amount_to_unlock)?;

                    LOCK_AMOUNT.save(
                        deps.storage,
                        (&info.sender, &asset.info.to_string()),
                        &locked_amount,
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
                let msg = Cw20ExecuteMsg::IncreaseAllowance {
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

    // return unspent funds back to the user.
    for (denom, amount) in unspent_sent_funds {
        let msg = build_transfer_token_to_user_msg(
            AssetInfo::NativeToken { denom: denom.clone() },
            info.sender.clone(),
            amount,
        )?;

        msgs.push(msg);
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