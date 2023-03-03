use cosmwasm_schema::{cw_serde, QueryResponses};

use crate::asset::{Asset, AssetExchangeRate, AssetInfo};

use crate::vault::{PoolType, SwapType};
use cosmwasm_std::{Addr, Binary, Decimal, DepsMut, Env, Event, MessageInfo, Response, StdError, StdResult, Uint128};
use std::fmt::{Display, Formatter, Result};
use cw_storage_plus::Item;

/// The default slippage (0.5%)
pub const DEFAULT_SPREAD: &str = "0.005";

/// The maximum allowed slippage (50%)
pub const MAX_SPREAD: &str = "0.5";

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x      Gneneric struct Types      x----------------x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// ## Description
#[cw_serde]
pub struct FeeStructs {
    pub total_fee_bps: u16,
}

impl Display for FeeStructs {
    fn fmt(&self, fmt: &mut Formatter) -> Result {
        fmt.write_str(format!("total_fee_bps : {}", self.total_fee_bps).as_str())
    }
}

/// ## Description
/// This struct describes the main control config of pool.
#[cw_serde]
pub struct Config {
    /// ID of contract which is allowed to create pools of this type
    pub pool_id: Uint128,
    /// The address of the LP token associated with this pool
    pub lp_token_addr: Addr,
    /// the vault contract address
    pub vault_addr: Addr,
    /// Assets supported by the pool
    pub assets: Vec<Asset>,
    /// The pools type (provided in a [`PoolType`])
    pub pool_type: PoolType,
    /// The Fee details of the pool
    pub fee_info: FeeStructs,
    /// The block time when pool liquidity was last updated
    pub block_time_last: u64,
}

/// ## Description
/// This helper struct is used for swap operations in the pool
#[cw_serde]
pub struct Trade {
    /// The number of tokens to be sent by the user to the Vault
    pub amount_in: Uint128,
    /// The number of tokens to be received by the user from the Vault
    pub amount_out: Uint128,
    /// The spread associated with the swap tx
    pub spread: Uint128,
}

/// ## Description
/// This enum is used to describe if the math computations (joins/exits/swaps) will be successful or not
#[cw_serde]
pub enum ResponseType {
    Success {},
    Failure(String),
}

impl Display for ResponseType {
    fn fmt(&self, fmt: &mut Formatter) -> Result {
        match self {
            ResponseType::Success {} => fmt.write_str("success"),
            ResponseType::Failure(error) => fmt.write_str(format!("error : {}", error).as_str()),
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
            ResponseType::Failure(_) => false,
        }
    }
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x      Instantiate, Execute Msgs and Queries       x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// ## Description
/// This struct describes the basic settings for creating a contract.
#[cw_serde]
pub struct InstantiateMsg {
    /// Pool ID
    pub pool_id: Uint128,
    /// The pools type (provided in a [`PoolType`])
    pub pool_type: PoolType,
    /// the vault contract address
    pub vault_addr: Addr,
    /// Address of the LP Token Contract
    pub lp_token_addr: Addr,
    /// Assets supported by the pool
    pub asset_infos: Vec<AssetInfo>,
    /// Native asset precisions
    pub native_asset_precisions: Vec<(String, u8)>,
    /// The Fee details of the pool
    pub fee_info: FeeStructs,
    /// Optional binary serialised parameters for custom pool types
    pub init_params: Option<Binary>,
}

/// ## Description
///
/// This struct describes the execute messages of the contract.
/// Each msg's params should be kept generic to allow addition of new pool types later, allow addition
/// of logic which may need those variables, even though those params might not be used by the current pools.
#[cw_serde]
pub enum ExecuteMsg {
    /// ## Description - Update updatable parameters related to Pool's configuration
    UpdateConfig { params: Option<Binary> },
    /// ## Description - Update total fee bps
    UpdateFee { total_fee_bps: u16 },
    /// ## Description - Executable only by Dexter Vault.  Updates locally stored asset balances state for the pool and updates the TWAP.
    UpdateLiquidity { assets: Vec<Asset> },
}

/// ## Description
/// This struct describes the query messages of the contract.
/// Each msg's params should be kept generic to allow addition of new pool types later, allow addition
/// of logic which may need those variables, even though those params might not be used by the current pools.
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// ## Description - Returns the current configuration of the pool.
    #[returns(ConfigResponse)]
    Config {},
    /// ## Description - Returns information about the Fees settings in a [`FeeResponse`] object.
    #[returns(FeeResponse)]
    FeeParams {},
    /// ## Description - Returns Pool ID which is of type [`Uint128`]
    #[returns(Uint128)]
    PoolId {},
    /// ## Description - Returns [`AfterJoinResponse`] type which contains - `return_assets` info, number of LP shares to be minted, the `response` of type [`ResponseType`]
    /// and `fee` of type [`Option<Asset>`] which is the fee to be charged
    #[returns(AfterJoinResponse)]
    OnJoinPool {
        assets_in: Option<Vec<Asset>>,
        // in future, it could be supplied instead of assets_in, to convey that the user should get
        // this much LP tokens and the pool should charge as much assets_in as it needs to give
        // mint_amount LP tokens back to user.
        mint_amount: Option<Uint128>,
    },
    /// ## Description - Returns [`AfterExitResponse`] type which contains - `assets_out` info, number of LP shares to be burnt, the `response` of type [`ResponseType`]
    ///  and `fee` of type [`Option<Asset>`] which is the fee to be charged
    #[returns(AfterExitResponse)]
    OnExitPool {
        exit_type: ExitType,
    },
    /// ## Description - Returns [`SwapResponse`] type which contains - `trade_params` info, the `response` of type [`ResponseType`] and `fee` of type [`Option<Asset>`] which is the fee to be charged
    #[returns(SwapResponse)]
    OnSwap {
        swap_type: SwapType,
        offer_asset: AssetInfo,
        ask_asset: AssetInfo,
        amount: Uint128,
        max_spread: Option<Decimal>,
        belief_price: Option<Decimal>,
    },
    /// ## Description - Returns information about the cumulative price of the asset in a [`CumulativePriceResponse`] object.
    #[returns(CumulativePriceResponse)]
    CumulativePrice {
        offer_asset: AssetInfo,
        ask_asset: AssetInfo,
    },
    /// ## Description - Returns information about the cumulative prices in a [`CumulativePricesResponse`] object.
    #[returns(CumulativePricesResponse)]
    CumulativePrices {},
}

/// This struct describes the ways one can choose to exit from a pool.
#[cw_serde]
pub enum ExitType {
    /// provide this to convey that only this much LP tokens should be burned,
    /// irrespective of how much assets you will get back.
    ExactLpBurn(Uint128),
    /// provide this to convey that you want exactly these assets out, irrespective of how much LP
    /// tokens need to be burned for that.
    ExactAssetsOut(Vec<Asset>),
}

/// ## Description
/// This struct describes a migration message.
/// We currently take no arguments for migrations.
#[cw_serde]
pub struct MigrateMsg {}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x     Response Types       x----------------x----------------x------
// ----------------x----------------x----------------x----------------x----------------x----------------

#[cw_serde]
pub struct ConfigResponse {
    /// ID of contract which is allowed to create pools of this type
    pub pool_id: Uint128,
    pub lp_token_addr: Addr,
    /// the vault contract address
    pub vault_addr: Addr,
    /// Assets supported by the pool
    pub assets: Vec<Asset>,
    /// The pools type (provided in a [`PoolType`])
    pub pool_type: PoolType,
    pub fee_info: FeeStructs,
    /// The last time block
    pub block_time_last: u64,
    /// Custom Math Config parameters are returned in binary format here
    pub math_params: Option<Binary>,
    pub additional_params: Option<Binary>,
}

/// ## Description - Helper struct for [`QueryMsg::OnJoinPool`]
#[cw_serde]
pub struct AfterJoinResponse {
    // Is a sorted list consisting of amount of info of tokens which will be provided by the user to the Vault as liquidity
    pub provided_assets: Vec<Asset>,
    // Is the amount of LP tokens to be minted
    pub new_shares: Uint128,
    // Is the response type :: Success or Failure
    pub response: ResponseType,
    // Is the fee to be charged
    pub fee: Option<Vec<Asset>>,
}

/// ## Description  - Helper struct for [`QueryMsg::OnExitPool`]
#[cw_serde]
pub struct AfterExitResponse {
    /// Sorted list of assets which will be transferred to the recipient against tokens being burnt
    pub assets_out: Vec<Asset>,
    /// Number of LP tokens to burn
    pub burn_shares: Uint128,
    /// Operation will be a `Success` or `Failure`
    pub response: ResponseType,
    /// Fee to be charged
    pub fee: Option<Vec<Asset>>,
}

/// ## Description
#[cw_serde]
pub struct FeeResponse {
    /// The total fees (in bps) charged by a pool of this type
    pub total_fee_bps: u16,
}

/// ## Description - Helper struct for [`QueryMsg::OnSwap`]
#[cw_serde]
pub struct SwapResponse {
    ///  Is of type [`Trade`] which contains all params related with the trade
    pub trade_params: Trade,
    /// Operation will be a `Success` or `Failure`
    pub response: ResponseType,
    /// Fee to be charged
    pub fee: Option<Asset>,
}

/// ## Description - Helper struct for [`QueryMsg::CumulativePrice`]
#[cw_serde]
pub struct CumulativePriceResponse {
    pub exchange_info: AssetExchangeRate,
    pub total_share: Uint128,
}

/// ## Description - Helper struct for [`QueryMsg::CumulativePrices`]
#[cw_serde]
pub struct CumulativePricesResponse {
    pub exchange_infos: Vec<AssetExchangeRate>,
    pub total_share: Uint128,
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x     Helper response functions       x----------------x------------
// ----------------x----------------x----------------x----------------x----------------x----------------

pub fn update_total_fee_bps(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    total_fee_bps: u16,
    config_item: Item<Config>,
) -> StdResult<Response> {
    let mut config = config_item.load(deps.storage)?;

    // Access Check :: Only Vault can execute this function
    if info.sender != config.vault_addr {
        return Err(StdError::generic_err("Unauthorized"));
    }

    config.fee_info.total_fee_bps = total_fee_bps;
    config_item.save(deps.storage, &config)?;

    let event = Event::new("dexter-pool::update_total_fee_bps")
        .add_attribute("total_fee_bps", config.fee_info.total_fee_bps.to_string());
    Ok(Response::new().add_event(event))
}

pub fn return_join_failure(error: String) -> AfterJoinResponse {
    AfterJoinResponse {
        provided_assets: vec![],
        new_shares: Uint128::zero(),
        response: ResponseType::Failure(error),
        fee: None,
    }
}

pub fn return_exit_failure(error: String) -> AfterExitResponse {
    AfterExitResponse {
        assets_out: vec![],
        burn_shares: Uint128::zero(),
        response: ResponseType::Failure(error),
        fee: None,
    }
}

pub fn return_swap_failure(error: String) -> SwapResponse {
    SwapResponse {
        trade_params: Trade {
            amount_in: Uint128::zero(),
            amount_out: Uint128::zero(),
            spread: Uint128::zero(),
        },
        response: ResponseType::Failure(error),
        fee: None,
    }
}
