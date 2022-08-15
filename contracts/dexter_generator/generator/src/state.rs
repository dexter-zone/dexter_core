use dexter::asset::{addr_validate_to_lower, AssetInfo};
use dexter::helper::OwnershipProposal;
use dexter::{
    generator::{PoolInfo, RestrictedVector, UserInfo, ExecuteOnReply, Config},
    DecimalCheckedOps,
};

use cosmwasm_std::{Addr, DepsMut, Decimal, StdResult, Storage, Uint128, Uint64};
use cw_storage_plus::{Item, Map};
use std::collections::HashMap;

// ----------------x----------------x--------------x----------------
// ----------------x     :: CONTRACT STORAGE ::    x------
// ----------------x----------------x--------------x----------------


/// Stores the contract config at the given key
pub const CONFIG: Item<Config> = Item::new("config");

/// This is a map that contains information about all generators.
/// The key is the address of a LP token, the value is an object of type [`PoolInfo`].
pub const POOL_INFO: Map<&Addr, PoolInfo> = Map::new("pool_info");

/// This is a map that contains information about all stakers.
/// The key is a concatenation of user address and LP token address, the value is an object of type [`UserInfo`].
pub const USER_INFO: Map<(&Addr, &Addr), UserInfo> = Map::new("user_info");

/// The key-value here maps proxy contract addresses to the associated reward assets
pub const PROXY_REWARD_ASSET: Map<&Addr, AssetInfo> = Map::new("proxy_reward_asset");

/// The item here stores the proposal to change contract ownership.
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");

/// The item used during chained Msg calls to keep store of which msg is to be called                  
pub const TMP_USER_ACTION: Item<Option<ExecuteOnReply>> = Item::new("tmp_user_action");


// ----------------x----------------x----------------x----------------
// ----------------x         State update fns        x----------------
// ----------------x----------------x----------------x----------------


/// Update user balance.
/// ## Params
/// * **user** is an object of type [`UserInfo`].
/// * **pool** is an object of type [`PoolInfo`].
/// * **amount** is an object of type [`Uint128`].
pub fn update_user_balance(
    mut user: UserInfo,
    pool: &PoolInfo,
    amount: Uint128,
) -> StdResult<UserInfo> {
    user.amount = amount;

    if !pool.accumulated_rewards_per_share.is_zero() {
        user.reward_debt = pool
            .accumulated_rewards_per_share
            .checked_mul_uint128(user.amount)?;
    };

    user.reward_debt_proxy = pool
        .accumulated_proxy_rewards_per_share
        .inner_ref()
        .iter()
        .map(|(proxy, rewards_per_share)| {
            let rewards_debt = rewards_per_share.checked_mul_uint128(user.amount)?;
            Ok((proxy.clone(), rewards_debt))
        })
        .collect::<StdResult<Vec<_>>>()?
        .into();

    Ok(user)
}

/// ### Description
/// Returns the vector of reward amount per proxy taking into account the amount of debited rewards.
pub fn accumulate_pool_proxy_rewards(
    pool: &PoolInfo,
    user: &UserInfo,
) -> StdResult<Vec<(Addr, Uint128)>> {
    if !pool
        .accumulated_proxy_rewards_per_share
        .inner_ref()
        .is_empty()
    {
        let rewards_debt_map: HashMap<_, _> =
            user.reward_debt_proxy.inner_ref().iter().cloned().collect();
        pool.accumulated_proxy_rewards_per_share
            .inner_ref()
            .iter()
            .map(|(proxy, rewards_per_share)| {
                let reward_debt = rewards_debt_map.get(proxy).cloned().unwrap_or_default();
                let pending_proxy_rewards = rewards_per_share
                    .checked_mul_uint128( user.amount )?
                    .saturating_sub(reward_debt);

                Ok((proxy.clone(), pending_proxy_rewards))
            })
            .collect()
    } else {
        Ok(vec![])
    }
}

/// ### Description
/// Saves map between a proxy and an asset info if it is not saved yet.
pub fn update_proxy_asset(deps: DepsMut, proxy_addr: &Addr) -> StdResult<()> {
    if !PROXY_REWARD_ASSET.has(deps.storage, proxy_addr) {
        let proxy_cfg: dexter::generator_proxy::ConfigResponse = deps
            .querier
            .query_wasm_smart(proxy_addr, &dexter::generator_proxy::QueryMsg::Config {})?;
        let asset = AssetInfo::Token {
            contract_addr: addr_validate_to_lower(deps.api, &proxy_cfg.reward_token_addr)?,
        };
        PROXY_REWARD_ASSET.save(deps.storage, proxy_addr, &asset)?
    }

    Ok(())
}