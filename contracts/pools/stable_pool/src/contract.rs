#[cfg(not(feature = "library"))]
use crate::error::ContractError;
use crate::math::{
    calc_ask_amount, calc_offer_amount, compute_d, AMP_PRECISION, MAX_AMP, MAX_AMP_CHANGE,
    MIN_AMP_CHANGING_TIME, N_COINS,
};
use crate::state::{
    MathConfig, StablePoolParams, StablePoolUpdateParams, Twap, CONFIG, MATHCONFIG, TWAPINFO,
};
use cosmwasm_std::{
    entry_point, from_binary, to_binary, Binary, Decimal, Deps, DepsMut, Env, Event,
    Fraction, MessageInfo, Response, StdError, StdResult, Uint128,
};
use std::str::FromStr;

use cw2::set_contract_version;

use dexter::asset::{Asset, AssetExchangeRate, AssetInfo};
use dexter::helper::{adjust_precision, calculate_underlying_fees, get_share_in_assets};
use dexter::pool::{
    return_exit_failure, return_join_failure, return_swap_failure, AfterExitResponse,
    AfterJoinResponse, Config, ConfigResponse, CumulativePriceResponse, CumulativePricesResponse,
    ExecuteMsg, FeeResponse, InstantiateMsg, MigrateMsg, QueryMsg, ResponseType, SwapResponse,
    Trade, DEFAULT_SLIPPAGE, MAX_ALLOWED_SLIPPAGE,
};
use dexter::querier::{query_supply, query_token_precision, query_vault_config};
use dexter::vault::{SwapType, FEE_PRECISION, TWAP_PRECISION};
use dexter::U256;

use std::vec;

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter::stableswap_pool";
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

    // Stableswap parameters
    let params: StablePoolParams = from_binary(&msg.init_params.unwrap())?;
    if params.amp == 0 || params.amp > MAX_AMP {
        return Err(ContractError::IncorrectAmp {});
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

    // Create [`Config`]
    let config = Config {
        pool_id: msg.pool_id,
        lp_token_addr: msg.lp_token_addr,
        vault_addr: msg.vault_addr.clone(),
        assets,
        pool_type: msg.pool_type,
        fee_info: msg.fee_info,
        block_time_last: env.block.time.seconds(),
    };

    // Create [`Twap`]
    let twap = Twap {
        price0_cumulative_last: Uint128::zero(),
        price1_cumulative_last: Uint128::zero(),
        block_time_last: 0,
    };

    // Create [`MathConfig`]
    let math_config = MathConfig {
        init_amp: params.amp * AMP_PRECISION,
        init_amp_time: env.block.time.seconds(),
        next_amp: params.amp * AMP_PRECISION,
        next_amp_time: env.block.time.seconds(),
    };

    // save config, mathconfig and twap to storage
    CONFIG.save(deps.storage, &config)?;
    TWAPINFO.save(deps.storage, &twap)?;
    MATHCONFIG.save(deps.storage, &math_config)?;

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
        ExecuteMsg::UpdateConfig { params } => update_config(deps, env, info, params),
        ExecuteMsg::UpdateFee { total_fee_bps } => update_total_fee_bps(deps, env, info, total_fee_bps),
        ExecuteMsg::UpdateLiquidity { assets } => {
            execute_update_pool_liquidity(deps, env, info, assets)
        }
    }
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

    let event = Event::new("dexter-pool::update_liquidity")
        .add_attribute("pool_id", config.pool_id.to_string())
        .add_attribute("vault_address", config.vault_addr)
        .add_attribute(
            "pool_assets",
            serde_json_wasm::to_string(&config.assets).unwrap(),
        )
        .add_attribute("block_time_last", twap.block_time_last.to_string());

    Ok(Response::new().add_event(event))
}

/// ## Description
/// Updates the pool's math configuration with the specified parameters in the `params` variable.
/// Returns a [`ContractError`] as a failure, otherwise returns a [`Response`] with the specified
/// attributes if the operation was successful
///
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
/// * **env** is an object of type [`Env`].
/// * **info** is an object of type [`MessageInfo`].
/// * **params** is an object of type [`Binary`]. These are the the new parameter values.
pub fn update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    params: Option<Binary>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let math_config = MATHCONFIG.load(deps.storage)?;
    let vault_config = query_vault_config(&deps.querier, config.vault_addr.clone().to_string())?;
    let params = params.unwrap();

    // Access Check :: Only Vault's Owner can execute this function
    if info.sender != vault_config.owner && info.sender != config.vault_addr {
        return Err(ContractError::Unauthorized {});
    }

    match from_binary::<StablePoolUpdateParams>(&params)? {
        StablePoolUpdateParams::StartChangingAmp {
            next_amp,
            next_amp_time,
        } => start_changing_amp(math_config, deps, env, next_amp, next_amp_time)?,
        StablePoolUpdateParams::StopChangingAmp {} => stop_changing_amp(math_config, deps, env)?,
    }

    Ok(Response::default())
}

pub fn update_total_fee_bps(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    total_fee_bps: u16,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    let vault_config = query_vault_config(&deps.querier, config.vault_addr.clone().to_string())?;

    // Access Check :: Only Vault's Owner can execute this function
    if info.sender != vault_config.owner {
        return Err(ContractError::Unauthorized {});
    }

    config.fee_info.total_fee_bps = total_fee_bps;
    CONFIG.save(deps.storage, &config)?;

    let event = Event::new("dexter-pool::update_total_fee_bps")
        .add_attribute("total_fee_bps", config.fee_info.total_fee_bps.to_string());
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
        QueryMsg::Config {} => to_binary(&query_config(deps, env)?),
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
/// ## Params
/// * **deps** is the object of type [`Deps`].
pub fn query_config(deps: Deps, env: Env) -> StdResult<ConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    let math_config: MathConfig = MATHCONFIG.load(deps.storage)?;
    let cur_amp = compute_current_amp(&math_config, &env)?;

    Ok(ConfigResponse {
        pool_id: config.pool_id,
        lp_token_addr: config.lp_token_addr,
        vault_addr: config.vault_addr,
        assets: config.assets,
        pool_type: config.pool_type,
        fee_info: config.fee_info,
        block_time_last: config.block_time_last,
        math_params: Some(to_binary(&math_config).unwrap()),
        additional_params: Some(
            to_binary(&StablePoolParams {
                amp: cur_amp.checked_div(AMP_PRECISION).unwrap(),
            })
            .unwrap(),
        ),
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

//--------x------------------x--------------x-----x-----
//--------x    Query :: OnJoin, OnExit, OnSwap    x-----
//--------x------------------x--------------x-----x-----

/// ## Description
/// Returns [`AfterJoinResponse`] type which contains -  
/// return_assets - Is of type [`Vec<Asset>`] and is a sorted list consisting of amount of info of tokens which are to be subtracted from
/// the token balances provided by the user to the Vault, to get the final list of token balances to be provided as Liquiditiy against the minted LP shares
/// new_shares - New LP shares which are to be minted
/// response - A [`ResponseType`] which is either `Success` or `Failure`, deteriming if the tx is accepted by the Pool's math computations or not
///
/// ## Params
/// assets_in - Of type [`Vec<Asset>`], a sorted list containing amount / info of token balances to be supplied as liquidity to the pool
/// * **deps** is the object of type [`Deps`].
/// STABLESWAP POOL -::- MATH LOGIC
/// -- Implementation - For Stableswap, user provides the exact number of assets he/she wants to supply as liquidity to the pool. We simply calculate the number of LP shares to be minted and return it to the user.
/// T.B.A
pub fn query_on_join_pool(
    deps: Deps,
    env: Env,
    assets_in: Option<Vec<Asset>>,
    _mint_amount: Option<Uint128>,
    slippage_tolerance: Option<Decimal>,
) -> StdResult<AfterJoinResponse> {
    // If the user has not provided any assets to be provided, then return a `Failure` response
    if assets_in.is_none() {
        return Ok(return_join_failure("No assets provided".to_string()));
    }

    // Load the config and math config from the storage
    let config: Config = CONFIG.load(deps.storage)?;
    let math_config: MathConfig = MATHCONFIG.load(deps.storage)?;

    // Check if both assets are provided
    if assets_in.clone().unwrap().len() != config.assets.len() {
        return Ok(return_join_failure(
            "Invalid number of assets provided".to_string(),
        ));
    }

    // Sort the assets in the order of the assets in the config
    let mut act_assets_in = assets_in.unwrap();
    act_assets_in.sort_by(|a, b| {
        a.info
            .to_string()
            .to_lowercase()
            .cmp(&b.info.to_string().to_lowercase())
    });

    // Since its a 2-token Pool, there will be only 2 assets
    let deposits: [Uint128; 2] = [act_assets_in[0].amount, act_assets_in[1].amount];

    // Total share of LP tokens minted by the pool
    let total_share = query_supply(&deps.querier, config.lp_token_addr.clone())?;

    // decimal precision for both pool tokens and the greatest precision of the two tokens
    let token_precision_0 = query_token_precision(&deps.querier, act_assets_in[0].info.clone())?;
    let token_precision_1 = query_token_precision(&deps.querier, act_assets_in[1].info.clone())?;
    let greater_precision = token_precision_0.max(token_precision_1);

    // Adjust deposit amounts to the precision of the pool tokens
    let deposit_amount_0 = adjust_precision(deposits[0], token_precision_0, greater_precision)?;
    let deposit_amount_1 = adjust_precision(deposits[1], token_precision_1, greater_precision)?;

    // Assert slippage tolerance
    let res = assert_slippage_tolerance(
        &slippage_tolerance,
        &deposits,
        &[config.assets[0].amount, config.assets[1].amount],
    );
    // return a `Failure` response if the slippage tolerance is not met
    if !res.is_success() {
        return Ok(return_join_failure(res.to_string()));
    }

    // Calculate the number of LP shares to be minted
    // if the total share is 0, then the number of shares to be minted is sqrt(deposit_amount_0 * deposit_amount_1)
    let new_shares = if total_share.is_zero() {
        // LP token precision
        let liquidity_token_precision = query_token_precision(
            &deps.querier,
            AssetInfo::Token {
                contract_addr: config.lp_token_addr,
            },
        )?;

        // Initial share = sqrt( product of deposit_amounts ) adjusted to the precision of the LP token and greatest precision of the two tokens
        adjust_precision(
            Uint128::new(
                (U256::from(deposit_amount_0.u128()) * U256::from(deposit_amount_1.u128()))
                    .integer_sqrt()
                    .as_u128(),
            ),
            greater_precision,
            liquidity_token_precision,
        )?
    } else {
        // Calculate current AMP value
        let leverage = compute_current_amp(&math_config, &env)?
            .checked_mul(u64::from(N_COINS))
            .unwrap();

        // Current pool balances adjusted to the greatest precision of the two tokens
        let mut pool_amount_0 = adjust_precision(
            config.assets[0].amount,
            token_precision_0,
            greater_precision,
        )?;

        let mut pool_amount_1 = adjust_precision(
            config.assets[1].amount,
            token_precision_1,
            greater_precision,
        )?;

        // Calculate invariant D before adding liquidity
        let d_before_addition_liquidity =
            compute_d(leverage, pool_amount_0.u128(), pool_amount_1.u128()).unwrap();

        // Updated pool balances after adding liquidity
        pool_amount_0 = pool_amount_0.checked_add(deposit_amount_0)?;
        pool_amount_1 = pool_amount_1.checked_add(deposit_amount_1)?;

        // Calculate invariant D after adding liquidity
        let d_after_addition_liquidity =
            compute_d(leverage, pool_amount_0.u128(), pool_amount_1.u128()).unwrap();

        // d after adding liquidity must be more than d before adding liquidity
        if d_before_addition_liquidity >= d_after_addition_liquidity {
            Uint128::zero();
        }

        total_share.multiply_ratio(
            d_after_addition_liquidity - d_before_addition_liquidity,
            d_before_addition_liquidity,
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
/// assets_out - Is of type [`Vec<Asset>`] and is a sorted list consisting of amount and info of tokens which are to be subtracted from the PoolInfo state stored in the Vault contract and tranfer from the Vault to the user
/// burn_shares - Number of LP shares to be burnt
/// response - A [`ResponseType`] which is either `Success` or `Failure`, deteriming if the tx is accepted by the Pool's math computations or not
///
/// ## Params
/// assets_out - Of type [`Vec<Asset>`], a sorted list containing amount / info of token balances user wants against the LP tokens transferred by the user to the Vault contract
/// * **deps** is the object of type [`Deps`].
/// STABLESWAP POOL -::- MATH LOGIC
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
    let total_share = query_supply(&deps.querier, config.lp_token_addr)?;

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
/// trade_params - Is of type [`Trade`] which contains all params related with the trade, including the number of assets to be traded, spread, and the fees to be paid
/// response - A [`ResponseType`] which is either `Success` or `Failure`, deteriming if the tx is accepted by the Pool's math computations or not
///
/// ## Params
///  swap_type - Is of type [`SwapType`] which is either `GiveIn`, `GiveOut` or `Custom`
///  offer_asset_info - Of type [`AssetInfo`] which is the asset info of the asset to be traded in the offer side of the trade
/// ask_asset_info - Of type [`AssetInfo`] which is the asset info of the asset to be traded in the ask side of the trade
/// amount - Of type [`Uint128`] which is the amount of assets to be traded on ask or offer side, based on the swap type
/// STABLESWAP POOL -::- MATH LOGIC
/// T.B.A
pub fn query_on_swap(
    deps: Deps,
    env: Env,
    swap_type: SwapType,
    offer_asset_info: AssetInfo,
    ask_asset_info: AssetInfo,
    amount: Uint128,
    max_spread: Option<Decimal>,
    belief_price: Option<Decimal>,
) -> StdResult<SwapResponse> {
    // Load the config and math config from the storage
    let config: Config = CONFIG.load(deps.storage)?;
    let math_config: MathConfig = MATHCONFIG.load(deps.storage)?;

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

    // decimal precision for both pool tokens and the greatest precision of the two tokens
    let offer_precision = query_token_precision(&deps.querier, offer_asset_info.clone())?;
    let ask_precision = query_token_precision(&deps.querier, ask_asset_info.clone())?;
    let greater_precision = offer_precision.max(ask_precision);

    // Offer asset and Ask asset
    let offer_asset: Asset;
    let ask_asset: Asset;
    let (mut calc_amount, mut spread_amount): (Uint128, Uint128);
    let total_fee: Uint128;

    // Based on swap_type, we set the amount to either offer_asset or ask_asset pool
    match swap_type {
        SwapType::GiveIn {} => {
            // Calculate the number of ask_asset tokens to be transferred to the recipient from the Vault
            (calc_amount, spread_amount) = match compute_swap(
                cur_offer_asset_bal,
                offer_precision,
                cur_ask_asset_bal,
                ask_precision,
                amount,
                compute_current_amp(&math_config, &env)?,
            ) {
                Ok(res) => res,
                Err(err) => {
                    return Ok(return_swap_failure(format!(
                        "Error during swap calculation: {}",
                        err
                    )))
                }
            };

            // Re-adjust for their token precisions
            calc_amount = adjust_precision(calc_amount, greater_precision, ask_precision)?;
            spread_amount = adjust_precision(spread_amount, greater_precision, ask_precision)?;
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
            (calc_amount, spread_amount, total_fee) = match compute_offer_amount(
                cur_offer_asset_bal,
                offer_precision,
                cur_ask_asset_bal,
                ask_precision,
                amount,
                config.fee_info.total_fee_bps,
                compute_current_amp(&math_config, &env)?,
            ) {
                Ok(res) => res,
                Err(err) => {
                    return Ok(return_swap_failure(format!(
                        "Error during swap calculation: {}",
                        err
                    )))
                }
            };

            // Calculate the protocol and dev fee
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

    if calc_amount.is_zero() {
        return Ok(return_swap_failure(
            "Computation error - calc_amount is zero".to_string(),
        ));
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

    let total_share = query_supply(&deps.querier, config.lp_token_addr)?;

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
/// Returns information about the cumulative price of the asset in a [`CumulativePricesResponse`] object.

/// ## Params
/// * **deps** is the object of type [`Deps`].
/// * **env** is the object of type [`Env`].
/// * **offer_asset** is the object of type [`AssetInfo`].
/// * **ask_asset** is the object of type [`AssetInfo`].
pub fn query_cumulative_prices(deps: Deps, env: Env) -> StdResult<CumulativePricesResponse> {
    // Load the config  and twap from the storage
    let twap: Twap = TWAPINFO.load(deps.storage)?;
    let config: Config = CONFIG.load(deps.storage)?;

    let total_share = query_supply(&deps.querier, config.lp_token_addr)?;

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

// --------x--------x--------x--------x--------x--------x------
// --------x--------x AMP UPDATE Helpers  x--------x--------x--
// --------x--------x--------x--------x--------x--------x------

/// ## Description
/// Start changing the AMP value. Returns a [`ContractError`] on failure, otherwise returns [`Ok`].
///
/// ## Params
/// * **mut math_config** is an object of type [`Config`]. This is a mutable reference to the pool's custom math configuration.
/// * **next_amp** is an object of type [`u64`]. This is the new value for AMP.
/// * **next_amp_time** is an object of type [`u64`]. This is the end time when the pool amplification will be equal to `next_amp`.
fn start_changing_amp(
    mut math_config: MathConfig,
    deps: DepsMut,
    env: Env,
    next_amp: u64,
    next_amp_time: u64,
) -> Result<(), ContractError> {
    // Validation checks
    if next_amp == 0 || next_amp > MAX_AMP {
        return Err(ContractError::IncorrectAmp {});
    }

    // Current and next AMP values
    let current_amp = compute_current_amp(&math_config, &env)?;
    let next_amp_with_precision = next_amp * AMP_PRECISION;

    // Max allowed AMP change checks
    if next_amp_with_precision * MAX_AMP_CHANGE < current_amp
        || next_amp_with_precision > current_amp * MAX_AMP_CHANGE
    {
        return Err(ContractError::MaxAmpChangeAssertion {});
    }

    // Time gap for AMP change checks
    let block_time = env.block.time.seconds();
    if block_time < math_config.init_amp_time + MIN_AMP_CHANGING_TIME
        || next_amp_time < block_time + MIN_AMP_CHANGING_TIME
    {
        return Err(ContractError::MinAmpChangingTimeAssertion {});
    }

    // Update the math config
    math_config.init_amp = current_amp;
    math_config.next_amp = next_amp_with_precision;
    math_config.init_amp_time = block_time;
    math_config.next_amp_time = next_amp_time;

    // Update the storage
    MATHCONFIG.save(deps.storage, &math_config)?;

    Ok(())
}

/// ## Description
/// Stop changing the AMP value. Returns [`Ok`].
///
/// ## Params
/// * **mut math_config** is an object of type [`MathConfig`]. This is a mutable reference to the pool's custom math configuration.
fn stop_changing_amp(mut math_config: MathConfig, deps: DepsMut, env: Env) -> StdResult<()> {
    let current_amp = compute_current_amp(&math_config, &env)?;
    let block_time = env.block.time.seconds();

    // Update and save the math config
    math_config.init_amp = current_amp;
    math_config.next_amp = current_amp;
    math_config.init_amp_time = block_time;
    math_config.next_amp_time = block_time;
    MATHCONFIG.save(deps.storage, &math_config)?;

    Ok(())
}

// --------x--------x--------x--------x--------x--------
// --------x--------x COMPUTATATIONS  x--------x--------
// --------x--------x--------x--------x--------x--------

/// ## Description
/// Returns the result of a swap.
/// ## Params
/// * **offer_pool** is an object of type [`Uint128`]. This is the total amount of offer assets in the pool.
/// * **offer_precision** is an object of type [`u8`]. This is the token precision used for the offer amount.
/// * **ask_pool** is an object of type [`Uint128`]. This is the total amount of ask assets in the pool.
/// * **ask_precision** is an object of type [`u8`]. This is the token precision used for the ask amount.
/// * **offer_amount** is an object of type [`Uint128`]. This is the amount of offer assets to swap.
/// * **amp** is an object of type [`u64`]. This is the pool amplification used to calculate the swap result.
fn compute_swap(
    offer_pool: Uint128,
    offer_precision: u8,
    ask_pool: Uint128,
    ask_precision: u8,
    offer_amount: Uint128,
    amp: u64,
) -> StdResult<(Uint128, Uint128)> {
    // offer => ask

    // Adjust offer asset and ask asset's current pool balances and the offer amount based on precision
    let greater_precision = offer_precision.max(ask_precision);
    let offer_pool = adjust_precision(offer_pool, offer_precision, greater_precision)?;
    let ask_pool = adjust_precision(ask_pool, ask_precision, greater_precision)?;
    let offer_amount = adjust_precision(offer_amount, offer_precision, greater_precision)?;

    // Calculate the ask asset amount to swap
    let return_amount = Uint128::new(
        calc_ask_amount(offer_pool.u128(), ask_pool.u128(), offer_amount.u128(), amp).unwrap(),
    );
    // We assume the assets should stay in a 1:1 ratio, so the true exchange rate is 1. So any exchange rate <1 could be considered the spread
    let spread_amount = offer_amount.saturating_sub(return_amount);

    Ok((return_amount, spread_amount))
}

/// ## Description
/// Returns an amount of offer assets for a specified amount of ask assets.
/// ## Params
/// * **offer_pool** is an object of type [`Uint128`]. This is the total amount of offer assets in the pool.
/// * **offer_precision** is an object of type [`u8`]. This is the token precision used for the offer amount.
/// * **ask_pool** is an object of type [`Uint128`]. This is the total amount of ask assets in the pool.
/// * **ask_precision** is an object of type [`u8`]. This is the token precision used for the ask amount.
/// * **ask_amount** is an object of type [`Uint128`]. This is the amount of ask assets to swap to.
/// * **commission_rate** is an object of type [`Decimal`]. This is the total amount of fees charged for the swap.
fn compute_offer_amount(
    offer_pool: Uint128,
    offer_precision: u8,
    ask_pool: Uint128,
    ask_precision: u8,
    ask_amount: Uint128,
    commission_rate: u16,
    amp: u64,
) -> StdResult<(Uint128, Uint128, Uint128)> {
    let commission_rate_decimals = Decimal::from_ratio(commission_rate, FEE_PRECISION);
    // ask => offer

    // Adjust balances based on their precisions
    let greater_precision = offer_precision.max(ask_precision);
    let offer_pool = adjust_precision(offer_pool, offer_precision, greater_precision)?;
    let ask_pool = adjust_precision(ask_pool, ask_precision, greater_precision)?;
    let ask_amount = adjust_precision(ask_amount, ask_precision, greater_precision)?;

    let one_minus_commission = Decimal::one() - commission_rate_decimals;
    let inv_one_minus_commission: Decimal = Decimal::one() / one_minus_commission;
    let before_commission_deduction = ask_amount * inv_one_minus_commission;

    let offer_amount = Uint128::new(
        calc_offer_amount(
            offer_pool.u128(),
            ask_pool.u128(),
            before_commission_deduction.u128(),
            amp,
        )
        .unwrap(),
    );

    // We assume the assets should stay in a 1:1 ratio, so the true exchange rate is 1. Any exchange rate < 1 could be considered the spread
    let spread_amount = offer_amount.saturating_sub(before_commission_deduction);

    let commission_amount = before_commission_deduction * commission_rate_decimals;

    let offer_amount = adjust_precision(offer_amount, greater_precision, offer_precision)?;
    let spread_amount = adjust_precision(spread_amount, greater_precision, ask_precision)?;
    let commission_amount = adjust_precision(commission_amount, greater_precision, ask_precision)?;

    Ok((offer_amount, spread_amount, commission_amount))
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
/// This is an internal function that enforces slippage tolerance for swaps.
/// Returns a [`ContractError`] on failure, otherwise returns [`Ok`].
///
/// ## Params
/// * **slippage_tolerance** is an object of type [`Option<Decimal>`]. This is the slippage tolerance to enforce.
/// * **deposits** are an array of [`Uint128`] type items. These are offer and ask amounts for a swap.
/// * **pools** are an array of [`Asset`] type items. These are total amounts of assets in the pool.
fn assert_slippage_tolerance(
    _slippage_tolerance: &Option<Decimal>,
    _deposits: &[Uint128; 2],
    _pools: &[Uint128; 2],
) -> ResponseType {
    // There is no slippage in the stable pool
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
            .unwrap_or(Uint128::zero());
        let calc_spread = Decimal::from_ratio(spread_amount, expected_return);
        if return_amount < expected_return && calc_spread > max_spread {
            return ResponseType::Failure(
                (ContractError::MaxSpreadAssertion {
                    spread_amount: calc_spread,
                })
                .to_string(),
            );
        }
    } else if calc_spread > max_spread {
        return ResponseType::Failure(
            (ContractError::MaxSpreadAssertion {
                spread_amount: calc_spread,
            })
            .to_string(),
        );
    }

    ResponseType::Success {}
}

// --------x--------x--------x--------x--------x--------x--------
// --------x--------x AMP COMPUTE Functions   x--------x---------
// --------x--------x--------x--------x--------x--------x--------

/// ## Description
/// Compute the current pool amplification coefficient (AMP).
///
/// ## Params
/// * **math_config** is an object of type [`MathConfig`].
fn compute_current_amp(math_config: &MathConfig, env: &Env) -> StdResult<u64> {
    // Get block time
    let block_time = env.block.time.seconds();

    // If we are in the period of AMP change, we calculate the latest new AMP
    if block_time < math_config.next_amp_time {
        // initial and final AMPs
        let init_amp = Uint128::from(math_config.init_amp);
        let next_amp = Uint128::from(math_config.next_amp);

        // time elapsed since AMP change init and the total time range of AMP change
        let elapsed_time =
            Uint128::from(block_time).checked_sub(Uint128::from(math_config.init_amp_time))?;
        let time_range = Uint128::from(math_config.next_amp_time)
            .checked_sub(Uint128::from(math_config.init_amp_time))?;

        // Calculate AMP based on if AMP is being increased or decreased
        if math_config.next_amp > math_config.init_amp {
            let amp_range = next_amp - init_amp;
            let res = init_amp + (amp_range * elapsed_time).checked_div(time_range)?;
            Ok(res.u128() as u64)
        } else {
            let amp_range = init_amp - next_amp;
            let res = init_amp - (amp_range * elapsed_time).checked_div(time_range)?;
            Ok(res.u128() as u64)
        }
    } else {
        Ok(math_config.next_amp)
    }
}

// /// ## Description
// /// Compute the current pool D value.
// fn query_compute_d(deps: Deps, env: Env, assets: Vec<Asset> ) -> StdResult<u128> {
//     let math_config = MATHCONFIG.load(deps.storage)?;

//     let amp = compute_current_amp(&math_config, &env)?;
//     let pools = config
//         .pair_info
//         .query_pools(&deps.querier, env.contract.address)?;
//     let leverage = Uint64::new(amp).checked_mul(Uint64::from(N_COINS))?;

//     compute_d(
//         leverage.u64(),
//         assets[0].amount.u128(),
//         assets[1].amount.u128(),
//     )
//     .ok_or_else(|| StdError::generic_err("Failed to calculate the D"))
// }

// --------x--------x--------x--------x--------x--------x---
// --------x--------x Migrate Function   x--------x---------
// --------x--------x--------x--------x--------x--------x---

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