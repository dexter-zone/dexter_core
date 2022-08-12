use std::convert::TryInto;
use std::str::FromStr;

use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Addr, Binary, Decimal, Decimal256, Deps, DepsMut,
    Env, Event, Fraction, MessageInfo, Reply, ReplyOn, Response, StdError, StdResult, SubMsg,
    Uint128, Uint256, WasmMsg,
};

use crate::response::MsgInstantiateContractResponse;
use cw2::set_contract_version;
use cw20::MinterResponse;
use dexter::asset::{
    addr_validate_to_lower, Asset, AssetExchangeRate, AssetInfo, Decimal256Ext, DecimalAsset,
};
use dexter::helper::{
    adjust_precision, get_lp_token_name, get_lp_token_symbol, get_share_in_assets, select_pools,
};
// use dexter::helper::decimal2decimal256;
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{
    return_exit_failure, return_join_failure, return_swap_failure, AfterExitResponse,
    AfterJoinResponse, Config, ConfigResponse, CumulativePriceResponse, CumulativePricesResponse,
    ExecuteMsg, FeeResponse, InstantiateMsg, MigrateMsg, QueryMsg, ResponseType, SwapResponse,
    Trade, WeightedParams,
};
use dexter::querier::query_supply;
use dexter::vault::{SwapType, TWAP_PRECISION};

use crate::error::ContractError;
use crate::math::{calculate_invariant, get_normalized_weight};
use crate::state::{
    get_precision, get_weight, store_precisions, store_weights, MathConfig, Twap, WeightedAsset,
    CONFIG, MATHCONFIG, TWAPINFO,
};
use crate::utils::{
    accumulate_prices, calc_single_asset_join, compute_offer_amount, compute_swap,
    maximal_exact_ratio_join,
};

use protobuf::Message;
use std::vec;

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter::fixed_weighted_pool";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// A `reply` call code ID of sub-message.
const INSTANTIATE_TOKEN_REPLY_ID: u64 = 1;

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x      Instantiate Contract : Execute function     x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

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
    mut deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // check valid token info
    msg.validate()?;

    // Validate number of assets
    if msg.asset_infos.len() > 9 || msg.asset_infos.len() < 2 {
        return Err(ContractError::InvalidNumberOfAssets {});
    }

    // Weights assigned to assets
    let WeightedParams { weights, exit_fee } = from_binary(&msg.init_params.unwrap())?;

    // Error if number of assets and weights provided do not match
    if msg.asset_infos.len() != weights.len() {
        return Err(ContractError::NumberOfAssetsAndWeightsMismatch {});
    }

    // Calculate total weight and the weight share of each asset in the pool and store it in the storage
    let total_weight = weights.iter().map(|(_, weight)| *weight).sum::<u128>();
    let mut asset_weights: Vec<(AssetInfo, Decimal)> = vec![];
    for (asset_info, asset_weight) in weights.iter() {
        let normalized_weight = get_normalized_weight(*asset_weight, total_weight);
        asset_weights.push((asset_info.clone(), normalized_weight));
    }
    store_weights(deps.branch(), asset_weights)?;

    // Store token precisions in the storage
    let greatest_precision = store_precisions(deps.branch(), &msg.asset_infos)?;

    // Initializing cumulative prices
    let mut cumulative_prices = vec![];
    for from_pool in &msg.asset_infos {
        for to_pool in &msg.asset_infos {
            if !from_pool.eq(to_pool) {
                cumulative_prices.push((from_pool.clone(), to_pool.clone(), Uint128::zero()))
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
        lp_token_addr: None,
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
        greatest_precision,
        exit_fee,
    };

    // Store config, MathConfig and twap in storage
    CONFIG.save(deps.storage, &config)?;
    MATHCONFIG.save(deps.storage, &math_config)?;
    TWAPINFO.save(deps.storage, &twap)?;

    // LP Token Name
    let token_name = get_lp_token_name(msg.pool_id.clone(), msg.lp_token_name.clone());

    // LP Token Symbol
    let token_symbol = get_lp_token_symbol( msg.lp_token_symbol.clone());
    // Create LP token
    let sub_msg: Vec<SubMsg> = vec![SubMsg {
        msg: WasmMsg::Instantiate {
            code_id: msg.lp_token_code_id.clone(),
            msg: to_binary(&TokenInstantiateMsg {
                name: token_name,
                symbol: token_symbol,
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: msg.vault_addr.clone().to_string(),
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
    // Get config
    let mut config: Config = CONFIG.load(deps.storage)?;

    // Validation check
    if config.lp_token_addr.is_some() {
        return Err(ContractError::Unauthorized {});
    }

    // get lp token address from reply
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
        ExecuteMsg::UpdateConfig { .. } => Err(ContractError::NonSupported {}),
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

    // Update state
    config.assets = assets;
    config.block_time_last = env.block.time.seconds();
    CONFIG.save(deps.storage, &config)?;

    // Convert Vec<Asset> to Vec<DecimalAsset> type
    let decimal_assets: Vec<DecimalAsset> =
        transform_to_decimal_asset(deps.as_ref(), &config.assets);

    // Accumulate prices for the assets in the pool
    if !accumulate_prices(
        deps.as_ref(),
        env,
        &mut config,
        math_config,
        &mut twap,
        &decimal_assets,
    )
    .is_ok()
    {
        return Err(ContractError::PricesUpdateFailed {});
    }
    TWAPINFO.save(deps.storage, &twap)?;

    let event = Event::new("dexter-pool::update-liquidity")
        .add_attribute("pool_id", config.pool_id.to_string());

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
            slippage_tolerance
        } => to_binary(&query_on_join_pool(deps, env, assets_in, mint_amount, slippage_tolerance )?),
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
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    let math_config: MathConfig = MATHCONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        pool_id: config.pool_id,
        lp_token_addr: config.lp_token_addr,
        vault_addr: config.vault_addr,
        assets: config.assets,
        pool_type: config.pool_type,
        fee_info: config.fee_info,
        block_time_last: config.block_time_last,
        math_params: Some(to_binary(&math_config).unwrap()),        
        additional_params: None
    })
}

/// ## Description
/// Returns information about the Fees settings in a [`FeeResponse`] object.
/// ## Params
/// * **deps** is the object of type [`Deps`].
pub fn query_fee_params(deps: Deps) -> StdResult<FeeResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    Ok(FeeResponse {
        total_fee_bps: config.fee_info.total_fee,
        swap_fee_dir: config.fee_info.swap_fee_dir
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
/// WEIGHTED POOL -::- MATH LOGIC
/// T.B.A -- User needs to provide the assets that he wants to use to Join the pool
pub fn query_on_join_pool(
    deps: Deps,
    _env: Env,
    assets_in: Option<Vec<Asset>>,
    mint_amount: Option<Uint128>,
    slippage_tolerance: Option<Decimal>,
) -> StdResult<AfterJoinResponse> {
    // If the user has not provided any assets to be provided, then return a `Failure` response
    if assets_in.is_none() {
        return Ok(return_join_failure("No assets provided".to_string()));
    }
    // Sort the assets in the order of the assets in the config
    let mut act_assets_in = assets_in.unwrap();
    act_assets_in.sort_by(|a, b| {
        a.info
            .to_string()
            .to_lowercase()
            .cmp(&b.info.to_string().to_lowercase())
    });

    // 1) Get pool current liquidity + and token weights
    // 2) If single token provided, do single asset join and exit.
    // 3) If multi-asset join, first do as much of a join as we can with no swaps.
    // 4) Update pool shares / liquidity / remaining tokens to join accordingly
    // 5) For every remaining token to LP, do a single asset join, and update pool shares / liquidity.
    //
    // Note that all single asset joins do incur swap fee.
    //
    // Since CalcJoinPoolShares is non-mutative, the steps for updating pool shares / liquidity are
    // more complex / don't just alter the state.

    // We should simplify this logic further in the future, using balancer multi-join equations.

    // 1) get all 'pool assets' (aka current pool liquidity + balancer weight)

    let config: Config = CONFIG.load(deps.storage)?;
    let math_config: MathConfig = MATHCONFIG.load(deps.storage)?;
    // Total share of LP tokens minted by the pool
    let total_share = query_supply(&deps.querier, config.lp_token_addr.clone().unwrap().clone())?;

    //  1) Get pool current liquidity + and token weights : Convert assets to WeightedAssets
    let mut pool_assets_weighted: Vec<WeightedAsset> = config
        .assets
        .iter()
        .map(|asset| {
            let weight = get_weight(deps.storage, &asset.info)?;
            Ok(WeightedAsset {
                asset: asset.clone(),
                weight,
            })
        })
        .collect::<StdResult<Vec<WeightedAsset>>>()?;

    // 2) If single token provided, do single asset join and exit.
    if act_assets_in.len() == 1 {
        let in_asset = act_assets_in[0].to_owned();
        let weighted_in_asset = pool_assets_weighted
            .iter()
            .find(|asset| asset.asset.info.equal(&in_asset.info))
            .unwrap();
        let num_shares: Uint128 = calc_single_asset_join(
            deps,
            &in_asset,
            config.fee_info.total_fee_bps,
            weighted_in_asset,
            total_share,
        )?;
        // Add assets which are omitted with 0 deposit
        pool_assets_weighted.iter().for_each(|pool_asset| {
            if !act_assets_in
                .iter()
                .any(|asset| asset.info.eq(&pool_asset.asset.info))
            {
                act_assets_in.push(Asset {
                    info: pool_asset.asset.info.clone(),
                    amount: Uint128::new(0),
                });
            }
        });
        // Return the response
        if !num_shares.is_zero() {
            return Ok(AfterJoinResponse {
                provided_assets: act_assets_in,
                new_shares: num_shares,
                response: dexter::pool::ResponseType::Success {},
            });
        }
    }

    // If more than one asset, all should be provided
    if act_assets_in.len() != pool_assets_weighted.len() {
        return Ok( return_join_failure(     "If more than one asset, all should be provided".to_string()     ) );
    }

    // 3) JoinPoolNoSwap with as many tokens as we can. (What is in perfect ratio)
    // * numShares is how many shares are perfectly matched.
    // * remainingTokensIn is how many coins we have left to join, that have not already been used.
    // if remaining coins is empty, logic is done (we joined all tokensIn)

    let (mut num_shares, remaining_tokens_in, err): (Uint128, Vec<Asset>, ResponseType) =
        if total_share.is_zero() {
            let decimal_assets: Vec<DecimalAsset> =
                transform_to_decimal_asset(deps, &act_assets_in);
            let invariance = calculate_invariant(
                pool_assets_weighted
                    .iter()
                    .map(|asset| asset.weight)
                    .collect(),
                decimal_assets,
            )?;
            // mint sqrt(invariance) lp tokens, no other tokens left
            (
                invariance.sqrt().to_uint128_with_precision(6u8)?,
                vec![],
                ResponseType::Success {},
            )
        } else {
            maximal_exact_ratio_join(act_assets_in.clone(), &pool_assets_weighted, total_share)?
        };

    if !err.is_success() {
        return Ok(return_join_failure(  err.to_string() ) );
    }
    if remaining_tokens_in.is_empty() {
        return Ok(AfterJoinResponse {
            provided_assets: act_assets_in,
            new_shares: num_shares,
            response: dexter::pool::ResponseType::Success {},
        });
    }

    // 4) Still more coins to join, so we update the effective pool state here to account for
    // join that just happened.
    // * We add the joined coins to our "current pool liquidity" object (poolAssetsByDenom)
    // * We increment a variable for our "newTotalShares" to add in the shares that've been added.
    let mut tokens_joined: Vec<Asset> = vec![];
    for (act_asset_in, remaining_token_in) in act_assets_in.iter().zip(remaining_tokens_in.iter()) {
        tokens_joined.push(Asset {
            info: act_asset_in.info.clone(),
            amount: act_asset_in
                .amount
                .clone()
                .checked_sub(remaining_token_in.amount.clone())?,
        });
    }
    update_pool_state_for_joins(&tokens_joined, &mut pool_assets_weighted);
    let mut new_total_shares = total_share.checked_add(num_shares)?;

    // 5) Now single asset join each remaining coin.
    for single_asset in remaining_tokens_in.iter() {
        let weighted_in_asset = pool_assets_weighted
            .iter()
            .find(|asset| asset.asset.info.equal(&single_asset.info))
            .unwrap();
        let new_num_shares_from_single: Uint128 = calc_single_asset_join(
            deps,
            single_asset,
            config.fee_info.total_fee_bps,
            weighted_in_asset,
            new_total_shares,
        )?;
        // update total shares
        new_total_shares = new_total_shares.checked_add(new_num_shares_from_single)?;
        // add to lp-tokens to mint
        num_shares += new_num_shares_from_single;
    }

    // Calculate the final tokens that have joined the pool.
    let mut final_tokens_joined: Vec<Asset> = vec![];
    for (token_joined, remaining_token_in) in tokens_joined.iter().zip(remaining_tokens_in.iter()) {
        final_tokens_joined.push(Asset {
            info: token_joined.info.clone(),
            amount: token_joined
                .amount
                .clone()
                .checked_add(remaining_token_in.amount.clone())?,
        });
    }

    let res = AfterJoinResponse {
        provided_assets: final_tokens_joined,
        new_shares: num_shares,
        response: dexter::pool::ResponseType::Success {},
    };
    Ok(res)
}

fn update_pool_state_for_joins(
    tokens_joined: &[Asset],
    pool_assets_weighted: &mut Vec<WeightedAsset>,
) {
    for asset in tokens_joined {
        for pool_asset in pool_assets_weighted.iter_mut() {
            if asset.info.equal(&pool_asset.asset.info) {
                pool_asset.asset.amount += asset.amount;
            }
        }
    }
}

/// ## Description
/// Returns [`AfterExitResponse`] type which contains -  
/// assets_out - Is of type [`Vec<Asset>`] and is a sorted list consisting of amount and info of tokens which are to be subtracted from the PoolInfo state stored in the Vault contract and tranfer from the Vault to the user
/// burn_shares - Number of LP shares to be burnt
/// response - A [`ResponseType`] which is either `Success` or `Failure`, determining if the tx is accepted by the Pool's math computations or not
///
/// ## Params
/// assets_out - Of type [`Vec<Asset>`], a sorted list containing amount / info of token balances user wants against the LP tokens transferred by the user to the Vault contract
/// * **deps** is the object of type [`Deps`].
/// WEIGHTED POOL -::- MATH LOGIC
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
    let total_share = query_supply(&deps.querier, config.lp_token_addr.unwrap().clone())?;
    let act_burn_shares = burn_amount.unwrap();

    // refundedShares = act_burn_shares * (1 - exit fee)
    // with 0 exit fee optimization

    // Calculate number of LP tokens to be refunded to the user
    // --> Weighted pool allows setting an exit fee for the pool which needs to be less than 3%
    let mut refunded_shares = act_burn_shares;
    if math_config.exit_fee.is_some() && !math_config.exit_fee.unwrap().is_zero() {
        let one_sub_exit_fee = Decimal::one() - math_config.exit_fee.unwrap();
        refunded_shares = act_burn_shares * one_sub_exit_fee;
    }

    // % of share to be burnt from the pool
    let share_out_ratio = Decimal::from_ratio(refunded_shares, total_share);

    // Vector of assets to be transferred to the user from the Vault contract
    let mut refund_assets: Vec<Asset> = vec![];
    for asset in config.assets.iter() {
        let asset_out = asset.amount * share_out_ratio;
        // Return a `Failure` response if the calculation of the amount of tokens to be burnt from the pool is not valid
        if asset_out > asset.amount {
            return Ok(return_exit_failure("Invalid asset out".to_string()));
        }
        // Add the asset to the vector of assets to be transferred to the user from the Vault contract
        refund_assets.push(Asset {
            info: asset.info.clone(),
            amount: asset_out,
        });
    }

    Ok(AfterExitResponse {
        assets_out: refund_assets,
        burn_shares: act_burn_shares,
        response: dexter::pool::ResponseType::Success {},
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
/// WEIGHTED POOL -::- MATH LOGIC
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
    let (offer_pool, ask_pool) = select_pools(
        Some(&offer_asset_info.clone()),
        Some(&ask_asset_info),
        &pools,
    )
    .unwrap_or_else(|_| {
        (
            DecimalAsset {
                info: pools[0].info.clone(),
                amount: Decimal256::zero(),
            },
            DecimalAsset {
                info: pools[1].info.clone(),
                amount: Decimal256::zero(),
            },
        )
    });

    // if there's 0 assets balance return failure response
    if offer_pool.amount.is_zero() || ask_pool.amount.is_zero() {
        return Ok(return_swap_failure("Invalid swap amounts".to_string()));
    }

    // Offer and ask asset precisions
    let offer_precision = get_precision(deps.storage, &offer_pool.info)?;
    let ask_precision = get_precision(deps.storage, &ask_pool.info)?;

    // Get the weights of offer and ask assets
    let offer_weight = get_weight(deps.storage, &offer_pool.info)?;
    let ask_weight = get_weight(deps.storage, &ask_pool.info)?;

    let offer_asset: Asset;
    let ask_asset: Asset;
    let (calc_amount, spread_amount): (Uint128, Uint128);
    let (total_fee, protocol_fee, dev_fee): (Uint128, Uint128, Uint128);

    // Based on swap_type, we set the amount to either offer_asset or ask_asset pool
    match swap_type {
        SwapType::GiveIn {} => {
            offer_asset = Asset {
                info: offer_asset_info.clone(),
                amount,
            };
            // Calculate the number of ask_asset tokens to be transferred to the recipient from the Vault contract
            (calc_amount, spread_amount) = compute_swap(
                deps.storage,
                &env,
                &offer_asset.to_decimal_asset(offer_precision)?,
                &offer_pool,
                offer_weight,
                &ask_pool,
                ask_weight,
            )
            .unwrap_or_else(|_| (Uint128::zero(), Uint128::zero()));
            // Calculate the commission fees
            (total_fee, protocol_fee, dev_fee) =
                config.fee_info.calculate_underlying_fees(calc_amount);

            ask_asset = Asset {
                info: ask_asset_info.clone(),
                amount: calc_amount.checked_sub(total_fee)?, // Subtract fee from return amount
            };
        }
        SwapType::GiveOut {} => {
            ask_asset = Asset {
                info: ask_asset_info.clone(),
                amount,
            };
            // Calculate the number of offer_asset tokens to be transferred from the trader to the Vault contract
            (calc_amount, spread_amount) = compute_offer_amount(
                deps.storage,
                &env,
                &ask_asset.to_decimal_asset(ask_precision)?,
                &ask_pool,
                ask_weight,
                &offer_pool,
                offer_weight,
            )
            .unwrap_or_else(|_| (Uint128::zero(), Uint128::zero()));

            // Calculate the commission fees
            (total_fee, protocol_fee, dev_fee) =
                config.fee_info.calculate_underlying_fees(calc_amount);
            offer_asset = Asset {
                info: offer_asset_info.clone(),
                amount: calc_amount,
            };
        }
        SwapType::Custom(_) => return Ok(return_swap_failure("SwapType not supported".to_string()))
    }

    Ok(SwapResponse {
        trade_params: Trade {
            amount_in: offer_asset.amount,
            amount_out: ask_asset.amount,
            spread: spread_amount,
            total_fee: total_fee,
            protocol_fee,
            dev_fee,
        },
        response: ResponseType::Success {},
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
    let mut config: Config = CONFIG.load(deps.storage)?;
    let math_config: MathConfig = MATHCONFIG.load(deps.storage)?;

    let total_share = query_supply(&deps.querier, config.lp_token_addr.clone().unwrap().clone())?;

    // Convert Vec<Asset> to Vec<DecimalAsset> type
    let decimal_assets: Vec<DecimalAsset> =
        transform_to_decimal_asset(deps, &config.assets.clone());

    // Accumulate prices of all assets in the config
    accumulate_prices(
        deps,
        env,
        &mut config,
        math_config,
        &mut twap,
        &decimal_assets,
    )
    .map_err(|err| StdError::generic_err(format!("{err}")))?;

    // Find the `cumulative_price` for the provided offer and ask asset in the stored TWAP. Error if not found
    let res_exchange_rate = twap
        .cumulative_prices
        .into_iter()
        .find(|(offer_asset, ask_asset, rate)| {
            offer_asset.eq(&offer_asset_info) && ask_asset.eq(&ask_asset_info)
        })
        .unwrap();

    // Return the cumulative price response
    let resp = CumulativePriceResponse {
        exchange_info: AssetExchangeRate {
            offer_info: res_exchange_rate.0.clone(),
            ask_info: res_exchange_rate.1.clone(),
            rate: res_exchange_rate.2.clone(),
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
    let mut config: Config = CONFIG.load(deps.storage)?;
    let math_config: MathConfig = MATHCONFIG.load(deps.storage)?;

    let total_share = query_supply(&deps.querier, config.lp_token_addr.clone().unwrap())?;

    // Convert Vec<Asset> to Vec<DecimalAsset> type
    let decimal_assets: Vec<DecimalAsset> = transform_to_decimal_asset(deps, &config.assets);

    // Accumulate prices of all assets in the config
    accumulate_prices(
        deps,
        env,
        &mut config,
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
pub fn transform_to_decimal_asset(deps: Deps, assets: &Vec<Asset>) -> Vec<DecimalAsset> {
    let decimal_assets = assets
        .iter()
        .map(|asset| {
            let precision = get_precision(deps.storage, &asset.info)?;
            asset.to_decimal_asset(precision)
        })
        .collect::<StdResult<Vec<DecimalAsset>>>()
        .unwrap();
    decimal_assets
}
