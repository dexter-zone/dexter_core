// use cosmos_sdk_proto::{cosmos::gov::v1beta1::{QueryParamsRequest, QueryParamsResponse, DepositParams, QueryProposalsRequest, ProposalStatus, QueryProposalsResponse, Proposal}, traits::Message};
use cosmwasm_std::{Addr, QuerierWrapper, QueryRequest};
use persistence_std::types::{cosmos::gov::v1::{
    Params as GovParams, Proposal, ProposalStatus, QueryParamsRequest, QueryParamsResponse,
    QueryProposalsRequest, QueryProposalsResponse,
}, cosmwasm::wasm::v1::MsgExecuteContract};
use serde::{Deserialize, Deserializer};

use crate::error::ContractError;

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

    let proposal_response: QueryProposalsResponse = querier
        .query(&QueryRequest::Stargate {
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



pub fn test() {

    // let value = match serde_cw_value::Value::deserialize(deserializer) {
    //     Ok(value) => value,
    //     Err(err) => {
    //         return Err(err);
    //     }
    // };

    
}
