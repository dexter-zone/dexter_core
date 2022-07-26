use crate::error::ContractError;
use crate::state::{Twap, CONFIG, TWAPINFO};
use std::convert::TryInto;

use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Addr, Binary, Decimal, Decimal256, Deps, DepsMut,
    Env, Event, Fraction, MessageInfo, Reply, ReplyOn, Response, StdError, StdResult, SubMsg,
    Uint128, Uint256, WasmMsg,
};

use crate::response::MsgInstantiateContractResponse;
use cw2::set_contract_version;
use cw20::MinterResponse;
use dexter::asset::{addr_validate_to_lower, Asset, AssetExchangeRate, AssetInfo};
use dexter::helpers::decimal2decimal256;
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{
    AfterExitResponse, AfterJoinResponse, Config, ConfigResponse, CumulativePriceResponse,
    CumulativePricesResponse, ExecuteMsg, FeeResponse, InstantiateMsg, MigrateMsg, QueryMsg,
    ResponseType, SwapResponse, Trade,
};
use dexter::querier::query_supply;
use dexter::vault::{SwapType, TWAP_PRECISION};

use protobuf::Message;
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

    let twap = Twap {
        price0_cumulative_last: Uint128::zero(),
        price1_cumulative_last: Uint128::zero(),
        block_time_last: 0,
    };

    CONFIG.save(deps.storage, &config)?;
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
    if let Some((price0_cumulative_new, price1_cumulative_new, block_time)) =
        accumulate_prices(env, &twap, config.assets[0].amount, config.assets[1].amount)?
    {
        twap.price0_cumulative_last = price0_cumulative_new;
        twap.price1_cumulative_last = price1_cumulative_new;
        twap.block_time_last = block_time;
        TWAPINFO.save(deps.storage, &twap)?;
    }

    let event = Event::new("dexter-pool::update-liquidity")
        .add_attribute("pool_id", config.pool_id.to_string())
        .add_attribute(
            config.assets[0].info.as_string(),
            twap.price0_cumulative_last.to_string(),
        )
        .add_attribute(
            config.assets[1].info.as_string(),
            twap.price1_cumulative_last.to_string(),
        )
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
        QueryMsg::OnJoinPool { assets_in } => to_binary(&query_on_join_pool(deps, env, assets_in)?),
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
/// Returns [`AfterExitResponse`] type which contains -  
/// assets_out - Is of type [`Vec<Asset>`] and is a sorted list consisting of amount and info of tokens which are to be subtracted from the PoolInfo state stored in the Vault contract and tranfer from the Vault to the user
/// burn_shares - Number of LP shares to be burnt
/// response - A [`ResponseType`] which is either `Success` or `Failure`, deteriming if the tx is accepted by the Pool's math computations or not
/// ## Params
/// assets_out - Of type [`Vec<Asset>`], a sorted list containing amount / info of token balances user wants against the LP tokens transferred by the user to the Vault contract
/// * **deps** is the object of type [`Deps`].
/// XYK POOL -::- MATH LOGIC
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

    // Number of tokens that will be transferred against the LP tokens burnt
    let assets_out = get_share_in_assets(config.assets, burn_amount, total_share);

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
    swap_type: SwapType,
    offer_asset_info: AssetInfo,
    ask_asset_info: AssetInfo,
    amount: Uint128,
) -> StdResult<SwapResponse> {
    let config: Config = CONFIG.load(deps.storage)?;

    let cur_offer_asset_bal: Uint128;
    let cur_ask_asset_bal: Uint128;

    // Get the current pool balance of the offer_asset and ask_asset
    if offer_asset_info.equal(&config.assets[0].info) {
        cur_offer_asset_bal = config.assets[0].amount;
        cur_ask_asset_bal = config.assets[1].amount;
    } else if offer_asset_info.equal(&config.assets[1].info) {
        cur_offer_asset_bal = config.assets[1].amount;
        cur_ask_asset_bal = config.assets[0].amount;
    } else {
        Ok(SwapResponse {
            trade_params: Trade {
                amount_in: Uint128::zero(),
                amount_out: Uint128::zero(),
                spread: Uint128::zero(),
                total_fee: Uint128::zero(),
                protocol_fee: Uint128::zero(),,
                dev_fee: Uint128::zero(),,
            },
            response: ResponseType::Failure {},
        })
    }

    let mut offer_asset: Asset;
    let mut ask_asset: Asset;
    let (calc_amount, spread_amount, commission_amount): (Uint128, Uint128, Uint128);

    // Based on swap_type, we set the amount to either offer_asset or ask_asset pool
    match swap_type {
        SwapType::GiveIn {} => {
            // Calculate the number of ask_asset tokens to be transferred to the recepient from the Vault
            (calc_amount, spread_amount, commission_amount) = compute_swap(
                cur_offer_asset_bal,
                cur_ask_asset_bal,
                amount,
                config.fee_info.total_fee_bps,
            )?;
            offer_asset = Asset {
                info: offer_asset_info.clone(),
                amount,
            };
            ask_asset = Asset {
                info: ask_asset_info.clone(),
                amount: calc_amount,
            };
        }
        SwapType::GiveOut {} => {
            // Calculate the number of offer_asset tokens to be transferred from the trader from the Vault
            (calc_amount, spread_amount, commission_amount) = compute_offer_amount(
                cur_offer_asset_bal,
                cur_ask_asset_bal,
                amount,
                config.fee_info.total_fee_bps,
            )?;
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

    let total_share = query_supply(&deps.querier, config.lp_token_addr.unwrap().clone())?;

    let mut price0_cumulative_last = twap.price0_cumulative_last;
    let mut price1_cumulative_last = twap.price1_cumulative_last;

    if let Some((price0_cumulative_new, price1_cumulative_new, _)) =
        accumulate_prices(env, &twap, config.assets[0].amount, config.assets[1].amount)?
    {
        price0_cumulative_last = price0_cumulative_new;
        price1_cumulative_last = price1_cumulative_new;
    }

    // Get exchange rate
    let exchange_rate: Uint128;
    if offer_asset.equal(&config.assets[0].info) && ask_asset.equal(&config.assets[1].info) {
        exchange_rate = price0_cumulative_last;
    } else if offer_asset.equal(&config.assets[1].info) && ask_asset.equal(&config.assets[0].info) {
        exchange_rate = price1_cumulative_last;
    } else {
        return Err(StdError::generic_err("Invalid asset"));
    }

    let resp = CumulativePriceResponse {
        exchange_info: AssetExchangeRate {
            offer_info: offer_asset,
            ask_info: ask_asset,
            rate: exchange_rate,
        },
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

    let mut price0_cumulative_last = twap.price0_cumulative_last;
    let mut price1_cumulative_last = twap.price1_cumulative_last;

    let mut exchange_infos: Vec<AssetExchangeRate> = vec![];

    if let Some((price0_cumulative_new, price1_cumulative_new, _)) =
        accumulate_prices(env, &twap, config.assets[0].amount, config.assets[1].amount)?
    {
        price0_cumulative_last = price0_cumulative_new;
        price1_cumulative_last = price1_cumulative_new;
    }

    exchange_infos.push(AssetExchangeRate {
        offer_info: config.assets[0].info,
        ask_info: config.assets[1].info,
        rate: price0_cumulative_last,
    });
    exchange_infos.push(AssetExchangeRate {
        offer_info: config.assets[0].info,
        ask_info: config.assets[1].info,
        rate: price1_cumulative_last,
    });

    Ok(CumulativePricesResponse {
        exchange_infos: exchange_infos,
        total_share,
    })
}

/// ## Description
/// Returns computed swap for the pool with specified parameters
/// ## Params
/// * **offer_pool** is the object of type [`Uint128`]. Sets the offer pool.
/// * **ask_pool** is the object of type [`Uint128`]. Sets the ask pool.
/// * **offer_amount** is the object of type [`Uint128`]. Sets the offer amount.
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

// / ## Description
// / Returns computed offer amount for the pool with specified parameters.
// / ## Params
// / * **offer_pool** is the object of type [`Uint128`]. Sets the offer pool.
// / * **ask_pool** is the object of type [`Uint128`]. Sets the ask pool.
// / * **offer_amount** is the object of type [`Uint128`]. Sets the ask amount.
// / * **commission_rate** is the object of type [`Decimal`]. Sets the commission rate.
fn compute_offer_amount(
    offer_pool: Uint128,
    ask_pool: Uint128,
    ask_amount: Uint128,
    commission_rate: Decimal,
) -> StdResult<(Uint128, Uint128, Uint128)> {
    // ask => offer
    // offer_amount = cp / (ask_pool - ask_amount / (1 - commission_rate)) - offer_pool
    let cp = Uint256::from(offer_pool) * Uint256::from(ask_pool);
    let one_minus_commission = Decimal256::one() - decimal2decimal256(commission_rate)?;
    let inv_one_minus_commission = Decimal256::one() / one_minus_commission;

    let offer_amount: Uint128 = cp
        .multiply_ratio(
            Uint256::from(1u8),
            Uint256::from(
                ask_pool.checked_sub(
                    (Uint256::from(ask_amount) * inv_one_minus_commission).try_into()?,
                )?,
            ),
        )
        .checked_sub(offer_pool.into())?
        .try_into()?;

    let before_commission_deduction = Uint256::from(ask_amount) * inv_one_minus_commission;
    let spread_amount = (offer_amount * Decimal::from_ratio(ask_pool, offer_pool))
        .checked_sub(before_commission_deduction.try_into()?)
        .unwrap_or_else(|_| Uint128::zero());
    let commission_amount = before_commission_deduction * decimal2decimal256(commission_rate)?;
    Ok((offer_amount, spread_amount, commission_amount.try_into()?))
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
// / * **config** is the object of type [`Config`].
// / * **x** is the balance of asset[0] within a pool
// / * **y** is the balance of asset[1] within a pool
pub fn accumulate_prices(
    env: Env,
    twap: &Twap,
    x: Uint128,
    y: Uint128,
) -> StdResult<Option<(Uint128, Uint128, u64)>> {
    let block_time = env.block.time.seconds();
    if block_time <= twap.block_time_last {
        return Ok(None);
    }

    // we have to shift block_time when any price is zero to not fill an accumulator with a new price to that period

    let time_elapsed = Uint128::from(block_time - twap.block_time_last);

    let mut pcl0 = twap.price0_cumulative_last;
    let mut pcl1 = twap.price1_cumulative_last;

    if !x.is_zero() && !y.is_zero() {
        let price_precision = Uint128::from(10u128.pow(TWAP_PRECISION.into()));
        pcl0 = twap.price0_cumulative_last.wrapping_add(
            time_elapsed
                .checked_mul(price_precision)?
                .multiply_ratio(y, x),
        );
        pcl1 = twap.price1_cumulative_last.wrapping_add(
            time_elapsed
                .checked_mul(price_precision)?
                .multiply_ratio(x, y),
        );
    };

    Ok(Some((pcl0, pcl1, block_time)))
}

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
