use std::ops::Sub;
use std::str::FromStr;

#[cfg(not(feature = "library"))]
use crate::error::ContractError;
use crate::state::{POOL_CREATION_REQUESTS, next_pool_creation_request_id, POOL_CREATION_REQUEST_PROPOSAL_ID};
use crate::utils::{query_gov_params, query_latest_governance_proposal};

use const_format::concatcp;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    entry_point, to_binary, Binary, CosmosMsg, Deps, DepsMut, Env, Event, MessageInfo, Response,
    StdError, StdResult, WasmMsg, Uint128, Coin, Addr,
};
use cw2::set_contract_version;
use dexter::asset::{Asset, AssetInfo};
use dexter::governance_admin::{ExecuteMsg, InstantiateMsg, QueryMsg};
use dexter::helper::{build_transfer_cw20_from_user_msg, EventExt, NO_PRIV_KEY_ADDR};
use dexter::querier::query_vault_config;
use dexter::vault::{ExecuteMsg as VaultExecuteMsg, PoolCreationFee};
use persistence_std::types::cosmos::gov::v1::MsgSubmitProposal;
use persistence_std::types::cosmwasm::wasm::v1::{ExecuteContractProposal, MsgExecuteContract};

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter-governance-admin";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
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
    let deposit_params = query_gov_params(&deps.querier)?;
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
    let funds_str = format!("Funds: {:?}", total_funds_needed);

    // return Err(ContractError::Std(StdError::generic_err(funds_str)));
    let mut messages = vec![];

    // validate that the funds sent are enough for native assets
    let funds_map = funds.into_iter().map(|c| (c.denom, c.amount)).collect::<std::collections::HashMap<String, Uint128>>();
    for asset in total_funds_needed {
        match asset.info {
            AssetInfo::NativeToken { denom } => {
                let amount = funds_map.get(&denom).cloned().unwrap_or(Uint128::zero());
                // TODO: return the extra funds back to the user
                if amount < asset.amount {
                    panic!("Insufficient funds sent for native asset {} - Amount Sent: {} - Needed Amount: {}, funds_str: {}", denom, amount, asset.amount, funds_str);
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

#[cw_serde]
pub struct MsgMintStkAtom {
    pub amount: String
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {

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
                info.funds.clone()
            )?;

            let pool_creation_request_id = next_pool_creation_request_id(deps.storage)?;
            POOL_CREATION_REQUESTS.save(deps.storage, pool_creation_request_id, &pool_creation_request)?;

            let msg_execute_contract = MsgExecuteContract { 
                // this is the governance module address to basically instruct 
                // that the governance is able to send a message which only it can execute
                sender: "persistence10d07y265gmmuvt4z0w9aw880jnsr700j5w4kch".to_string(), 
                contract: env.contract.address.to_string(), 
                msg: to_binary(&dexter::governance_admin::ExecuteMsg::ResumeCreatePool { pool_creation_request_id })?.to_vec(), 
                funds: vec![] 
            };

            // we'll create a proposal to create a pool
            let proposal_msg = MsgSubmitProposal {
                title,
                metadata: "test".to_string(),
                summary: "test".to_string(),
                initial_deposit: vec![],
                proposer:env.contract.address.to_string(), 
                messages: vec![msg_execute_contract.to_any()],
            };

            messages.push(
                CosmosMsg::Stargate {
                    type_url: "/cosmos.gov.v1.MsgSubmitProposal".to_string(),
                    value: proposal_msg.into(),
                }
            );

            // CosmosMsg::Custom(MsgMintStkAtom {
            //     amount: "1000000".to_string()
            // });

            // // add a message to return callback to the contract post proposal creation so we can find the
            // // proposal id of the proposal we just created. This can be just found by querying the latest proposal id
            // // and doing a verification on the proposal content
            // let callback_msg = dexter::governance_admin::ExecuteMsg::PostGovernanceProposalCreationCallback {
            //     pool_creation_request_id: pool_creation_request_id
            // };

            // messages.push(
            //     CosmosMsg::Wasm(WasmMsg::Execute {
            //         contract_addr: env.contract.address.to_string(),
            //         msg: to_binary(&callback_msg)?,
            //         funds: vec![],
            //     })
            //     .into(),
            // );

            let event = Event::from_info(concatcp!(CONTRACT_NAME, "::create_pool_creation_proposal"), &info)
                .add_attribute("pool_creation_request_id", pool_creation_request_id.to_string());

            Ok(Response::new().add_messages(messages).add_event(event))
        },
        ExecuteMsg::PostGovernanceProposalCreationCallback { pool_creation_request_id } => {

            // proposal has been successfully created at this point, we can query the governance module and find the proposal id
            // and store it in the state
            let latest_proposal = query_latest_governance_proposal(env.contract.address, &deps.querier)?;

            // validate the proposal content to make sure that pool creation request id matches.
            // this is more of a sanity check
            
            // let proposal_content = latest_proposal.messages.first().unwrap();
            // let execute_contract_proposal_content = MsgExecuteContract::try_from(proposal_content.value.as_slice())?
            //     .map_err(|_| ContractError::Std(StdError::generic_err("failed to decode proposal content")))?;

            // let resume_create_pool_msg = dexter::governance_admin::ExecuteMsg::ResumeCreatePool { pool_creation_request_id };
            // let resume_create_pool_msg_bytes = to_binary(&resume_create_pool_msg).unwrap();

            // if execute_contract_proposal_content.msg != resume_create_pool_msg_bytes {
            //     return Err(ContractError::Std(StdError::generic_err("proposal content does not match")));
            // }

            // store the proposal id in the state
            POOL_CREATION_REQUEST_PROPOSAL_ID.save(deps.storage, pool_creation_request_id, &latest_proposal.id)?;

            let event = Event::from_info(concatcp!(CONTRACT_NAME, "::post_governance_proposal_creation_callback"), &info)
                .add_attribute("pool_creation_request_id", pool_creation_request_id.to_string())
                .add_attribute("proposal_id", latest_proposal.id.to_string());

            Ok(Response::default().add_event(event))
        }
        ExecuteMsg::ResumeCreatePool { pool_creation_request_id } => {

            // the proposal has passed, we can now resume the pool creation in the vault directly
            // get the pool creation request
            let pool_creation_request = POOL_CREATION_REQUESTS.load(deps.storage, pool_creation_request_id)?;
            let mut messages: Vec<CosmosMsg> = vec![];

            // create a message for vault
            let vault_addr = deps.api.addr_validate(&pool_creation_request.vault_addr)?;
            let create_pool_msg = VaultExecuteMsg::CreatePoolInstance {
                pool_type: pool_creation_request.pool_type.clone(),
                fee_info: pool_creation_request.fee_info.clone(),
                native_asset_precisions: pool_creation_request.native_asset_precisions.clone(),
                init_params: pool_creation_request.init_params.clone(),
                asset_infos: pool_creation_request.asset_info.clone()
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

            // add a message to return callback to the contract post proposal creation so we can find the
            // pool id of the pool we just created. This can be just found by querying the latest pool id from the vault
            // We also need to join the pool with the bootstrapping amount
            let callback_msg = dexter::governance_admin::ExecuteMsg::ResumeJoinPool {
                pool_creation_request_id
            };

            messages.push(
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: env.contract.address.to_string(),
                    msg: to_binary(&callback_msg)?,
                    funds: vec![],
                })
                .into(),
            );

            let event = Event::from_info(concatcp!(CONTRACT_NAME, "::resume_create_pool"), &info)
                .add_attribute("pool_creation_request_id", pool_creation_request_id.to_string());

            Ok(Response::new().add_messages(messages).add_event(event))
        },
        ExecuteMsg::ResumeJoinPool { pool_creation_request_id } => {

            
            let pool_creation_request = POOL_CREATION_REQUESTS.load(deps.storage, pool_creation_request_id)?;

            // find the pool id from the vault by querying the vault for the next pool id
            let vault_config = query_vault_config(&deps.querier, pool_creation_request.vault_addr.to_string())?;
            let mut messages: Vec<CosmosMsg> = vec![];

            let pool_id = vault_config.next_pool_id.checked_sub(Uint128::from(1u128))?;

            // check if the pool creation request has a bootstrapping amount
            if let Some(bootstrapping_amount) = pool_creation_request.bootstrapping_amount {
                  // now we can just join the pool
                let join_pool_msg = dexter::vault::ExecuteMsg::JoinPool {
                    pool_id,
                    recipient: Some(pool_creation_request.bootstrapping_liquidity_owner),
                    assets: Some(bootstrapping_amount),
                    min_lp_to_receive: None,
                    auto_stake: None,
                };

                // add the message to the list of messages
                messages.push(
                    CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: pool_creation_request.vault_addr.to_string(),
                        msg: to_binary(&join_pool_msg)?,
                        funds: vec![],
                    })
                    .into(),
                );
            }

            // // check if the pool creation request has reward schedules
            // if let Some(reward_schedules) = pool_creation_request.reward_schedules {
            //     for reward_schedule in reward_schedules {
            //         let add_reward_schedule_msg = dexter::multi_staking::ExecuteMsg::ProposeRewardSchedule {
            //             pool_id,
            //             start_time: reward_schedule.start_time,
            //             end_time: reward_schedule.end_time,
            //             epoch_amount: reward_schedule.amount,
            //         };

            //         // add the message to the list of messages
            //         messages.push(
            //             CosmosMsg::Wasm(WasmMsg::Execute {
            //                 contract_addr: pool_creation_request.vault_addr.to_string(),
            //                 msg: to_binary(&add_reward_schedule_msg)?,
            //                 funds: vec![],
            //             })
            //             .into(),
            //         );
            //     }
            // }


            // // add the message to the list of messages
            // let mut messages: Vec<CosmosMsg> = vec![];
            // messages.push(
            //     CosmosMsg::Wasm(WasmMsg::Execute {
            //         contract_addr: vault_addr.to_string(),
            //         msg: to_binary(&join_pool_msg)?,
            //         funds: vec![],
            //     })
            //     .into(),
            // );

            let event = Event::from_info(concatcp!(CONTRACT_NAME, "::resume_join_pool"), &info);

            let res = Response::new()
                .add_messages(messages)
                .add_event(event);

            Ok(res)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    return Err(StdError::generic_err("unsupported query"));
}

#[cw_serde]
pub struct MigrateMsg {}

// migrate handler
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    return Ok(Response::default())
}
