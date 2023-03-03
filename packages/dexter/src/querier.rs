use crate::pool;
use crate::{asset::AssetInfo, vault};
use cosmwasm_std::{
    to_binary, Addr, BalanceResponse, BankQuery, QuerierWrapper, QueryRequest, StdResult, Uint128,
    WasmQuery, Coin,
};
use cw20::{BalanceResponse as Cw20BalanceResponse, Cw20QueryMsg, TokenInfoResponse};

const NATIVE_TOKEN_PRECISION: u8 = 6;

/// ## Description
/// Returns the balance of the denom at the specified account address.
/// ## Params
/// * **querier** is the object of type [`QuerierWrapper`].
/// * **account_addr** is the object of type [`Addr`].
/// * **denom** is the object of type [`String`].
pub fn query_balance(
    querier: &QuerierWrapper,
    account_addr: Addr,
    denom: String,
) -> StdResult<Uint128> {
    let balance: BalanceResponse = querier.query(&QueryRequest::Bank(BankQuery::Balance {
        address: String::from(account_addr),
        denom,
    }))?;
    Ok(balance.amount.amount)
}

/// ## Description
/// Returns the token balance at the specified contract address.
/// ## Params
/// * **querier** is the object of type [`QuerierWrapper`].
/// * **contract_addr** is the object of type [`Addr`]. Sets the address of the contract for which
/// the balance will be requested
/// * **account_addr** is the object of type [`Addr`].
pub fn query_token_balance(
    querier: &QuerierWrapper,
    contract_addr: Addr,
    account_addr: Addr,
) -> StdResult<Uint128> {
    // load balance from the token contract
    querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: String::from(contract_addr),
            msg: to_binary(&Cw20QueryMsg::Balance {
                address: String::from(account_addr),
            })?,
        }))
        .map(|res: Cw20BalanceResponse| Ok(res.balance))
        .unwrap_or(Ok(Uint128::zero()))
}

/// ## Description
/// Returns the total supply at the specified contract address.
/// ## Params
/// * **querier** is the object of type [`QuerierWrapper`].
/// * **contract_addr** is the object of type [`Addr`].
pub fn query_supply(querier: &QuerierWrapper, contract_addr: Addr) -> StdResult<Uint128> {
    let res: TokenInfoResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: String::from(contract_addr),
        msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
    }))?;

    Ok(res.total_supply)
}

/// ## Description
/// Returns the token precision at the specified asset of type [`AssetInfo`].
/// ## Params
/// * **querier** is the object of type [`QuerierWrapper`].
/// * **asset_info** is the object of type [`AssetInfo`].
pub fn query_token_precision(querier: &QuerierWrapper, asset_info: AssetInfo) -> StdResult<u8> {
    Ok(match asset_info {
        AssetInfo::NativeToken { denom: _ } => NATIVE_TOKEN_PRECISION,
        AssetInfo::Token { contract_addr } => {
            let res: TokenInfoResponse =
                querier.query_wasm_smart(contract_addr, &Cw20QueryMsg::TokenInfo {})?;

            res.decimals
        }
    })
}

/// Query total supply of a denom
pub fn query_denom_supply(querier: &QuerierWrapper, denom: String) -> StdResult<Uint128> {
    let res: Coin = querier.query_supply(denom)?;
    Ok(res.amount)
}

// Query token info for a cw20 token
pub fn query_token_info(querier: &QuerierWrapper, contract_addr: Addr) -> StdResult<TokenInfoResponse> {
    querier.query_wasm_smart(contract_addr, &Cw20QueryMsg::TokenInfo {})
}

/// Returns the configuration for the Vault contract.
/// ## Params
/// * **querier** is an object of type [`QuerierWrapper`].
///
/// * **factory_contract** is an object of type [`impl Into<String>`] which is the Dexter Vault contract address.
pub fn query_vault_config(
    querier: &QuerierWrapper,
    vault_contract: String,
) -> StdResult<vault::ConfigResponse> {
    querier.query_wasm_smart(vault_contract, &vault::QueryMsg::Config {})
}

/// Returns the configuration for the Pool contract.
/// ## Params
/// * **querier** is an object of type [`QuerierWrapper`].
/// * **pool_contract** is an object of type [`String`] which is the Dexter Vault contract address.
pub fn config_info_by_pool(
    querier: &QuerierWrapper,
    pool_contract: String,
) -> StdResult<pool::ConfigResponse> {
    let config: pool::ConfigResponse =
        querier.query_wasm_smart(pool_contract, &pool::QueryMsg::Config {})?;
    Ok(config)
}
