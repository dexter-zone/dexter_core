pub mod cosmos_msgs;

use std::str::FromStr;

use cosmwasm_std::{Addr, Coin, Deps, QuerierWrapper, QueryRequest, StdError, Uint128};
use dexter::multi_staking::QueryMsg as MultiStakingQueryMsg;
use persistence_std::types::cosmos::gov::v1::{
    Params as GovParams, Proposal, ProposalStatus, QueryParamsRequest, QueryParamsResponse,
    QueryProposalRequest, QueryProposalResponse, QueryProposalsRequest, QueryProposalsResponse,
};

use crate::{contract::ContractResult, error::ContractError};

pub fn query_proposal_min_deposit_amount(deps: Deps) -> Result<Vec<Coin>, ContractError> {
    let deposit_params = query_gov_params(&deps.querier)?;

    let proposal_deposit = deposit_params.min_deposit;

    let mut coins = vec![];

    for coin in proposal_deposit {
        coins.push(Coin {
            denom: coin.denom,
            amount: Uint128::from_str(&coin.amount).unwrap(),
        })
    }

    Ok(coins)
}

pub fn query_gov_params(querier: &QuerierWrapper) -> Result<GovParams, ContractError> {
    let governance_params_query = QueryParamsRequest {
        params_type: String::from("deposit"),
    };

    let params_response: QueryParamsResponse = querier.query(&QueryRequest::Stargate {
        path: String::from("/cosmos.gov.v1.Query/Params"),
        data: governance_params_query.into(),
    })?;

    // let params_str = format!("Params response: {:?}", params_response);
    // return Err(ContractError::Std(StdError::generic_err(params_str)));

    // let params: QueryParamsResponse = cosmwasm_std::from_binary(&governance_config)?;

    Ok(params_response.params.unwrap())
}

pub fn query_latest_governance_proposal(
    depositor_addr: Addr,
    querier: &QuerierWrapper,
) -> Result<Proposal, ContractError> {
    let q = QueryProposalsRequest {
        proposal_status: ProposalStatus::Unspecified.into(),
        pagination: None,
        voter: "".to_string(),
        depositor: depositor_addr.to_string(),
    };

    let proposal_response: QueryProposalsResponse = querier.query(&QueryRequest::Stargate {
        path: String::from("/cosmos.gov.v1.Query/Proposals"),
        data: q.into(),
    })?;

    // find the proposal with the highest id which ideally should be the latest proposal
    let latest_proposal = proposal_response
        .proposals
        .iter()
        .max_by(|a, b| a.id.cmp(&b.id))
        .unwrap();

    Ok(latest_proposal.clone())
}

pub fn query_gov_proposal_by_id(
    querier: &QuerierWrapper,
    proposal_id: u64,
) -> Result<Proposal, ContractError> {
    let q = QueryProposalRequest { proposal_id };

    let proposal_response: QueryProposalResponse = querier.query(&QueryRequest::Stargate {
        path: String::from("/cosmos.gov.v1.Query/Proposal"),
        data: q.into(),
    })?;

    proposal_response.proposal.ok_or_else(|| {
        ContractError::Std(StdError::generic_err(format!(
            "Proposal with id {} not found",
            proposal_id
        )))
    })
}

pub fn query_allowed_lp_tokens(
    multistaking_contract_addr: &Addr,
    querier: &QuerierWrapper,
) -> ContractResult<Vec<Addr>> {
    let response: Vec<Addr> = querier.query_wasm_smart(
        multistaking_contract_addr,
        &MultiStakingQueryMsg::AllowedLPTokensForReward {},
    )?;
    Ok(response)
}
