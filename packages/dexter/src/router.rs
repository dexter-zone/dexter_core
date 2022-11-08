use crate::asset::AssetInfo;
use crate::pool::ResponseType;
use crate::vault::SwapType;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{to_binary, Addr, CosmosMsg, Decimal, StdResult, Uint128, WasmMsg};

pub const MAX_SWAP_OPERATIONS: usize = 50;

#[cw_serde]
pub struct InstantiateMsg {
    /// The dexter Vault contract address
    pub dexter_vault: String,
}

#[cw_serde]
pub struct Config {
    /// The dexter vault contract address
    pub dexter_vault: Addr,
}

/// This enum describes a swap operation.
#[cw_serde]
pub struct HopSwapRequest {
    /// Pool Id via which the swap is to be routed
    pub pool_id: Uint128,
    /// The offer asset
    pub asset_in: AssetInfo,
    ///  The ask asset
    pub asset_out: AssetInfo,
    pub max_spread: Option<Decimal>,
    pub belief_price: Option<Decimal>,
}

/// This structure describes the execute messages available in the contract.
#[cw_serde]
pub enum ExecuteMsg {
    /// ExecuteMultihopSwap processes multiple swaps via dexter pools
    ExecuteMultihopSwap {
        multiswap_request: Vec<HopSwapRequest>,
        offer_amount: Uint128,
        recipient: Option<Addr>,
        minimum_receive: Option<Uint128>,
    },
    /// Callbacks; only callable by the contract itself.
    Callback(CallbackMsg),
}

#[cw_serde]
pub enum CallbackMsg {
    ContinueHopSwap {
        multiswap_request: Vec<HopSwapRequest>,
        offer_asset: AssetInfo,
        prev_ask_amount: Uint128,
        recipient: Addr,
        minimum_receive: Uint128,
    },
}

// Modified from
// https://github.com/CosmWasm/cosmwasm-plus/blob/v0.2.3/packages/cw20/src/receiver.rs#L15
impl CallbackMsg {
    pub fn to_cosmos_msg(&self, contract_addr: &Addr) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: String::from(contract_addr),
            msg: to_binary(&ExecuteMsg::Callback(self.clone()))?,
            funds: vec![],
        }))
    }
}

/// This structure describes the query messages available in the contract.
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    /// SimulateMultihopSwap simulates multi-hop swap operations
    #[returns(SimulateMultiHopResponse)]
    SimulateMultihopSwap {
        multiswap_request: Vec<HopSwapRequest>,
        swap_type: SwapType,
        amount: Uint128,
    },
}

#[cw_serde]
pub struct ConfigResponse {
    /// The dexter vault contract address
    pub dexter_vault: String,
}

#[cw_serde]
pub struct SimulateMultiHopResponse {
    pub swap_operations: Vec<SimulatedTrade>,
    pub response: ResponseType,
}

#[cw_serde]
pub struct SimulatedTrade {
    pub pool_id: Uint128,
    pub asset_in: AssetInfo,
    pub offered_amount: Uint128,
    pub asset_out: AssetInfo,
    pub received_amount: Uint128,
}

/// This structure describes a migration message.
/// We currently take no arguments for migrations.
#[cw_serde]
pub struct MigrateMsg {}

pub fn return_swap_sim_failure(
    suc_swaps: Vec<SimulatedTrade>,
    error: String,
) -> SimulateMultiHopResponse {
    SimulateMultiHopResponse {
        swap_operations: suc_swaps,
        response: ResponseType::Failure(error),
    }
}
