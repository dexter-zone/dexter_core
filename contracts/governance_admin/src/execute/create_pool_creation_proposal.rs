#[cfg(not(feature = "library"))]
use std::collections::HashSet;

use crate::add_wasm_execute_msg;
use crate::contract::{ContractResult, CONTRACT_NAME};
use crate::error::ContractError;
use crate::query::query_pool_creation_funds::find_total_funds_needed;
use crate::state::{next_pool_creation_request_id, POOL_CREATION_REQUEST_DATA};
use crate::utils::queries::{query_gov_params, query_proposal_min_deposit_amount};

use const_format::concatcp;
use cosmwasm_std::{
    to_binary, Addr, Coin, CosmosMsg, Deps, DepsMut, Env, Event, MessageInfo, Response, Uint128,
};
use dexter::asset::{Asset, AssetInfo};
use dexter::constants::GOV_MODULE_ADDRESS;
use dexter::governance_admin::{
    GovernanceProposalDescription, PoolCreateRequestContextData, PoolCreationRequest,
    PoolCreationRequestStatus,
};
use dexter::helper::{
    build_transfer_cw20_from_user_msg, build_transfer_token_to_user_msg, EventExt,
};
use persistence_std::types::cosmos::base::v1beta1::Coin as StdCoin;
use persistence_std::types::cosmos::gov::v1::MsgSubmitProposal;
use persistence_std::types::cosmwasm::wasm::v1::MsgExecuteContract;

/// Validates a create pool request, particularly the following checks
/// 1. Bootstrapping liquidity owner must be a valid address
/// 2. Native asset precision must be specified for all the native assets in the pool
/// 3. Bootstrapping amount if set, must include all the assets in the pool
/// 4. Reward schedules start block time should be a governance proposal voting period later than the current block time
fn validate_create_pool_request(
    env: &Env,
    deps: &DepsMut,
    gov_voting_period: u64,
    pool_creation_request: &PoolCreationRequest,
) -> Result<(), ContractError> {
    // Bootstrapping liquidity owner must be a valid address
    deps.api
        .addr_validate(&pool_creation_request.bootstrapping_liquidity_owner)?;

    // validate vault address
    deps.api.addr_validate(&pool_creation_request.vault_addr)?;

    // native asset precision must be specified for all the native assets in the pool
    for asset in pool_creation_request.asset_info.clone() {
        match asset {
            AssetInfo::NativeToken { denom } => {
                let native_asset_precision = pool_creation_request
                    .native_asset_precisions
                    .iter()
                    .find(|native_asset_precision| native_asset_precision.denom == denom);
                if native_asset_precision.is_none() {
                    return Err(ContractError::InvalidNativeAssetPrecisionList {});
                }
            }
            _ => {}
        }
    }

    // bootstrapping amount if set, must include all the assets in the pool
    if let Some(bootstrapping_amount) = &pool_creation_request.bootstrapping_amount {
        // bootstrapping amount must be greater than 0 for all the assets if it is specified
        for asset in bootstrapping_amount {
            if asset.amount.is_zero() {
                return Err(ContractError::BootstrappingAmountMustBeGreaterThanZero {});
            }
        }

        let asset_info = pool_creation_request
            .asset_info
            .iter()
            .cloned()
            .collect::<HashSet<AssetInfo>>();

        let bootstapping_amount_asset_info = bootstrapping_amount
            .iter()
            .map(|asset| asset.info.clone())
            .collect::<HashSet<AssetInfo>>();

        if asset_info != bootstapping_amount_asset_info {
            return Err(ContractError::BootstrappingAmountMismatchAssets {});
        }
    }

    // reward schedules start block time should be a governance proposal voting period later than the current block time
    if let Some(reward_schedules) = &pool_creation_request.reward_schedules {
        for reward_schedule in reward_schedules {
            if reward_schedule.start_block_time
                < env.block.time.plus_seconds(gov_voting_period).seconds()
            {
                return Err(ContractError::InvalidRewardScheduleStartBlockTime {});
            }

            if reward_schedule.end_block_time <= reward_schedule.start_block_time {
                return Err(ContractError::InvalidRewardScheduleEndBlockTime {});
            }
        }
    }

    Ok(())
}

/// Validates if the funds sent by the user are enough to create the pool and other operations
/// and if yes, then transfers the funds to this contract in case of CW20 tokens since they are not sent along with the message
/// In case of native tokens, the extra funds are returned back to the user
pub fn validate_sent_amount_and_transfer_needed_assets(
    deps: &Deps,
    env: &Env,
    sender: &Addr,
    total_funds_needed: &Vec<Asset>,
    funds: Vec<Coin>,
) -> Result<Vec<CosmosMsg>, ContractError> {
    let mut messages = vec![];

    // validate that the funds sent are enough for native assets
    let funds_map = funds
        .into_iter()
        .map(|c| (c.denom, c.amount))
        .collect::<std::collections::HashMap<String, Uint128>>();

    for asset in total_funds_needed {
        match &asset.info {
            AssetInfo::NativeToken { denom } => {
                let amount = funds_map.get(denom).cloned().unwrap_or(Uint128::zero());
                // TODO: return the extra funds back to the user
                if amount < asset.amount {
                    return Err(ContractError::InsufficientFundsSent {
                        denom: denom.to_string(),
                        amount_sent: amount,
                        needed_amount: asset.amount,
                    });
                }

                // return the extra funds back to the user if any
                if amount > asset.amount {
                    messages.push(build_transfer_token_to_user_msg(
                        asset.info.clone(),
                        sender.clone(),
                        amount.checked_sub(asset.amount)?,
                    )?);
                }
            }
            AssetInfo::Token { contract_addr } => {
                // check if the contract has enough allowance to spend the funds
                let spend_limit = AssetInfo::query_spend_limits(
                    &contract_addr,
                    sender,
                    &deps.api.addr_validate(&env.contract.address.to_string())?,
                    &deps.querier,
                )?;

                if asset.amount > spend_limit {
                    return Err(ContractError::InsufficientSpendLimit {
                        token_addr: contract_addr.to_string(),
                        current_approval: spend_limit,
                        needed_approval_for_spend: asset.amount,
                    });
                }

                // transfer the funds from the user to this contract
                let transfer_msg = build_transfer_cw20_from_user_msg(
                    contract_addr.to_string(),
                    sender.to_string(),
                    env.contract.address.to_string(),
                    asset.amount,
                )?;

                // add the message to the list of messages
                messages.push(transfer_msg);
            }
        }
    }

    Ok(messages)
}

/// Creates a proposal to create a pool
/// The proposal is created by the governance admin contract on behalf of the user to enable easy accounting of funds for pool creation
/// Pool creation follows the following steps:
/// 1. User calls this contract with a pool creation request and required funds and(or) approval to spend funds in case of CW20 tokens
/// 2. This contract verifies the funds, and transfers the funds to this contract in case of CW20 tokens. The custody of the funds is transferred to the governance admin contract.
/// 3. This contract stores the pool creation request in its state.
/// 3. Then, this contract creates a proposal to resume the pool creation process, which returns a callback to itself with the pool creation request id.
/// 4. If the proposal is passed, governance module of the chain will call the callback with the pool creation request id.
/// 5. This contract will then resume the pool creation process and create the pool in the vault contract.
/// 6. If specified, it will also bootstrap the pool with the bootstrapping amount.
/// 7. If specified, it will also create the reward schedules for the pool in the multi-staking contract.
/// 8. If the pool creation fails or if the proposal is rejected, the user can request all the funds back by executing the `ClaimRefund` message.
/// 9. If the pool creation is successful, the user can request Proposal Deposit amount by the same `ClaimRefund` message.
pub fn execute_create_pool_creation_proposal(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_description: GovernanceProposalDescription,
    pool_creation_request: PoolCreationRequest,
) -> ContractResult<Response> {
    let gov_params = query_gov_params(&deps.querier)?;
    // first order of business, ensure the money is sent along with the message
    let gov_proposal_min_deposit_amount = query_proposal_min_deposit_amount(deps.as_ref())?;

    validate_create_pool_request(
        &env,
        &deps,
        gov_params
            .voting_period
            .ok_or(ContractError::VotingPeriodNull)?
            .seconds as u64,
        &pool_creation_request,
    )?;

    // find total needed first
    let user_total_deposit_funds = find_total_funds_needed(
        deps.as_ref(),
        &gov_proposal_min_deposit_amount,
        &pool_creation_request,
    )?;

    let mut messages = validate_sent_amount_and_transfer_needed_assets(
        &deps.as_ref(),
        &env,
        &info.sender,
        &user_total_deposit_funds.total_deposit,
        info.funds.clone(),
    )?;

    let pool_creation_request_id = next_pool_creation_request_id(deps.storage)?;
    POOL_CREATION_REQUEST_DATA.save(
        deps.storage,
        pool_creation_request_id,
        &PoolCreateRequestContextData {
            status: PoolCreationRequestStatus::PendingProposalCreation,
            request_sender: info.sender.clone(),
            total_funds_acquired_from_user: user_total_deposit_funds.total_deposit,
            user_deposits_detailed: user_total_deposit_funds.deposit_breakdown,
            pool_creation_request,
        },
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
        title: proposal_description.title,
        metadata: proposal_description.metadata,
        summary: proposal_description.summary,
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

    // add a message to return callback to the contract post proposal creation so we can find the
    // proposal id of the proposal we just created. This can be just found by querying the latest proposal id
    // and doing a verification on the proposal content
    let callback_msg =
        dexter::governance_admin::ExecuteMsg::PostGovernanceProposalCreationCallback {
            gov_proposal_type:
                dexter::governance_admin::GovAdminProposalRequestType::PoolCreationRequest {
                    request_id: pool_creation_request_id,
                },
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
