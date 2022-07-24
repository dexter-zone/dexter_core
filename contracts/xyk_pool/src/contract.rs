use crate::error::ContractError;
use crate::state::{CONFIG, MATHCONFIG};
use std::convert::TryInto;

use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Addr, Binary, Coin, CosmosMsg, Decimal, Decimal256,
    Deps, DepsMut, Env, Event, Fraction, MessageInfo, Reply, ReplyOn, Response, StdError,
    StdResult, SubMsg, Uint128, Uint256, WasmMsg,
};

use crate::response::MsgInstantiateContractResponse;
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};
use dexter::asset::{addr_validate_to_lower, Asset, AssetInfo};
// use dexter::generator::Cw20HookMsg as GeneratorHookMsg;
use dexter::vault::{PoolInfo, PoolType, SwapKind};
// use dexter::pool::{ConfigResponse, DEFAULT_SLIPPAGE, MAX_ALLOWED_SLIPPAGE};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{
    SwapResponse,
    AfterJoinResponse,
    Config, // SimulationResponse, TWAP_PRECISION, , CumulativePricesResponse, Cw20HookMsg, ReverseSimulationResponse,
    ConfigResponse,
    ExecuteMsg,
    FeeResponse,
    InstantiateMsg,
    MigrateMsg,
    // PoolResponse,
    QueryMsg,
    ResponseType,
};
use dexter::querier::{query_fee_info, query_supply, query_vault_config};
use protobuf::Message;
use std::str::FromStr;
use std::vec;

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter::xyk_pool";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// A `reply` call code ID of sub-message.
const INSTANTIATE_TOKEN_REPLY_ID: u64 = 1;

/// ## Description
/// Creates a new contract with the specified parameters in the [`InstantiateMsg`].
/// Returns the [`Response`] with the specified attributes if the operation was successful, or a [`ContractError`] if the contract was not created
/// ## Params
/// * **deps** is the object of type [`DepsMut`].
///
/// * **env** is the object of type [`Env`].
///
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

    // Create [`Asset`] from [`AssetInfo`]
    let assets = msg
        .asset_infos
        .iter()
        .map(|a| Asset {
            info: a.clone(),
            amount: Uint128::zero(),
        })
        .collect();

    let config = Config {
        pool_id: msg.pool_id,
        lp_token_addr: None,
        vault_addr: msg.vault_addr,
        assets,
        pool_type: msg.pool_type,
        fee_info: msg.fee_info,
        block_time_last: env.block.time.seconds(),
    };

    CONFIG.save(deps.storage, &config)?;

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

    // Ok(Response::new().add_submessages(sub_msg))
}

/// # Description
/// The entry point to the contract for processing the reply from the submessage
/// # Params
/// * **deps** is the object of type [`DepsMut`].
///
/// * **_env** is the object of type [`Env`].
///
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
///
/// * **env** is the object of type [`Env`].
///
/// * **info** is the object of type [`MessageInfo`].
///
/// * **msg** is the object of type [`ExecuteMsg`].
///
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        // ExecuteMsg::UpdateConfig { .. } => Err(ContractError::NonSupported {}),
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

    // Acess Check :: Only Vault can execute this function
    if info.sender != config.vault_addr {
        return Err(ContractError::Unauthorized {});
    }

    // Update state
    config.assets = assets;
    config.block_time_last = env.block.time.seconds();
    CONFIG.save(deps.storage, &config)?;

    // Accumulate prices for oracle
    // if let Some((price0_cumulative_new, price1_cumulative_new, block_time)) =
    //     accumulate_prices(env, &config, pools[0].amount, pools[1].amount)?
    // {
    //     config.price0_cumulative_last = price0_cumulative_new;
    //     config.price1_cumulative_last = price1_cumulative_new;
    //     config.block_time_last = block_time;
    //     CONFIG.save(deps.storage, &config)?;
    // }

    let event =
        Event::new("dexter-pool::update-liquidity").add_attribute("pool_id", pool_id.to_string());
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
        QueryMsg::OnJoinPool { assets_in } => to_binary(&query_on_join_pool(deps, env, assets_in)?),
        QueryMsg::OnExitPool {
            assets_out,
            burn_amount,
        } => to_binary(&query_on_exit_pool(deps, assets_out, burn_amount)?),
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
        // QueryMsg::CumulativePrice { asset } => {
        //     to_binary(&query_cumulative_price(deps, env, asset)?)
        // }
        // QueryMsg::CumulativePrices {} => to_binary(&query_cumulative_prices(deps, env)?),
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
/// XYK POOL -::- MATH LOGIC
/// T.B.A
pub fn query_on_join_pool(
    deps: Deps,
    env: Env,
    assets_in: Vec<Asset>,
) -> StdResult<AfterJoinResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    assets_in.sort_by(|a, b| {
        a.info
            .to_string()
            .to_lowercase()
            .cmp(&b.info.to_string().to_lowercase())
    });

    // Since its a XYK Pool, there will be only 2 assets
    let deposits: [Uint128; 2] = [assets_in[0].amount, assets_in[1].amount];

    // Total share of LP tokens minted by the pool
    let total_share = query_supply(&deps.querier, config.lp_token_addr.unwrap().clone())?;

    let new_shares = if total_share.is_zero() {
        // Initial share = collateral amount. Square root of product of deposit token balances
        Uint128::new(
            (dexter::U256::from(deposits[0].u128()) * dexter::U256::from(deposits[1].u128()))
                .integer_sqrt()
                .as_u128(),
        )
    } else {
        // min(1, 2)
        // 1. sqrt(deposit_0 * exchange_rate_0_to_1 * deposit_0) * (total_share / sqrt(pool_0 * pool_1))
        // == deposit_0 * total_share / pool_0
        // 2. sqrt(deposit_1 * exchange_rate_1_to_0 * deposit_1) * (total_share / sqrt(pool_0 * pool_1))
        // == deposit_1 * total_share / pool_1
        std::cmp::min(
            deposits[0].multiply_ratio(total_share, config.assets[0].amount),
            deposits[1].multiply_ratio(total_share, config.assets[1].amount),
        )
    };

    let return_assets = assets_in
        .iter()
        .map(|a| Asset {
            info: a.info.clone(),
            amount: Uint128::zero(),
        })
        .collect();

    let res = AfterJoinResponse {
        return_assets,
        new_shares,
        response: dexter::pool::ResponseType::Success {},
    };

    Ok(res)
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
/// XYK POOL -::- MATH LOGIC
/// T.B.A
pub fn query_on_exit_pool(
    deps: Deps,
    env: Env,
    assets_out: Option<Vec<Asset>>,
    burn_amount: Uint128,
) -> StdResult<Vec<Asset>> {
    let config: Config = CONFIG.load(deps.storage)?;

    // Total share of LP tokens minted by the pool
    let total_share = query_supply(&deps.querier, config.lp_token_addr.unwrap().clone())?;

    // Number of tokens that will be transferred against the LP tokens burnt
    let refund_assets = get_share_in_assets(config.assets, burn_amount, total_share);
    Ok(refund_assets)
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

    let offer_asset: Asset;
    let ask_asset: Asset;

    // Based on swap_type, we set the amount to either offer_asset or ask_asset pool
    match swap_type {
        SwapKind::In {} => {
            offer_asset = Asset {
                info: offer_asset_info.clone(),
                amount,
            };
            ask_asset = Asset {
                info: ask_asset_info.clone(),
                amount: Uint128::zero(),
            };
        }
        SwapKind::Out {} => {
            offer_asset = Asset {
                info: offer_asset_info.clone(),
                amount: Uint128::zero(),
            };
            ask_asset = Asset {
                info: ask_asset_info.clone(),
                amount,
            };
        }
    }


    let cur_offer_asset_bal: Uint128;
    let ask_pool: Uint128;

    if offer_asset.info.equal(&config.assets[0].info) {
        offer_pool = config.assets[0].amount;
        ask_pool = config.assets[1].amount;
    } else if offer_asset.info.equal(&config.assets[1].info) {
        offer_pool = config.assets[1].amount;
        ask_pool = config.assets[0].amount;
    } else {
        return Err(StdError::generic_err(
            "Given offer asset doesn't belong to pairs",
        ));
    }


    let offer_amount = offer_asset.amount;
    let (return_amount, spread_amount, commission_amount) = compute_swap(
        offer_pool.amount,
        ask_pool.amount,
        offer_amount,
        Decimal::from_ratio(
            Uint128::from(config.fee_info.total_fee_bps),
            Uint128::from(1000u128),
        ),
    )?;

    let protocol_fee = commission_amount * config.fee_info.protocol_fee_bps;
    let dev_fee = commission_amount * config.fee_info.dev_fee_bps;

    Ok(SwapResponse {
        return_amount,
        spread_amount,
        commission_amount,
        protocol_fee,
        dev_fee,
    })
}

/// ## Description
/// Returns computed swap for the pool with specified parameters
/// ## Params
/// * **offer_pool** is the object of type [`Uint128`]. Sets the offer pool.
///
/// * **ask_pool** is the object of type [`Uint128`]. Sets the ask pool.
///
/// * **offer_amount** is the object of type [`Uint128`]. Sets the offer amount.
///
/// * **commission_rate** is the object of type [`Decimal`]. Sets the commission rate.
pub fn compute_swap(
    offer_pool: Uint128,
    ask_pool: Uint128,
    offer_amount: Uint128,
    commission_rate: Decimal,
) -> StdResult<(Uint128, Uint128, Uint128)> {
    let offer_pool: Uint256 = offer_pool.into();
    let ask_pool: Uint256 = ask_pool.into();
    let offer_amount: Uint256 = offer_amount.into();
    let commission_rate = decimal2decimal256(commission_rate)?;

    // offer => ask
    // ask_amount = (ask_pool - cp / (offer_pool + offer_amount))
    let cp: Uint256 = offer_pool * ask_pool;
    let return_amount: Uint256 = (Decimal256::from_ratio(ask_pool, 1u8)
        - Decimal256::from_ratio(cp, offer_pool + offer_amount))
        * Uint256::from(1u8);

    // calculate spread & commission
    let spread_amount: Uint256 =
        (offer_amount * Decimal256::from_ratio(ask_pool, offer_pool)) - return_amount;
    let commission_amount: Uint256 = return_amount * commission_rate;

    // commission will be absorbed to pool
    let return_amount: Uint256 = return_amount - commission_amount;
    Ok((
        return_amount.try_into()?,
        spread_amount.try_into()?,
        commission_amount.try_into()?,
    ))
}

/// ## Description
/// Used for migration of contract. Returns the default object of type [`Response`].
/// ## Params
/// * **_deps** is the object of type [`DepsMut`].
///
/// * **_env** is the object of type [`Env`].
///
/// * **_msg** is the object of type [`MigrateMsg`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}

/// ## Description
/// Converts [`Decimal`] to [`Decimal256`].
pub fn decimal2decimal256(dec_value: Decimal) -> StdResult<Decimal256> {
    Decimal256::from_atomics(dec_value.atomics(), dec_value.decimal_places()).map_err(|_| {
        StdError::generic_err(format!(
            "Failed to convert Decimal {} to Decimal256",
            dec_value
        ))
    })
}

/// ## Description
/// Returns the share of assets.
/// ## Params
/// * **pools** are an array of [`Asset`] type items.
/// * **burn_amount** denotes the number of LP tokens to be burnt and is the object of type [`Uint128`].
/// * **total_share** is total supply of LP token and is the object of type [`Uint128`].
pub fn get_share_in_assets(
    pools: Vec<Asset>,
    burn_amount: Uint128,
    total_share: Uint128,
) -> Vec<Asset> {
    let mut share_ratio = Decimal::zero();
    // % share of LP tokens to be burnt in total Pool
    if !total_share.is_zero() {
        share_ratio = Decimal::from_ratio(burn_amount, total_share);
    }
    pools
        .iter()
        .map(|a| Asset {
            info: a.info.clone(),
            amount: a.amount * share_ratio,
        })
        .collect()
}

// / ## Description
// / Shifts block_time when any price is zero to not fill an accumulator with a new price to that period.
// / ## Params
// / * **env** is the object of type [`Env`].
// /
// / * **config** is the object of type [`Config`].
// /
// / * **x** is the balance of asset[0] within a pool
// /
// / * **y** is the balance of asset[1] within a pool
// pub fn accumulate_prices(
//     env: Env,
//     config: &Config,
//     x: Uint128,
//     y: Uint128,
// ) -> StdResult<Option<(Uint128, Uint128, u64)>> {
//     let block_time = env.block.time.seconds();
//     if block_time <= config.block_time_last {
//         return Ok(None);
//     }

//     // we have to shift block_time when any price is zero to not fill an accumulator with a new price to that period

//     let time_elapsed = Uint128::from(block_time - config.block_time_last);

//     let mut pcl0 = config.price0_cumulative_last;
//     let mut pcl1 = config.price1_cumulative_last;

//     if !x.is_zero() && !y.is_zero() {
//         let price_precision = Uint128::from(10u128.pow(TWAP_PRECISION.into()));
//         pcl0 = config.price0_cumulative_last.wrapping_add(
//             time_elapsed
//                 .checked_mul(price_precision)?
//                 .multiply_ratio(y, x),
//         );
//         pcl1 = config.price1_cumulative_last.wrapping_add(
//             time_elapsed
//                 .checked_mul(price_precision)?
//                 .multiply_ratio(x, y),
//         );
//     };

//     Ok(Some((pcl0, pcl1, block_time)))
// }

/// ## Description
/// Returns information about the simulation of the swap in a [`SimulationResponse`] object.
/// ## Params
/// * **deps** is the object of type [`Deps`].
///
/// * **offer_asset** is the object of type [`Asset`].
pub fn query_simulation(deps: Deps, offer_asset: Asset) -> StdResult<SimulationResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    let contract_addr = config.pair_info.contract_addr.clone();

    let pools: [Asset; 2] = config.pair_info.query_pools(&deps.querier, contract_addr)?;

    let offer_pool: Asset;
    let ask_pool: Asset;
    if offer_asset.info.equal(&pools[0].info) {
        offer_pool = pools[0].clone();
        ask_pool = pools[1].clone();
    } else if offer_asset.info.equal(&pools[1].info) {
        offer_pool = pools[1].clone();
        ask_pool = pools[0].clone();
    } else {
        return Err(StdError::generic_err(
            "Given offer asset doesn't belong to pairs",
        ));
    }

    // Get fee info from factory
    let fee_info = query_fee_info(
        &deps.querier,
        config.factory_addr,
        config.pair_info.pool_type,
    )?;

    let (return_amount, spread_amount, commission_amount) = compute_swap(
        offer_pool.amount,
        ask_pool.amount,
        offer_asset.amount,
        fee_info.total_fee_rate,
    )?;

    Ok(SimulationResponse {
        return_amount,
        spread_amount,
        commission_amount,
    })
// }

// /// ## Description
// /// Returns information about the reverse simulation in a [`ReverseSimulationResponse`] object.
// /// ## Params
// /// * **deps** is the object of type [`Deps`].
// ///
// /// * **ask_asset** is the object of type [`Asset`].
// pub fn query_reverse_simulation(
//     deps: Deps,
//     ask_asset: Asset,
// ) -> StdResult<ReverseSimulationResponse> {
//     let config: Config = CONFIG.load(deps.storage)?;
//     let contract_addr = config.pair_info.contract_addr.clone();

//     let pools: [Asset; 2] = config.pair_info.query_pools(&deps.querier, contract_addr)?;

//     let offer_pool: Asset;
//     let ask_pool: Asset;
//     if ask_asset.info.equal(&pools[0].info) {
//         ask_pool = pools[0].clone();
//         offer_pool = pools[1].clone();
//     } else if ask_asset.info.equal(&pools[1].info) {
//         ask_pool = pools[1].clone();
//         offer_pool = pools[0].clone();
//     } else {
//         return Err(StdError::generic_err(
//             "Given ask asset doesn't belong to pairs",
//         ));
//     }

//     // Get fee info from factory
//     let fee_info = query_fee_info(
//         &deps.querier,
//         config.factory_addr,
//         config.pair_info.pool_type,
//     )?;

//     let (offer_amount, spread_amount, commission_amount) = compute_offer_amount(
//         offer_pool.amount,
//         ask_pool.amount,
//         ask_asset.amount,
//         fee_info.total_fee_rate,
//     )?;

//     Ok(ReverseSimulationResponse {
//         offer_amount,
//         spread_amount,
//         commission_amount,
//     })
// }

// / ## Description
// / Returns information about the cumulative prices in a [`CumulativePricesResponse`] object.
// / ## Params
// / * **deps** is the object of type [`Deps`].
// /
// / * **env** is the object of type [`Env`].
// pub fn query_cumulative_price(deps: Deps, env: Env, asset: AssetInfo) -> StdResult<CumulativePriceResponse> {
//     let config: Config = CONFIG.load(deps.storage)?;
//     let (assets, total_share) = pool_info(deps, config.clone())?;

//     let mut price0_cumulative_last = config.price0_cumulative_last;
//     let mut price1_cumulative_last = config.price1_cumulative_last;

//     if let Some((price0_cumulative_new, price1_cumulative_new, _)) =
//         accumulate_prices(env, &config, assets[0].amount, assets[1].amount)?
//     {
//         price0_cumulative_last = price0_cumulative_new;
//         price1_cumulative_last = price1_cumulative_new;
//     }

//     let resp = CumulativePriceResponse {
//         price_cumulative_last,
//         total_share
//     };

//     Ok(resp)
// }

// /// ## Description
// /// Returns information about the cumulative prices in a [`CumulativePricesResponse`] object.
// /// ## Params
// /// * **deps** is the object of type [`Deps`].
// ///
// /// * **env** is the object of type [`Env`].
// pub fn query_cumulative_prices(deps: Deps, env: Env,) -> StdResult<CumulativePriceResponse> {
//     let config: Config = CONFIG.load(deps.storage)?;
//     let (assets, total_share) = pool_info(deps, config.clone())?;

//     let mut price0_cumulative_last = config.price0_cumulative_last;
//     let mut price1_cumulative_last = config.price1_cumulative_last;

//     if let Some((price0_cumulative_new, price1_cumulative_new, _)) =
//         accumulate_prices(env, &config, assets[0].amount, assets[1].amount)?
//     {
//         price0_cumulative_last = price0_cumulative_new;
//         price1_cumulative_last = price1_cumulative_new;
//     }

//     let resp = CumulativePriceResponse {
//         price_cumulative_last,
//         total_share
//     };

//     Ok(resp)
// }

// / ## Description
// / Returns computed offer amount for the pool with specified parameters.
// / ## Params
// / * **offer_pool** is the object of type [`Uint128`]. Sets the offer pool.
// /
// / * **ask_pool** is the object of type [`Uint128`]. Sets the ask pool.
// /
// / * **offer_amount** is the object of type [`Uint128`]. Sets the ask amount.
// /
// / * **commission_rate** is the object of type [`Decimal`]. Sets the commission rate.
// fn compute_offer_amount(
//     offer_pool: Uint128,
//     ask_pool: Uint128,
//     ask_amount: Uint128,
//     commission_rate: Decimal,
// ) -> StdResult<(Uint128, Uint128, Uint128)> {
//     // ask => offer
//     // offer_amount = cp / (ask_pool - ask_amount / (1 - commission_rate)) - offer_pool
//     let cp = Uint256::from(offer_pool) * Uint256::from(ask_pool);
//     let one_minus_commission = Decimal256::one() - decimal2decimal256(commission_rate)?;
//     let inv_one_minus_commission = Decimal256::one() / one_minus_commission;

//     let offer_amount: Uint128 = cp
//         .multiply_ratio(
//             Uint256::from(1u8),
//             Uint256::from(
//                 ask_pool.checked_sub(
//                     (Uint256::from(ask_amount) * inv_one_minus_commission).try_into()?,
//                 )?,
//             ),
//         )
//         .checked_sub(offer_pool.into())?
//         .try_into()?;

//     let before_commission_deduction = Uint256::from(ask_amount) * inv_one_minus_commission;
//     let spread_amount = (offer_amount * Decimal::from_ratio(ask_pool, offer_pool))
//         .checked_sub(before_commission_deduction.try_into()?)
//         .unwrap_or_else(|_| Uint128::zero());
//     let commission_amount = before_commission_deduction * decimal2decimal256(commission_rate)?;
//     Ok((offer_amount, spread_amount, commission_amount.try_into()?))
// }

// /// ## Description
// /// Returns an [`ContractError`] on failure, otherwise if `belief_price` and `max_spread` both are given, we compute new spread else we just use swap
// /// spread to check `max_spread`.
// /// ## Params
// /// * **belief_price** is the object of type [`Option<Decimal>`]. Sets the belief price.
// ///
// /// * **max_spread** is the object of type [`Option<Decimal>`]. Sets the maximum spread.
// ///
// /// * **offer_amount** is the object of type [`Uint128`]. Sets the offer amount.
// ///
// /// * **return_amount** is the object of type [`Uint128`]. Sets the return amount.
// ///
// /// * **spread_amount** is the object of type [`Uint128`]. Sets the spread amount.
// pub fn assert_max_spread(
//     belief_price: Option<Decimal>,
//     max_spread: Option<Decimal>,
//     offer_amount: Uint128,
//     return_amount: Uint128,
//     spread_amount: Uint128,
// ) -> Result<(), ContractError> {
//     let default_spread = Decimal::from_str(DEFAULT_SLIPPAGE)?;
//     let max_allowed_spread = Decimal::from_str(MAX_ALLOWED_SLIPPAGE)?;

//     let max_spread = max_spread.unwrap_or(default_spread);
//     if max_spread.gt(&max_allowed_spread) {
//         return Err(ContractError::AllowedSpreadAssertion {});
//     }

//     if let Some(belief_price) = belief_price {
//         let expected_return = offer_amount * belief_price.inv().unwrap();
//         let spread_amount = expected_return
//             .checked_sub(return_amount)
//             .unwrap_or_else(|_| Uint128::zero());

//         if return_amount < expected_return
//             && Decimal::from_ratio(spread_amount, expected_return) > max_spread
//         {
//             return Err(ContractError::MaxSpreadAssertion {});
//         }
//     } else if Decimal::from_ratio(spread_amount, return_amount + spread_amount) > max_spread {
//         return Err(ContractError::MaxSpreadAssertion {});
//     }

//     Ok(())
// }

// /// ## Description
// /// Ensures each prices are not dropped as much as slippage tolerance rate.
// /// Returns an [`ContractError`] on failure, otherwise returns [`Ok`].
// /// ## Params
// /// * **slippage_tolerance** is the object of type [`Option<Decimal>`].
// ///
// /// * **deposits** are an array of [`Uint128`] type items.
// ///
// /// * **pools** are an array of [`Asset`] type items.
// fn assert_slippage_tolerance(
//     slippage_tolerance: Option<Decimal>,
//     deposits: &[Uint128; 2],
//     pools: &[Asset; 2],
// ) -> Result<(), ContractError> {
//     let default_slippage = Decimal::from_str(DEFAULT_SLIPPAGE)?;
//     let max_allowed_slippage = Decimal::from_str(MAX_ALLOWED_SLIPPAGE)?;

//     let slippage_tolerance = slippage_tolerance.unwrap_or(default_slippage);
//     if slippage_tolerance.gt(&max_allowed_slippage) {
//         return Err(ContractError::AllowedSpreadAssertion {});
//     }

//     let slippage_tolerance: Decimal256 = decimal2decimal256(slippage_tolerance)?;
//     let one_minus_slippage_tolerance = Decimal256::one() - slippage_tolerance;
//     let deposits: [Uint256; 2] = [deposits[0].into(), deposits[1].into()];
//     let pools: [Uint256; 2] = [pools[0].amount.into(), pools[1].amount.into()];

//     // Ensure each prices are not dropped as much as slippage tolerance rate
//     if Decimal256::from_ratio(deposits[0], deposits[1]) * one_minus_slippage_tolerance
//         > Decimal256::from_ratio(pools[0], pools[1])
//         || Decimal256::from_ratio(deposits[1], deposits[0]) * one_minus_slippage_tolerance
//             > Decimal256::from_ratio(pools[1], pools[0])
//     {
//         return Err(ContractError::MaxSlippageAssertion {});
//     }

//     Ok(())
// }
