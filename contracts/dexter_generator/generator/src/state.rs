use dexter::asset::AssetInfo;
use dexter::helper::OwnershipProposal;
use dexter::{
    generator::{Config, ExecuteOnReply, PoolInfo, UserInfo},
    DecimalCheckedOps,
};

use cosmwasm_std::{Addr, DepsMut, StdResult, Uint128};
use cw_storage_plus::{Item, Map};

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

    if !pool.accumulated_proxy_rewards_per_share.is_zero() {
        user.reward_debt_proxy = pool
            .accumulated_proxy_rewards_per_share
            .checked_mul_uint128(user.amount)?;
    }

    Ok(user)
}

/// ### Description
/// Saves map between a proxy and an asset info if it is not saved yet.
pub fn update_proxy_asset(deps: DepsMut, proxy_addr: &Addr) -> StdResult<()> {
    if !PROXY_REWARD_ASSET.has(deps.storage, proxy_addr) {
        let proxy_cfg: dexter::generator_proxy::ConfigResponse = deps
            .querier
            .query_wasm_smart(proxy_addr, &dexter::generator_proxy::QueryMsg::Config {})?;
        let asset = proxy_cfg.reward_token;
        PROXY_REWARD_ASSET.save(deps.storage, proxy_addr, &asset)?
    }

    Ok(())
}
