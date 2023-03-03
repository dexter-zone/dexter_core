use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    to_binary, Addr, Api, BankMsg, Coin, ConversionOverflowError, CosmosMsg, Decimal256, Fraction,
    MessageInfo, QuerierWrapper, StdError, StdResult, Uint128, Uint256, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use std::fmt;

use crate::querier::{query_balance, query_token_balance};

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    {{AssetInfo}} struct Type    x----------------x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// This enum describes available Token types.
#[cw_serde]
#[derive(Hash, Eq)]
pub enum AssetInfo {
    /// Non-native Token
    Token { contract_addr: Addr },
    /// Native token
    NativeToken { denom: String },
}

impl PartialOrd for AssetInfo {
    
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.to_string().to_lowercase().cmp(&other.to_string().to_lowercase()))
    }
}

impl Ord for AssetInfo {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_string().to_lowercase().cmp(&other.to_string().to_lowercase())
    }
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

    pub fn native_token(denom: String) -> Self {
        AssetInfo::NativeToken { denom }
    }

    pub fn token(contract_addr: Addr) -> Self {
        AssetInfo::Token { contract_addr }
    }


    pub fn as_string(&self) -> String {
        match self {
            AssetInfo::NativeToken { denom } => denom.to_string(),
            AssetInfo::Token { contract_addr } => contract_addr.to_string().to_lowercase(),
        }
    }

    /// Returns true if the caller is a native token. Otherwise returns false.
    pub fn is_native_token(&self) -> bool {
        match self {
            AssetInfo::NativeToken { .. } => true,
            AssetInfo::Token { .. } => false,
        }
    }

    /// Returns the balance of token in a pool contract.
    pub fn query_for_balance(&self, querier: &QuerierWrapper, addr: Addr) -> StdResult<Uint128> {
        match self {
            AssetInfo::Token { contract_addr, .. } => {
                query_token_balance(querier, contract_addr.clone(), addr)
            }
            AssetInfo::NativeToken { denom, .. } => query_balance(querier, addr, denom.to_string()),
        }
    }

    /// Returns True if the calling token is the same as the token specified in the input parameters.  Otherwise returns False.
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
                api.addr_validate(contract_addr.as_str())?;
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

    /// Returns a message of type [`CosmosMsg`].
    /// For native tokens of type [`AssetInfo`] uses the default method [`BankMsg::Send`] to send a token amount to a recipient.
    /// For a token of type [`AssetInfo`] we use the default method [`Cw20ExecuteMsg::Transfer`]
    ///
    /// ## Params
    /// * **recipient** is the address where the funds will be sent.
    pub fn create_transfer_msg(&self, recipient: Addr, amount: Uint128) -> StdResult<CosmosMsg> {
        match &self {
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

    /// Returns the number of decimals that a token has.
    /// ## Params
    /// * **querier** is an object of type [`QuerierWrapper`].
    pub fn decimals(&self, native_asset_input_precisions: &Vec<(String, u8)>, querier: &QuerierWrapper) -> StdResult<u8> {
        let decimals = match &self {
            AssetInfo::NativeToken { denom } => {
                let precision = native_asset_input_precisions
                    .iter()
                    .find(|(asset_denom, _)| asset_denom == denom)
                    .unwrap()
                    .1;
                precision
            },
            AssetInfo::Token { contract_addr } => {
                let res: cw20::TokenInfoResponse =
                    querier.query_wasm_smart(contract_addr, &cw20::Cw20QueryMsg::TokenInfo {})?;

                res.decimals
            }
        };

        Ok(decimals)
    }

    pub fn denom(&self) -> StdResult<String> {
        match &self {
            AssetInfo::NativeToken { denom } => {
                Ok(denom.to_string())
            },
            AssetInfo::Token { contract_addr: _ } => {
                Err(StdError::generic_err("Not a native token"))
            }
        }
    }

    pub fn contract_addr(&self) -> StdResult<String> {
        match &self {
            AssetInfo::NativeToken { denom: _ } => {
                Err(StdError::generic_err("Not a CW20 token"))
            },
            AssetInfo::Token { contract_addr } => {
                Ok(contract_addr.to_string())
            }
        }
    }
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x     {{Asset}} struct Type       x----------------x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// ## Description - This enum describes a asset (native or CW20).
#[cw_serde]
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

    pub fn new(info: AssetInfo, amount: Uint128) -> Self {
        Self { info, amount }
    }

    pub fn new_native(denom: String, amount: Uint128) -> Self {
        Self::new(AssetInfo::native_token(denom), amount)
    }

    pub fn new_token(contract_addr: Addr, amount: Uint128) -> Self {
        Self::new(AssetInfo::token(contract_addr), amount)
    }


    /// Returns true if the token is native. Otherwise returns false.
    pub fn is_native_token(&self) -> bool {
        self.info.is_native_token()
    }

    /// Returns a message of type [`CosmosMsg`].
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

    pub fn to_decimal_asset(&self, precision: impl Into<u32>) -> StdResult<DecimalAsset> {
        Ok(DecimalAsset {
            info: self.info.clone(),
            amount: Decimal256::with_precision(self.amount, precision.into())?,
        })
    }

    pub fn to_scaled_decimal_asset(&self, precision: impl Into<u32>, scaling_factor: Decimal256) -> StdResult<DecimalAsset> {
        self.to_decimal_asset(precision)?.with_scaling_factor(scaling_factor)
    }
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    {{AssetExchangeRate}} struct Type    x----------------x--------
// ----------------x----------------x----------------x----------------x----------------x----------------

#[cw_serde]
pub struct AssetExchangeRate {
    pub offer_info: AssetInfo,
    pub ask_info: AssetInfo,
    pub rate: Uint128,
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x {{DecimalAsset}} struct Type    x----------------x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// ## Description
/// This struct describes a Terra asset as decimal.
#[cw_serde]
pub struct DecimalAsset {
    pub info: AssetInfo,
    pub amount: Decimal256,
}

impl DecimalAsset {

    pub fn with_scaling_factor(&self, scaling_factor: Decimal256) -> StdResult<DecimalAsset> {
        let amount = self.amount.with_scaling_factor(scaling_factor)?;
        Ok(DecimalAsset {
            info: self.info.clone(),
            amount,
        })
    }

    pub fn without_scaling_factor(&self, scaling_factor: Decimal256) -> StdResult<DecimalAsset> {
        let amount = self.amount.without_scaling_factor(scaling_factor)?;
        Ok(DecimalAsset {
            info: self.info.clone(),
            amount,
        })
    }
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x {{Decimal256Ext}} trait Type   x----------------x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

pub trait Decimal256Ext {
    fn to_uint256(&self) -> Uint256;

    fn to_uint128_with_precision(&self, precision: impl Into<u32>) -> StdResult<Uint128>;

    fn to_uint256_with_precision(&self, precision: impl Into<u32>) -> StdResult<Uint256>;

    fn from_integer(i: impl Into<Uint256>) -> Self;

    fn checked_multiply_ratio(
        &self,
        numerator: Decimal256,
        denominator: Decimal256,
    ) -> StdResult<Decimal256>;

    fn with_precision(
        value: impl Into<Uint256>,
        precision: impl Into<u32>,
    ) -> StdResult<Decimal256>;

    fn with_scaling_factor(
        &self,
        scaling_factor: Decimal256,
    ) -> StdResult<Decimal256>;

    fn without_scaling_factor(
        &self,
        scaling_factor: Decimal256,
    ) -> StdResult<Decimal256>;

    fn saturating_sub(self, other: Decimal256) -> Decimal256;
}

impl Decimal256Ext for Decimal256 {
    fn to_uint256(&self) -> Uint256 {
        self.numerator() / self.denominator()
    }

    fn to_uint128_with_precision(&self, precision: impl Into<u32>) -> StdResult<Uint128> {
        let value = self.atomics();
        let precision = precision.into();

        value
            .checked_div(10u128.pow(self.decimal_places() - precision).into())?
            .try_into()
            .map_err(|o: ConversionOverflowError| {
                StdError::generic_err(format!("Error converting {}", o.value))
            })
    }

    fn to_uint256_with_precision(&self, precision: impl Into<u32>) -> StdResult<Uint256> {
        let value = self.atomics();
        let precision = precision.into();

        value
            .checked_div(10u128.pow(self.decimal_places() - precision).into())
            .map_err(|_| StdError::generic_err("DivideByZeroError"))
    }

    fn from_integer(i: impl Into<Uint256>) -> Self {
        Decimal256::from_ratio(i.into(), 1u8)
    }

    fn checked_multiply_ratio(
        &self,
        numerator: Decimal256,
        denominator: Decimal256,
    ) -> StdResult<Decimal256> {
        let numerator = numerator.atomics();
        let denominator = denominator.atomics();

        self.checked_mul(
            Decimal256::checked_from_ratio(numerator, denominator)
                .map_err(|_| StdError::generic_err("CheckedFromRatioError"))?,
        )
        .map_err(|_| StdError::generic_err("OverflowError"))
    }

    fn with_precision(
        value: impl Into<Uint256>,
        precision: impl Into<u32>,
    ) -> StdResult<Decimal256> {
        Decimal256::from_atomics(value, precision.into())
            .map_err(|_| StdError::generic_err("Decimal256 range exceeded"))
    }

    fn saturating_sub(self, other: Decimal256) -> Decimal256 {
        Decimal256::new(self.atomics().saturating_sub(other.atomics()))
    }

    #[inline]
    fn with_scaling_factor(
        &self,
        scaling_factor: Decimal256,
    ) -> StdResult<Decimal256> {
        // Divide by scaling factor
        let amount = self.checked_div(scaling_factor)
            .map_err(|e| StdError::generic_err(format!("Error while scaling decimal asset: {}", e)))?;

        Ok(amount)
    }

    #[inline(always)]
    fn without_scaling_factor(
        &self,
        scaling_factor: Decimal256,
    ) -> StdResult<Decimal256> {            
        Ok(self.checked_mul(scaling_factor)?)
    }
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x      Some Helper functions      x----------------x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// Returns a lowercased, validated address upon success if present. Otherwise returns [`None`].
/// ## Params
/// * **addr** is an object of type [`Addr`]
pub fn addr_opt_validate(api: &dyn Api, addr: &Option<String>) -> StdResult<Option<Addr>> {
    addr.as_ref()
        .map(|addr| api.addr_validate(addr))
        .transpose()
}

// const TOKEN_SYMBOL_MAX_LENGTH: usize = 4;

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
