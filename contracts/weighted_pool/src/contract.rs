use crate::error::ContractError;
use crate::state::{Twap, CONFIG, MathConfig, MATHCONFIG, TWAPINFO, get_normalized_weight};
use std::convert::TryInto;

use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Addr, Binary, Decimal, Decimal256, Deps, DepsMut,
    Env, Event, Fraction, MessageInfo, Reply, ReplyOn, Response, StdError, StdResult, SubMsg,
    Uint128, Uint256, WasmMsg,
};

use crate::response::MsgInstantiateContractResponse;
use cw2::set_contract_version;
use cw20::MinterResponse;
use dexter::helpers::{select_pools, check_swap_parameters};
use dexter::asset::{addr_validate_to_lower, Asset, AssetExchangeRate, AssetInfo};
use dexter::helpers::decimal2decimal256;
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{
    AfterExitResponse, AfterJoinResponse, Config, ConfigResponse, CumulativePriceResponse,
    CumulativePricesResponse, ExecuteMsg, FeeResponse, InstantiateMsg, MigrateMsg, QueryMsg,
    ResponseType, SwapResponse, Trade,
};
use crate::utils::{
    accumulate_prices,calc_ask_amount, calc_offer_amount};

use dexter::querier::query_supply;
use dexter::vault::{SwapKind, TWAP_PRECISION};

use protobuf::Message;
use std::vec;

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter::WEIGHTED_pool";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// A `reply` call code ID of sub-message.
const INSTANTIATE_TOKEN_REPLY_ID: u64 = 1;

/// ## Description
/// Creates a new contract with the specified parameters in the [`InstantiateMsg`].
/// Returns the [`Response`] with the specified attributes if the operation was successful, or a [`ContractError`] if the contract was not created
/// ## Params
/// * **deps** is the object of type [`DepsMut`].
/// * **env** is the object of type [`Env`].
/// * **_info** is the object of type [`MessageInfo`].
/// * **msg** is a message of type [`InstantiateMsg`] which contains the basic settings for creating a contract
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // check valid token info
    msg.validate()?;

    let weights: Vec<Uint128> = from_binary(&msg.init_params.unwrap())?;
    let greatest_precision = store_precisions(deps.branch(), &msg.asset_infos)?;


    // Create [`Asset`] from [`AssetInfo`]
    let assets = msg
        .asset_infos
        .iter()
        .map(|a| Asset {
            info: a.clone(),
            amount: Uint128::zero(),
        })
        .collect();
    
    // Number of assets and weights must be equal
    if assets.len() != weights.len() {
        return Err(ContractError::InvalidParams(
            "Number of assets and weights must be equal".to_string(),
        ));
    }
    
    // Initializing cumulative prices
    let mut cumulative_prices = vec![];
    for from_pool in &msg.asset_infos {
        for to_pool in &msg.asset_infos {
            if !from_pool.eq(to_pool) {
                cumulative_prices.push((from_pool.clone(), to_pool.clone(), Uint128::zero()))
            }
        }
    }

    let config = Config {
        pool_id: msg.pool_id,
        lp_token_addr: None,
        vault_addr: msg.vault_addr,
        assets,
        pool_type: msg.pool_type,
        fee_info: msg.fee_info,
        block_time_last: env.block.time.seconds(),
    };

    let twap = Twap {
        cumulative_prices: cumulative_prices,
        block_time_last: 0,
    };

    let math_config = MathConfig {
        weights: weights,
        greatest_precision: 0,
    };    

    CONFIG.save(deps.storage, &config)?;
    MATHCONFIG.save(deps.storage, &math_config)?;
    TWAPINFO.save(deps.storage, &twap)?;

    // LP Token Name
    let mut token_name = msg.pool_id.to_string() + "-Dexter-LP".to_string().as_str();
    if !msg.lp_token_name.is_none() {
        token_name = msg.pool_id.to_string()
            + "-DEX-LP-".to_string().as_str()
            + msg.lp_token_name.unwrap().as_str();
    }

    // LP Token Symbol
    let mut token_symbol = "LP-".to_string() + msg.pool_id.to_string().as_str();
    if !msg.lp_token_symbol.is_none() {
        token_symbol = msg.lp_token_symbol.unwrap();
    }

    // Create LP token
    let sub_msg: Vec<SubMsg> = vec![SubMsg {
        msg: WasmMsg::Instantiate {
            code_id: msg.lp_token_code_id,
            msg: to_binary(&TokenInstantiateMsg {
                name: token_name,
                symbol: token_symbol,
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: env.contract.address.to_string(),
                    cap: None,
                }),
                marketing: None,
            })?,
            funds: vec![],
            admin: None,
            label: String::from("Dexter LP token"),
        }
        .into(),
        id: INSTANTIATE_TOKEN_REPLY_ID,
        gas_limit: None,
        reply_on: ReplyOn::Success,
    }];
    Ok(Response::new().add_submessages(sub_msg))
}

/// # Description
/// The entry point to the contract for processing the reply from the submessage
/// # Params
/// * **deps** is the object of type [`DepsMut`].
/// * **_env** is the object of type [`Env`].
/// * **msg** is the object of type [`Reply`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    if config.lp_token_addr.is_some() {
        return Err(ContractError::Unauthorized {});
    }

    let data = msg.result.unwrap().data.unwrap();
    let res: MsgInstantiateContractResponse =
        Message::parse_from_bytes(data.as_slice()).map_err(|_| {
            StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
        })?;

    config.lp_token_addr = Some(addr_validate_to_lower(
        deps.api,
        res.get_contract_address(),
    )?);

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("liquidity_token_addr", config.lp_token_addr.unwrap()))
}

/// ## Description
/// Available the execute messages of the contract.
/// ## Params
/// * **deps** is the object of type [`Deps`].
/// * **env** is the object of type [`Env`].
/// * **info** is the object of type [`MessageInfo`].
/// * **msg** is the object of type [`ExecuteMsg`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig { .. } => Err(ContractError::NonSupported {}),
        ExecuteMsg::UpdateLiquidity { assets } => {
            execute_update_pool_liquidity(deps, env, info, assets)
        }
    }
}

/// ## Description
/// Admin Access by Vault :: Callable only by Dexter::Vault --> Updates locally stored asset balances state
/// Operation --> Updates locally stored [`Asset`] state
/// Returns an [`ContractError`] on failure, otherwise returns the [`Response`] with the specified
/// attributes if the operation was successful.
/// ## Params
/// * **assets** is a field of type [`Vec<Asset>`]. It is a sorted list of `Asset` which contain the token type details and new updates balances of tokens as accounted by the pool
pub fn execute_update_pool_liquidity(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    assets: Vec<Asset>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;
    let mut twap: Twap = TWAPINFO.load(deps.storage)?;

    // Acess Check :: Only Vault can execute this function
    if info.sender != config.vault_addr {
        return Err(ContractError::Unauthorized {});
    }

    // Update state
    config.assets = assets;
    config.block_time_last = env.block.time.seconds();
    CONFIG.save(deps.storage, &config)?;

    // Accumulate prices for the assets in the pool

    if !accumulate_prices(deps.as_ref(), env, &mut twap, &pools).is_ok() {
        return Err(ContractError::InvalidState {
            msg: "Failed to accumulate prices".to_string(),
        });
    }
    TWAPINFO.save(deps.storage, &twap)?;

    let event = Event::new("dexter-pool::update-liquidity")
        .add_attribute("pool_id", config.pool_id.to_string())
        .add_attribute("block_time_last", twap.block_time_last.to_string());

    Ok(Response::new().add_event(event))
}

/// ## Description
/// Available the query messages of the contract.
/// ## Params
/// * **deps** is the object of type [`Deps`].
/// * **_env** is the object of type [`Env`].
/// * **msg** is the object of type [`QueryMsg`].
/// ## Queries
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::FeeParams {} => to_binary(&query_fee_params(deps)?),
        QueryMsg::PoolId {} => to_binary(&query_pool_id(deps)?),
        QueryMsg::OnJoinPool { assets_in , mint_amount} => to_binary(&query_on_join_pool(deps, env, assets_in, mint_amount)?),
        QueryMsg::OnExitPool {
            assets_out,
            burn_amount,
        } => to_binary(&query_on_exit_pool(deps, env, assets_out, burn_amount)?),
        QueryMsg::OnSwap {
            swap_type,
            offer_asset,
            ask_asset,
            amount,
        } => to_binary(&query_on_swap(
            deps,
            env,
            swap_type,
            offer_asset,
            ask_asset,
            amount,
        )?),
        QueryMsg::CumulativePrice {
            offer_asset,
            ask_asset,
        } => to_binary(&query_cumulative_price(deps, env, offer_asset, ask_asset)?),
        QueryMsg::CumulativePrices {} => to_binary(&query_cumulative_prices(deps, env)?),
    }
}

/// ## Description
/// Returns information about the controls settings in a [`ConfigResponse`] object.
/// ## Params
/// * **deps** is the object of type [`Deps`].
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        pool_id: config.pool_id,
        lp_token_addr: config.lp_token_addr,
        vault_addr: config.vault_addr,
        assets: config.assets,
        pool_type: config.pool_type,
        fee_info: config.fee_info,
        block_time_last: config.block_time_last,
    })
}

/// ## Description
/// Returns information about the Fees settings in a [`FeeResponse`] object.
/// ## Params
/// * **deps** is the object of type [`Deps`].
pub fn query_fee_params(deps: Deps) -> StdResult<FeeResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    Ok(FeeResponse {
        total_fee_bps: config.fee_info.total_fee_bps,
        protocol_fee_bps: config.fee_info.protocol_fee_bps,
        dev_fee_bps: config.fee_info.dev_fee_bps,
        dev_fee_collector: config.fee_info.dev_addr_bps,
    })
}

/// ## Description
/// Returns information Pool ID which is of type [`Uint128`]
/// ## Params
/// * **deps** is the object of type [`Deps`].
pub fn query_pool_id(deps: Deps) -> StdResult<Uint128> {
    let config: Config = CONFIG.load(deps.storage)?;
    Ok(config.pool_id)
}

/// ## Description
/// Returns [`AfterJoinResponse`] type which contains -  
/// return_assets - Is of type [`Vec<Asset>`] and is a sorted list consisting of amount of info of tokens which are to be subtracted from
/// the token balances provided by the user to the Vault, to get the final list of token balances to be provided as Liquiditiy against the minted LP shares
/// new_shares - New LP shares which are to be minted
/// response - A [`ResponseType`] which is either `Success` or `Failure`, deteriming if the tx is accepted by the Pool's math computations or not
/// ## Params
/// assets_in - Of type [`Vec<Asset>`], a sorted list containing amount / info of token balances to be supplied as liquidity to the pool
/// * **deps** is the object of type [`Deps`].
/// WEIGHTED POOL -::- MATH LOGIC
/// T.B.A
pub fn query_on_join_pool(
    deps: Deps,
    env: Env,
    assets_in: Option<Vec<Asset>>,
    mint_amount: Option<Vec<Asset>>,
) -> StdResult<AfterJoinResponse> {
    let config: Config = CONFIG.load(deps.storage)?;


    // Total share of LP tokens minted by the pool
    let total_share = query_supply(&deps.querier, config.lp_token_addr.unwrap().clone())?;

    let normalized_weights : Vec<u128>;

    normalized_weights = config.assets.iter().map(|a| {
        let weight = WEIGHTS.load(deps.storage, a.info.to_string() )?;
        weight.u128()
    }).collect();

    let assets_in = calc_tokens_in_given_exact_lp_minted(config.assets, normalized_weights, total_share, mint_amount, config.fee_info.total_fee_bps)?;

    let res = AfterJoinResponse {
        return_assets,
        new_shares: mint_amount,
        response: dexter::pool::ResponseType::Success {},
    };

    Ok(res)
}

/// ## Description
/// Returns [`AfterExitResponse`] type which contains -  
/// assets_out - Is of type [`Vec<Asset>`] and is a sorted list consisting of amount and info of tokens which are to be subtracted from the PoolInfo state stored in the Vault contract and tranfer from the Vault to the user
/// burn_shares - Number of LP shares to be burnt
/// response - A [`ResponseType`] which is either `Success` or `Failure`, deteriming if the tx is accepted by the Pool's math computations or not
/// ## Params
/// assets_out - Of type [`Vec<Asset>`], a sorted list containing amount / info of token balances user wants against the LP tokens transferred by the user to the Vault contract
/// * **deps** is the object of type [`Deps`].
/// WEIGHTED POOL -::- MATH LOGIC
/// T.B.A
pub fn query_on_exit_pool(
    deps: Deps,
    env: Env,
    assets_out: Option<Vec<Asset>>,
    burn_amount: Uint128,
) -> StdResult<AfterExitResponse> {
    let config: Config = CONFIG.load(deps.storage)?;

    // Total share of LP tokens minted by the pool
    let total_share = query_supply(&deps.querier, config.lp_token_addr.unwrap().clone())?;

    let normalized_weights : Vec<u128>;

    normalized_weights = config.assets.iter().map(|a| {
        let weight = WEIGHTS.load(deps.storage, a.info.to_string() )?;
        weight.u128()
    }).collect();



    // Number of tokens that will be transferred against the LP tokens burnt
    let assets_out = calc_tokens_out_given_exact_lp_burnt(config.assets, normalized_weights, total_share, burn_amount, config.fee_info.total_fee_bps)?;

    Ok(AfterExitResponse {
        assets_out,
        burn_shares: burn_amount,
        response: dexter::pool::ResponseType::Success {},
    })
}

// Returns number of LP shares that will be minted
pub fn query_on_swap(
    deps: Deps,
    env: Env,
    swap_type: SwapKind,
    offer_asset_info: AssetInfo,
    ask_asset_info: AssetInfo,
    amount: Uint128,
) -> StdResult<SwapResponse> {
    let config: Config = CONFIG.load(deps.storage)?;

    let pools = config.assets.into_iter().map(|pool| {
        let token_precision = get_precision(deps.storage, &pool.info)?;
        Ok(Asset {
            amount: adjust_precision(pool.amount, token_precision, math_config.greatest_precision)?,
            ..pool
        })
    }).collect::<StdResult<Vec<_>>>()?;

    let (offer_pool, ask_pool) = select_pools(Some(&offer_asset_info), ask_asset_info.as_ref(), &pools)?;

    if check_swap_parameters(offer_pool.amount, ask_pool.amount, offer_asset.amount).is_err() {
        // return Ok(SimulationResponse {
        //     return_amount: Uint128::zero(),
        //     spread_amount: Uint128::zero(),
        //     commission_amount: Uint128::zero(),
        // });
    }

    // Get normalized weights of the pools
    let offer_p_weight = WEIGHTS.load(deps.storage)?.get(&offer_pool.info.to_string()).unwrap();
    let ask_p_weight = WEIGHTS.load(deps.storage)?.get(&ask_pool.info.to_string()).unwrap();

    let mut offer_asset: Asset;
    let mut ask_asset: Asset;
    let (spread_amount, commission_amount): (Uint128, Uint128);
    let mut calc_amount = Uint128::zero();

    // Based on swap_type, we set the amount to either offer_asset or ask_asset pool
    match swap_type {
        SwapKind::In {} => {
            // Calculate the number of ask_asset tokens to be transferred to the recepient from the Vault
            (calc_amount, spread_amount, commission_amount) = calc_offer_amount(
                offer_pool.amount,
                offer_p_weight,
                ask_pool.amount,
                ask_p_weight,
                amount
            );            
            offer_asset = Asset {
                info: offer_asset_info.clone(),
                amount,
            };
            ask_asset = Asset {
                info: ask_asset_info.clone(),
                amount: calc_amount,
            };
        }
        SwapKind::Out {} => {
            // Calculate the number of offer_asset tokens to be transferred from the trader from the Vault
            (calc_amount, spread_amount, commission_amount) =  calc_ask_amount(
                offer_pool.amount,
                offer_p_weight,
                ask_pool.amount,
                ask_p_weight,
                amount
            );
            offer_asset = Asset {
                info: offer_asset_info.clone(),
                amount: calc_amount,
            };
            ask_asset = Asset {
                info: ask_asset_info.clone(),
                amount,
            };
        }
    }

    let protocol_fee = commission_amount * config.fee_info.protocol_fee_bps;
    let dev_fee = commission_amount * config.fee_info.dev_fee_bps;

    Ok(SwapResponse {
        trade_params: Trade {
            amount_in: offer_asset.amount,
            amount_out: ask_asset.amount,
            spread: spread_amount,
            total_fee: commission_amount,
            protocol_fee,
            dev_fee,
        },
        response: ResponseType::Success {},
    })
}

// / ## Description
// / Returns information about the cumulative price of the asset in a [`CumulativePriceResponse`] object.
// / ## Params
// / * **deps** is the object of type [`Deps`].
// / * **env** is the object of type [`Env`].
// / * **offer_asset** is the object of type [`AssetInfo`].
// / * **ask_asset** is the object of type [`AssetInfo`].
pub fn query_cumulative_price(
    deps: Deps,
    env: Env,
    offer_asset: AssetInfo,
    ask_asset: AssetInfo,
) -> StdResult<CumulativePriceResponse> {
    let twap: Twap = TWAPINFO.load(deps.storage)?;
    let config: Config = CONFIG.load(deps.storage)?;

    // let total_share = query_supply(&deps.querier, config.lp_token_addr.unwrap().clone())?;

    // accumulate_prices(deps, env, &mut config, &assets)
    // .map_err(|err| StdError::generic_err(format!("{err}")))?;

    let resp = CumulativePriceResponse {
        exchange_info: _,
        total_share,
    };

    Ok(resp)
}

// /// ## Description
// /// Returns information about the cumulative prices in a [`CumulativePricesResponse`] object.
// /// ## Params
// /// * **deps** is the object of type [`Deps`].
// /// * **env** is the object of type [`Env`].
pub fn query_cumulative_prices(deps: Deps, env: Env) -> StdResult<CumulativePriceResponse> {
    let twap: Twap = TWAPINFO.load(deps.storage)?;
    let config: Config = CONFIG.load(deps.storage)?;

    let total_share = query_supply(&deps.querier, config.lp_token_addr.unwrap().clone())?;

    accumulate_prices(deps, env, &mut config, &assets)
    .map_err(|err| StdError::generic_err(format!("{err}")))?;

    Ok(CumulativePricesResponse {
        exchange_infos: twap.cumulative_prices,
        total_share,
    })
}

// /// ## Description
// /// Returns the share of assets.
// /// ## Params
// /// * **pools** are an array of [`Asset`] type items.
// /// * **burn_amount** denotes the number of LP tokens to be burnt and is the object of type [`Uint128`].
// /// * **total_share** is total supply of LP token and is the object of type [`Uint128`].
// pub fn get_share_in_assets(
//     pools: Vec<Asset>,
//     burn_amount: Uint128,
//     total_share: Uint128,
// ) -> Vec<Asset> {
//     let mut share_ratio = Decimal::zero();
//     // % share of LP tokens to be burnt in total Pool
//     if !total_share.is_zero() {
//         share_ratio = Decimal::from_ratio(burn_amount, total_share);
//     }
//     pools
//         .iter()
//         .map(|a| Asset {
//             info: a.info.clone(),
//             amount: a.amount * share_ratio,
//         })
//         .collect()
// }

/// ## Description
/// Used for migration of contract. Returns the default object of type [`Response`].
/// ## Params
/// * **_deps** is the object of type [`DepsMut`].
/// * **_env** is the object of type [`Env`].
/// * **_msg** is the object of type [`MigrateMsg`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
