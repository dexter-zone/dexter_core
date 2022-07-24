use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::pool::QueryMsg as PoolQueryMsg;
use crate::querier::{query_balance, query_token_balance, query_token_symbol};
use crate::vault::PoolType;
use cosmwasm_std::{
    to_binary, Addr, Api, BankMsg, Coin, CosmosMsg, Deps, MessageInfo, QuerierWrapper, StdError,
    StdResult, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse};

/// ## Description - This enum describes a asset (native or CW20).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Asset {
    /// Information about an asset stored in a [`AssetInfo`] struct
    pub info: AssetInfo,
    /// A token amount
    pub amount: Uint128,
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.amount, self.info)
    }
}

impl Asset {
    /// Returns true if the token is native. Otherwise returns false.
    /// ## Params
    /// * **self** is the type of the caller object.
    pub fn is_native_token(&self) -> bool {
        self.info.is_native_token()
    }

    /// Returns a message of type [`CosmosMsg`].
    ///
    /// For native tokens of type [`AssetInfo`] uses the default method [`BankMsg::Send`] to send a token amount to a recipient.
    /// For a token of type [`AssetInfo`] we use the default method [`Cw20ExecuteMsg::Transfer`]
    ///
    /// ## Params
    /// * **recipient** is the address where the funds will be sent.
    pub fn into_msg(self, recipient: Addr) -> StdResult<CosmosMsg> {
        let amount = self.amount;

        match &self.info {
            AssetInfo::Token { contract_addr } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: recipient.to_string(),
                    amount,
                })?,
                funds: vec![],
            })),
            AssetInfo::NativeToken { denom } => Ok(CosmosMsg::Bank(BankMsg::Send {
                to_address: recipient.to_string(),
                amount: vec![Coin {
                    denom: denom.to_string(),
                    amount,
                }],
            })),
        }
    }

    /// Validates an amount of native tokens being sent. Returns [`Ok`] if successful, otherwise returns [`Err`].
    /// ## Params
    /// * **message_info** is an object of type [`MessageInfo`]
    pub fn assert_sent_native_token_balance(&self, message_info: &MessageInfo) -> StdResult<()> {
        if let AssetInfo::NativeToken { denom } = &self.info {
            match message_info.funds.iter().find(|x| x.denom == *denom) {
                Some(coin) => {
                    if self.amount == coin.amount {
                        Ok(())
                    } else {
                        Err(StdError::generic_err("Native token balance mismatch between the argument and the transferred"))
                    }
                }
                None => {
                    if self.amount.is_zero() {
                        Ok(())
                    } else {
                        Err(StdError::generic_err("Native token balance mismatch between the argument and the transferred"))
                    }
                }
            }
        } else {
            Ok(())
        }
    }
}

/// This enum describes available Token types.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfo {
    /// Non-native Token
    Token { contract_addr: Addr },
    /// Native token
    NativeToken { denom: String },
}

impl fmt::Display for AssetInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AssetInfo::NativeToken { denom } => write!(f, "{}", denom),
            AssetInfo::Token { contract_addr } => write!(f, "{}", contract_addr),
        }
    }
}

impl AssetInfo {
    /// Returns true if the caller is a native token. Otherwise returns false.
    /// ## Params
    /// * **self** is the caller object type
    pub fn as_string(&self) -> String {
        match self {
            AssetInfo::NativeToken { denom } => denom.to_string(),
            AssetInfo::Token { contract_addr } => contract_addr.to_string().to_lowercase(),
        }
    }

    /// Returns true if the caller is a native token. Otherwise returns false.
    /// ## Params
    /// * **self** is the caller object type
    pub fn is_native_token(&self) -> bool {
        match self {
            AssetInfo::NativeToken { .. } => true,
            AssetInfo::Token { .. } => false,
        }
    }

    /// Returns the balance of token in a pool.
    /// ## Params
    /// * **self** is the type of the caller object.
    ///
    /// * **pool_addr** is the address of the contract whose token balance we check.
    pub fn query_pool(&self, querier: &QuerierWrapper, pool_addr: Addr) -> StdResult<Uint128> {
        match self {
            AssetInfo::Token { contract_addr, .. } => {
                query_token_balance(querier, contract_addr.clone(), pool_addr)
            }
            AssetInfo::NativeToken { denom, .. } => {
                query_balance(querier, pool_addr, denom.to_string())
            }
        }
    }

    /// Returns True if the calling token is the same as the token specified in the input parameters.  Otherwise returns False.
    /// ## Params
    /// * **asset** is object of type [`AssetInfo`].
    pub fn equal(&self, asset: &AssetInfo) -> bool {
        match self {
            AssetInfo::Token { contract_addr, .. } => {
                let self_contract_addr = contract_addr;
                match asset {
                    AssetInfo::Token { contract_addr, .. } => self_contract_addr == contract_addr,
                    AssetInfo::NativeToken { .. } => false,
                }
            }
            AssetInfo::NativeToken { denom, .. } => {
                let self_denom = denom;
                match asset {
                    AssetInfo::Token { .. } => false,
                    AssetInfo::NativeToken { denom, .. } => self_denom == denom,
                }
            }
        }
    }

    /// If the caller object is a native token of type ['AssetInfo`] then his `denom` field converts to a byte string.
    /// If the caller object is a token of type ['AssetInfo`] then his `contract_addr` field converts to a byte string.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            AssetInfo::NativeToken { denom } => denom.as_bytes(),
            AssetInfo::Token { contract_addr } => contract_addr.as_bytes(),
        }
    }

    /// Returns [`Ok`] if the token of type [`AssetInfo`] is in lowercase and valid. Otherwise returns [`Err`].
    pub fn check(&self, api: &dyn Api) -> StdResult<()> {
        match self {
            AssetInfo::Token { contract_addr } => {
                addr_validate_to_lower(api, contract_addr.as_str())?;
            }
            AssetInfo::NativeToken { denom } => {
                if !denom.starts_with("ibc/") && denom != &denom.to_lowercase() {
                    return Err(StdError::generic_err(format!(
                        "Non-IBC token denom {} should be lowercase",
                        denom
                    )));
                }
            }
        }
        Ok(())
    }

    /// Returns the number of native tokens being sent
    pub fn get_sent_native_token_balance(&self, message_info: &MessageInfo) -> Uint128 {
        if let AssetInfo::NativeToken { denom } = &self {
            match message_info.funds.iter().find(|x| x.denom == *denom) {
                Some(coin) => {
                    return coin.amount;
                }
                None => {
                    return Uint128::zero();
                }
            }
        } else {
            return Uint128::zero();
        }
    }
}

// /// This structure stores the main parameters for a  pair
// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
// pub struct PoolInfo {
//     /// Asset information for the two assets in the pool
//     pub asset_infos: Vec<AssetInfo>,
//     /// Pair contract address
//     pub contract_addr: Addr,
//     /// Pair LP token address
//     pub liquidity_token: Addr,
//     /// The pool type (xyk, stableswap etc) available in [`PoolType`]
//     pub pool_type: PoolType,
// }

// impl PoolInfo {
//     /// Returns the balance for each asset in the pool.
//     /// * **contract_addr** is pair's pool address.
//     pub fn query_pools(
//         &self,
//         querier: &QuerierWrapper,
//         contract_addr: Addr,
//     ) -> StdResult<[Asset; 2]> {
//         Ok([
//             Asset {
//                 amount: self.asset_infos[0].query_pool(querier, contract_addr.clone())?,
//                 info: self.asset_infos[0].clone(),
//             },
//             Asset {
//                 amount: self.asset_infos[1].query_pool(querier, contract_addr)?,
//                 info: self.asset_infos[1].clone(),
//             },
//         ])
//     }
// }

// /// This structure stores the main parameters for a  pair
// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
// pub struct WeightedPoolInfo {
//     /// Asset information for the two assets in the pool
//     pub asset_infos: Vec<AssetInfo>,
//     /// Pair contract address
//     pub contract_addr: Addr,
//     /// Pair LP token address
//     pub liquidity_token: Addr,
//     /// The pool type (xyk, stableswap, weighted etc) available in [`PoolType`]
//     pub pool_type: PoolType,
// }

// impl WeightedPoolInfo {
//     /// Returns the balance for each asset in the pool.
//     /// * **contract_addr** is pair's pool address.
//     pub fn query_pools(
//         &self,
//         querier: &QuerierWrapper,
//         contract_addr: Addr,
//     ) -> StdResult<Vec<Asset>> {
//         let mut response: Vec<Asset> = vec![];

//         for asset in self.asset_infos.clone() {
//             let asset = Asset {
//                 amount: asset.query_pool(querier, contract_addr.clone())?,
//                 info: asset.clone(),
//             };
//             response.push(asset);
//         }
//         Ok(response)
//     }
// }

/// Returns a lowercased, validated address upon success. Otherwise returns [`Err`]
pub fn addr_validate_to_lower(api: &dyn Api, addr: &str) -> StdResult<Addr> {
    if addr.to_lowercase() != addr {
        return Err(StdError::generic_err(format!(
            "Address {} should be lowercase",
            addr
        )));
    }
    api.addr_validate(addr)
}

/// Returns a lowercased, validated address upon success if present. Otherwise returns [`None`].
/// ## Params
/// * **addr** is an object of type [`Addr`]
pub fn addr_opt_validate(api: &dyn Api, addr: &Option<String>) -> StdResult<Option<Addr>> {
    addr.as_ref()
        .map(|addr| addr_validate_to_lower(api, addr))
        .transpose()
}

// const TOKEN_SYMBOL_MAX_LENGTH: usize = 4;

/// Returns a formatted LP token name
// pub fn format_lp_token_name(
//     pool_id: Uint128,
//     asset_infos: Vec<AssetInfo>,
//     querier: &QuerierWrapper,
// ) -> StdResult<String> {
//     let mut short_symbols: Vec<String> = vec![];
//     for asset_info in asset_infos {
//         let short_symbol = match asset_info {
//             AssetInfo::NativeToken { denom } => {
//                 denom.chars().take(TOKEN_SYMBOL_MAX_LENGTH).collect()
//             }
//             AssetInfo::Token { contract_addr } => {
//                 let token_symbol = query_token_symbol(querier, contract_addr)?;
//                 token_symbol.chars().take(TOKEN_SYMBOL_MAX_LENGTH).collect()
//             }
//         };
//         short_symbols.push(short_symbol);
//     }
//     Ok(format!("{}-{}-LP", short_symbols[0], short_symbols[1]).to_uppercase())
// }

/// Returns an [`Asset`] object representing a native token and an amount of tokens.
/// ## Params
/// * **denom** is a [`String`] that represents the native asset denomination.
/// * **amount** is a [`Uint128`] representing an amount of native assets.
pub fn native_asset(denom: String, amount: Uint128) -> Asset {
    Asset {
        info: AssetInfo::NativeToken { denom },
        amount,
    }
}

/// Returns an [`Asset`] object representing a non-native token and an amount of tokens.
/// ## Params
/// * **contract_addr** is a [`Addr`]. It is the address of the token contract.
/// * **amount** is a [`Uint128`] representing an amount of tokens.
pub fn token_asset(contract_addr: Addr, amount: Uint128) -> Asset {
    Asset {
        info: AssetInfo::Token { contract_addr },
        amount,
    }
}

/// Returns an [`AssetInfo`] object representing the denomination for a native asset.
/// ## Params
/// * **denom** is a [`String`] object representing the denomination of the native asset.
pub fn native_asset_info(denom: String) -> AssetInfo {
    AssetInfo::NativeToken { denom }
}

/// Returns an [`AssetInfo`] object representing the address of a token contract.
/// ## Params
/// * **contract_addr** is a [`Addr`] object representing the address of a token contract.
pub fn token_asset_info(contract_addr: Addr) -> AssetInfo {
    AssetInfo::Token { contract_addr }
}

// / Returns [`PoolInfo`] by specified pool address.
// pub fn pair_info_by_pool(deps: Deps, pool: Addr) -> StdResult<PoolInfo> {
//     let minter_info: MinterResponse = deps
//         .querier
//         .query_wasm_smart(pool, &Cw20QueryMsg::Minter {})?;

//     let pair_info: PoolInfo = deps
//         .querier
//         .query_wasm_smart(minter_info.minter, &PoolQueryMsg::Pair {})?;

//     Ok(pair_info)
// }
