use const_format::concatcp;
use cosmwasm_std::{
    to_binary, Addr, Coin, CosmosMsg, DepsMut, Env, MessageInfo, QuerierWrapper, Response,
    StdError, Uint128, Event,
};
use dexter::{
    asset::{Asset, AssetInfo},
    helper::EventExt,
    governance_admin::{
        FundsCategory, GovernanceProposalDescription, RewardScheduleCreationRequest,
        RewardScheduleCreationRequestsState, RewardSchedulesCreationRequestStatus, UserDeposit,
    },
};
use persistence_std::types::{
    cosmos::gov::v1::MsgSubmitProposal,
    cosmwasm::wasm::v1::MsgExecuteContract,
    cosmos::base::v1beta1::Coin as StdCoin,
};

use crate::{
    add_wasm_execute_msg,
    contract::{ContractResult, GOV_MODULE_ADDRESS, CONTRACT_NAME},
    error::ContractError,
    execute::create_pool_creation_proposal::validate_sent_amount_and_transfer_needed_assets,
    state::{next_reward_schedule_request_id, REWARD_SCHEDULE_REQUESTS},
    utils::{query_allowed_lp_tokens, query_proposal_min_deposit_amount, query_gov_params},
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
        // reward schedules start block time should be a govermance proposal voting period later than the current block time
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
                return Err(ContractError::Std(StdError::generic_err(format!(
                    "LP token address is required for reward schedule creation request"
                ))));
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
            return Err(ContractError::Std(StdError::generic_err(format!(
                "LP token {} is not allowed for reward distribution",
                lp_token
            ))));
        }
    }

    Ok(())
}

pub fn total_needed_funds(
    requests: &Vec<RewardScheduleCreationRequest>,
    gov_proposal_min_deposit_amount: &Vec<Coin>,
) -> ContractResult<(Vec<UserDeposit>, Vec<Asset>)> {
    let mut total_funds_map = std::collections::HashMap::new();
    let mut user_deposits_detailed = vec![];

    let mut proposal_deposit_assets = vec![];
    for coin in gov_proposal_min_deposit_amount {
        let asset_info = AssetInfo::native_token(coin.denom.clone());
        let amount: Uint128 = total_funds_map
            .get(&asset_info)
            .cloned()
            .unwrap_or_default();
        let c_amount = coin.amount;
        total_funds_map.insert(asset_info.clone(), amount.checked_add(c_amount)?);
        proposal_deposit_assets.push(Asset {
            info: asset_info,
            amount: c_amount,
        });
    }

    user_deposits_detailed.push(UserDeposit {
        category: FundsCategory::ProposalDeposit,
        assets: proposal_deposit_assets,
    });

    for reward_schedule in requests {
        let amount: Uint128 = total_funds_map
            .get(&reward_schedule.asset)
            .cloned()
            .unwrap_or_default();

        total_funds_map.insert(
            reward_schedule.asset.clone(),
            amount.checked_add(reward_schedule.amount)?,
        );

        user_deposits_detailed.push(UserDeposit {
            category: FundsCategory::RewardScheduleAmount,
            assets: vec![Asset {
                info: reward_schedule.asset.clone(),
                amount: reward_schedule.amount,
            }],
        });
    }

    let total_funds: Vec<Asset> = total_funds_map
        .into_iter()
        .map(|(k, v)| Asset { info: k, amount: v })
        .collect();

    Ok((user_deposits_detailed, total_funds))
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

    // validate multistaking_contract address
    deps.api.addr_validate(&multistaking_contract.to_string())?;

    // TODO(ajeet): should validate multistaking_contract address?
    
    let gov_params = query_gov_params(&deps.querier)?;
    let gov_voting_period = gov_params.voting_period.ok_or(ContractError::VotingPeriodNull)?.seconds as u64;

    let lp_tokens = validate_create_reward_schedules_request(
        &env,
        gov_voting_period,
        &reward_schedules,
    )?;

    // validate that all the requested LP tokens are already whitelisted for reward distribution
    validate_lp_token_allowed(&multistaking_contract, lp_tokens, &deps.querier)?;

    let gov_proposal_min_deposit_amount = query_proposal_min_deposit_amount(deps.as_ref())?;
    let (user_deposits_detailed, total_needed_funds) =
        total_needed_funds(&reward_schedules, &gov_proposal_min_deposit_amount)?;

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
