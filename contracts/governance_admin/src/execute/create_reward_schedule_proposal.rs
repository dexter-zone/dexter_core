use cosmwasm_std::{
    to_binary, Addr, CosmosMsg, DepsMut, Env, MessageInfo, QuerierWrapper, Response, StdError,
    Uint128,
};
use dexter::{
    asset::{Asset, AssetInfo},
    governance_admin::{
        GovernanceProposalDescription, RewardScheduleCreationRequest,
        RewardScheduleCreationRequestsState, RewardSchedulesCreationRequestStatus,
    },
    helper::build_transfer_cw20_from_user_msg,
};
use persistence_std::types::{
    cosmos::gov::v1::MsgSubmitProposal, cosmwasm::wasm::v1::MsgExecuteContract,
};

use crate::{
    add_wasm_execute_msg,
    contract::{ContractResult, GOV_MODULE_ADDRESS},
    error::ContractError,
    state::{next_reward_schedule_request_id, REWARD_SCHEDULE_REQUESTS},
    utils::query_allowed_lp_tokens,
};

pub fn validate_lp_token_allowed(
    multistaking_contract: &Addr,
    lp_tokens: Vec<Addr>,
    querier: &QuerierWrapper,
) -> ContractResult<()> {
    // query the multi-staking contract to find the current allowed tokens
    // if any of the requested tokens are not in the allowed list, return an error
    let allowed_tokens = query_allowed_lp_tokens(multistaking_contract, querier)?;
    // validate that all the requested LP tokens are already whitelisted for reward distribution
    let allowed_tokens_set = allowed_tokens
        .into_iter()
        .collect::<std::collections::HashSet<Addr>>();
    for lp_token in lp_tokens {
        if !allowed_tokens_set.contains(&lp_token) {
            return Err(ContractError::Std(StdError::generic_err(format!(
                "LP token {} is not allowed for reward distribution",
                lp_token
            ))));
        }
    }

    Ok(())
}

pub fn validate_or_transfer_assets(
    deps: &DepsMut,
    env: &Env,
    info: MessageInfo,
    reward_schedules: &Vec<RewardScheduleCreationRequest>,
) -> ContractResult<Vec<CosmosMsg>> {
    let sender = info.sender;
    let mut msgs = vec![];

    let mut total_funds_map = std::collections::HashMap::new();

    for reward_schedule in reward_schedules {
        let amount: Uint128 = total_funds_map
            .get(&reward_schedule.asset)
            .cloned()
            .unwrap_or_default();

        total_funds_map.insert(
            reward_schedule.asset.clone(),
            amount.checked_add(reward_schedule.amount)?,
        );
    }

    // validate that the funds sent are enough for native assets
    let funds_map = info
        .funds
        .into_iter()
        .map(|c| (c.denom, c.amount))
        .collect::<std::collections::HashMap<String, Uint128>>();

    let total_funds: Vec<Asset> = total_funds_map
        .into_iter()
        .map(|(k, v)| Asset { info: k, amount: v })
        .collect();

    for asset in total_funds {
        match asset.info {
            AssetInfo::NativeToken { denom } => {
                let amount = funds_map.get(&denom).cloned().unwrap_or(Uint128::zero());
                // TODO: return the extra funds back to the user
                if amount < asset.amount {
                    return Err(ContractError::InsufficientFundsSent {
                        denom: denom.to_string(),
                        amount_sent: amount,
                        needed_amount: asset.amount,
                    });
                }
            }
            AssetInfo::Token { contract_addr } => {
                // check if the contract has enough allowance to spend the funds
                let spend_limit = AssetInfo::query_spend_limits(
                    &contract_addr,
                    &sender,
                    &deps
                        .api
                        .addr_validate(&env.contract.address.to_string())
                        .unwrap(),
                    &deps.querier,
                )
                .unwrap();

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
                )
                .unwrap();

                // add the message to the list of messages
                msgs.push(transfer_msg);
            }
        }
    }

    Ok(msgs)
}

pub fn execute_create_reward_schedule_creation_proposal(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_description: GovernanceProposalDescription,
    multistaking_contract: Addr,
    reward_schedules: Vec<RewardScheduleCreationRequest>,
) -> ContractResult<Response> {
    let mut msgs: Vec<CosmosMsg> = vec![];

    // validate that LP tokens are all not none
    let mut lp_tokens = vec![];

    for reward_schedule in &reward_schedules {
        match &reward_schedule.lp_token_addr {
            None => {
                return Err(ContractError::Std(StdError::generic_err(format!(
                    "LP token address is required for reward schedule creation request"
                ))));
            }
            Some(lp_token) => {
                lp_tokens.push(lp_token.clone());
            }
        }
    }

    // validate that all the requested LP tokens are already whitelisted for reward distribution
    validate_lp_token_allowed(&multistaking_contract, lp_tokens, &deps.querier)?;

    // validatate all the funds are being sent or approved for transfer and transfer them to the contract
    let mut transfer_msgs = validate_or_transfer_assets(&deps, &env, info, &reward_schedules)?;
    msgs.append(&mut transfer_msgs);

    // store the reward schedule creation request
    let next_reward_schedules_creation_request_id = next_reward_schedule_request_id(deps.storage)?;

    REWARD_SCHEDULE_REQUESTS.save(
        deps.storage,
        next_reward_schedules_creation_request_id,
        &RewardScheduleCreationRequestsState {
            status: RewardSchedulesCreationRequestStatus::PendingProposalCreation,
            multistaking_contract_addr: multistaking_contract,
            reward_schedule_creation_requests: reward_schedules.clone(),
        },
    )?;

    // create a proposal for approving the reward schedule creation
    let create_reward_schedule_proposal_msg =
        dexter::governance_admin::ExecuteMsg::ResumeCreateRewardSchedules {
            reward_schedules_creation_request_id: next_reward_schedules_creation_request_id,
        };

    let msg = MsgExecuteContract {
        sender: GOV_MODULE_ADDRESS.to_string(),
        contract: env.contract.address.to_string(),
        msg: to_binary(&create_reward_schedule_proposal_msg)?.to_vec(),
        funds: vec![],
    };

    let proposal_msg = MsgSubmitProposal {
        title: proposal_description.title,
        metadata: proposal_description.metadata,
        summary: proposal_description.summary,
        initial_deposit: vec![],
        proposer: env.contract.address.to_string(),
        messages: vec![msg.to_any()],
    };

    msgs.push(CosmosMsg::Stargate {
        type_url: "/cosmos.gov.v1.MsgSubmitProposal".to_string(),
        value: proposal_msg.into(),
    });

    let callback_msg =
        dexter::governance_admin::ExecuteMsg::PostGovernanceProposalCreationCallback {
            gov_proposal_type:
                dexter::governance_admin::GovAdminProposalType::RewardSchedulesCreationRequest {
                    request_id: next_reward_schedules_creation_request_id,
                },
        };

    add_wasm_execute_msg!(msgs, env.contract.address, callback_msg, vec![]);

    Ok(Response::new().add_messages(msgs))
}
