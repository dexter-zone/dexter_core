use crate::asset::{addr_validate_to_lower, Asset, AssetInfo, DecimalAsset};
use crate::error::ContractError;
use crate::vault::FEE_PRECISION;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    attr, to_binary, Addr, BankMsg, Coin, CosmosMsg, Decimal, Decimal256, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw20_base::msg::ExecuteMsg as CW20ExecuteMsg;
use cw_storage_plus::Item;
use itertools::Itertools;

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x       Ownership Update helper functions          x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// ## Description
/// Describes the basic settings for creating a request for a change of ownership.
#[cw_serde]
pub struct OwnershipProposal {
    /// a new ownership.
    pub owner: Addr,
    /// time to live a request
    pub ttl: u64,
}

/// ## Description - Creates a new request to change ownership. Only owner can execute it.
/// `new_owner` is a new owner.
/// `expires_in` is the validity period of the offer to change the owner.
/// `owner` is the current owner.
/// `proposal` is the object of type [`OwnershipProposal`].
pub fn propose_new_owner(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    new_owner: String,
    expires_in: u64,
    owner: Addr,
    proposal: Item<OwnershipProposal>,
) -> StdResult<Response> {
    if info.sender != owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    let new_owner = addr_validate_to_lower(deps.api, new_owner.as_str())?;

    // check that owner is not the same
    if new_owner == owner {
        return Err(StdError::generic_err("New owner cannot be same"));
    }

    proposal.save(
        deps.storage,
        &OwnershipProposal {
            owner: new_owner.clone(),
            ttl: env.block.time.seconds() + expires_in,
        },
    )?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "propose_new_owner"),
        attr("new_owner", new_owner),
    ]))
}

/// ## Description - Removes a request to change ownership. Only owner can execute it
/// `owner` is the current owner.
/// `proposal` is the object of type [`OwnershipProposal`].
pub fn drop_ownership_proposal(
    deps: DepsMut,
    info: MessageInfo,
    owner: Addr,
    proposal: Item<OwnershipProposal>,
) -> StdResult<Response> {
    if info.sender != owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    proposal.remove(deps.storage);
    Ok(Response::new().add_attributes(vec![attr("action", "drop_ownership_proposal")]))
}

/// ## Description
/// New owner claims ownership. Only new proposed owner can execute it
/// `proposal` is the object of type [`OwnershipProposal`].
/// `callback` is a type of callback function that takes two parameters of type [`DepsMut`] and [`Addr`].
pub fn claim_ownership(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    ownership_proposal: Item<OwnershipProposal>,
    callback: fn(DepsMut, Addr) -> StdResult<()>,
) -> StdResult<Response> {
    let proposal: OwnershipProposal = ownership_proposal
        .load(deps.storage)
        .map_err(|_| StdError::generic_err("Ownership proposal not found"))?;

    if info.sender != proposal.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    if env.block.time.seconds() > proposal.ttl {
        return Err(StdError::generic_err("Ownership proposal expired"));
    }

    ownership_proposal.remove(deps.storage);
    callback(deps, proposal.owner.clone())?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "claim_ownership"),
        attr("new_owner", proposal.owner),
    ]))
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x        Transfer tokens helper functions          x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// @dev Helper function which returns a cosmos wasm msg to transfer cw20 tokens to a recipient address
/// @param recipient : Address to be transferred cw20 tokens to
/// @param token_contract_address : Contract address of the cw20 token to transfer
/// @param amount : Number of tokens to transfer
pub fn build_transfer_cw20_token_msg(
    recipient: Addr,
    token_contract_address: String,
    amount: Uint128,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_contract_address,
        msg: to_binary(&CW20ExecuteMsg::Transfer {
            recipient: recipient.into(),
            amount,
        })?,
        funds: vec![],
    }))
}

/// @dev Helper function which returns a cosmos wasm msg to send native tokens to recipient
/// @param recipient : Contract Address to be transferred native tokens to
/// @param denom : Native token to transfer
/// @param amount : Number of tokens to transfer
pub fn build_send_native_asset_msg(
    recipient: Addr,
    denom: &str,
    amount: Uint128,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Bank(BankMsg::Send {
        to_address: recipient.into(),
        amount: vec![Coin {
            denom: denom.to_string(),
            amount,
        }],
    }))
}

/// Helper Function. Returns CosmosMsg which transfers CW20 Tokens from owner to recipient. (Transfers DEX from user to itself )
pub fn build_transfer_cw20_from_user_msg(
    cw20_token_address: String,
    owner: String,
    recipient: String,
    amount: Uint128,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cw20_token_address,
        funds: vec![],
        msg: to_binary(&cw20::Cw20ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        })?,
    }))
}

/// Helper Function. Returns CosmosMsg which transfers CW20 Tokens from owner to recipient. (Transfers DEX from user to itself )
pub fn build_transfer_token_to_user_msg(
    asset: AssetInfo,
    recipient: Addr,
    amount: Uint128,
) -> StdResult<CosmosMsg> {
    match asset {
        AssetInfo::NativeToken { denom } => {
            Ok(build_send_native_asset_msg(recipient, &denom, amount)?)
        }
        AssetInfo::Token { contract_addr } => Ok(build_transfer_cw20_token_msg(
            recipient,
            contract_addr.to_string(),
            amount,
        )?),
    }
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x        Pools / Swap :  Helper functions          x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// ## Description
/// Select offer and ask pools based on given offer and ask infos.
/// This function works with pools with up to 5 assets. Returns (offer_pool, ask_pool) in case of success.
/// If it is impossible to define offer and ask pools, returns [`ContractError`].
/// ## Params
/// * **offer_asset_info** - asset info of the offer asset.
/// * **ask_asset_info** - asset info of the ask asset.
/// * **pools** - list of pools.
pub fn select_pools(
    offer_asset_info: Option<&AssetInfo>,
    ask_asset_info: Option<&AssetInfo>,
    pools: &[DecimalAsset],
) -> Result<(DecimalAsset, DecimalAsset), ContractError> {
    // if pool is only contains 2 assets
    if pools.len() == 2 {
        match (offer_asset_info, ask_asset_info) {
            (Some(offer_asset_info), _) => {
                let (offer_ind, offer_pool) = pools
                    .iter()
                    .find_position(|pool| pool.info.eq(offer_asset_info))
                    .ok_or(ContractError::AssetMismatch {})?;
                Ok((offer_pool.clone(), pools[(offer_ind + 1) % 2].clone()))
            }
            (_, Some(ask_asset_info)) => {
                let (ask_ind, ask_pool) = pools
                    .iter()
                    .find_position(|pool| pool.info.eq(ask_asset_info))
                    .ok_or(ContractError::AssetMismatch {})?;
                Ok((pools[(ask_ind + 1) % 2].clone(), ask_pool.clone()))
            }
            _ => Err(ContractError::VariableAssetMissed {}), // Should always be unreachable
        }
    } else if let (Some(offer_asset_info), Some(ask_asset_info)) =
        (offer_asset_info, ask_asset_info)
    {
        // Error if same assets
        if ask_asset_info.eq(offer_asset_info) {
            return Err(ContractError::SameAssets {});
        }
        // Find offer and ask pools
        let offer_pool = pools
            .iter()
            .find(|pool| pool.info.eq(offer_asset_info))
            .ok_or(ContractError::AssetMismatch {})?;
        let ask_pool = pools
            .iter()
            .find(|pool| pool.info.eq(ask_asset_info))
            .ok_or(ContractError::AssetMismatch {})?;

        Ok((offer_pool.clone(), ask_pool.clone()))
    } else {
        Err(ContractError::VariableAssetMissed {}) // Should always be unreachable
    }
}

/// Checks swap parameters. Otherwise returns [`Err`]
/// ## Params
/// * **offer_amount** is a [`Uint128`] representing an amount of offer tokens.
/// * **ask_amount** is a [`Uint128`] representing an amount of ask tokens.
/// * **swap_amount** is a [`Uint128`] representing an amount to swap.
pub fn check_swap_parameters(
    offer_amount: Uint128,
    ask_amount: Uint128,
    swap_amount: Uint128,
) -> StdResult<()> {
    if offer_amount.is_zero() || ask_amount.is_zero() {
        return Err(StdError::generic_err("One of the pools is empty"));
    }

    if swap_amount.is_zero() {
        return Err(StdError::generic_err("Swap amount must not be zero"));
    }

    Ok(())
}

/// ## Description
/// Returns the share of assets.
/// ## Params
/// * **pools** are an array of [`Asset`] type items.
/// * **amount** is the object of type [`Uint128`].
/// * **total_share** is the object of type [`Uint128`].
pub fn get_share_in_assets(pools: Vec<Asset>, amount: Uint128, total_share: Uint128) -> Vec<Asset> {
    let mut share_ratio = Decimal::zero();
    if !total_share.is_zero() {
        share_ratio = Decimal::from_ratio(amount, total_share);
    }
    pools
        .iter()
        .map(|a| Asset {
            info: a.info.clone(),
            amount: a.amount * share_ratio,
        })
        .collect()
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x        Generic Math :: Helper functions          x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

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
/// Return a value using a newly specified precision.
/// ## Params
/// * **value** is an object of type [`Uint128`]. This is the value that will have its precision adjusted.
/// * **current_precision** is an object of type [`u8`]. This is the `value`'s current precision
/// * **new_precision** is an object of type [`u8`]. This is the new precision to use when returning the `value`.
pub fn adjust_precision(
    value: Uint128,
    current_precision: u8,
    new_precision: u8,
) -> StdResult<Uint128> {
    Ok(match current_precision.cmp(&new_precision) {
        std::cmp::Ordering::Equal => value,
        std::cmp::Ordering::Less => value.checked_mul(Uint128::new(
            10_u128.pow((new_precision - current_precision) as u32),
        ))?,
        std::cmp::Ordering::Greater => value.checked_div(Uint128::new(
            10_u128.pow((current_precision - new_precision) as u32),
        ))?,
    })
}

/// Returns LP token name to be set for a new LP token being initialized
///
/// ## Params
/// * **pool_id** is an object of type [`Uint128`] and is the ID of the pool being created
/// * **lp_token_name** is an object of type Option[`String`], provided as an input by the user creating the pool
pub fn get_lp_token_name(pool_id: Uint128, lp_token_name: Option<String>) -> String {
    let mut token_name = pool_id.to_string() + "-Dex-LP".to_string().as_str();
    if !lp_token_name.is_none() {
        token_name = pool_id.to_string() + "-" + lp_token_name.unwrap().as_str();
    }
    return token_name;
}

/// Returns LP token symbol to be set for a new LP token being initialized
///
/// ## Params
/// * **pool_id** is an object of type [`Uint128`] and is the ID of the pool being created
/// * **lp_token_symbol** is an object of type Option[`String`], provided as an input by the user creating the pool
pub fn get_lp_token_symbol(lp_token_symbol: Option<String>) -> String {
    // numbers in symbol not supported
    let mut token_symbol = "DEX-LP".to_string();
    if !lp_token_symbol.is_none() {
        token_symbol = "LP_".to_string() + lp_token_symbol.unwrap().as_str();
    }
    return token_symbol;
}

pub fn is_valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.len() < 3 || bytes.len() > 50 {
        return false;
    }
    true
}

pub fn is_valid_symbol(symbol: &str) -> bool {
    let bytes = symbol.as_bytes();
    if bytes.len() < 3 || bytes.len() > 12 {
        return false;
    }
    for byte in bytes.iter() {
        if (*byte != 45) && (*byte < 65 || *byte > 90) && (*byte < 97 || *byte > 122) {
            return false;
        }
    }
    true
}

/// Retusn the number of native tokens sent by the user
/// ## Params
/// * **message_info** is an object of type [`MessageInfo`]
pub fn find_sent_native_token_balance(message_info: &MessageInfo, denom: &str) -> Uint128 {
    let empty_coin = Coin::new(0u128, denom);
    let coin = message_info
        .funds
        .iter()
        .find(|x| x.clone().denom == denom)
        .unwrap_or_else(|| &empty_coin);
    coin.amount
}

// Returns the number of tokens charged as total fee
pub fn calculate_underlying_fees(amount: Uint128, total_fee_bps: u16) -> Uint128 {
    amount * Decimal::from_ratio(total_fee_bps, FEE_PRECISION)
}
