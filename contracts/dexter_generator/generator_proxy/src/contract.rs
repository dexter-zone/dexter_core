use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, SubMsg, Uint128, WasmMsg,
};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg};

use crate::error::ContractError;
use crate::state::{Config, CONFIG};
use dexter::generator_proxy::{
    CallbackMsg, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
};
use dexter::helper::build_transfer_token_to_user_msg;
use dexter::ref_staking::{
    Cw20HookMsg as StakingCw20HookMsg, ExecuteMsg as StakingExecuteMsg,
    QueryMsg as StakingQueryMsg, StakerInfoResponse,
};

use cw2::set_contract_version;

// version info for migration info
const CONTRACT_NAME: &str = "dexter-generator-proxy";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        generator_contract_addr: deps.api.addr_validate(&msg.generator_contract_addr)?,
        pair_addr: deps.api.addr_validate(&msg.pair_addr)?,
        lp_token_addr: deps.api.addr_validate(&msg.lp_token_addr)?,
        reward_contract_addr: deps.api.addr_validate(&msg.reward_contract_addr)?,
        reward_token: msg.reward_token,
    };
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::UpdateRewards {} => update_rewards(deps),
        ExecuteMsg::SendRewards { account, amount } => send_rewards(deps, info, account, amount),
        ExecuteMsg::Withdraw { account, amount } => withdraw(deps, env, info, account, amount),
        ExecuteMsg::EmergencyWithdraw { account, amount } => {
            withdraw(deps, env, info, account, amount)
        }
        ExecuteMsg::Callback(msg) => handle_callback(deps, env, info, msg),
    }
}

pub fn handle_callback(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CallbackMsg,
) -> Result<Response, ContractError> {
    // Callback functions can only be called this contract itself
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }
    match msg {
        CallbackMsg::TransferLpTokensAfterWithdraw {
            account,
            prev_lp_balance,
        } => transfer_lp_tokens_after_withdraw(deps, env, account, prev_lp_balance),
    }
}

/// @dev Receives LP tokens sent by Generator contract.
/// Stakes them with the Anchor LP Staking contract
fn receive_cw20(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let mut response = Response::new();
    let cfg = CONFIG.load(deps.storage)?;

    if let Ok(Cw20HookMsg::Deposit {}) = from_binary(&cw20_msg.msg) {
        if cw20_msg.sender != cfg.generator_contract_addr || info.sender != cfg.lp_token_addr {
            return Err(ContractError::Unauthorized {});
        }
        response
            .messages
            .push(SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cfg.lp_token_addr.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: cfg.reward_contract_addr.to_string(),
                    amount: cw20_msg.amount,
                    msg: to_binary(&StakingCw20HookMsg::Bond {})?,
                })?,
            })));
    } else {
        return Err(ContractError::IncorrectCw20HookMessageVariant {});
    }
    Ok(response)
}

/// @dev Claims pending rewards from the proxy LP staking contract
fn update_rewards(deps: DepsMut) -> Result<Response, ContractError> {
    let mut response = Response::new();
    let cfg = CONFIG.load(deps.storage)?;

    response
        .messages
        .push(SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.reward_contract_addr.to_string(),
            funds: vec![],
            msg: to_binary(&StakingExecuteMsg::Withdraw {})?,
        })));

    Ok(response)
}

/// @dev Transfers proxy rewards
/// @param account : User to which proxy tokens are to be transferred
/// @param amount : Number of proxy to be transferred
fn send_rewards(
    deps: DepsMut,
    info: MessageInfo,
    account: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let mut response = Response::new();
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.generator_contract_addr {
        return Err(ContractError::Unauthorized {});
    };
    response
        .messages
        .push(SubMsg::new(build_transfer_token_to_user_msg(
            cfg.reward_token,
            account,
            amount,
        )?));
    Ok(response)
}

/// @dev Withdraws LP Tokens from the staking contract. Rewards are NOT claimed when withdrawing LP tokens
/// @param account : User to which LP tokens are to be transferred
/// @param amount : Number of LP to be unstaked and transferred
fn withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    account: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let mut response = Response::new();
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.generator_contract_addr {
        return Err(ContractError::Unauthorized {});
    };

    // current LP Tokens balance
    let prev_lp_balance = {
        let res: BalanceResponse = deps.querier.query_wasm_smart(
            &cfg.lp_token_addr,
            &Cw20QueryMsg::Balance {
                address: env.contract.address.to_string(),
            },
        )?;
        res.balance
    };

    // withdraw from the end reward contract
    response.messages.push(SubMsg::new(WasmMsg::Execute {
        contract_addr: cfg.reward_contract_addr.to_string(),
        funds: vec![],
        msg: to_binary(&StakingExecuteMsg::Unbond { amount })?,
    }));

    // Callback function
    response.messages.push(SubMsg::new(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        funds: vec![],
        msg: to_binary(&ExecuteMsg::Callback(
            CallbackMsg::TransferLpTokensAfterWithdraw {
                account,
                prev_lp_balance,
            },
        ))?,
    }));

    Ok(response)
}

pub fn transfer_lp_tokens_after_withdraw(
    deps: DepsMut,
    env: Env,
    account: Addr,
    prev_lp_balance: Uint128,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // Calculate number of LP Tokens withdrawn from the staking contract
    let amount = {
        let res: BalanceResponse = deps.querier.query_wasm_smart(
            &cfg.lp_token_addr,
            &Cw20QueryMsg::Balance {
                address: env.contract.address.to_string(),
            },
        )?;
        res.balance - prev_lp_balance
    };

    Ok(Response::new().add_message(WasmMsg::Execute {
        contract_addr: cfg.lp_token_addr.to_string(),
        funds: vec![],
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: account.to_string(),
            amount,
        })?,
    }))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    let cfg = CONFIG.load(deps.storage)?;
    match msg {
        QueryMsg::Config {} => to_binary(&ConfigResponse {
            generator_contract_addr: cfg.generator_contract_addr.to_string(),
            pair_addr: cfg.pair_addr.to_string(),
            lp_token_addr: cfg.lp_token_addr.to_string(),
            reward_contract_addr: cfg.reward_contract_addr.to_string(),
            reward_token: cfg.reward_token,
        }),
        QueryMsg::Deposit {} => {
            let res: StakerInfoResponse = deps.querier.query_wasm_smart(
                cfg.reward_contract_addr,
                &StakingQueryMsg::StakerInfo {
                    staker: env.contract.address.to_string(),
                    block_time: Some(env.block.time.seconds()),
                },
            )?;
            let deposit_amount = res.bond_amount;
            to_binary(&deposit_amount)
        }
        QueryMsg::Reward {} => {
            let reward_amount = cfg
                .reward_token
                .query_for_balance(&deps.querier, env.contract.address)?;
            to_binary(&reward_amount)
        }
        QueryMsg::PendingToken {} => {
            let res: StakerInfoResponse = deps.querier.query_wasm_smart(
                cfg.reward_contract_addr,
                &StakingQueryMsg::StakerInfo {
                    staker: env.contract.address.to_string(),
                    block_time: Some(env.block.time.seconds()),
                },
            )?;
            let pending_reward = res.pending_reward;
            to_binary(&Some(pending_reward))
        }
        QueryMsg::RewardInfo {} => {
            let config = CONFIG.load(deps.storage)?;
            to_binary(&config.reward_token)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
