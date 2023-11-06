use const_format::concatcp;
use cosmwasm_std::{
    to_binary, Addr, CosmosMsg, DepsMut, Env, Event, MessageInfo, QuerierWrapper, Response,
};
use dexter::{
    governance_admin::{
        GovernanceProposalDescription, RewardScheduleCreationRequest,
        RewardScheduleCreationRequestsState, RewardSchedulesCreationRequestStatus,
    },
    helper::EventExt, constants::GOV_MODULE_ADDRESS,
};
use persistence_std::types::{
    cosmos::base::v1beta1::Coin as StdCoin, cosmos::gov::v1::MsgSubmitProposal,
    cosmwasm::wasm::v1::MsgExecuteContract,
};

use crate::{
    add_wasm_execute_msg,
    contract::{ContractResult, CONTRACT_NAME},
    error::ContractError,
    execute::create_pool_creation_proposal::validate_sent_amount_and_transfer_needed_assets,
    query::query_reward_schedule_creation_funds::find_total_needed_funds,
    state::{next_reward_schedule_request_id, REWARD_SCHEDULE_REQUESTS},
    utils::queries::{query_allowed_lp_tokens, query_gov_params, query_proposal_min_deposit_amount},
};

pub fn validate_create_reward_schedules_request(
    env: &Env,
    gov_voting_period: u64,
    reward_schedules: &Vec<RewardScheduleCreationRequest>,
) -> Result<Vec<Addr>, ContractError> {
    if reward_schedules.len() == 0 {
        return Err(ContractError::EmptyRewardSchedule {});
    }

    let voting_end_time = env.block.time.plus_seconds(gov_voting_period).seconds();
    for reward_schedule in reward_schedules {
        // reward schedules start block time should be a governance proposal voting period later than the current block time
        if reward_schedule.start_block_time < voting_end_time {
            return Err(ContractError::InvalidRewardScheduleStartBlockTime {});
        }

        // end block time must be greater than start block time
        if reward_schedule.end_block_time <= reward_schedule.start_block_time {
            return Err(ContractError::InvalidRewardScheduleEndBlockTime {});
        }
    }

    let mut lp_tokens = vec![];

    // Validate that LP tokens are all not none
    for reward_schedule in reward_schedules {
        match &reward_schedule.lp_token_addr {
            None => {
                return Err(ContractError::LpTokenNotAllowed);
            }
            Some(lp_token) => {
                lp_tokens.push(lp_token.clone());
            }
        }
    }

    Ok(lp_tokens)
}

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
            return Err(ContractError::LpTokenNull);
        }
    }

    Ok(())
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
    // TODO(ajeet): should validate multistaking_contract address?

    let gov_params = query_gov_params(&deps.querier)?;
    let gov_voting_period = gov_params
        .voting_period
        .ok_or(ContractError::VotingPeriodNull)?
        .seconds as u64;

    let lp_tokens =
        validate_create_reward_schedules_request(&env, gov_voting_period, &reward_schedules)?;

    // validate that all the requested LP tokens are already whitelisted for reward distribution
    validate_lp_token_allowed(&multistaking_contract, lp_tokens, &deps.querier)?;

    let gov_proposal_min_deposit_amount = query_proposal_min_deposit_amount(deps.as_ref())?;
    let (user_deposits_detailed, total_needed_funds) =
        find_total_needed_funds(&reward_schedules, &gov_proposal_min_deposit_amount)?;

    // validatate all the funds are being sent or approved for transfer and transfer them to the contract
    let mut transfer_msgs = validate_sent_amount_and_transfer_needed_assets(
        &deps.as_ref(),
        &env,
        &info.sender,
        &total_needed_funds,
        info.funds.clone(),
    )?;
    msgs.append(&mut transfer_msgs);

    // store the reward schedule creation request
    let next_reward_schedules_creation_request_id = next_reward_schedule_request_id(deps.storage)?;

    let reward_schedule_creation_request = RewardScheduleCreationRequestsState {
        status: RewardSchedulesCreationRequestStatus::PendingProposalCreation,
        request_sender: info.sender.clone(),
        multistaking_contract_addr: multistaking_contract,
        reward_schedule_creation_requests: reward_schedules.clone(),
        user_deposits_detailed,
        total_funds_acquired_from_user: total_needed_funds,
    };

    REWARD_SCHEDULE_REQUESTS.save(
        deps.storage,
        next_reward_schedules_creation_request_id,
        &reward_schedule_creation_request,
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
        initial_deposit: gov_proposal_min_deposit_amount
            .iter()
            .map(|c| StdCoin {
                denom: c.denom.clone(),
                amount: c.amount.to_string(),
            })
            .collect(),
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
                dexter::governance_admin::GovAdminProposalRequestType::RewardSchedulesCreationRequest {
                    request_id: next_reward_schedules_creation_request_id,
                },
        };

    add_wasm_execute_msg!(msgs, env.contract.address, callback_msg, vec![]);

    let event = Event::from_info(
        concatcp!(CONTRACT_NAME, "::create_reward_schedule_proposal"),
        &info,
    )
    .add_attribute(
        "reward_schedules_creation_request_id",
        next_reward_schedules_creation_request_id.to_string(),
    );

    Ok(Response::new().add_messages(msgs).add_event(event))
}
