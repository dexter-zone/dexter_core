#[cfg(not(feature = "library"))]
use crate::error::ContractError;
use crate::state::{Config, CONFIG};

use cosmwasm_std::{
    attr, entry_point, to_binary, Attribute, Binary, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult,
};
use cw2::set_contract_version;
use dexter::asset::{addr_validate_to_lower, Asset, AssetInfo};
use dexter::keeper::{BalancesResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use dexter::querier::query_vault_config;

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter-keeper";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x      Instantiate Contract : Execute function     x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// ## Description
/// Creates a new contract with the specified parameters in [`InstantiateMsg`].
/// Returns a default object of type [`Response`] if the operation was successful,
/// or a [`ContractError`] if the contract was not created.
///
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
/// * **_env** is an object of type [`Env`].
/// * **_info** is an object of type [`MessageInfo`].
/// * **msg** is a message of type [`InstantiateMsg`] which contains the parameters used for creating the contract.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let cfg = Config {
        vault_contract: addr_validate_to_lower(deps.api, &msg.vault_contract)?,
        dex_token_contract: None,
        staking_contract: None,
    };

    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::default())
}

// ----------------x----------------x----------------x------------------x----------------x----------------
// ----------------x----------------x  Execute function :: Entry Point  x----------------x----------------
// ----------------x----------------x----------------x------------------x----------------x----------------

/// ## Description
/// Exposes execute functions available in the contract.
/// ## Params
/// * **deps** is an object of type [`Deps`].
/// * **env** is an object of type [`Env`].
/// * **_info** is an object of type [`MessageInfo`].
/// * **msg** is an object of type [`ExecuteMsg`].
///
/// * **ExecuteMsg::UpdateConfig {
///             vault_contract,
///             staking_contract,
///         }** Updates general contract settings stores in the [`Config`].
///
/// * **ExecuteMsg::DistributeFees {}** Private method used by the contract to distribute collected Fees.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig {
            dex_token_contract,
            staking_contract,
        } => update_config(deps, info, dex_token_contract, staking_contract),
    }
}

/// ## Description
/// Updates general contarct parameters. Returns a [`ContractError`] on failure or the [`Config`]
/// data will be updated if the transaction is successful.
///
/// ## Params
/// * **dex_token_contract** is an [`Option`] field of type [`String`]. This is the address of the DEX token contract.
/// * **staking_contract** is an [`Option`] field of type [`String`]. This is the address of the DEX token staking contract.
///
/// ##Executor
/// Only the owner can execute this.
fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    dex_token_contract: Option<String>,
    staking_contract: Option<String>,
) -> Result<Response, ContractError> {
    let mut attributes = vec![attr("action", "set_config")];

    let mut config = CONFIG.load(deps.storage)?;
    let vault_config_res = query_vault_config(&deps.querier, config.vault_contract.to_string())?;

    // Permission check
    if info.sender != vault_config_res.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Set DEX token contract
    if let Some(dex_token_contract) = dex_token_contract {
        if config.dex_token_contract.is_some() {
            return Err(ContractError::DexTokenAlreadySet {});
        }
        config.dex_token_contract = Some(addr_validate_to_lower(deps.api, &dex_token_contract)?);
        attributes.push(Attribute::new("dex_token_contract", &dex_token_contract));
    };

    // Set Staking contract
    if let Some(staking_contract) = staking_contract {
        if config.staking_contract.is_some() {
            return Err(ContractError::StakingAddrAlreadySet {});
        }
        config.staking_contract = Some(addr_validate_to_lower(deps.api, &staking_contract)?);
        attributes.push(Attribute::new("staking_contract", &staking_contract));
    };

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attributes(attributes))
}

// ----------------x----------------x---------------------x-----------------------x----------------x----------------
// ----------------x----------------x  :::: Keeper::QUERIES Implementation   ::::  x----------------x----------------
// ----------------x----------------x---------------------x-----------------------x----------------x----------------

/// ## Description
/// Exposes all the queries available in the contract.
///
/// # Params
/// * **deps** is an object of type [`DepsMut`].
/// * **env** is an object of type [`Env`].
/// * **msg** is an object of type [`QueryMsg`].
///
/// ## Queries
/// * **QueryMsg::Config {}** Returns the Keeper contract configuration using a [`ConfigResponse`] object.
/// * **QueryMsg::Balances { assets }** Returns the balances of certain tokens accrued by the Keeper
///
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_get_config(deps)?),
        QueryMsg::Balances { assets } => to_binary(&query_get_balances(deps, env, assets)?),
    }
}

/// ## Description
/// Returns information about the Keeper configuration using a [`ConfigResponse`] object.
/// ## Params
/// * **deps** is an object of type [`Deps`].
fn query_get_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        dex_token_contract: config.dex_token_contract,
        vault_contract: config.vault_contract,
        staking_contract: config.staking_contract,
    })
}

/// ## Description
/// Returns Keeper's fee token balances for specific tokens using a [`ConfigResponse`] object.
/// ## Params
/// * **deps** is an object of type [`Deps`].
/// * **env** is an object of type [`Env`].
/// * **assets** is a vector that contains objects of type [`AssetInfo`]. These are the assets for which we query the Keeper's balances.
fn query_get_balances(deps: Deps, env: Env, assets: Vec<AssetInfo>) -> StdResult<BalancesResponse> {
    let mut resp = BalancesResponse { balances: vec![] };

    for a in assets {
        // Get balance
        let balance = a.query_for_balance(&deps.querier, env.contract.address.clone())?;
        if !balance.is_zero() {
            resp.balances.push(Asset {
                info: a,
                amount: balance,
            })
        }
    }

    Ok(resp)
}
