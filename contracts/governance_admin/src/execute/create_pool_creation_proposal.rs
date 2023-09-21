use crate::add_wasm_execute_msg;
use crate::contract::{ContractResult, CONTRACT_NAME, GOV_MODULE_ADDRESS};
#[cfg(not(feature = "library"))]
use crate::error::ContractError;
use crate::state::{next_pool_creation_request_id, POOL_CREATION_REQUESTS};
use crate::utils::query_proposal_min_deposit_amount;

use const_format::concatcp;
use cosmwasm_std::{
    to_binary, Addr, Coin, CosmosMsg, Deps, DepsMut, Env, Event, MessageInfo, Response, Uint128
};
use dexter::asset::{Asset, AssetInfo};
use dexter::governance_admin::PoolCreationRequest;
use dexter::helper::{build_transfer_cw20_from_user_msg, EventExt};
use dexter::querier::query_vault_config;
use dexter::vault::PoolCreationFee;
use persistence_std::types::cosmos::base::v1beta1::Coin as StdCoin;
use persistence_std::types::cosmos::gov::v1::MsgSubmitProposal;
use persistence_std::types::cosmwasm::wasm::v1::MsgExecuteContract;

// Sums up the requirements in terms of pool creation fee, pool bootstrapping amount and reward schedule
// amounts and returns it
// This can later be used to validate if the user has sent enough funds to create the pool and
// transfer Cw20 token to this contract for further processing
fn find_total_funds_needed(
    deps: Deps,
    gov_proposal_min_deposit_amount: &Vec<Coin>,
    pool_creation_request_proposal: &dexter::governance_admin::PoolCreationRequest,
) -> Result<Vec<Asset>, ContractError> {
    // let mut total_funds = vec![];
    let mut total_funds_map = std::collections::HashMap::new();
    let vault_addr = deps
        .api
        .addr_validate(&pool_creation_request_proposal.vault_addr)
        .unwrap();

    // find the pool creation fee by querying the vault contract currently
    let vault_config = query_vault_config(&deps.querier, vault_addr.to_string())?;
    let pool_creation_fee = vault_config.pool_creation_fee;

    // add the proposal deposit to the total funds.
    // We need to query the gov module to figure this out
    for coin in gov_proposal_min_deposit_amount {
        let asset_info = AssetInfo::native_token(coin.denom.clone());
        let amount: Uint128 = total_funds_map
            .get(&asset_info)
            .cloned()
            .unwrap_or_default();
        let c_amount = coin.amount;
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
            let amount = total_funds_map
                .get(&asset.info)
                .cloned()
                .unwrap_or_default();
            total_funds_map.insert(asset.info.clone(), amount.checked_add(asset.amount)?);
        }
    }

    // add the reward schedule amounts to the total funds
    if let Some(reward_schedules) = &pool_creation_request_proposal.reward_schedules {
        for reward_schedule in reward_schedules {
            let amount = total_funds_map
                .get(&reward_schedule.asset)
                .cloned()
                .unwrap_or_default();
            total_funds_map.insert(
                reward_schedule.asset.clone(),
                amount.checked_add(reward_schedule.amount)?,
            );
        }
    }

    let total_funds = total_funds_map
        .into_iter()
        .map(|(k, v)| Asset { info: k, amount: v })
        .collect();
    Ok(total_funds)
}

/// Validates if the funds sent by the user are enough to create the pool and other operations
/// and if yes, then transfers the funds to this contract in case of CW20 tokens since they are not sent along with the message
/// In case of native tokens, the extra funds are returned back to the user
fn validate_or_transfer_assets(
    deps: Deps,
    env: Env,
    sender: &Addr,
    gov_proposal_min_deposit_amount: &Vec<Coin>,
    pool_creation_request_proposal: &dexter::governance_admin::PoolCreationRequest,
    funds: Vec<Coin>,
) -> Result<Vec<CosmosMsg>, ContractError> {
    // find total needed first
    let total_funds_needed = find_total_funds_needed(
        deps,
        gov_proposal_min_deposit_amount,
        pool_creation_request_proposal,
    )?;
    let funds_str = format!("Funds: {:?}", total_funds_needed);

    // return Err(ContractError::Std(StdError::generic_err(funds_str)));
    let mut messages = vec![];

    // validate that the funds sent are enough for native assets
    let funds_map = funds
        .into_iter()
        .map(|c| (c.denom, c.amount))
        .collect::<std::collections::HashMap<String, Uint128>>();
    for asset in total_funds_needed {
        match asset.info {
            AssetInfo::NativeToken { denom } => {
                let amount = funds_map.get(&denom).cloned().unwrap_or(Uint128::zero());
                // TODO: return the extra funds back to the user
                if amount < asset.amount {
                    panic!("Insufficient funds sent for native asset {} - Amount Sent: {} - Needed Amount: {}, funds_str: {}", denom, amount, asset.amount, funds_str);
                }
            }
            AssetInfo::Token { contract_addr } => {
                // check if the contract has enough allowance to spend the funds
                let spend_limit = AssetInfo::query_spend_limits(
                    &contract_addr,
                    sender,
                    &deps
                        .api
                        .addr_validate(&env.contract.address.to_string())
                        .unwrap(),
                    &deps.querier,
                )
                .unwrap();

                if asset.amount > spend_limit {
                    panic!("Insufficient spend limit cw20 asset {}", contract_addr);
                }

                // transfer the funds from the user to this contract
                let transfer_msg = build_transfer_cw20_from_user_msg(
                    contract_addr.to_string(),
                    sender.to_string(),
                    env.contract.address.to_string(),
                    asset.amount,
                )
                .unwrap();

                // add the message to the list of messages
                messages.push(transfer_msg);
            }
        }
    }

    Ok(messages)
}

pub fn execute_create_pool_creation_proposal(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    title: String,
    summary: String,
    metadata: String,
    pool_creation_request: PoolCreationRequest,
) -> ContractResult<Response> {
    // first order of business, ensure the money is sent along with the message
    let gov_proposal_min_deposit_amount = query_proposal_min_deposit_amount(deps.as_ref())?;
    let mut messages = validate_or_transfer_assets(
        deps.as_ref(),
        env.clone(),
        &info.sender,
        &gov_proposal_min_deposit_amount,
        &pool_creation_request,
        info.funds.clone(),
    )?;

    let pool_creation_request_id = next_pool_creation_request_id(deps.storage)?;
    POOL_CREATION_REQUESTS.save(
        deps.storage,
        pool_creation_request_id,
        &pool_creation_request,
    )?;

    let msg_execute_contract = MsgExecuteContract {
        // this is the governance module address to basically instruct
        // that the governance is able to send a message which only it can execute
        sender: GOV_MODULE_ADDRESS.to_string(),
        contract: env.contract.address.to_string(),
        msg: to_binary(&dexter::governance_admin::ExecuteMsg::ResumeCreatePool {
            pool_creation_request_id,
        })?
        .to_vec(),
        funds: vec![],
    };

    // we'll create a proposal to create a pool
    let proposal_msg = MsgSubmitProposal {
        title,
        metadata,
        summary,
        initial_deposit: gov_proposal_min_deposit_amount
            .iter()
            .map(|c| StdCoin {
                denom: c.denom.clone(),
                amount: c.amount.to_string(),
            })
            .collect(),
        proposer: env.contract.address.to_string(),
        messages: vec![msg_execute_contract.to_any()],
    };

    messages.push(CosmosMsg::Stargate {
        type_url: "/cosmos.gov.v1.MsgSubmitProposal".to_string(),
        value: proposal_msg.into(),
    });

    // // add a message to return callback to the contract post proposal creation so we can find the
    // // proposal id of the proposal we just created. This can be just found by querying the latest proposal id
    // // and doing a verification on the proposal content
    let callback_msg =
        dexter::governance_admin::ExecuteMsg::PostGovernanceProposalCreationCallback {
            pool_creation_request_id,
        };

    add_wasm_execute_msg!(messages, env.contract.address, callback_msg, vec![]);    

    let event = Event::from_info(
        concatcp!(CONTRACT_NAME, "::create_pool_creation_proposal"),
        &info,
    )
    .add_attribute(
        "pool_creation_request_id",
        pool_creation_request_id.to_string(),
    );

    Ok(Response::new().add_messages(messages).add_event(event))
}
