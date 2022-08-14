use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::asset::{Asset, AssetExchangeRate, AssetInfo};

use crate::vault::{PoolType, SwapType};

use cosmwasm_std::{Addr, Binary, Decimal, StdError, StdResult, Uint128};
use std::fmt::{Display, Formatter, Result};
use crate::helper::{is_valid_name, is_valid_symbol};

/// The default slippage (5%)
pub const DEFAULT_SLIPPAGE: &str = "0.005";

/// The maximum allowed slippage (50%)
pub const MAX_ALLOWED_SLIPPAGE: &str = "0.5";

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x      Gneneric struct Types      x----------------x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// ## Description
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct FeeStructs {  
    pub total_fee_bps: u16 
}



/// ## Description
/// This structure describes the main control config of pair.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// ID of contract which is allowed to create pools of this type
    pub pool_id: Uint128,
    pub lp_token_addr: Option<Addr>,
    /// the vault contract address
    pub vault_addr: Addr,
    /// Assets supported by the pool
    pub assets: Vec<Asset>,
    /// The pools type (provided in a [`PoolType`])
    pub pool_type: PoolType,
    pub fee_info: FeeStructs,
    /// The last time block
    pub block_time_last: u64,
}

/// ## Description
/// This structure describes the basic settings for creating a contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Trade {
    pub amount_in: Uint128,
    pub amount_out: Uint128,
    pub spread: Uint128,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResponseType {
    Success {},
    Failure (String),
}

impl Display for ResponseType {
    fn fmt(&self, fmt: &mut Formatter) -> Result {
        match self {
            ResponseType::Success {} => fmt.write_str("success"),
            ResponseType::Failure (error) => fmt.write_str(format!("error : {}", error).as_str()),
        }
    }
}

impl ResponseType {
    /// Returns true if the ResponseType is success. Otherwise returns false.
    /// ## Params
    /// * **self** is the type of the caller object.
    pub fn is_success(&self) -> bool {
        match self {
            ResponseType::Success {} => true,
            ResponseType::Failure (_)  => false,
        }
    }
}



// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x      Instantiate, Execute Msgs and Queries       x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------


/// ## Description
/// This structure describes the basic settings for creating a contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// Pool ID
    pub pool_id: Uint128,
    /// The pools type (provided in a [`PoolType`])
    pub pool_type: PoolType,
    /// the vault contract address
    pub vault_addr: Addr,
    /// Assets supported by the pool
    pub asset_infos: Vec<AssetInfo>,
    pub fee_info: FeeStructs,
    pub lp_token_code_id: u64,
    pub lp_token_name: Option<String>,
    pub lp_token_symbol: Option<String>,
    /// Optional binary serialised parameters for custom pool types
    pub init_params: Option<Binary>,
}

impl InstantiateMsg {
    pub fn validate(&self) -> StdResult<()> {
        // Check name, symbol for LP Token

        if !self.lp_token_name.clone().is_none()
            && !is_valid_name(self.lp_token_name.as_ref().unwrap())
        {
            return Err(StdError::generic_err(
                "Name is not in the expected format (3-50 UTF-8 bytes)",
            ));
        }
        if !self.lp_token_symbol.is_none()
            && !is_valid_symbol(&self.lp_token_symbol.as_ref().unwrap())
        {
            return Err(StdError::generic_err(
                "Ticker symbol is not in expected format [a-zA-Z\\-]{3,12}",
            ));
        }
        Ok(())
    }
}


/// ## Description
///
/// This structure describes the execute messages of the contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// ## Description
    UpdateConfig {params: Option<Binary>},
    UpdateLiquidity { assets: Vec<Asset> },
}

/// ## Description
/// This structure describes the query messages of the contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    FeeParams {},
    PoolId {},
    OnJoinPool {
        assets_in: Option<Vec<Asset>>,
        mint_amount: Option<Uint128>,
        slippage_tolerance: Option<Decimal>,
    },
    OnExitPool {
        assets_out: Option<Vec<Asset>>,
        burn_amount: Option<Uint128>,
    },
    OnSwap {
        swap_type: SwapType,
        offer_asset: AssetInfo,
        ask_asset: AssetInfo,
        amount: Uint128,
        max_spread: Option<Decimal>,
        belief_price: Option<Decimal>,
    },
    CumulativePrice {
        offer_asset: AssetInfo,
        ask_asset: AssetInfo,
    },
    CumulativePrices {},
}


/// ## Description
/// This structure describes a migration message.
/// We currently take no arguments for migrations.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}



// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x     Response Types       x----------------x----------------x------
// ----------------x----------------x----------------x----------------x----------------x----------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    /// ID of contract which is allowed to create pools of this type
    pub pool_id: Uint128,
    pub lp_token_addr: Option<Addr>,
    /// the vault contract address
    pub vault_addr: Addr,
    /// Assets supported by the pool
    pub assets: Vec<Asset>,
    /// The pools type (provided in a [`PoolType`])
    pub pool_type: PoolType,
    pub fee_info: FeeStructs,
    /// The last time block
    pub block_time_last: u64,
    /// Custom Math Config parameters are reutrned in binary format here
    pub math_params: Option<Binary>,
    pub additional_params: Option<Binary>,
}


/// ## Description
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AfterJoinResponse {
    pub provided_assets: Vec<Asset>,
    pub new_shares: Uint128,
    pub response: ResponseType,
    pub fee: Option<Asset>,
}


/// ## Description
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AfterExitResponse {
    /// Assets which will be transferred to the recipient against tokens being burnt
    pub assets_out: Vec<Asset>,
    /// Number of LP tokens to burn
    pub burn_shares: Uint128,
    /// Operation will be a `Success` or `Failure`
    pub response: ResponseType,
    pub fee: Option<Asset>,
}

/// ## Description
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct FeeResponse {
    /// The total fees (in bps) charged by a pool of this type
    pub total_fee_bps: u16,
}

/// ## Description
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SwapResponse {
    pub trade_params: Trade,
    /// Operation will be a `Success` or `Failure`
    pub response: ResponseType,
    pub fee: Option<Asset>,
}

/// ## Description
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CumulativePriceResponse {
    pub exchange_info: AssetExchangeRate,
    pub total_share: Uint128,
}

/// ## Description
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CumulativePricesResponse {
    pub exchange_infos: Vec<AssetExchangeRate>,
    pub total_share: Uint128,
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x     Helper response functions       x----------------x------------
// ----------------x----------------x----------------x----------------x----------------x----------------


pub fn return_join_failure(error: String) -> AfterJoinResponse {
    AfterJoinResponse { provided_assets: vec![], new_shares: Uint128::zero(), response: ResponseType::Failure (error), fee: None, }
}

pub fn return_exit_failure(error: String) -> AfterExitResponse {
    AfterExitResponse { assets_out: vec![], burn_shares: Uint128::zero(), response: ResponseType::Failure (error), fee: None }
}


pub fn return_swap_failure(error: String) -> SwapResponse {
    SwapResponse {
        trade_params: Trade {
            amount_in: Uint128::zero(),
            amount_out: Uint128::zero(),
            spread: Uint128::zero(),
        },
        response: ResponseType::Failure (error),
        fee: None,
    }
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x     Custom Init Params       x----------------x------------
// ----------------x----------------x----------------x----------------x----------------x----------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct WeightedParams{
    pub weights: Vec<(AssetInfo, u128)>,
    pub exit_fee: Option<Decimal>,
}
