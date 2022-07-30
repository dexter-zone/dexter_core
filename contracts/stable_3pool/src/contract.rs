use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, Decimal, Decimal256, Deps, DepsMut,
    Env, Event, MessageInfo, Reply, ReplyOn, Response, StdError, StdResult, SubMsg,
    Uint128, Uint256, WasmMsg, from_binary
};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};
use protobuf::Message;
use std::str::FromStr;
use std::vec;
use std::convert::TryInto;
use std::collections::HashMap;

use crate::response::MsgInstantiateContractResponse;
use crate::state::{TWAPINFO, CONFIG, MATHCONFIG, PRECISIONS, MathConfig, Twap , StablePoolParams, StablePoolUpdateParams, store_precisions, get_precision};
use crate::error::ContractError;

use dexter::pool::{
    AfterExitResponse, AfterJoinResponse, Config, ConfigResponse, CumulativePriceResponse,
    CumulativePricesResponse, ExecuteMsg, FeeResponse, InstantiateMsg, MigrateMsg, QueryMsg,
    ResponseType, SwapResponse, Trade,return_join_failure, return_swap_failure, return_exit_failure
};
use crate::math::{
    compute_d, AMP_PRECISION, MAX_AMP, MAX_AMP_CHANGE,
    MIN_AMP_CHANGING_TIME,
};
use crate::utils::{accumulate_prices, compute_swap, compute_offer_amount};

use dexter::asset::{addr_validate_to_lower, Asset, AssetInfo,Decimal256Ext, DecimalAsset, AssetExchangeRate};
use dexter::vault::{SwapType, TWAP_PRECISION};
use dexter::helper::{adjust_precision, get_share_in_assets, get_lp_token_name,get_lp_token_symbol, select_pools };
use dexter::querier::{ query_supply, query_vault_config, query_token_precision};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::U256;
use dexter::DecimalCheckedOps;


/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "dexter::stable3swap_pool";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// A `reply` call code ID of sub-message.
const INSTANTIATE_TOKEN_REPLY_ID: u64 = 1;

/// An LP token precision.
const LP_TOKEN_PRECISION: u8 = 6;

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

    // check valid token info
    msg.validate()?;    

    // Validate number of assets
    if msg.asset_infos.len() > 5 || msg.asset_infos.len() < 2 {
        return Err(ContractError::InvalidNumberOfAssets {});
    }

    // Stable3swap parameters
    let params: StablePoolParams = from_binary(&msg.init_params.unwrap())?;
    if params.amp == 0 || params.amp > MAX_AMP {
        return Err(ContractError::IncorrectAmp {});
    }

    // store precisions for assets in storage
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
        init_amp:  params.amp * AMP_PRECISION,
        init_amp_time: env.block.time.seconds(),
        next_amp:  params.amp * AMP_PRECISION,
        next_amp_time: env.block.time.seconds(),
        greatest_precision
    };

    // Store config, MathConfig and twap in storage
    CONFIG.save(deps.storage, &config)?;
    MATHCONFIG.save(deps.storage, &math_config)?;
    TWAPINFO.save(deps.storage, &twap)?;

    // LP Token Name
    let token_name = get_lp_token_name(msg.pool_id.clone(),msg.lp_token_name );

    // LP Token Symbol
    let token_symbol = get_lp_token_symbol(msg.pool_id.clone(),msg.lp_token_symbol );

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
                    minter: msg.vault_addr.to_string(),
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
/// 
/// # Params
/// * **msg** is the object of type [`Reply`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    // Get config
    let mut config : Config = CONFIG.load(deps.storage)?;

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
        ExecuteMsg::UpdateConfig { params } => update_config(deps, env, info, params),
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

    // Update state
    config.assets = assets;
    config.block_time_last = env.block.time.seconds();
    CONFIG.save(deps.storage, &config)?;

    // Accumulate prices for the assets in the pool
    if !accumulate_prices(deps.as_ref(), env, &mut config, &mut twap, &config.assets.clone()).is_ok() {
        return Err(ContractError::PricesUpdateFailed  {});
    }
    TWAPINFO.save(deps.storage, &twap)?;

    let event = Event::new("dexter-pool::update-liquidity")
        .add_attribute("pool_id", config.pool_id.to_string());
 
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
    params: Binary,
) -> Result<Response, ContractError> {

    let config = CONFIG.load(deps.storage)?;
    let math_config = MATHCONFIG.load(deps.storage)?;
    let vault_config = query_vault_config(&deps.querier, config.vault_addr.clone().to_string())?;

    // Access Check :: Only Vault's Owner can execute this function
    if info.sender != vault_config.owner {
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
        QueryMsg::OnJoinPool { assets_in, mint_amount } => to_binary(&query_on_join_pool(deps, env, assets_in, mint_amount)?),
        QueryMsg::OnExitPool {
            assets_out,
            burn_amount,
        } => to_binary(&query_on_exit_pool(deps , env, assets_out, burn_amount)?),
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
        protocol_fee_percent: config.fee_info.protocol_fee_percent,
        dev_fee_percent: config.fee_info.dev_fee_percent,
        dev_fee_collector: config.fee_info.developer_addr,
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
/// STABLE-3-SWAP POOL -::- MATH LOGIC
/// -- Implementation - For Stable-3-swap, user provides the exact number of assets he/she wants to supply as liquidity to the pool. We simply calculate the number of LP shares to be minted and return it to the user.
/// T.B.A
pub fn query_on_join_pool(
    deps: Deps,
    env: Env,
    assets_in: Option<Vec<Asset>>,
    _mint_amount: Option<Uint128>,
) -> StdResult<AfterJoinResponse> {

    // If the user has not provided any assets to be provided, then return a `Failure` response
    if assets_in.is_none() {
        return Ok(return_join_failure());
    }
    
    // Load the config and math config from the storage
    let config: Config = CONFIG.load(deps.storage)?;
    let math_config: MathConfig = MATHCONFIG.load(deps.storage)?;

    // Sort the assets in the order of the assets in the config
    let mut act_assets_in = assets_in.unwrap();
    act_assets_in.sort_by(|a, b| {
        a.info
            .to_string()
            .to_lowercase()
            .cmp(&b.info.to_string().to_lowercase())
    });

    // Get Asset  stored in state for each asset in a HashMap
    let token_pools: HashMap<_, _> = config
        .assets
        .into_iter()
        .map(|pool| (pool.info, pool.amount))
        .collect();

    let mut non_zero_flag = false;

    // get asset info for each asset in the list provided by the user to its pool mapping
    let mut assets_collection = act_assets_in.clone().into_iter()
        .map(|asset| {
            // Check that at least one asset is non-zero
            if !asset.amount.is_zero() {
                non_zero_flag = true;
            }

            // Get appropriate pool
            let token_pool = token_pools
                .get(&asset.info)
                .copied()
                .ok_or_else(|| return Ok(return_join_failure()))?;

            Ok((asset, token_pool))
        })
        .collect::<Result<Vec<_>, ContractError>>()?;

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
        }
    });

    // If there's no non-zero assets in the list provided by the user, then return a `Failure` response
    if !non_zero_flag {
        return Ok(return_join_failure());
    }

    // Adjust for precision
    for (deposit, pool) in assets_collection.iter_mut() {
        // We cannot put a zero amount into an empty pool.
        if deposit.amount.is_zero() && pool.is_zero() {
            return Ok(return_join_failure());
        }

        // Adjusting to the greatest precision
        let coin_precision = get_precision(deps.storage, &deposit.info)?;
        deposit.amount =
            adjust_precision(deposit.amount, coin_precision, math_config.greatest_precision)?;
        *pool = adjust_precision(*pool, coin_precision, math_config.greatest_precision)?;
    }
    
    // Compute amp parameter
    let n_coins = config.assets.len() as u8;
    let amp = compute_current_amp(&math_config, &env)?.checked_mul(n_coins.into()).unwrap_or_else(|| 0u64.into());

    // If AMP value is invalid, then return a `Failure` response
    if amp == 0u64 {
        return Ok(return_join_failure());
    }

    // Initial invariant (D)
    let old_balances = assets_collection
        .iter()
        .map(|(_, pool)| *pool)
        .collect_vec();
        let init_d = compute_d(amp.into(), &old_balances, math_config.greatest_precision)?;

    // Invariant (D) after deposit added
    let mut new_balances = assets_collection
        .iter()
        .map(|(deposit, pool)| Ok(pool.checked_add(deposit.amount)?))
        .collect::<StdResult<Vec<_>>>()?;
    let deposit_d = compute_d(amp.into(), &new_balances, math_config.greatest_precision)?;

    // Total share of LP tokens minted by the pool
    let total_share = query_supply(&deps.querier, config.lp_token_addr.clone().unwrap().clone())?;

    // Calculate the number of LP shares to be minted
    let mint_amount = if total_share.is_zero() {
        deposit_d
    } else {
        // Get fee info from the factory
        let fee_info =  config.fee_info.clone();

        // total_fee_bps * N_COINS / (4 * (N_COINS - 1))
        let fee = fee_info
            .total_fee_bps
            .checked_mul(Decimal::from_ratio(n_coins, 4 * (n_coins - 1)))?;

        let fee = Decimal256::new(fee.atomics().into());

        for i in 0..n_coins as usize {
            let ideal_balance = deposit_d.checked_multiply_ratio(old_balances[i], init_d)?;
            let difference = if ideal_balance > new_balances[i] {
                ideal_balance - new_balances[i]
            } else {
                new_balances[i] - ideal_balance
            };
            // Fee will be charged only during imbalanced provide i.e. if invariant D was changed
            new_balances[i] -= fee.checked_mul(difference)?;
        }

        let after_fee_d = compute_d(amp, &new_balances, math_config.greatest_precision)?;

        Decimal256::with_precision(total_share, math_config.greatest_precision)?
            .checked_multiply_ratio(after_fee_d.saturating_sub(init_d), init_d)?
    };

    let mint_amount = mint_amount.to_uint128_with_precision(math_config.greatest_precision)?;

    // If the mint amount is zero, then return a `Failure` response
    if mint_amount.is_zero() {
        return Ok(return_join_failure());
    }

    let res = AfterJoinResponse {
        provided_assets: act_assets_in,
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
/// 
/// ## Params
/// assets_out - Of type [`Vec<Asset>`], a sorted list containing amount / info of token balances user wants against the LP tokens transferred by the user to the Vault contract
/// * **deps** is the object of type [`Deps`].
/// STABLE-3-SWAP POOL -::- MATH LOGIC
/// T.B.A
pub fn query_on_exit_pool(
    deps: Deps,
    env: Env,
    assets_out: Option<Vec<Asset>>,
    burn_amount: Option<Uint128>,
) -> StdResult<AfterExitResponse> {
    let config: Config = CONFIG.load(deps.storage)?;

    // Total share of LP tokens minted by the pool
    let total_share = query_supply(&deps.querier, config.lp_token_addr.unwrap().clone())?;

    let act_burn_amount;
    let refund_assets;

    let pools = config.assets;
    // If no assets are provided, we just burn the LP tokens and return the underlying assets based on their share in the pool
    if assets_out.is_none() {
        act_burn_amount = burn_amount;
        refund_assets = get_share_in_assets(pools.clone(), burn_amount, total_share);
    } else {
        // Imbalanced withdraw
        act_burn_amount = imbalanced_withdraw(deps.as_ref(), &env, &config, math_config,  burn_amount, &assets_out.unwrap()).unwrap_or_else(|_| 0u128.into());
        refund_assets = assets_out.unwrap();
    }


    Ok(AfterExitResponse {
        assets_out: refund_assets,
        burn_shares: act_burn_amount,
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
/// STABLES-3-WAP POOL -::- MATH LOGIC
/// T.B.A
pub fn query_on_swap(
    deps: Deps,
    env: Env,
    swap_type: SwapType,
    offer_asset_info: AssetInfo,
    ask_asset_info: AssetInfo,
    amount: Uint128,
) -> StdResult<SwapResponse> {

    // Load the config and math config from the storage
    let config: Config = CONFIG.load(deps.storage)?;
    let math_config: MathConfig = MATHCONFIG.load(deps.storage)?;

    // Convert Asset to DecimalAsset types
    let pools = config
        .assets
        .into_iter()
        .map(|mut pool| {
            let token_precision = get_precision(deps.storage, &pool.info)?;
            Ok(DecimalAsset {
                info: pool.info,
                amount: Decimal256::with_precision(pool.amount, token_precision)?,
            })
        })
        .collect::<StdResult<Vec<_>>>()?;

    // Get the current balances of the Offer and ask assets from the supported assets list
    let (offer_pool, ask_pool) = select_pools( Some(&offer_asset_info.clone()), Some(&ask_asset_info), &pools)
                                                            .unwrap_or_else(|_| (DecimalAsset {
                                                                info: pools[0].info.clone(),
                                                                amount: Decimal256::zero()
                                                            }, DecimalAsset {
                                                                info: pools[1].info.clone(),
                                                                amount: Decimal256::zero()
                                                            }));

    // if there's 0 assets balance return failure response
    if offer_pool.amount.is_zero() || ask_pool.amount.is_zero() {
        return Ok( return_swap_failure() ) 
    }

    // Offer and ask asset precisions
    let offer_precision = get_precision(deps.storage, &offer_pool.info)?;
    let ask_precision = get_precision(deps.storage, &ask_pool.info)?;

    let mut offer_asset: Asset;
    let mut ask_asset: Asset;
    let (mut calc_amount, mut spread_amount): (Uint128, Uint128);
    let (total_fee, protocol_fee, dev_fee): (Uint128, Uint128, Uint128);    

    // Based on swap_type, we set the amount to either offer_asset or ask_asset pool
    match swap_type {
        
        SwapType::GiveIn {} => {
            // Calculate the number of ask_asset tokens to be transferred to the recipient from the Vault contract
            (calc_amount, spread_amount)  = compute_swap(
                deps.storage,
                &env,
                &math_config,
                &offer_asset.to_decimal_asset(offer_precision)?,
                &offer_pool,
                &ask_pool,
                &pools,
            ).unwrap_or_else(|_| (Uint128::zero(), Uint128::zero()));
            // Calculate the commission fees 
            (total_fee, protocol_fee, dev_fee) = config.fee_info.calculate_underlying_fees(calc_amount);

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
            // Calculate the number of offer_asset tokens to be transferred from the trader from the Vault contract
            (calc_amount, spread_amount, total_fee) = compute_offer_amount(
                deps.storage,
                &env,
                &math_config,
                &ask_asset.to_decimal_asset(ask_precision)?,
                &offer_pool,
                &ask_pool,
                &pools,
                config.fee_info.total_fee_bps,
                math_config.greatest_precision
            ).unwrap_or_else(|_| (Uint128::zero(), Uint128::zero(), Uint128::zero()));
                
            // Calculate the protocol and dev fee
            (protocol_fee, dev_fee) = config.fee_info.calculate_total_fee_breakup(total_fee);            
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
            return Ok( return_swap_failure() ) 
        }        
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
    // Load the config  and twap from the storage
    let twap: Twap = TWAPINFO.load(deps.storage)?;
    let config: Config = CONFIG.load(deps.storage)?;

    let total_share = query_supply(&deps.querier, config.lp_token_addr.unwrap().clone())?;

    // Convert Asset to DecimalAsset types
    let decimal_assets = config.assets.iter()
                                                .cloned()
                                                .map(|asset| {
                                                    let precision = get_precision(deps.storage, &asset.info)?;
                                                    asset.to_decimal_asset(precision)
                                                })
                                                .collect::<StdResult<Vec<DecimalAsset>>>()?;
    
    // Accumulate prices of all assets in the config
    accumulate_prices(deps, env, &mut config, math_config, &decimal_assets)
        .map_err(|err| StdError::generic_err(format!("{err}")))?;

    // Find the `cumulative_price` for the provided offer and ask asset in the stored TWAP. Error if not found
    let (offer_asset_, ask_asset_, rate) = twap.cumulative_prices.iter()
                                                .find_position(|cumulative_price| cumulative_price[0].eq(offer_asset_) && cumulative_price[1].eq(ask_asset_))
                                                .ok_or(ContractError::AssetMismatch {})?;

    // Return the cumulative price response
    let resp = CumulativePriceResponse {
        exchange_info: AssetExchangeRate {
            offer_info: offer_asset_,
            ask_info: ask_asset_,
            rate,
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


/// ## Description
/// Imbalanced withdraw liquidity from the pool. Returns a [`ContractError`] on failure,
/// otherwise returns the number of LP tokens to burn.
/// ## Params
/// * **deps** is an object of type [`Deps`].
/// * **env** is an object of type [`Env`].
/// * **config** is an object of type [`Config`].
/// * **provided_amount** is an object of type [`Uint128`]. This is the amount of provided LP tokens to withdraw liquidity with.
/// * **assets** is array with objects of type [`Asset`]. It specifies the assets amount to withdraw.
fn imbalanced_withdraw(
    deps: Deps,
    env: &Env,
    config: &Config,
    math_config: &MathConfig,
    provided_amount: Uint128,
    assets: &[Asset],
) -> Result<Uint128, ContractError> {
    check_assets(deps.api, assets)?;

    if assets.len() > config.assets.len() {
        return Err(ContractError::InvalidNumberOfAssets {});
    }

    let pools: HashMap<_, _> = config
        .assets
        .into_iter()
        .map(|pool| (pool.info, pool.amount))
        .collect();

    let mut assets_collection = assets
        .iter()
        .cloned()
        .map(|asset| {
            // Get appropriate pool
            let mut pool = pools
                .get(&asset.info)
                .copied()
                .ok_or_else(|| ContractError::InvalidAsset(asset.info.to_string()))?;

            // Adjusting to the greatest precision
            let coin_precision = get_precision(deps.storage, &asset.info)?;
            pool = adjust_precision(pool, coin_precision, math_config.greatest_precision)?;

            Ok((asset, pool))
        })
        .collect::<Result<Vec<_>, ContractError>>()?;

    // If some assets are omitted then add them explicitly with 0 withdraw amount
    pools.into_iter().for_each(|(pool_info, pool_amount)| {
        if !assets.iter().any(|asset| asset.info == pool_info) {
            assets_collection.push((
                Asset {
                    amount: Uint128::zero(),
                    info: pool_info,
                },
                pool_amount,
            ));
        }
    });

    let n_coins = config.assets.asset_infos.len() as u8;

    let amp = compute_current_amp(config, env)?.checked_mul(n_coins.into())?;

    // Initial invariant (D)
    let old_balances = assets_collection
        .iter()
        .map(|(_, pool)| *pool)
        .collect_vec();
    let init_d = compute_d(amp, &old_balances)?;

    // Invariant (D) after assets withdrawn
    let mut new_balances = assets_collection
        .iter()
        .map(|(withdraw, pool)| Ok(pool.checked_sub(withdraw.amount)?))
        .collect::<StdResult<Vec<_>>>()?;
    let withdraw_d = compute_d(amp, &new_balances, math_config.greatest_precision)?;

    // total_fee_bps * N_COINS / (4 * (N_COINS - 1))
    let fee = config.fee_info
        .total_fee_bps
        .checked_mul(Decimal::from_ratio(n_coins, 4 * (n_coins - 1)))?;

    for i in 0..n_coins as usize {
        let ideal_balance = withdraw_d.checked_multiply_ratio(old_balances[i], init_d)?;
        let difference = if ideal_balance > new_balances[i] {
            ideal_balance - new_balances[i]
        } else {
            new_balances[i] - ideal_balance
        };
        new_balances[i] = new_balances[i].checked_sub(fee.checked_mul_uint128(difference)?)?;
    }

    let after_fee_d = compute_d(amp, &new_balances, math_config.greatest_precision)?;

    let total_share = query_supply(&deps.querier, &config.lp_token_addr.unwrap())?;
    // How many tokens do we need to burn to withdraw asked assets?
    let burn_amount = total_share
        .checked_multiply_ratio(init_d - after_fee_d, init_d)?
        .checked_add(Uint128::from(1u8))?; // In case of rounding errors - make it unfavorable for the "attacker"

    let burn_amount = adjust_precision(burn_amount, math_config.greatest_precision, LP_TOKEN_PRECISION)?;

    if burn_amount > provided_amount {
        return Err(StdError::generic_err(format!(
            "Not enough LP tokens. You need {} LP tokens.",
            burn_amount
        ))
        .into());
    }

    Ok(burn_amount)
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
        let time_range =
            Uint128::from(math_config.next_amp_time).checked_sub(Uint128::from(math_config.init_amp_time))?;

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