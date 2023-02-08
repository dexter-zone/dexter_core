use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    entry_point, from_binary, to_binary, Binary, Decimal, Decimal256, Deps, DepsMut, Env,
    Event, Fraction, MessageInfo, Response, StdError, StdResult, Uint128, Uint256, Uint64,
};
use cw2::set_contract_version;
use itertools::Itertools;
use std::collections::HashMap;
use std::str::FromStr;
use std::vec;

use crate::error::ContractError;
use crate::math::{compute_d, AMP_PRECISION, MAX_AMP, MAX_AMP_CHANGE, MIN_AMP_CHANGING_TIME};
use crate::state::{
    get_precision, store_precisions, MathConfig, StablePoolParams, StablePoolUpdateParams, Twap,
    CONFIG, MATHCONFIG, TWAPINFO,
};
use crate::utils::{accumulate_prices, compute_offer_amount, compute_swap};
use dexter::pool::{
    return_exit_failure, return_join_failure, return_swap_failure, AfterExitResponse,
    AfterJoinResponse, Config, ConfigResponse, CumulativePriceResponse, CumulativePricesResponse,
    ExecuteMsg, FeeResponse, InstantiateMsg, MigrateMsg, QueryMsg, ResponseType, SwapResponse,
    Trade, DEFAULT_SLIPPAGE, MAX_ALLOWED_SLIPPAGE, update_total_fee_bps
};

use dexter::asset::{Asset, AssetExchangeRate, AssetInfo, Decimal256Ext, DecimalAsset};
use dexter::helper::{calculate_underlying_fees, get_share_in_assets, select_pools};
use dexter::querier::{query_supply, query_vault_config};
use dexter::vault::{SwapType, FEE_PRECISION};

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter::stable5swap_pool";
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
    mut deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Validate number of assets
    if msg.asset_infos.len() > 5 || msg.asset_infos.len() < 2 {
        return Err(ContractError::InvalidNumberOfAssets {});
    }

    // Stable5swap parameters
    let params: StablePoolParams = from_binary(&msg.init_params.unwrap())?;
    if params.amp == 0 || params.amp > MAX_AMP {
        return Err(ContractError::IncorrectAmp {});
    }

    // store precisions for assets in storage
    let greatest_precision = store_precisions(deps.branch(), &msg.asset_infos)?;
    // We cannot have precision greater than what is supported by Decimal type
    if greatest_precision > (Decimal::DECIMAL_PLACES as u8) {
        return Err(ContractError::InvalidGreatestPrecision);
    }

    // Initializing cumulative prices
    let mut cumulative_prices = vec![];
    for from_asset in &msg.asset_infos {
        for to_asset in &msg.asset_infos {
            if from_asset.as_string() != to_asset.as_string() {
                cumulative_prices.push((from_asset.clone(), to_asset.clone(), Uint128::zero()))
            }
        }
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

    let config = Config {
        pool_id: msg.pool_id.clone(),
        lp_token_addr: msg.lp_token_addr,
        vault_addr: msg.vault_addr.clone(),
        assets,
        pool_type: msg.pool_type.clone(),
        fee_info: msg.fee_info.clone(),
        block_time_last: env.block.time.seconds(),
    };

    let twap = Twap {
        cumulative_prices: cumulative_prices,
        block_time_last: 0,
    };

    let math_config = MathConfig {
        init_amp: params.amp * AMP_PRECISION,
        init_amp_time: env.block.time.seconds(),
        next_amp: params.amp * AMP_PRECISION,
        next_amp_time: env.block.time.seconds(),
        greatest_precision,
    };

    // Store config, MathConfig and twap in storage
    CONFIG.save(deps.storage, &config)?;
    MATHCONFIG.save(deps.storage, &math_config)?;
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
        ExecuteMsg::UpdateConfig { params } => update_config(deps, env, info, params),
        ExecuteMsg::UpdateFee { total_fee_bps } => {
            update_total_fee_bps(deps, env, info, total_fee_bps, CONFIG)
                .map_err(|e| e.into())
        },
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
    let math_config: MathConfig = MATHCONFIG.load(deps.storage)?;
    let mut twap: Twap = TWAPINFO.load(deps.storage)?;

    // Acess Check :: Only Vault can execute this function
    if info.sender != config.vault_addr {
        return Err(ContractError::Unauthorized {});
    }

    // Convert Vec<Asset> to Vec<DecimalAsset> type
    let decimal_assets: Vec<DecimalAsset> =
        transform_to_decimal_asset(deps.as_ref(), config.assets.clone());

    // Accumulate prices for the assets in the pool
    if accumulate_prices(
        deps.as_ref(),
        env.clone(),
        math_config,
        &mut twap,
        &decimal_assets,
    )
    .is_ok()
    // TWAP computation can fail in certain edge cases (when pool is empty for eg), for which you need
    // to allow tx to be successful rather than failing the tx. Accumulated prices can be used to
    // calculate TWAP oracle prices later and letting the tx be successful even when price accumulation
    // fails doesn't cause any issues.
    {
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

// ----------------x----------------x---------------------x-----------------------x----------------x----------------
// ----------------x----------------x  :::: Stable5 POOL::QUERIES Implementation   ::::  x----------------x----------------
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
/// return_assets - Is of type [`Vec<Asset>`] and is a sorted list consisting of amount and info of tokens which are to be subtracted from
/// the token balances provided by the user to the Vault, to get the final list of token balances to be provided as Liquiditiy against the minted LP shares
/// new_shares - New LP shares which are to be minted
/// response - A [`ResponseType`] which is either `Success` or `Failure`, deteriming if the tx is accepted by the Pool's math computations or not
///
/// ## Params
/// assets_in - Of type [`Vec<Asset>`], a sorted list containing amount / info of token balances to be supplied as liquidity to the pool
/// _mint_amount - Of type [`Option<Uint128>`], optional parameter which tells the number of LP tokens to be minted
/// STABLE-5-SWAP POOL -::- MATH LOGIC
/// -- Implementation - For STABLE-5-swap, user provides the exact number of assets he/she wants to supply as liquidity to the pool. We simply calculate the number of LP shares to be minted and return it to the user.
/// T.B.A
pub fn query_on_join_pool(
    deps: Deps,
    env: Env,
    assets_in: Option<Vec<Asset>>,
    _mint_amount: Option<Uint128>,
    _slippage_tolerance: Option<Decimal>,
) -> StdResult<AfterJoinResponse> {
    // If the user has not provided any assets to be provided, then return a `Failure` response
    if assets_in.is_none() {
        return Ok(return_join_failure("No assets provided".to_string()));
    }

    // Load the config and math config from the storage
    let config: Config = CONFIG.load(deps.storage)?;
    let math_config: MathConfig = MATHCONFIG.load(deps.storage)?;

    // Sort the assets in the order of the assets in the config
    let mut act_assets_in = assets_in.unwrap();

    // Get Asset stored in state for each asset in a HashMap
    let token_pools: HashMap<_, _> = config
        .assets
        .clone()
        .into_iter()
        .map(|pool| (pool.info, pool.amount))
        .collect();

    let mut non_zero_flag = false;
    // get asset info for each asset in the list provided by the user to its pool mapping
    let mut assets_collection = act_assets_in
        .clone()
        .into_iter()
        .map(|asset| {
            // Check that at least one asset is non-zero
            if !asset.amount.is_zero() {
                non_zero_flag = true;
            }

            // Get appropriate pool
            let token_pool = token_pools.get(&asset.info).copied().unwrap();
            Ok((asset, token_pool))
        })
        .collect::<Result<Vec<_>, ContractError>>()
        .unwrap();

    // If there's no non-zero assets in the list provided by the user, then return a `Failure` response
    if !non_zero_flag {
        return Ok(return_join_failure(
            "No non-zero assets provided".to_string(),
        ));
    }

    // If some assets are omitted then add them explicitly with 0 deposit
    token_pools.iter().for_each(|(pool_info, pool_amount)| {
        if !act_assets_in.iter().any(|asset| asset.info.eq(pool_info)) {
            assets_collection.push((
                Asset {
                    amount: Uint128::zero(),
                    info: pool_info.clone(),
                },
                *pool_amount,
            ));
            act_assets_in.push(Asset {
                amount: Uint128::zero(),
                info: pool_info.clone(),
            });
        }
    });

    // Sort assets because the vault expects them in order
    act_assets_in.sort_by(|a, b| {
        a.info
            .to_string()
            .to_lowercase()
            .cmp(&b.info.to_string().to_lowercase())
    });

    // Check asset definitions and make sure no asset is repeated
    let mut previous_asset: String = "".to_string();
    for asset in act_assets_in.iter() {
        if previous_asset == asset.info.as_string() {
            return Ok(return_join_failure(
                "Repeated assets in asset_in".to_string(),
            ));
        }
        previous_asset = asset.info.as_string();
    }

    // We cannot put a zero amount into an empty pool.
    // This means that the first time we bootstrap the pool, we have to provide liquidity for all the assets in the pool.
    for (deposit, pool) in assets_collection.iter_mut() {
        if deposit.amount.is_zero() && pool.is_zero() {
            return Ok(return_join_failure(
                "Cannot deposit zero into an empty pool".to_string(),
            ));
        }
    }

    // Convert to Decimal types
    let assets_collection = assets_collection
        .iter()
        .cloned()
        .map(|(asset, pool)| {
            let coin_precision = get_precision(deps.storage, &asset.info)?;
            Ok((
                asset.to_decimal_asset(coin_precision)?,
                Decimal256::with_precision(pool, coin_precision)?,
            ))
        })
        .collect::<StdResult<Vec<(DecimalAsset, Decimal256)>>>()?;

    // Compute amp parameter
    let amp = compute_current_amp(&math_config, &env).unwrap_or(0u64.into());

    // If AMP value is invalid, then return a `Failure` response
    if amp == 0u64 {
        return Ok(return_join_failure("Invalid amp value".to_string()));
    }

    let n_coins = config.assets.len() as u8;

    // Initial invariant (D)
    let old_balances = assets_collection
        .iter()
        .map(|(_, pool)| *pool)
        .collect_vec();
    let init_d = compute_d(amp.into(), &old_balances, math_config.greatest_precision)?;

    // Invariant (D) after deposit added
    let mut new_balances = assets_collection
        .iter()
        .map(|(deposit, pool)| Ok(pool + deposit.amount))
        .collect::<StdResult<Vec<_>>>()?;
    let deposit_d = compute_d(amp.into(), &new_balances, math_config.greatest_precision)?;

    // Total share of LP tokens minted by the pool
    let total_share = query_supply(&deps.querier, config.lp_token_addr)?;

    // Tokens to be charged as Fee
    let mut fee_tokens: Vec<Asset> = vec![];

    // Calculate the number of LP shares to be minted
    let mint_amount = if total_share.is_zero() {
        deposit_d
    } else {
        // Get fee info from the factory
        let fee_info = config.fee_info.clone();

        // Calculate fee using the curve formula:
        // fee = total_fee_bps * N_COINS / (4 * (N_COINS - 1))
        // specified here:
        // https://github.com/curvefi/curve-contract/blob/master/contracts/pools/3pool/StableSwap3Pool.vy#L274
        //
        // Based on the docs here:
        // https://resources.curve.fi/lp/understanding-curve-pools#what-are-curve-fees
        // It seems fees are calculated so as to keep it in between (0, sawp_fee/2).
        // If pool has two coins, then deposit/withdraw fee is exactly half of the swap fee.
        // As number of coins increases, this would keep decreasing to 0.
        let fee = Decimal::from_ratio(fee_info.total_fee_bps, FEE_PRECISION)
            .checked_mul(Decimal::from_ratio(n_coins, 4 * (n_coins - 1)))?;

        let fee = Decimal256::new(fee.atomics().into());

        for i in 0..n_coins as usize {
            // Asset Info for token i
            let asset_info = assets_collection[i].0.info.clone();
            let ideal_balance = deposit_d.checked_multiply_ratio(old_balances[i], init_d)?;

            let difference = if ideal_balance > new_balances[i] {
                ideal_balance - new_balances[i]
            } else {
                new_balances[i] - ideal_balance
            };

            // Fee will be charged only during imbalanced provide i.e. if invariant D was changed
            let fee_charged = fee.checked_mul(difference)?;
            fee_tokens.push(Asset {
                amount: fee_charged
                    .to_uint128_with_precision(get_precision(deps.storage, &asset_info)?)?,
                info: asset_info.clone(),
            });
            new_balances[i] -= fee_charged;
        }

        let after_fee_d = compute_d(
            Uint64::from(amp),
            &new_balances,
            math_config.greatest_precision,
        )?;

        let tokens_to_mint =
            Decimal256::with_precision(total_share, math_config.greatest_precision)?
                .checked_multiply_ratio(after_fee_d.saturating_sub(init_d), init_d)?;
        tokens_to_mint
    };

    let mint_amount = mint_amount.to_uint128_with_precision(math_config.greatest_precision)?;

    // If the mint amount is zero, then return a `Failure` response
    if mint_amount.is_zero() {
        return Ok(return_join_failure("Mint amount is zero".to_string()));
    }

    let res = AfterJoinResponse {
        provided_assets: act_assets_in,
        new_shares: mint_amount,
        response: dexter::pool::ResponseType::Success {},
        fee: Some(fee_tokens),
    };

    Ok(res)
}

/// ## Description
/// Returns [`AfterExitResponse`] type which contains -
/// assets_out - Is of type [`Vec<Asset>`] and is a sorted list consisting of amount and info of tokens which are to be subtracted from the PoolInfo state stored in the Vault contract and transfer from the Vault to the user
/// burn_shares - Number of LP shares to be burnt
/// response - A [`ResponseType`] which is either `Success` or `Failure`, deteriming if the tx is accepted by the Pool's math computations or not
///
/// ## Params
/// assets_out - Of type [`Vec<Asset>`], a sorted list containing amount / info of token balances user wants against the LP tokens transferred by the user to the Vault contract
/// * **deps** is the object of type [`Deps`].
/// STABLE-5-SWAP POOL -::- MATH LOGIC
/// T.B.A
pub fn query_on_exit_pool(
    deps: Deps,
    env: Env,
    assets_out: Option<Vec<Asset>>,
    burn_amount: Option<Uint128>,
) -> StdResult<AfterExitResponse> {
    // If the user has not provided number of LP tokens to be burnt, then return a `Failure` response
    if burn_amount.is_none() || burn_amount.unwrap().is_zero() {
        return Ok(return_exit_failure("Burn amount is zero".to_string()));
    }

    let config: Config = CONFIG.load(deps.storage)?;
    let math_config: MathConfig = MATHCONFIG.load(deps.storage)?;

    // Total share of LP tokens minted by the pool
    let total_share = query_supply(&deps.querier, config.lp_token_addr.clone())?;

    // Check asset definitions and make sure no asset is repeated
    if assets_out.is_some() {
        let mut assets_out_ = assets_out.clone().unwrap();
        // first sort the assets
        assets_out_.sort_by(|a, b| {
            a.info
                .to_string()
                .to_lowercase()
                .cmp(&b.info.to_string().to_lowercase())
        });
        let mut previous_asset: String = "".to_string();
        for asset in assets_out_.iter() {
            if previous_asset == asset.info.as_string() {
                return Ok(return_exit_failure(
                    "Repeated assets in asset_in".to_string(),
                ));
            }
            previous_asset = asset.info.as_string();
        }
    }

    let act_burn_amount;
    let mut fees: Vec<Asset> = vec![];
    let mut refund_assets;

    let pools = config.assets.clone();
    // If no assets are provided, we just burn the LP tokens and return the underlying assets based on their share in the pool
    if assets_out.is_none() {
        act_burn_amount = burn_amount.unwrap();
        refund_assets = get_share_in_assets(pools, act_burn_amount, total_share);
    } else {
        // Imbalanced withdraw
        let imb_wd_res: ImbalancedWithdrawResponse = match imbalanced_withdraw(
            deps,
            &env,
            &config,
            &math_config,
            burn_amount.unwrap(),
            &assets_out.clone().unwrap(),
            total_share,
        ) {
            Ok(res) => res,
            Err(err) => {
                return Ok(return_exit_failure(format!(
                    "Error during imbalanced_withdraw: {}",
                    err.to_string()
                )));
            }
        };
        act_burn_amount = imb_wd_res.burn_amount;
        fees = imb_wd_res.fee;
        refund_assets = assets_out.unwrap();
    }

    // although this check isn't required given the current state of code, but better to be safe than sorry.
    if act_burn_amount.is_zero() {
        return Ok(return_exit_failure("Burn amount is zero".to_string()));
    }

    // If some assets are omitted then add them explicitly with 0 deposit
    config.assets.iter().for_each(|pool_info| {
        if !refund_assets
            .iter()
            .any(|asset| asset.info.eq(&pool_info.info))
        {
            refund_assets.push(Asset {
                amount: Uint128::zero(),
                info: pool_info.info.clone(),
            });
        }
    });

    // Sort the refund assets
    refund_assets.sort_by(|a, b| {
        a.info
            .to_string()
            .to_lowercase()
            .cmp(&b.info.to_string().to_lowercase())
    });

    Ok(AfterExitResponse {
        assets_out: refund_assets,
        burn_shares: act_burn_amount,
        response: dexter::pool::ResponseType::Success {},
        fee: Some(fees),
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

    // Convert Asset to DecimalAsset types
    let pools = config
        .assets
        .into_iter()
        .map(|pool| {
            let token_precision = get_precision(deps.storage, &pool.info)?;
            Ok(DecimalAsset {
                info: pool.info,
                amount: Decimal256::with_precision(pool.amount, token_precision)?,
            })
        })
        .collect::<StdResult<Vec<_>>>()?;

    // Get the current balances of the Offer and ask assets from the supported assets list
    let (offer_pool, ask_pool) = match select_pools(
        &offer_asset_info.clone(),
        &ask_asset_info,
        &pools,
    ) {
        Ok(res) => res,
        Err(err) => {
            return Ok(return_swap_failure(format!(
                "Error during pool selection: {}",
                err
            )))
        }
    };

    // if there's 0 assets balance return failure response
    if offer_pool.amount.is_zero() || ask_pool.amount.is_zero() {
        return Ok(return_swap_failure(
            "Swap pool balances cannot be zero".to_string(),
        ));
    }

    // Offer and ask asset precisions
    let offer_precision = get_precision(deps.storage, &offer_pool.info)?;
    // let ask_precision = get_precision(deps.storage, &ask_pool.info)?;

    let offer_asset: Asset;
    let ask_asset: Asset;
    let (calc_amount, spread_amount): (Uint128, Uint128);
    let total_fee: Uint128;

    // Based on swap_type, we set the amount to either offer_asset or ask_asset pool
    match swap_type {
        SwapType::GiveIn {} => {
            offer_asset = Asset {
                info: offer_asset_info.clone(),
                amount,
            };
            
            // Calculate the commission fees
            total_fee = calculate_underlying_fees(amount, config.fee_info.total_fee_bps);
            let offer_amount_after_fee = amount.checked_sub(total_fee)?;

            let offer_asset_after_fee = Asset {
                info: offer_asset_info.clone(),
                amount: offer_amount_after_fee,
            }.to_decimal_asset(offer_precision)?;

            // Calculate the number of ask_asset tokens to be transferred to the recipient from the Vault contract
            (calc_amount, spread_amount) = match compute_swap(
                deps.storage,
                &env,
                &math_config,
                &offer_asset_after_fee,
                &offer_pool,
                &ask_pool,
                &pools,
            ) {
                Ok(res) => res,
                Err(err) => {
                    return Ok(return_swap_failure(format!(
                        "Error during swap calculation: {}",
                        err
                    )))
                }
            };

            ask_asset = Asset {
                info: ask_asset_info.clone(),
                amount: calc_amount,
            };
        }
        SwapType::GiveOut {} => {
            ask_asset = Asset {
                info: ask_asset_info.clone(),
                amount,
            };

            // Calculate the number of offer_asset tokens to be transferred from the trader from the Vault contract
            (calc_amount, spread_amount, total_fee) = match compute_offer_amount(
                deps.storage,
                &env,
                &math_config,
                &ask_asset,
                &offer_pool,
                &ask_pool,
                &pools,
                config.fee_info.total_fee_bps,
                math_config.greatest_precision,
            ) {
                Ok(res) => res,
                Err(err) => {
                    return Ok(return_swap_failure(format!(
                        "Error during offer amount calculation: {}",
                        err
                    )))
                }
            };
            
            offer_asset = Asset {
                info: offer_asset_info.clone(),
                amount: calc_amount,
            };
        }
        SwapType::Custom(_) => {
            return Ok(return_swap_failure("SwapType not supported".to_string()))
        }
    }

    // Check if the calculated amount is valid
    // although this check isn't required given the current state of code, but better to be safe than sorry.
    if calc_amount.is_zero() {
        return Ok(return_swap_failure(
            "Computation error - calc_amount is zero".to_string(),
        ));
    }

    // Check the max spread limit (if it was specified)
    let spread_check = assert_max_spread(
        belief_price,
        max_spread,
        offer_asset.amount - total_fee,
        ask_asset.amount,
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
            info: offer_asset_info.clone(),
            amount: total_fee,
        }),
    })
}

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
    offer_asset_info: AssetInfo,
    ask_asset_info: AssetInfo,
) -> StdResult<CumulativePriceResponse> {
    // Load the config, mathconfig  and twap from the storage
    let mut twap: Twap = TWAPINFO.load(deps.storage)?;
    let config: Config = CONFIG.load(deps.storage)?;
    let math_config: MathConfig = MATHCONFIG.load(deps.storage)?;

    let total_share = query_supply(&deps.querier, config.lp_token_addr)?;

    // Convert Vec<Asset> to Vec<DecimalAsset> type
    let decimal_assets: Vec<DecimalAsset> = transform_to_decimal_asset(deps, config.assets);

    // Accumulate prices of all assets in the config
    accumulate_prices(
        deps,
        env,
        math_config,
        &mut twap,
        &decimal_assets,
    )
    .map_err(|err| StdError::generic_err(format!("{err}")))?;

    // Find the `cumulative_price` for the provided offer and ask asset in the stored TWAP. Error if not found
    let res_exchange_rate = twap
        .cumulative_prices
        .into_iter()
        .find_position(|(offer_asset, ask_asset, _)| {
            offer_asset.eq(&offer_asset_info) && ask_asset.eq(&ask_asset_info)
        })
        .unwrap();

    // Return the cumulative price response
    let resp = CumulativePriceResponse {
        exchange_info: AssetExchangeRate {
            offer_info: res_exchange_rate.1 .0.clone(),
            ask_info: res_exchange_rate.1 .1.clone(),
            rate: res_exchange_rate.1 .2.clone(),
        },
        total_share,
    };

    Ok(resp)
}

/// ## Description
/// Returns information about the cumulative prices in a [`CumulativePricesResponse`] object.
/// ## Params
/// * **deps** is the object of type [`Deps`].
/// * **env** is the object of type [`Env`].
pub fn query_cumulative_prices(deps: Deps, env: Env) -> StdResult<CumulativePricesResponse> {
    let mut twap: Twap = TWAPINFO.load(deps.storage)?;
    let config: Config = CONFIG.load(deps.storage)?;
    let math_config: MathConfig = MATHCONFIG.load(deps.storage)?;

    let total_share = query_supply(&deps.querier, config.lp_token_addr)?;

    // Convert Vec<Asset> to Vec<DecimalAsset> type
    let decimal_assets: Vec<DecimalAsset> = transform_to_decimal_asset(deps, config.assets);

    // Accumulate prices of all assets in the config
    accumulate_prices(
        deps,
        env,
        math_config,
        &mut twap,
        &decimal_assets,
    )
    .map_err(|err| StdError::generic_err(format!("{err}")))?;

    // Prepare the cumulative prices response
    let mut asset_exchange_rates: Vec<AssetExchangeRate> = Vec::new();
    for (offer_asset, ask_asset, rate) in twap.cumulative_prices.clone() {
        asset_exchange_rates.push(AssetExchangeRate {
            offer_info: offer_asset.clone(),
            ask_info: ask_asset.clone(),
            rate: rate.clone(),
        });
    }

    Ok(CumulativePricesResponse {
        exchange_infos: asset_exchange_rates,
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
/// ## Params
/// * **mut math_config** is an object of type [`MathConfig`]. This is a mutable reference to the pool's custom math configuration.
fn stop_changing_amp(mut math_config: MathConfig, deps: DepsMut, env: Env) -> StdResult<()> {
    let current_amp = compute_current_amp(&math_config, &env)?;
    let block_time = env.block.time.seconds();

    math_config.init_amp = current_amp;
    math_config.next_amp = current_amp;
    math_config.init_amp_time = block_time;
    math_config.next_amp_time = block_time;

    // now (block_time < next_amp_time) is always False, so we return the saved AMP
    MATHCONFIG.save(deps.storage, &math_config)?;

    Ok(())
}

// --------x--------x--------x--------x--------x--------
// --------x--------x COMPUTATATIONS  x--------x--------
// --------x--------x--------x--------x--------x--------

/// ## Description - This struct describes the Fee configuration supported by a particular pool type.
#[cw_serde]
pub struct ImbalancedWithdrawResponse {
    pub burn_amount: Uint128,
    pub fee: Vec<Asset>,
}

/// ## Description
/// Imbalanced withdraw liquidity from the pool. Returns a [`ContractError`] on failure,
/// otherwise returns the number of LP tokens to burn.
/// ## Params
/// * **deps** is an object of type [`Deps`].
/// * **env** is an object of type [`Env`].
/// * **config** is an object of type [`Config`].
/// * **provided_amount** is an object of type [`Uint128`]. This is the amount of provided LP tokens to withdraw liquidity with.
/// * **assets** is array with objects of type [`Asset`]. It specifies the assets amount to withdraw. It is assumed that the assets are unique.
fn imbalanced_withdraw(
    deps: Deps,
    env: &Env,
    config: &Config,
    math_config: &MathConfig,
    provided_amount: Uint128,
    assets: &[Asset],
    total_share: Uint128,
) -> Result<ImbalancedWithdrawResponse, ContractError> {
    if assets.len() > config.assets.len() {
        return Err(ContractError::InvalidNumberOfAssets {});
    }

    // Store Pool balances in a hashMap
    let pools: HashMap<_, _> = config
        .assets
        .clone()
        .into_iter()
        .map(|pool| (pool.info, pool.amount))
        .collect();

    let mut assets_collection = assets
        .iter()
        .cloned()
        .map(|asset| {
            let precision = get_precision(deps.storage, &asset.info)?;
            // Get appropriate pool
            let pool = pools
                .get(&asset.info)
                .copied()
                .ok_or_else(|| ContractError::InvalidAsset(asset.info.to_string()))?;

            Ok((
                asset.to_decimal_asset(precision)?,
                Decimal256::with_precision(pool, precision)?,
            ))
        })
        .collect::<Result<Vec<_>, ContractError>>()?;

    // If some assets are omitted then add them explicitly with 0 withdraw amount
    pools
        .into_iter()
        .try_for_each(|(pool_info, pool_amount)| -> StdResult<()> {
            if !assets.iter().any(|asset| asset.info == pool_info) {
                let precision = get_precision(deps.storage, &pool_info)?;

                assets_collection.push((
                    DecimalAsset {
                        amount: Decimal256::zero(),
                        info: pool_info,
                    },
                    Decimal256::with_precision(pool_amount, precision)?,
                ));
            }
            Ok(())
        })?;

    let n_coins = config.assets.len() as u8;
    let amp = Uint64::from(compute_current_amp(math_config, env)?);

    // Initial invariant (D)
    let old_balances = assets_collection
        .iter()
        .map(|(_, pool)| *pool)
        .collect_vec();

    let init_d = compute_d(
        amp,
        &old_balances,
        math_config.greatest_precision,
    )?;

    // Invariant (D) after assets withdrawn
    let mut new_balances = assets_collection
        .iter()
        .map(|(withdraw, pool)| Ok(pool.checked_sub(withdraw.amount)?))
        .collect::<StdResult<Vec<_>>>()?;
    let withdraw_d = compute_d(
        amp,
        &new_balances,
        math_config.greatest_precision,
    )?;

    // total_fee_bps * N_COINS / (4 * (N_COINS - 1))
    let fee = Decimal::from_ratio(config.fee_info.total_fee_bps, FEE_PRECISION)
        .checked_mul(Decimal::from_ratio(n_coins, 4 * (n_coins - 1)))?;

    let fee = Decimal256::new(fee.atomics().into());

    // Tokens to be charged as Fee
    let mut fee_tokens: Vec<Asset> = vec![];

    // Fee is applied
    for i in 0..n_coins as usize {
        // Asset Info for token i
        let asset_info = assets_collection[i].0.info.clone();

        let ideal_balance = withdraw_d.checked_multiply_ratio(old_balances[i], init_d)?;
        let difference = if ideal_balance > new_balances[i] {
            ideal_balance - new_balances[i]
        } else {
            new_balances[i] - ideal_balance
        };
        let fee_charged = fee.checked_mul(difference)?;

        new_balances[i] -= fee_charged;
        fee_tokens.push(Asset {
            amount: fee_charged
                .to_uint128_with_precision(get_precision(deps.storage, &asset_info)?)?,
            info: asset_info,
        });
    }

    // New invariant (D) after fee applied
    let after_fee_d = compute_d(
        amp,
        &new_balances,
        math_config.greatest_precision,
    )?;

    let total_share = Uint256::from(total_share);

    // How many tokens do we need to burn to withdraw asked assets?
    let burn_amount = total_share
        .checked_multiply_ratio(
            init_d.atomics().checked_sub(after_fee_d.atomics())?,
            init_d.atomics(),
        )?
        .checked_add(Uint256::from(1u8))?; // In case of rounding errors - make it unfavorable for the "attacker"

    let burn_amount = burn_amount.try_into()?;

    if burn_amount > provided_amount {
        return Err(StdError::generic_err(format!(
            "Not enough LP tokens. You need {} LP tokens.",
            burn_amount
        ))
        .into());
    }

    Ok(ImbalancedWithdrawResponse {
        burn_amount,
        fee: fee_tokens,
    })
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

    if let Some(belief_price) = belief_price {
        let expected_return = offer_amount
            * belief_price
                .inv()
                .ok_or_else(|| {
                    ResponseType::Failure(
                        (ContractError::Std(StdError::generic_err(
                            "Invalid belief_price. Check the input values.",
                        )))
                        .to_string(),
                    )
                })
                .unwrap();

        let spread_amount = expected_return.saturating_sub(return_amount);
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

// --------x--------x--------x--------x--------x--------x---
// --------x--------x Helper Functions   x--------x---------
// --------x--------x--------x--------x--------x--------x---

/// ## Description
/// Converts [`Vec<Asset>`] to [`Vec<DecimalAsset>`].
pub fn transform_to_decimal_asset(deps: Deps, assets: Vec<Asset>) -> Vec<DecimalAsset> {
    assets
        .iter()
        .cloned()
        .map(|asset| {
            let precision = get_precision(deps.storage, &asset.info)?;
            asset.to_decimal_asset(precision)
        })
        .collect::<StdResult<Vec<DecimalAsset>>>()
        .unwrap()
}
