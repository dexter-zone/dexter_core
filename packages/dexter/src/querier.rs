use crate::asset::{Asset, AssetInfo};
// use crate::pool::{QueryMsg as PoolQueryMsg, ReverseSimulationResponse, SimulationResponse};
use crate::vault::{
    ConfigResponse as VaultConfigResponse, FeeInfo, PoolConfigResponse, PoolType,
    QueryMsg as VaultQueryMsg,
};

use cosmwasm_std::{
    to_binary, Addr, AllBalanceResponse, BalanceResponse, BankQuery, Coin, Decimal, QuerierWrapper,
    QueryRequest, StdResult, Uint128, WasmQuery,
};

use cw20::{BalanceResponse as Cw20BalanceResponse, Cw20QueryMsg, TokenInfoResponse};

const NATIVE_TOKEN_PRECISION: u8 = 6;

/// ## Description
/// Returns the balance of the denom at the specified account address.
/// ## Params
/// * **querier** is the object of type [`QuerierWrapper`].
///
/// * **account_addr** is the object of type [`Addr`].
///
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
/// Returns the total balance for all coins at the specified account address.
/// ## Params
/// * **querier** is the object of type [`QuerierWrapper`].
///
/// * **account_addr** is the object of type [`Addr`].
pub fn query_all_balances(querier: &QuerierWrapper, account_addr: Addr) -> StdResult<Vec<Coin>> {
    let all_balances: AllBalanceResponse =
        querier.query(&QueryRequest::Bank(BankQuery::AllBalances {
            address: String::from(account_addr),
        }))?;
    Ok(all_balances.amount)
}

/// ## Description
/// Returns the token balance at the specified contract address.
/// ## Params
/// * **querier** is the object of type [`QuerierWrapper`].
///
/// * **contract_addr** is the object of type [`Addr`]. Sets the address of the contract for which
/// the balance will be requested
///
/// * **account_addr** is the object of type [`Addr`].
pub fn query_token_balance(
    querier: &QuerierWrapper,
    contract_addr: Addr,
    account_addr: Addr,
) -> StdResult<Uint128> {
    // load balance from the token contract
    let res: Cw20BalanceResponse = querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: String::from(contract_addr),
            msg: to_binary(&Cw20QueryMsg::Balance {
                address: String::from(account_addr),
            })?,
        }))
        .unwrap_or_else(|_| Cw20BalanceResponse {
            balance: Uint128::zero(),
        });

    Ok(res.balance)
}

/// ## Description
/// Returns the token symbol at the specified contract address.
/// ## Params
/// * **querier** is the object of type [`QuerierWrapper`].
///
/// * **contract_addr** is the object of type [`Addr`].
pub fn query_token_symbol(querier: &QuerierWrapper, contract_addr: Addr) -> StdResult<String> {
    let res: TokenInfoResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: String::from(contract_addr),
        msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
    }))?;

    Ok(res.symbol)
}

/// ## Description
/// Returns the total supply at the specified contract address.
/// ## Params
/// * **querier** is the object of type [`QuerierWrapper`].
///
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

/// ## Description
/// Returns the config of factory contract address.
/// ## Params
/// * **querier** is the object of type [`QuerierWrapper`].
///
/// * **vault_contract** is the object of type [`Addr`].
pub fn query_vault_config(
    querier: &QuerierWrapper,
    vault_contract: Addr,
) -> StdResult<VaultConfigResponse> {
    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: vault_contract.to_string(),
        msg: to_binary(&VaultQueryMsg::Config {})?,
    }))
}

/// ## Description
/// Returns the fee information at the specified pair type.
/// ## Params
/// * **vault_contract** is the object of type [`Addr`].
/// * **pool_type** is the object of type [`PoolType`].
pub fn query_fee_info(
    querier: &QuerierWrapper,
    vault_contract: Addr,
    pool_type: PoolType,
) -> StdResult<FeeInfo> {
    let res: PoolConfigResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: vault_contract.to_string(),
        msg: to_binary(&VaultQueryMsg::PoolConfig { pool_type })?,
    }))?;

    Ok(res.fee_info)
}

// /// ## Description
// /// Returns information about the simulation of the swap in a [`SimulationResponse`] object.
// /// ## Params
// /// * **querier** is the object of type [`QuerierWrapper`].
// ///
// /// * **pair_contract** is the object of type [`Addr`].
// ///
// /// * **offer_asset** is the object of type [`Asset`].
// pub fn simulate(
//     querier: &QuerierWrapper,
//     pair_contract: Addr,
//     offer_asset: &Asset,
// ) -> StdResult<SimulationResponse> {
//     querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
//         contract_addr: pair_contract.to_string(),
//         msg: to_binary(&PoolQueryMsg::Simulation {
//             offer_asset: offer_asset.clone(),
//         })?,
//     }))
// }

// /// ## Description
// /// Returns information about the reverse simulation in a [`ReverseSimulationResponse`] object.
// /// ## Params
// /// * **querier** is the object of type [`QuerierWrapper`].
// ///
// /// * **pair_contract** is the object of type [`Addr`].
// ///
// /// * **ask_asset** is the object of type [`Asset`].
// pub fn reverse_simulate(
//     querier: &QuerierWrapper,
//     pair_contract: &Addr,
//     ask_asset: &Asset,
// ) -> StdResult<ReverseSimulationResponse> {
//     querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
//         contract_addr: pair_contract.to_string(),
//         msg: to_binary(&PoolQueryMsg::ReverseSimulation {
//             ask_asset: ask_asset.clone(),
//         })?,
//     }))
// }
