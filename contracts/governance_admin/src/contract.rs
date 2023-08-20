use std::str::FromStr;

#[cfg(not(feature = "library"))]
use crate::error::ContractError;
use crate::state::{CreatePoolTempData, CREATE_POOL_TEMP_DATA};
use crate::utils::query_gov_deposit_params;

use const_format::concatcp;
use cosmos_sdk_proto::Any;
use cosmos_sdk_proto::cosmos::gov::v1beta1::MsgSubmitProposal;
use cosmos_sdk_proto::cosmwasm::wasm::v1::ExecuteContractProposal;
use cosmos_sdk_proto::traits::MessageExt;
use cosmwasm_std::{
    entry_point, to_binary, Binary, CosmosMsg, Deps, DepsMut, Env, Event, MessageInfo, Response,
    StdError, StdResult, WasmMsg, Uint128, Coin, Addr,
};
use cw2::set_contract_version;
use dexter::asset::{self, Asset, AssetInfo};
use dexter::governance_admin::{ExecuteMsg, InstantiateMsg, QueryMsg};
use dexter::helper::{build_transfer_cw20_from_user_msg, EventExt, NO_PRIV_KEY_ADDR};
use dexter::querier::query_vault_config;
use dexter::vault::{ExecuteMsg as VaultExecuteMsg, PoolCreationFee};

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter-governance-admin";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CONTRACT_VERSION_V1: &str = "1.0.0";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new().add_event(Event::from_info(
        concatcp!(CONTRACT_NAME, "::instantiate"),
        &info,
    )))
}

// Sums up the requirements in terms of pool creation fee, pool bootstrapping amount and reward schedule
// amounts and returns it
// This can later be used to validate if the user has sent enough funds to create the pool and
// transfer Cw20 token to this contract for further processing
pub fn find_total_funds_needed(
    deps: Deps,
    pool_creation_request_proposal: &dexter::governance_admin::PoolCreationRequest,
) -> Result<Vec<Asset>, ContractError> {

    // let mut total_funds = vec![];
    let mut total_funds_map = std::collections::HashMap::new();
    let vault_addr = deps.api.addr_validate(&pool_creation_request_proposal.vault_addr).unwrap();
    
    // find the pool creation fee by querying the vault contract currently
    let vault_config = query_vault_config(&deps.querier, vault_addr.to_string())?;
    let pool_creation_fee = vault_config.pool_creation_fee;

    // add the proposal deposit to the total funds.
    // We need to query the gov module to figure this out   
    let deposit_params = query_gov_deposit_params(&deps.querier)?;
    let proposal_deposit = deposit_params.min_deposit;

    for coin in proposal_deposit {
        let asset_info = AssetInfo::native_token(coin.denom);
        let amount: Uint128 = total_funds_map.get(&asset_info).cloned().unwrap_or_default();
        let c_amount = Uint128::from_str(&coin.amount).unwrap();
        total_funds_map.insert(asset_info, amount.checked_add(c_amount)?);
    }

    // add the pool creation fee to the total funds
    if let PoolCreationFee::Enabled { fee } = pool_creation_fee {
        let amount = total_funds_map.get(&fee.info).cloned().unwrap_or_default();
        total_funds_map.insert(fee.info, amount.checked_add(fee.amount)?);
    }

    // add the bootstrapping amount to the total funds
    if let Some(bootstrapping_amount) = &pool_creation_request_proposal.bootstrapping_amount {
        for asset in bootstrapping_amount {
            let amount = total_funds_map.get(&asset.info).cloned().unwrap_or_default();
            total_funds_map.insert(asset.info.clone(), amount.checked_add(asset.amount)?);
        }
    }

    // add the reward schedule amounts to the total funds
    if let Some(reward_schedules) = &pool_creation_request_proposal.reward_schedules {
        for reward_schedule in reward_schedules {
            let amount = total_funds_map.get(&reward_schedule.asset).cloned().unwrap_or_default();
            total_funds_map.insert(reward_schedule.asset.clone(), amount.checked_add(reward_schedule.amount)?);
        }
    }

    let total_funds = total_funds_map.into_iter().map(|(k, v)| Asset { info: k, amount: v }).collect();
    Ok(total_funds)
}

/// Validates if the funds sent by the user are enough to create the pool and other operations
/// and if yes, then transfers the funds to this contract in case of CW20 tokens since they are not sent along with the message
/// In case of native tokens, the extra funds are returned back to the user
pub fn validate_or_transfer_assets(
    deps: Deps,
    env: Env,
    sender: &Addr,
    pool_creation_request_proposal: &dexter::governance_admin::PoolCreationRequest,
    funds: Vec<Coin>
) -> Result<Vec<CosmosMsg>, ContractError> {
    // find total needed first
    let total_funds_needed = find_total_funds_needed(deps, pool_creation_request_proposal)?;
    let mut messages = vec![];

    // validate that the funds sent are enough for native assets
    let funds_map = funds.into_iter().map(|c| (c.denom, c.amount)).collect::<std::collections::HashMap<String, Uint128>>();
    for asset in total_funds_needed {
        match asset.info {
            AssetInfo::NativeToken { denom } => {
                let amount = funds_map.get(&denom).cloned().unwrap_or_default();
                // TODO: return the extra funds back to the user
                if amount < asset.amount {
                    panic!("Insufficient funds sent for native asset {}", denom);
                }
            },
            AssetInfo::Token { contract_addr } => {
                // check if the contract has enough allowance to spend the funds
                let spend_limit = AssetInfo::query_spend_limits(
                    &contract_addr,
                    sender,
                    &deps.api.addr_validate(&env.contract.address.to_string()).unwrap(),
                    &deps.querier,
                ).unwrap();

                if asset.amount > spend_limit {
                    panic!("Insufficient spend limit cw20 asset {}", contract_addr);
                }

                // transfer the funds from the user to this contract
                let transfer_msg = build_transfer_cw20_from_user_msg(
                    contract_addr.to_string(),
                    sender.to_string(),
                    env.contract.address.to_string(),
                    asset.amount,
                ).unwrap();

                // add the message to the list of messages
                messages.push(transfer_msg);
            }
        }
    }

    Ok(messages)

}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    if info.sender.to_string() != NO_PRIV_KEY_ADDR {
        return Err(ContractError::Unauthorized {});
    }

    match msg {
        ExecuteMsg::ExecuteMsgs { msgs } => {
            // validate that all funds were sent along with the message. Ideally this contract should not hold any funds.
            let mut res = Response::new();
            let mut event = Event::from_info(concatcp!(CONTRACT_NAME, "::execute_msgs"), &info);
            // log if this part of a transaction or not
            event = match env.transaction {
                None => event.add_attribute("tx", "none"),
                Some(tx) => event.add_attribute("tx", tx.index.to_string()),
            };
            res = res.add_messages(msgs).add_event(event);
            Ok(res)
        },

        ExecuteMsg::CreatePoolCreationProposal {
            title,
            description,
            pool_creation_request
        } => {

            // first order of business, ensure the money is sent along with the message
            let mut messages = validate_or_transfer_assets(
                deps.as_ref(),
                env.clone(),
                &info.sender,
                &pool_creation_request,
                info.funds
            )?;

            let execute_contract_proposal_content = ExecuteContractProposal { 
                title,
                description,
                run_as: env.contract.address.to_string(),
                contract: env.contract.address.to_string(),
                msg: to_binary(&dexter::governance_admin::ExecuteMsg::ResumeCreatePool { 
                    pool_creation_request_id: Uint128::from(1u128)
                })?.to_vec(),
                funds: vec![],
            };

            let proposal_content = execute_contract_proposal_content.to_any()
                .map_err(|_| ContractError::Std(StdError::generic_err("failed to serialize proposal content")))?;

            // we'll create a proposal to create a pool
            let proposal_msg = MsgSubmitProposal { 
                content: Some(proposal_content), 
                initial_deposit: vec![],
                proposer: env.contract.address.to_string(),
            };

            // add a message to return callback to the contract post proposal creation so we can find the
            // proposal id of the proposal we just created. This can be just found by querying the latest proposal id
            // and doing a verification on the proposal content


            

            panic!("not implemented")
        },
        ExecuteMsg::PostGovernanceProposalCreationCallback { proposal_creation_request_id } => {
            panic!("not implemented")
        }
        ExecuteMsg::ResumeCreatePool { pool_creation_request_id } => {
            panic!("not implemented")
        },
        ExecuteMsg::ResumeJoinPool { pool_creation_request_id } => {
            panic!("not implemented")
        }
        ExecuteMsg::CreateNewPool {
            vault_addr,
            bootstrapping_amount_payer,
            pool_type,
            fee_info,
            native_asset_precisions,
            assets,
            init_params,
        } => {
            let vault_addr = deps.api.addr_validate(&vault_addr)?;

            let bootstrapping_amount_payer_addr =
                deps.api.addr_validate(&bootstrapping_amount_payer)?;
            let mut messages = vec![];
            // validate that all funds were sent along with the message. Ideally this contract should not hold any funds.

            // for native assets funds should be sent along and
            // for CW20 assets, permission must be given to governance admin contract to spend these funds by the proposal creator.
            for asset in &assets {
                match &asset.info {
                    asset::AssetInfo::NativeToken { denom } => {
                        // check if funds were sent along
                        let sent_amount = info
                            .funds
                            .iter()
                            .find(|c| c.denom == denom.clone())
                            .map(|c| c.amount)
                            .unwrap_or_default();

                        // validate sent amount with amount needed
                        if asset.amount != sent_amount {
                            return Err(ContractError::InsufficientBalance);
                        }

                        // validate the
                    }
                    asset::AssetInfo::Token { contract_addr } => {
                        // add a message to transfer funds from the user's address to contract_address and later to vault
                        // query the limit of spendable amount from the contract first
                        // check if the amount needed is more than limit
                        let spend_limit = AssetInfo::query_spend_limits(
                            &contract_addr,
                            &bootstrapping_amount_payer_addr,
                            &env.contract.address,
                            &deps.querier,
                        )?;

                        if asset.amount > spend_limit {
                            return Err(ContractError::InsuffiencentFundsSent);
                        }

                        let transfer_msg = build_transfer_cw20_from_user_msg(
                            contract_addr.to_string(),
                            bootstrapping_amount_payer.clone(),
                            env.contract.address.to_string(),
                            asset.amount,
                        )?;

                        // add the message to the list of messages
                        messages.push(transfer_msg);

                        // create a message to allow spending of funds by vault from governance admin contract
                        let approve_msg: cw20::Cw20ExecuteMsg =
                            cw20::Cw20ExecuteMsg::IncreaseAllowance {
                                spender: vault_addr.to_string(),
                                amount: asset.amount,
                                expires: None,
                            };

                        let cosmos_msg = CosmosMsg::Wasm(WasmMsg::Execute {
                            contract_addr: contract_addr.to_string(),
                            msg: to_binary(&approve_msg)?,
                            funds: vec![],
                        });

                        // add the message to the list of messages
                        messages.push(cosmos_msg.into());
                    }
                }
            }

            // now we can just create the pool
            let create_pool_msg = dexter::vault::ExecuteMsg::CreatePoolInstance {
                pool_type: pool_type.clone(),
                fee_info: fee_info.clone(),
                native_asset_precisions: native_asset_precisions.clone(),
                init_params: init_params.clone(),
                asset_infos: assets.iter().map(|a| a.info.clone()).collect(),
            };

            // add the message to the list of messages
            messages.push(
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: vault_addr.to_string(),
                    msg: to_binary(&create_pool_msg)?,
                    funds: vec![],
                })
                .into(),
            );

            // query vault to get the pool id that this pool should supposedly have
            let vault_config = query_vault_config(&deps.querier, vault_addr.to_string())?;
            let pool_id = vault_config.next_pool_id;

            // store the temp data for later use i.e. to join pool with mentioned asset amounts
            let temp_data = CreatePoolTempData {
                assumed_pool_id: pool_id,
                vault_addr: vault_addr.to_string(),
                bootstrapping_amount_payer: bootstrapping_amount_payer.to_string(),
                pool_type: pool_type.to_string(),
                fee_info: fee_info.clone(),
                native_asset_precisions: native_asset_precisions.clone(),
                assets: assets.clone(),
                init_params: init_params.clone(),
            };

            CREATE_POOL_TEMP_DATA.save(deps.storage, &temp_data)?;

            let mut res = Response::new();

            let mut event = Event::from_info(concatcp!(CONTRACT_NAME, "::create_new_pool"), &info)
                .add_attribute("pool_type", pool_type.to_string())
                .add_attribute("assets", serde_json_wasm::to_string(&assets).unwrap())
                .add_attribute("native_asset_precisions", serde_json_wasm::to_string(&native_asset_precisions).unwrap());


            if let Some(fee_info) = fee_info {
                event = event.add_attribute("fee_info", serde_json_wasm::to_string(&fee_info).unwrap());
            }

            // messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            //     contract_addr: env.contract.address.to_string(),
            //     msg: to_binary(&ExecuteMsg::ResumeJoinPool {})?,
            //     funds: vec![],
            // }));

            res = res.add_messages(messages);
            res = res.add_event(event);
            Ok(res)
        }

        ExecuteMsg::ResumeJoinPool { pool_creation_request_id } => {
            // get temp data and notice the pool id
            let temp_data = CREATE_POOL_TEMP_DATA.load(deps.storage);
            if temp_data.is_err() {
                return Err(ContractError::Unauthorized {});
            }

            let temp_data = temp_data.unwrap();
            let assumed_pool_id = temp_data.assumed_pool_id;
            let vault_addr = deps.api.addr_validate(&temp_data.vault_addr)?;

            // query vault config
            let vault_config = query_vault_config(&deps.querier, vault_addr.to_string())?;
            // validate that the next pool id is incremented by 1
            if vault_config.next_pool_id != assumed_pool_id.checked_add(Uint128::from(1u128))? {
                return Err(ContractError::Unauthorized {});
            }

            // now we can just join the pool
            let join_pool_msg = dexter::vault::ExecuteMsg::JoinPool {
                pool_id: assumed_pool_id,
                recipient: Some(temp_data.bootstrapping_amount_payer),
                assets: Some(temp_data.assets),
                min_lp_to_receive: None,
                auto_stake: None,
            };

            // add the message to the list of messages
            let mut messages: Vec<CosmosMsg> = vec![];
            messages.push(
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: vault_addr.to_string(),
                    msg: to_binary(&join_pool_msg)?,
                    funds: vec![],
                })
                .into(),
            );

            let event = Event::from_info(concatcp!(CONTRACT_NAME, "::resume_join_pool"), &info);

            let res = Response::new()
                .add_messages(messages)
                .add_event(event);

            Ok(res)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    return Err(StdError::generic_err("unsupported query"));
}
