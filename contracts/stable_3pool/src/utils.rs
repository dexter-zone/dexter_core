use std::cmp::Ordering;

use cosmwasm_std::{
    to_binary, wasm_execute, Addr, Api, CosmosMsg, Decimal, Deps, Env, QuerierWrapper, StdResult,
    Storage, Uint128, Uint64,
};
use cw20::Cw20ExecuteMsg;
use itertools::Itertools;

use dexter::asset::{Asset, AssetInfo, AssetInfoExt};
use dexter::pool::TWAP_PRECISION;
use dexter::querier::{query_fee_info, query_vault_config};
use dexter::DecimalCheckedOps;

use crate::error::ContractError;
use crate::math::calc_y;
use crate::state::{get_precision, Config};

/// ## Description
/// Helper function to check if the given asset infos are valid.
pub(crate) fn check_asset_infos(
    api: &dyn Api,
    asset_infos: &[AssetInfo],
) -> Result<(), ContractError> {
    if !asset_infos.iter().all_unique() {
        return Err(ContractError::DoublingAssets {});
    }

    asset_infos
        .iter()
        .try_for_each(|asset_info| asset_info.check(api))
        .map_err(Into::into)
}

/// ## Description
/// Helper function to check that the assets in a given array are valid.
pub(crate) fn check_assets(api: &dyn Api, assets: &[Asset]) -> Result<(), ContractError> {
    let asset_infos = assets.iter().map(|asset| asset.info.clone()).collect_vec();
    check_asset_infos(api, &asset_infos)
}

/// ## Description
/// Checks that cw20 token is part of the pool. Returns [`Ok(())`] in case of success,
/// otherwise [`ContractError`].
/// ## Params
/// * **config** is an object of type [`Config`].
///
/// * **cw20_sender** is cw20 token address which is being checked.
pub(crate) fn check_cw20_in_pool(config: &Config, cw20_sender: &Addr) -> Result<(), ContractError> {
    for asset_info in &config.assets.info {
        match asset_info {
            AssetInfo::Token { contract_addr } if contract_addr == cw20_sender => return Ok(()),
            _ => {}
        }
    }

    Err(ContractError::Unauthorized {})
}


/// ## Description
/// Compute the current pool amplification coefficient (AMP).
/// ## Params
/// * **config** is an object of type [`Config`].
///
/// * **env** is an object of type [`Env`].
pub(crate) fn compute_current_amp(config: &Config, env: &Env) -> StdResult<Uint64> {
    let block_time = env.block.time.seconds();
    if block_time < config.next_amp_time {
        let elapsed_time: Uint128 = block_time.saturating_sub(config.init_amp_time).into();
        let time_range = config
            .next_amp_time
            .saturating_sub(config.init_amp_time)
            .into();
        let init_amp = Uint128::from(config.init_amp);
        let next_amp = Uint128::from(config.next_amp);

        if next_amp > init_amp {
            let amp_range = next_amp - init_amp;
            let res = init_amp + (amp_range * elapsed_time).checked_div(time_range)?;
            Ok(res.try_into()?)
        } else {
            let amp_range = init_amp - next_amp;
            let res = init_amp - (amp_range * elapsed_time).checked_div(time_range)?;
            Ok(res.try_into()?)
        }
    } else {
        Ok(Uint64::from(config.next_amp))
    }
}

/// ## Description
/// Returns a value using a newly specified precision.
/// ## Params
/// * **value** is an object of type [`Uint128`]. This is the value that will have its precision adjusted.
///
/// * **current_precision** is an object of type [`u8`]. This is the `value`'s current precision
///
/// * **new_precision** is an object of type [`u8`]. This is the new precision to use when returning the `value`.
pub(crate) fn adjust_precision(
    value: Uint128,
    current_precision: u8,
    new_precision: u8,
) -> StdResult<Uint128> {
    Ok(match current_precision.cmp(&new_precision) {
        Ordering::Equal => value,
        Ordering::Less => value.checked_mul(Uint128::new(
            10_u128.pow((new_precision - current_precision) as u32),
        ))?,
        Ordering::Greater => value.checked_div(Uint128::new(
            10_u128.pow((current_precision - new_precision) as u32),
        ))?,
    })
}

/// ## Description
/// Return the amount of tokens that a specific amount of LP tokens would withdraw.
/// ## Params
/// * **pools** is an array of [`Asset`] type items. These are the assets available in the pool.
///
/// * **amount** is an object of type [`Uint128`]. This is the amount of LP tokens to calculate underlying amounts for.
///
/// * **total_share** is an object of type [`Uint128`]. This is the total amount of LP tokens currently issued by the pool.
pub(crate) fn get_share_in_assets(
    pools: &[Asset],
    amount: Uint128,
    total_share: Uint128,
) -> Vec<Asset> {
    let mut share_ratio = Decimal::zero();
    if !total_share.is_zero() {
        share_ratio = Decimal::from_ratio(amount, total_share);
    }

    pools
        .iter()
        .map(|pool| Asset {
            info: pool.info.clone(),
            amount: pool.amount * share_ratio,
        })
        .collect()
}

/// Structure for internal use which represents swap result.
pub(crate) struct SwapResult {
    pub return_amount: Uint128,
    pub spread_amount: Uint128,
    pub commission_amount: Uint128,
}

/// ## Description
/// Returns the result of a swap in form of a [`SwapResult`] object. In case of error, returns [`ContractError`].
/// ## Params
/// * **storage** is an object of type [`Storage`].
/// * **env** is an object of type [`Env`].
/// * **config** is an object of type [`Config`].
/// * **offer_asset** is an object of type [`Asset`]. This is the asset that is being offered.
/// * **offer_pool** is an object of type [`Uint128`]. This is the total amount of offer assets in the pool.
/// * **ask_pool** is an object of type [`Uint128`]. This is the total amount of ask assets in the pool.
/// * **pools** is an array of [`Asset`] type items. These are the assets available in the pool.
pub(crate) fn compute_swap(
    storage: &dyn Storage,
    env: &Env,
    config: &Config,
    offer_asset: &Asset,
    offer_pool: &Asset,
    ask_pool: &Asset,
    pools: &[Asset],
) -> Result<SwapResult, ContractError> {
    let token_precision = get_precision(storage, &offer_asset.info)?;
    let offer_amount = adjust_precision(
        offer_asset.amount,
        token_precision,
        config.greatest_precision,
    )?;

    let new_ask_pool = calc_y(
        &offer_asset.info,
        &ask_pool.info,
        offer_pool.amount.checked_add(offer_amount)?,
        pools,
        compute_current_amp(config, env)?,
    )?;

    let token_precision = get_precision(storage, &ask_pool.info)?;
    let new_ask_pool = adjust_precision(new_ask_pool, config.greatest_precision, token_precision)?;
    let mut return_amount = ask_pool.amount.checked_sub(new_ask_pool)?;

    let commission_amount = config.fee_info.total_fee_bps.checked_mul_uint128(return_amount)?;
    return_amount = return_amount.saturating_sub(commission_amount);

    // We consider swap rate 1:1 in stable swap thus any difference is considered as spread.
    let spread_amount = offer_asset.amount.saturating_sub(return_amount);

    Ok(SwapResult {
        return_amount,
        spread_amount,
        commission_amount
    })
}

/// ## Description
/// Accumulate token prices for the assets in the pool.
/// ## Params
/// * **deps** is an object of type [`Deps`].
///
/// * **env** is an object of type [`Env`].
///
/// * **config** is an object of type [`Config`].
///
/// * **pools** is an array of [`Asset`] type items. These are the assets available in the pool.
pub fn accumulate_prices(
    deps: Deps,
    env: Env,
    config: &mut Config,
    pools: &[Asset],
) -> Result<(), ContractError> {
    let block_time = env.block.time.seconds();
    if block_time <= config.block_time_last {
        return Ok(());
    }

    let greater_precision = config.greatest_precision.max(TWAP_PRECISION);

    let time_elapsed = Uint128::from(block_time - config.block_time_last);

    let immut_config = config.clone();
    for (from, to, value) in config.cumulative_prices.iter_mut() {
        let offer_asset = from.with_balance(adjust_precision(
            Uint128::from(1u8),
            0u8,
            greater_precision,
        )?);

        let (offer_pool, ask_pool) = select_pools(Some(from), Some(to), pools)?;
        let SwapResult { return_amount, .. } = compute_swap(
            deps.storage,
            &env,
            &immut_config,
            &offer_asset,
            &offer_pool,
            &ask_pool,
            pools,
        )?;

        *value = value.wrapping_add(time_elapsed.checked_mul(return_amount)?);
    }

    config.block_time_last = block_time;

    Ok(())
}
