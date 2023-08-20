use cosmos_sdk_proto::{cosmos::gov::v1beta1::{QueryParamsRequest, QueryParamsResponse, DepositParams, QueryProposalsRequest, ProposalStatus, QueryProposalsResponse, Proposal}, traits::Message};
use cosmwasm_std::{QuerierWrapper, QueryRequest, to_binary, StdResult, Binary, Deps, StdError, Addr};

use crate::error::ContractError;

pub fn query_gov_deposit_params(
    querier: &QuerierWrapper,
) -> Result<DepositParams, ContractError> {
    let governance_params_query = QueryParamsRequest {
        params_type: String::from("deposit"),
    };

    let governance_config: Binary = querier.query(&QueryRequest::Stargate {
        path: String::from("cosmos.gov.v1beta1.Query/Params"),
        data: to_binary(&governance_params_query.encode_to_vec())?,
    })?;

    let params: QueryParamsResponse = QueryParamsResponse::decode(governance_config.as_slice())
        .map_err(|_| ContractError::Std(StdError::generic_err("Unable to parse governance params")))?;

    let deposit_params = params.deposit_params.unwrap();
    Ok(deposit_params)
}

pub fn query_latest_governance_proposal(
    depositor_addr: Addr,
    querier: &QuerierWrapper
) -> Result<Proposal, ContractError> {
    let q = QueryProposalsRequest {
        proposal_status: ProposalStatus::VotingPeriod.into(),
        pagination: None,
        voter: "".to_string(),
        depositor: depositor_addr.to_string(),
    };

    let proposals: Binary = querier.query(&QueryRequest::Stargate {
        path: String::from("cosmos.gov.v1beta1.Query/Proposals"),
        data: to_binary(&q.encode_to_vec()).unwrap(),
    }).unwrap();

    let proposals_response: QueryProposalsResponse = QueryProposalsResponse::decode(proposals.as_slice()).unwrap();
    let proposals = proposals_response.proposals;
    
    // find the proposal with the highest id which ideally should be the latest proposal
    let latest_proposal = proposals.iter().max_by(|a, b| a.proposal_id.cmp(&b.proposal_id)).unwrap();
    
    Ok(latest_proposal.clone())
}
