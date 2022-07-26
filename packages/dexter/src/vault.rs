use crate::asset::{Asset, AssetInfo};
use cosmwasm_std::{Addr, Decimal, Uint128, Binary};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Result};

const MAX_TOTAL_FEE_BPS: cosmwasm_std::Decimal =Decimal::new(Uint128::new(10_000));
const MAX_PROTOCOL_FEE_BPS: cosmwasm_std::Decimal =Decimal::new(Uint128::new(10_000));
const MAX_DEV_FEE_BPS: cosmwasm_std::Decimal =Decimal::new(Uint128::new(10_000));

pub const TWAP_PRECISION:cosmwasm_std::Decimal =Decimal::new(Uint128::new(9));

// / This enum describes available Pool types.
// / ## Available pool types
// / ```
// / # use dexter::vault::PoolType::{Stable, Xyk, Weighted, meta-stable, Custom};
// / Xyk {};
// / Stable {};
// / Weighted {};
// / MetaStable {};
// / Custom(String::from("Custom"));
// / ```
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PoolType {
    /// XYK pool type
    Xyk {},
    /// Stable pool type
    Stable {},
    /// Weighted pool type
    Weighted {},
    /// Meta-Stable pool type
    MetaStable {},
    /// Custom pool type
    Custom(String),
}

// Return a raw encoded string representing the name of each pool type
impl Display for PoolType {
    fn fmt(&self, fmt: &mut Formatter) -> Result {
        match self {
            PoolType::Xyk {} => fmt.write_str("xyk"),
            PoolType::Stable {} => fmt.write_str("stable"),
            PoolType::Weighted {} => fmt.write_str("weighted"),
            PoolType::MetaStable {} => fmt.write_str("metastable"),
            PoolType::Custom(pool_type) => fmt.write_str(format!("custom-{}", pool_type).as_str()),
        }
    }
}

/// This enum describes available Swap types.
/// ## Available swap types
/// ```
/// # use dexter::vault::SwapType::{GiveOut, GiveIn};
/// GiveIn {};
/// GiveOut {};
/// ```
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SwapType {
    GiveIn {},
    GiveOut {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct FeeInfo {
    pub total_fee_bps: Decimal,
    pub protocol_fee_bps: Decimal,
    pub dev_fee_bps: Decimal,
    pub dev_addr_bps: Option<Addr>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SwapKind {
    In {},
    Out {},
}

impl Display for SwapKind {
    fn fmt(&self, fmt: &mut Formatter) -> Result {
        match self {
            SwapKind::In {} => fmt.write_str("in"),
            SwapKind::Out {} => fmt.write_str("out"),
        }
    }
}

/// ## Description - This structure describes the main control config of factory.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SingleSwapRequest {
    pub pool_id: Uint128,
    pub asset_in: AssetInfo,
    pub asset_out: AssetInfo,
    pub swap_type: SwapKind,
    pub amount: Uint128,
}

/// ## Description - This structure describes the main control config of factory.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// The Contract address that used for controls settings for factory, pools and tokenomics contracts
    pub owner: Addr,
    /// IDs and configs of contracts that are allowed to instantiate pools
    pub pool_configs: Vec<PoolConfig>,
    pub lp_token_code_id: u64,
    /// contract address to send fees to
    pub fee_collector: Option<Addr>,
    pub generator_address: Option<Addr>,
    pub next_pool_id: Uint128,
}

/// This structure stores a pool type's configuration.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolConfig {
    /// ID of contract which is allowed to create pools of this type
    pub code_id: u64,
    /// The pools type (provided in a [`PoolType`])
    pub pool_type: PoolType,
    pub fee_info: FeeInfo,
    /// Whether a pool type is disabled or not. If it is disabled, new pools cannot be
    /// created, but existing ones can still read the pool configuration
    pub is_disabled: bool,
    /// Setting this to true means that pools of this type will not be able
    /// to get added to generator
    pub is_generator_disabled: bool,
}

/// ## Description - This is an intermediate structure for storing the key of a pair and used in reply of submessage.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TmpPoolInfo {
    pub pool_id: Uint128,
    pub assets: Vec<AssetInfo>,
}

/// This structure stores a pool type's configuration.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfo {
    /// ID of contract which is allowed to create pools of this type
    pub pool_id: Uint128,
    /// Address of the Pool Contract    
    pub pool_addr: Option<Addr>,
    /// Address of the LP Token Contract    
    pub lp_token_addr: Option<Addr>,
    /// Assets and their respective balances
    pub assets: Vec<Asset>,
    /// The pools type (provided in a [`PoolType`])
    pub pool_type: PoolType,
    /// The address to which the collected developer fee is transferred
    pub dev_addr_bps: Option<Addr>,
    /// Address having admin priviledges on the pool. Will be used when the 'Self-balancing Index Pools' feature will be made live   
    pub pool_manager: Option<String>,
}

impl PoolConfig {
    /// This method is used to check fee bps.
    /// ## Params
    /// `&self` is the type of the caller object.
    pub fn valid_fee_bps(&self) -> bool {
        self.fee_info.total_fee_bps <= MAX_TOTAL_FEE_BPS
            && self.fee_info.protocol_fee_bps <= MAX_PROTOCOL_FEE_BPS
            && self.fee_info.dev_fee_bps <= MAX_DEV_FEE_BPS
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    /// IDs and configs of contracts that are allowed to instantiate pools
    pub pool_configs: Vec<PoolConfig>,
    pub lp_token_code_id: u64,
    pub fee_collector: Option<String>,
    pub generator_address: Option<String>,
}

// struct BatchSwapStep {
//     bytes32 poolId;
//     uint256 assetInIndex;
//     uint256 assetOutIndex;
//     uint256 amount;
//     bytes userData;
// }

/// ## Description -  This structure describes the execute messages of the contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // Receives LP Tokens when removing Liquidity
    Receive(Cw20ReceiveMsg),
    /// UpdateConfig updates updatable Config parameters
    UpdateConfig {
        lp_token_code_id: Option<u64>,
        fee_collector: Option<String>,
        generator_address: Option<String>,
    },
    UpdatePoolConfig {
        pool_type: PoolType,
        is_disabled: Option<bool>,
        is_generator_disabled: Option<bool>,
    },
    /// CreatePool instantiates pool contract
    CreatePool {
        pool_type: PoolType,
        asset_infos: Vec<AssetInfo>,
        lp_token_name: Option<String>,
        lp_token_symbol: Option<String>,
        pool_manager: Option<String>,
        init_params: Option<Binary>,
    },
    JoinPool {
        pool_id: Uint128,
        recepient: Option<String>,
        assets: Vec<Asset>,
        auto_stake: Option<bool>,
    },
    Swap {
        swap_request: SingleSwapRequest,
        limit: Option<Uint128>,
        deadline: Option<Uint128>,
        recepient: Option<String>,
    },
    // BatchSwap {
    //     swap_kind: SwapKind,
    //     batch_swap_steps: Vec<BatchSwapStep>,
    //     assets: Vec<Asset>,
    //     limit: Option<Vec<Uint128>>,
    //     deadline: Option<Uint128>,
    // },
    /// Deregister removes a previously created pair
    // Deregister { asset_infos: Vec<AssetInfo> },
    /// ProposeNewOwner creates an offer for a new owner. The validity period of the offer is set in the `expires_in` variable.
    ProposeNewOwner {
        owner: String,
        expires_in: u64,
    },
    /// DropOwnershipProposal removes the existing offer for the new owner.
    DropOwnershipProposal {},
    /// Used to claim(approve) new owner proposal, thus changing contract's owner
    ClaimOwnership {},
}

/// ## Description
/// This structure describes a CW20 hook message.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// Withdrawing liquidity from the pool
    ExitPool {
        pool_id: Uint128,
        recepient: Option<String>,
        assets: Option<Vec<Asset>>,
        burn_amount: Uint128,
    },
}

/// ## Description -  This structure describes the query messages of the contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Config returns controls settings that specified in custom [`ConfigResponse`] structure
    Config {},
    PoolConfig {
        pool_type: PoolType,
    },
    GetPoolById {
        pool_id: Uint128,
    },
    GetPoolByAddress {
        pool_addr: Addr,
    },
    // QuerybatchSwap {
    //     swap_kind: SwapKind,
    //     batch_swap_steps: Vec<BatchSwapStep>,
    //     assets: Vec<Asset>,
    // },
}

/// ## Description -  A custom struct for each query response that returns controls settings of contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: Addr,
    pub lp_token_code_id: u64,
    pub fee_collector: Option<Addr>,
    pub generator_address: Option<Addr>,
    pub pool_configs: Vec<PoolConfig>,
}

pub type PoolConfigResponse = PoolConfig;
pub type PoolInfoResponse = PoolInfo;

/// ## Description -  This structure describes a migration message.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
