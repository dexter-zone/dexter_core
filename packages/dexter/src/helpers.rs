use crate::asset::{addr_validate_to_lower, AssetInfo};
use cosmwasm_std::{
    attr, to_binary, Addr, BankMsg, Coin, CosmosMsg, Decimal, Decimal256, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw20_base::msg::{ExecuteMsg as CW20ExecuteMsg, QueryMsg as Cw20QueryMsg};
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// ## Description
/// Describes the basic settings for creating a request for a change of ownership.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OwnershipProposal {
    /// a new ownership.
    pub owner: Addr,
    /// time to live a request
    pub ttl: u64,
}

/// @dev Helper function which returns a cosmos wasm msg to transfer cw20 tokens to a recepient address
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

/// @dev Helper function which returns a cosmos wasm msg to send native tokens to recepient
/// @param recipient : Contract Address to be transferred native tokens to
/// @param denom : Native token to transfer
/// @param amount : Number of tokens to transfer
pub fn build_send_native_asset_msg(
    deps: Deps,
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

/// Helper Function. Returns CosmosMsg which transfers CW20 Tokens from owner to recepient. (Transfers DEX from user to itself )
pub fn build_transfer_cw20_from_user_msg(
    cw20_token_address: String,
    owner: String,
    recepient: String,
    amount: Uint128,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cw20_token_address,
        funds: vec![],
        msg: to_binary(&cw20::Cw20ExecuteMsg::TransferFrom {
            owner,
            recipient: recepient,
            amount,
        })?,
    }))
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

/// Checks swap parameters. Otherwise returns [`Err`]
/// ## Params
/// * **offer_amount** is a [`Uint128`] representing an amount of offer tokens.
///
/// * **ask_amount** is a [`Uint128`] representing an amount of ask tokens.
///
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

/// Validates an amount of native tokens being sent. Returns [`Ok`] if successful, otherwise returns [`Err`].
/// ## Params
/// * **message_info** is an object of type [`MessageInfo`]
pub fn validate_sent_native_token_balance(
    message_info: &MessageInfo,
    asset_info: AssetInfo,
    amount: Uint128,
) -> StdResult<()> {
    if let AssetInfo::NativeToken { denom } = asset_info {
        match message_info.funds.iter().find(|x| x.denom == *denom) {
            Some(coin) => {
                if amount == coin.amount {
                    Ok(())
                } else {
                    Err(StdError::generic_err(
                        "Native token balance mismatch between the argument and the transferred",
                    ))
                }
            }
            None => {
                if amount.is_zero() {
                    Ok(())
                } else {
                    Err(StdError::generic_err(
                        "Native token balance mismatch between the argument and the transferred",
                    ))
                }
            }
        }
    } else {
        Ok(())
    }
}

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
fn adjust_precision(
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