use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, Decimal, Decimal256, Deps, DepsMut, Env, Event, Fraction,
    MessageInfo, Response, StdError, StdResult, Uint128, Uint256,
};
use cw2::set_contract_version;
use std::convert::TryInto;
use std::str::FromStr;
use std::vec;

use crate::state::{Twap, CONFIG, TWAPINFO};

use dexter::asset::{Asset, AssetExchangeRate, AssetInfo};
use dexter::error::ContractError;
use dexter::helper::{calculate_underlying_fees, decimal2decimal256, get_share_in_assets};
use dexter::pool::{
    return_exit_failure, return_join_failure, return_swap_failure, AfterExitResponse,
    AfterJoinResponse, Config, ConfigResponse, CumulativePriceResponse, CumulativePricesResponse,
    ExecuteMsg, FeeResponse, InstantiateMsg, MigrateMsg, QueryMsg, ResponseType, SwapResponse,
    Trade, DEFAULT_SLIPPAGE, MAX_ALLOWED_SLIPPAGE,
};
use dexter::querier::query_supply;
use dexter::vault::{SwapType, TWAP_PRECISION};

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter::xyk_pool";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x      Instantiate Contract : Execute function     x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// ## Description
/// Creates a new contract with the specified parameters in the [`InstantiateMsg`].
/// Returns the [`Response`] with the specified attributes if the operation was successful, or a [`ContractError`] if the contract was not created
///
/// ## Params
/// * **msg** is a message of type [`InstantiateMsg`] which contains the basic settings for creating a contract
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    if msg.asset_infos.len() != 2 {
        return Err(ContractError::InvalidNumberOfAssets {
            max_assets: Uint128::from(2u128),
        });
    }

    // Create [`Asset`] from [`AssetInfo`]
    let assets = msg
        .asset_infos
        .iter()
        .map(|a| Asset {
            info: a.clone(),
            amount: Uint128::zero(),
        })
        .collect();

    // Create Config
    let config = Config {
        pool_id: msg.pool_id,
        lp_token_addr: None,
        vault_addr: msg.vault_addr.clone(),
        assets,
        pool_type: msg.pool_type,
        fee_info: msg.fee_info,
        block_time_last: env.block.time.seconds(),
    };

    // Create TWAP
    let twap = Twap {
        price0_cumulative_last: Uint128::zero(),
        price1_cumulative_last: Uint128::zero(),
        block_time_last: 0,
    };

    // Store config and twap
    CONFIG.save(deps.storage, &config)?;
    TWAPINFO.save(deps.storage, &twap)?;

    Ok(Response::new())
}

// ----------------x----------------x----------------x------------------x----------------x----------------
// ----------------x----------------x  Execute function :: Entry Point  x----------------x----------------
// ----------------x----------------x----------------x------------------x----------------x----------------

/// ## Description
/// Available the execute messages of the contract.
///
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
        ExecuteMsg::SetLpToken { lp_token_addr } => set_lp_token(deps, env, info, lp_token_addr),
        ExecuteMsg::UpdateConfig { .. } => Err(ContractError::NonSupported {}),
        ExecuteMsg::UpdateLiquidity { assets } => {
            execute_update_pool_liquidity(deps, env, info, assets)
        }
    }
}

/// ## Description
/// Admin Access by Vault :: Callable only by Dexter::Vault --> Sets LP token address once it is instiantiated.
///                          Returns an [`ContractError`] on failure, otherwise returns the [`Response`] with the specified attributes if the operation was successful.
///
/// ## Params
/// * **lp_token_addr** is a field of type [`Addr`]. It is the address of the LP token.
pub fn set_lp_token(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    lp_token_addr: Addr,
) -> Result<Response, ContractError> {
    // Get config
    let mut config: Config = CONFIG.load(deps.storage)?;

    // Acess Check :: Only Vault can execute this function
    if info.sender != config.vault_addr {
        return Err(ContractError::Unauthorized {});
    }

    // Update state
    config.lp_token_addr = Some(lp_token_addr);
    CONFIG.save(deps.storage, &config)?;

    let event = Event::new("dexter-pool::set_lp_token")
        .add_attribute("lp_token_addr", config.lp_token_addr.unwrap().to_string());
    Ok(Response::new().add_event(event))
}

/// ## Description
/// Admin Access by Vault :: Callable only by Dexter::Vault --> Updates locally stored asset balances state. Operation --> Updates locally stored [`Asset`] state
///                          Returns an [`ContractError`] on failure, otherwise returns the [`Response`] with the specified attributes if the operation was successful.
///
/// ## Params
/// * **assets** is a field of type [`Vec<Asset>`]. It is a sorted list of `Asset` which contain the token type details and new updates balances of tokens as accounted by the pool
pub fn execute_update_pool_liquidity(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    assets: Vec<Asset>,
) -> Result<Response, ContractError> {
    // Get config and twap info
    let mut config: Config = CONFIG.load(deps.storage)?;
    let mut twap: Twap = TWAPINFO.load(deps.storage)?;

    // Acess Check :: Only Vault can execute this function
    if info.sender != config.vault_addr {
        return Err(ContractError::Unauthorized {});
    }

    // Accumulate prices for the assets in the pool
    if let Some((price0_cumulative_new, price1_cumulative_new, block_time)) = accumulate_prices(
        env.clone(),
        &twap,
        config.assets[0].amount,
        config.assets[1].amount,
    )? {
        twap.price0_cumulative_last = price0_cumulative_new;
        twap.price1_cumulative_last = price1_cumulative_new;
        twap.block_time_last = block_time;
        TWAPINFO.save(deps.storage, &twap)?;
    }

    // Update state
    config.assets = assets;
    config.block_time_last = env.block.time.seconds();
    CONFIG.save(deps.storage, &config)?;

    let event = Event::new("dexter-pool::update-liquidity")
        .add_attribute("pool_id", config.pool_id.to_string())
        .add_attribute("vault_address", config.vault_addr)
        .add_attribute("pool_assets", serde_json_wasm::to_string(&config.assets).unwrap())
        .add_attribute("block_time_last", twap.block_time_last.to_string());

    Ok(Response::new().add_event(event))
}

// ----------------x----------------x---------------------x-----------------------x----------------x----------------
// ----------------x----------------x  :::: XYK POOL::QUERIES Implementation   ::::  x----------------x----------------
// ----------------x----------------x---------------------x-----------------------x----------------x----------------

/// ## Description
/// Available the query messages of the contract.
///
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
        QueryMsg::OnJoinPool {
            assets_in,
            mint_amount,
            slippage_tolerance,
        } => to_binary(&query_on_join_pool(
            deps,
            env,
            assets_in,
            mint_amount,
            slippage_tolerance,
        )?),
        QueryMsg::OnExitPool {
            assets_out,
            burn_amount,
        } => to_binary(&query_on_exit_pool(deps, env, assets_out, burn_amount)?),
        QueryMsg::OnSwap {
            swap_type,
            offer_asset,
            ask_asset,
            amount,
            max_spread,
            belief_price,
        } => to_binary(&query_on_swap(
            deps,
            env,
            swap_type,
            offer_asset,
            ask_asset,
            amount,
            max_spread,
            belief_price,
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
///
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
        math_params: None,
        additional_params: None,
    })
}

/// ## Description
/// Returns information about the Fees settings in a [`FeeResponse`] object.
/// ## Params
///
/// * **deps** is the object of type [`Deps`].
pub fn query_fee_params(deps: Deps) -> StdResult<FeeResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    Ok(FeeResponse {
        total_fee_bps: config.fee_info.total_fee_bps,
    })
}

/// ## Description
/// Returns Pool ID which is of type [`Uint128`]
///
/// ## Params
/// * **deps** is the object of type [`Deps`].
pub fn query_pool_id(deps: Deps) -> StdResult<Uint128> {
    let config: Config = CONFIG.load(deps.storage)?;
    Ok(config.pool_id)
}

//--------x------------------x--------------x-----x-----
//--------x    Query :: OnJoin, OnExit, OnSwap    x-----
//--------x------------------x--------------x-----x-----

/// ## Description
/// Returns [`AfterJoinResponse`] type which contains -  
/// provided_assets - Is of type [`Vec<Asset>`] and is a sorted list consisting of amount of info of tokens which will be provided
///  by the user to the Vault
/// new_shares - New LP shares which are to be minted
/// response - A [`ResponseType`] which is either `Success` or `Failure`, deteriming if the tx is accepted by the Pool's math computations or not
/// fee - A [`Option<Asset>`] which is the fee to be charged. XYK pool doesn't charge any fee on pool Joins, so it is `None`
///
/// ## Params
/// assets_in - Of type [`Vec<Asset>`], a sorted list containing amount / info of token balances to be supplied as liquidity to the pool
/// * **deps** is the object of type [`Deps`].
/// XYK POOL -::- MATH LOGIC
/// -- Implementation - For XYK, user provides the exact number of assets he/she wants to supply as liquidity to the pool. We simply calculate the number of LP shares to be minted and return it to the user.
/// T.B.A
pub fn query_on_join_pool(
    deps: Deps,
    _env: Env,
    assets_in: Option<Vec<Asset>>,
    _mint_amount: Option<Uint128>,
    slippage_tolerance: Option<Decimal>,
) -> StdResult<AfterJoinResponse> {
    // If the user has not provided any assets to be provided, then return a `Failure` response
    if assets_in.is_none() {
        return Ok(return_join_failure("No assets provided".to_string()));
    }

    // Load the config from the storage
    let config: Config = CONFIG.load(deps.storage)?;

    // Sort the assets in the order of the assets in the config
    let mut act_assets_in = assets_in.unwrap();
    act_assets_in.sort_by(|a, b| {
        a.info
            .to_string()
            .to_lowercase()
            .cmp(&b.info.to_string().to_lowercase())
    });

    // Since its a XYK Pool, there will be only 2 assets
    let deposits: [Uint128; 2] = [act_assets_in[0].amount, act_assets_in[1].amount];

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
        // Assert slippage tolerance
        let res = assert_slippage_tolerance(
            slippage_tolerance,
            &deposits,
            &[config.assets[0].amount, config.assets[1].amount],
        );
        // return a `Failure` response if the slippage tolerance is not met
        if !res.is_success() {
            return Ok(return_join_failure(res.to_string()));
        }

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

    let res = AfterJoinResponse {
        provided_assets: act_assets_in,
        new_shares,
        response: dexter::pool::ResponseType::Success {},
        fee: None,
    };

    Ok(res)
}

/// ## Description
/// Returns [`AfterExitResponse`] type which contains -  
/// assets_out - Is of type [`Vec<Asset>`] and is a sorted list consisting of number of tokens to be transferred to the recepient from the Vault
/// burn_shares - Number of LP shares to be burnt
/// response - A [`ResponseType`] which is either `Success` or `Failure`, deteriming if the tx is accepted by the Pool's math computations or not
/// fee - A [`Option<Asset>`] which is the fee to be charged. XYK pool doesn't charge any fee on pool Exits, so it is `None`
///
/// ## Params
/// assets_out - Of type [`Vec<Asset>`], a sorted list containing amount / info of token balances user wants against the LP tokens transferred by the user to the Vault contract
/// * **deps** is the object of type [`Deps`].
/// XYK POOL -::- MATH LOGIC
/// T.B.A
pub fn query_on_exit_pool(
    deps: Deps,
    _env: Env,
    _assets_out: Option<Vec<Asset>>,
    burn_amount: Option<Uint128>,
) -> StdResult<AfterExitResponse> {
    // If the user has not provided number of LP tokens to be burnt, then return a `Failure` response
    if burn_amount.is_none() || burn_amount.unwrap().is_zero() {
        return Ok(return_exit_failure(
            "Invalid number of LP tokens to burn amount".to_string(),
        ));
    }

    // Load the config from the storage
    let config: Config = CONFIG.load(deps.storage)?;

    // Total share of LP tokens minted by the pool
    let total_share = query_supply(&deps.querier, config.lp_token_addr.unwrap().clone())?;

    // Number of tokens that will be transferred against the LP tokens burnt
    let assets_out = get_share_in_assets(config.assets, burn_amount.unwrap(), total_share);

    Ok(AfterExitResponse {
        assets_out,
        burn_shares: burn_amount.unwrap(),
        response: dexter::pool::ResponseType::Success {},
        fee: None,
    })
}

/// ## Description
/// Returns [`SwapResponse`] type which contains -  
/// trade_params - Is of type [`Trade`] which contains all params related with the trade, including the number of assets to be traded, spread
/// response - A [`ResponseType`] which is either `Success` or `Failure`, deteriming if the tx is accepted by the Pool's math computations or not
/// fee - A [`Option<Asset>`] which is the fee to be charged. XYK pool charges fee in `AskAsset` on swaps`
///
/// ## Params
///  swap_type - Is of type [`SwapType`] which is either `GiveIn`, `GiveOut` or `Custom`
///  offer_asset_info - Of type [`AssetInfo`] which is the asset info of the asset to be traded in the offer side of the trade
/// ask_asset_info - Of type [`AssetInfo`] which is the asset info of the asset to be traded in the ask side of the trade
/// amount - Of type [`Uint128`] which is the amount of assets to be traded on ask or offer side, based on the swap type
/// XYK POOL -::- MATH LOGIC
/// T.B.A
pub fn query_on_swap(
    deps: Deps,
    _env: Env,
    swap_type: SwapType,
    offer_asset_info: AssetInfo,
    ask_asset_info: AssetInfo,
    amount: Uint128,
    max_spread: Option<Decimal>,
    belief_price: Option<Decimal>,
) -> StdResult<SwapResponse> {
    // Load the config from the storage
    let config: Config = CONFIG.load(deps.storage)?;

    // Current asset balances on the pool
    let cur_offer_asset_bal: Uint128;
    let cur_ask_asset_bal: Uint128;

    // Get the current pool balance of the offer_asset and ask_asset
    if offer_asset_info.equal(&config.assets[0].info)
        && ask_asset_info.equal(&config.assets[1].info)
    {
        cur_offer_asset_bal = config.assets[0].amount;
        cur_ask_asset_bal = config.assets[1].amount;
    } else if offer_asset_info.equal(&config.assets[1].info)
        && ask_asset_info.equal(&config.assets[0].info)
    {
        cur_offer_asset_bal = config.assets[1].amount;
        cur_ask_asset_bal = config.assets[0].amount;
    } else {
        return Ok(return_swap_failure("assets mismatch".to_string()));
    }

    // Offer asset and Ask asset
    let offer_asset: Asset;
    let ask_asset: Asset;
    let (calc_amount, spread_amount): (Uint128, Uint128);
    let total_fee: Uint128;

    // Based on swap_type, we set the amount to either offer_asset or ask_asset pool
    match swap_type {
        SwapType::GiveIn {} => {
            // Calculate the number of ask_asset tokens to be transferred to the recipient from the Vault
            (calc_amount, spread_amount) =
                compute_swap(cur_offer_asset_bal, cur_ask_asset_bal, amount)
                    .unwrap_or_else(|_| (Uint128::zero(), Uint128::zero()));
            // Calculate the commission fees

            total_fee = calculate_underlying_fees(calc_amount, config.fee_info.total_fee_bps);
            offer_asset = Asset {
                info: offer_asset_info.clone(),
                amount,
            };
            ask_asset = Asset {
                info: ask_asset_info.clone(),
                amount: calc_amount.checked_sub(total_fee)?, // Subtract fee from return amount
            };
        }
        SwapType::GiveOut {} => {
            // Calculate the number of offer_asset tokens to be transferred from the trader from the Vault
            let before_commission_deduction: Uint128;
            (calc_amount, spread_amount, before_commission_deduction) = compute_offer_amount(
                cur_offer_asset_bal,
                cur_ask_asset_bal,
                amount,
                config.fee_info.total_fee_bps,
            )
            .unwrap_or_else(|_| (Uint128::zero(), Uint128::zero(), Uint128::zero()));
            // Calculate the commission fees
            total_fee = calculate_underlying_fees(
                before_commission_deduction,
                config.fee_info.total_fee_bps,
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
        SwapType::Custom(_) => {
            return Ok(return_swap_failure("SwapType not supported".to_string()))
        }
    }

    // Check the max spread limit (if it was specified)
    let spread_check = assert_max_spread(
        belief_price,
        max_spread,
        offer_asset.amount,
        ask_asset.amount + total_fee,
        spread_amount,
    );
    if !spread_check.is_success() {
        return Ok(return_swap_failure(spread_check.to_string()));
    }

    Ok(SwapResponse {
        trade_params: Trade {
            amount_in: offer_asset.amount,
            amount_out: ask_asset.amount,
            spread: spread_amount,
        },
        response: ResponseType::Success {},
        fee: Some(Asset {
            info: ask_asset_info.clone(),
            amount: total_fee,
        }),
    })
}

//--------x------------------x------x-----x-----
//--------x    Query :: TWAP Functions    x-----
//--------x------------------x------x-----x-----

/// ## Description
/// Returns information about the cumulative price of the asset in a [`CumulativePriceResponse`] object.

/// ## Params
/// * **deps** is the object of type [`Deps`].
/// * **env** is the object of type [`Env`].
/// * **offer_asset** is the object of type [`AssetInfo`].
/// * **ask_asset** is the object of type [`AssetInfo`].
pub fn query_cumulative_price(
    deps: Deps,
    env: Env,
    offer_asset: AssetInfo,
    ask_asset: AssetInfo,
) -> StdResult<CumulativePriceResponse> {
    // Load the config  and twap from the storage
    let twap: Twap = TWAPINFO.load(deps.storage)?;
    let config: Config = CONFIG.load(deps.storage)?;

    let total_share = query_supply(&deps.querier, config.lp_token_addr.unwrap().clone())?;

    let mut price0_cumulative_last = twap.price0_cumulative_last;
    let mut price1_cumulative_last = twap.price1_cumulative_last;

    // Calculate the cumulative price of the offer_asset and ask_asset
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

    // return response
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

/// ## Description
/// Returns information about the cumulative prices in a [`CumulativePricesResponse`] object.
/// ## Params
pub fn query_cumulative_prices(deps: Deps, env: Env) -> StdResult<CumulativePricesResponse> {
    // Load the config  and twap from the storage
    let twap: Twap = TWAPINFO.load(deps.storage)?;
    let config: Config = CONFIG.load(deps.storage)?;

    let total_share = query_supply(&deps.querier, config.lp_token_addr.unwrap().clone())?;

    let mut price0_cumulative_last = twap.price0_cumulative_last;
    let mut price1_cumulative_last = twap.price1_cumulative_last;

    let mut exchange_infos: Vec<AssetExchangeRate> = vec![];

    // Calculate the cumulative price of the offer_asset and ask_asset
    if let Some((price0_cumulative_new, price1_cumulative_new, _)) = accumulate_prices(
        env,
        &twap,
        config.assets[0].amount.clone(),
        config.assets[1].amount.clone(),
    )? {
        price0_cumulative_last = price0_cumulative_new;
        price1_cumulative_last = price1_cumulative_new;
    }

    // Get exchange rate for each asset pair and add to the response
    exchange_infos.push(AssetExchangeRate {
        offer_info: config.assets[0].info.clone(),
        ask_info: config.assets[1].info.clone(),
        rate: price0_cumulative_last,
    });
    exchange_infos.push(AssetExchangeRate {
        offer_info: config.assets[1].info.clone(),
        ask_info: config.assets[0].info.clone(),
        rate: price1_cumulative_last,
    });

    Ok(CumulativePricesResponse {
        exchange_infos: exchange_infos,
        total_share,
    })
}

//--------x-----x------x-----x-----
//--------x    MigrateMsg    x-----
//--------x-----x------x-----x-----

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

// ----------------x----------------x----------------x----------------x----------------x----------
// ----------------x----------------x   MATH Computations    x----------------x-------------------
// ----------------x----------------x----------------x----------------x----------------x----------

/// ## Description
/// Returns computed swap for the pool with specified parameters
/// ## Params
/// * **offer_pool** is the object of type [`Uint128`]. Sets the offer pool.
/// * **ask_pool** is the object of type [`Uint128`]. Sets the ask pool.
/// * **offer_amount** is the object of type [`Uint128`]. Sets the offer amount.
pub fn compute_swap(
    offer_pool: Uint128,
    ask_pool: Uint128,
    offer_amount: Uint128,
) -> StdResult<(Uint128, Uint128)> {
    let offer_pool: Uint256 = offer_pool.into();
    let ask_pool: Uint256 = ask_pool.into();
    let offer_amount: Uint256 = offer_amount.into();

    // offer => ask
    // ask_amount = (ask_pool - cp / (offer_pool + offer_amount))
    let cp: Uint256 = offer_pool * ask_pool;
    let return_amount: Uint256 = (Decimal256::from_ratio(ask_pool, 1u8)
        - Decimal256::from_ratio(cp, offer_pool + offer_amount))
        * Uint256::from(1u8);

    // calculate spread & commission
    let spread_amount: Uint256 =
        (offer_amount * Decimal256::from_ratio(ask_pool, offer_pool)) - return_amount;

    // commission will be absorbed to pool
    // let return_amount: Uint256 = return_amount - commission_amount;
    Ok((return_amount.try_into()?, spread_amount.try_into()?))
}

/// ## Description
/// Returns computed offer amount for the pool with specified parameters.
/// ## Params
/// * **offer_pool** is the object of type [`Uint128`]. Sets the offer pool.
/// * **ask_pool** is the object of type [`Uint128`]. Sets the ask pool.
/// * **offer_amount** is the object of type [`Uint128`]. Sets the ask amount.
/// * **commission_rate** is the object of type [`Decimal`]. Sets the commission rate.
pub fn compute_offer_amount(
    offer_pool: Uint128,
    ask_pool: Uint128,
    ask_amount: Uint128,
    commission_rate: u16,
) -> StdResult<(Uint128, Uint128, Uint128)> {
    // ask => offer
    // offer_amount = cp / (ask_pool - ask_amount / (1 - commission_rate)) - offer_pool
    let cp = Uint256::from(offer_pool) * Uint256::from(ask_pool);
    let one_minus_commission =
        Decimal256::one() - decimal2decimal256(Decimal::from_ratio(commission_rate, 10_000u16))?;
    let inv_one_minus_commission = Decimal256::one() / one_minus_commission;

    let before_commission_deduction = Uint256::from(ask_amount) * inv_one_minus_commission;

    let offer_amount: Uint128 = cp
        .multiply_ratio(
            Uint256::from(1u8),
            Uint256::from(
                ask_pool.checked_sub((Uint256::from(before_commission_deduction)).try_into()?)?,
            ),
        )
        .checked_sub(offer_pool.into())?
        .try_into()?;

    let spread_amount = (offer_amount * Decimal::from_ratio(ask_pool, offer_pool))
        .checked_sub(before_commission_deduction.try_into()?)
        .unwrap_or_else(|_| Uint128::zero());

    Ok((
        offer_amount,
        spread_amount,
        before_commission_deduction.try_into()?,
    ))
}

/// ## Description
/// Shifts block_time when any price is zero to not fill an accumulator with a new price to that period.
/// ## Params
/// * **env** is the object of type [`Env`].
/// * **config** is the object of type [`Config`].
/// * **x** is the balance of asset[0] within a pool
/// * **y** is the balance of asset[1] within a pool
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
/// This is an internal function that enforces slippage tolerance for swaps.
/// Returns a [`ContractError`] on failure, otherwise returns [`Ok`].
/// ## Params
/// * **slippage_tolerance** is an object of type [`Option<Decimal>`]. This is the slippage tolerance to enforce.
/// * **deposits** are an array of [`Uint128`] type items. These are offer and ask amounts for a swap.
/// * **pools** are an array of [`Uint128`] type items. These are total amounts of assets in the pool.
fn assert_slippage_tolerance(
    slippage_tolerance: Option<Decimal>,
    deposits: &[Uint128; 2],
    pools: &[Uint128; 2],
) -> ResponseType {
    let default_slippage = Decimal::from_str(DEFAULT_SLIPPAGE).unwrap();
    let max_allowed_slippage = Decimal::from_str(MAX_ALLOWED_SLIPPAGE).unwrap();

    let slippage_tolerance = slippage_tolerance.unwrap_or(default_slippage);
    if slippage_tolerance.gt(&max_allowed_slippage) {
        return ResponseType::Failure((ContractError::AllowedSpreadAssertion {}).to_string());
    }

    let slippage_tolerance: Decimal256 = decimal2decimal256(slippage_tolerance).unwrap();
    let one_minus_slippage_tolerance = Decimal256::one() - slippage_tolerance;
    let deposits: [Uint256; 2] = [deposits[0].into(), deposits[1].into()];
    // let pools: [Uint256; 2] = [pools[0].amount.into(), pools[1].amount.into()];

    // Ensure each price does not change more than what the slippage tolerance allows
    if Decimal256::from_ratio(deposits[0], deposits[1]) * one_minus_slippage_tolerance
        > Decimal256::from_ratio(pools[0], pools[1])
        || Decimal256::from_ratio(deposits[1], deposits[0]) * one_minus_slippage_tolerance
            > Decimal256::from_ratio(pools[1], pools[0])
    {
        return ResponseType::Failure((ContractError::MaxSlippageAssertion {}).to_string());
    }

    ResponseType::Success {}
}

/// ## Description
/// Returns a [`ContractError`] on failure.
/// If `belief_price` and `max_spread` are both specified, we compute a new spread, otherwise we just use the swap spread to check `max_spread`.
///
/// ## Params
/// * **belief_price** is an object of type [`Option<Decimal>`]. This is the belief price used in the swap.
/// * **max_spread** is an object of type [`Option<Decimal>`]. This is the max spread allowed so that the swap can be executed successfuly.
/// * **offer_amount** is an object of type [`Uint128`]. This is the amount of assets to swap.
/// * **return_amount** is an object of type [`Uint128`]. This is the amount of assets to receive from the swap.
/// * **spread_amount** is an object of type [`Uint128`]. This is the spread used in the swap.
pub fn assert_max_spread(
    belief_price: Option<Decimal>,
    max_spread: Option<Decimal>,
    offer_amount: Uint128,
    return_amount: Uint128,
    spread_amount: Uint128,
) -> ResponseType {
    let default_spread = Decimal::from_str(DEFAULT_SLIPPAGE).unwrap();
    let max_allowed_spread = Decimal::from_str(MAX_ALLOWED_SLIPPAGE).unwrap();

    let max_spread = max_spread.unwrap_or(default_spread);
    if max_spread.gt(&max_allowed_spread) {
        return ResponseType::Failure((ContractError::AllowedSpreadAssertion {}).to_string());
    }
    let calc_spread = Decimal::from_ratio(spread_amount, return_amount + spread_amount);

    // If belief price is provided, we compute a new spread
    if let Some(belief_price) = belief_price {
        let expected_return = offer_amount * belief_price.inv().unwrap();
        let spread_amount = expected_return
            .checked_sub(return_amount)
            .unwrap_or_else(|_| Uint128::zero());
        let calc_spread = Decimal::from_ratio(spread_amount, expected_return);
        if return_amount < expected_return && calc_spread > max_spread {
            return ResponseType::Failure(
                (ContractError::MaxSpreadAssertion {
                    spread_amount: calc_spread,
                })
                .to_string(),
            );
        }
    }
    // check if spread limit is respected or not
    else if calc_spread > max_spread {
        return ResponseType::Failure(
            (ContractError::MaxSpreadAssertion {
                spread_amount: calc_spread,
            })
            .to_string(),
        );
    }

    ResponseType::Success {}
}
