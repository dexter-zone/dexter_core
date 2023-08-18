use cosmos_sdk_proto::{cosmos::gov::v1beta1::{QueryParamsRequest, QueryParamsResponse, DepositParams}, traits::Message};
use cosmwasm_std::{QuerierWrapper, QueryRequest, to_binary, StdResult, Binary, Deps, StdError};

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
