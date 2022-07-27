use crate::asset::{Asset, AssetInfo};
use cosmwasm_std::{Addr,Decimal, Uint128, Binary};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Result};

// TWAP PRECISION is 9 decimal places
pub const TWAP_PRECISION: u16 = 9u16;

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    {{PoolType}} enum Type    x----------------x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// This enum describes available Pool types.
/// ## Available pool types
/// ```
/// Xyk
/// Stable
/// Weighted
/// MetaStable
/// Custom(String::from("Custom"));
/// ```
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

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    {{SwapType}} enum Type    x----------------x-------------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// This enum describes available Swap types.
/// ## Available swap types
/// ```
/// GiveIn ::   When we have the number of tokens being provided by the user to the pool in the swap request
/// GiveOut :: When we have the number of tokens to be sent to the user from the pool in the swap request
/// ```
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SwapType {
    GiveIn {},
    GiveOut {},
    /// Custom swap type
    Custom(String),
}

impl Display for SwapType {
    fn fmt(&self, fmt: &mut Formatter) -> Result {
        match self {
            SwapType::GiveIn {} => fmt.write_str("give-in"),
            SwapType::GiveOut {} => fmt.write_str("give-out"),
            SwapType::Custom(swap_type) => fmt.write_str(format!("custom-{}", swap_type).as_str()),
        }
    }
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    {{FeeInfo}} struct Type    x----------------x-------------------
// ----------------x----------------x----------------x----------------x----------------x----------------

// Maximum total commission in bps that can be charged on any supported pool by Dexter
const MAX_TOTAL_FEE_BPS: Decimal =Decimal::new(Uint128::new(10_000));
// Maximum total protocol fee as % of the commission fee that can be charged on any supported pool by Dexter
const MAX_PROTOCOL_FEE_PERCENT: u16 = 50;
// Maximum dev protocol fee as % of the commission fee that can be charged on any supported pool by Dexter
const MAX_DEV_FEE_PERCENT: u16 = 25;


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct FeeInfo {
    pub total_fee_bps: Decimal,
    pub protocol_fee_percent: u16,
    pub dev_fee_percent: u16,
    pub developer_addr: Option<Addr>,
}

impl FeeInfo { 

    /// This method is used to check fee bps.
    pub fn valid_fee_info(&self) -> bool {
        self.total_fee_bps <= MAX_TOTAL_FEE_BPS
            && self.protocol_fee_percent <= MAX_PROTOCOL_FEE_PERCENT
            && self.dev_fee_percent <= MAX_DEV_FEE_PERCENT
    }    

    // Returns the number of tokens charged as total fee, protocol fee and dev fee 
    pub fn calculate_underlying_fees(&self, amount: Uint128) -> (Uint128,Uint128,Uint128) {
        // let commission_rate = decimal2decimal256(self.total_fee_bps)?;

        let total_fee: Uint128 = amount * self.total_fee_bps;
        let protocol_fee: Uint128 = total_fee *  Decimal::from_ratio(self.protocol_fee_percent, Uint128::from(100u128));
        let dev_fee: Uint128 = total_fee *  Decimal::from_ratio(self.dev_fee_percent, Uint128::from(100u128));

        (total_fee, protocol_fee, dev_fee)
    }
}


// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    Generic struct Types      x----------------x-------------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// ## Description - This structure describes the main control config of Vault.
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
    /// ID of contract which is used to create pools of this type
    pub code_id: u64,
    /// The pools type (provided in a [`PoolType`])
    pub pool_type: PoolType,
    pub fee_info: FeeInfo,
    /// Whether a pool type is disabled or not. If it is disabled, new pools cannot be
    /// created, but existing ones can still read the pool configuration
    pub is_disabled: bool,
    /// Setting this to true means that pools of this type will not be able
    /// to get added to generator
    pub is_generator_disabled: bool
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
    pub developer_addr: Option<Addr>,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SingleSwapRequest {
    pub pool_id: Uint128,
    pub asset_in: AssetInfo,
    pub asset_out: AssetInfo,
    pub swap_type: SwapType,
    pub amount: Uint128,
}

// struct BatchSwapStep {
//     bytes32 poolId;
//     uint256 assetInIndex;
//     uint256 assetOutIndex;
//     uint256 amount;
//     bytes userData;
// }


// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    Instantiate, Execute Msgs and Queries      x----------------x--
// ----------------x----------------x----------------x----------------x----------------x----------------


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    /// IDs and configs of contracts that are allowed to instantiate pools
    pub pool_configs: Vec<PoolConfig>,
    pub lp_token_code_id: u64,
    pub fee_collector: Option<String>,
    pub generator_address: Option<String>,
}


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
        new_fee_info: Option<FeeInfo>,
    },
    /// CreatePool instantiates pool contract
    CreatePool {
        pool_type: PoolType,
        asset_infos: Vec<AssetInfo>,
        lp_token_name: Option<String>,
        lp_token_symbol: Option<String>,
        init_params: Option<Binary>,
    },
    JoinPool {
        pool_id: Uint128,
        recipient: Option<String>,
        assets: Option<Vec<Asset>>,
        lp_to_mint: Option<Uint128>,
        auto_stake: Option<bool>,
    },
    Swap {
        swap_request: SingleSwapRequest,
        limit: Option<Uint128>,
        deadline: Option<Uint128>,
        recipient: Option<String>,
    },
    // BatchSwap {
    //     swap_kind: SwapType,
    //     batch_swap_steps: Vec<BatchSwapStep>,
    //     assets: Vec<Asset>,
    //     limit: Option<Vec<Uint128>>,
    //     deadline: Option<Uint128>,
    // },
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
        recipient: Option<String>,
        assets: Option<Vec<Asset>>,
        burn_amount: Option<Uint128>,
    },
}

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
        pool_addr: String,
    },
    // QuerybatchSwap {
    //     swap_kind: SwapType,
    //     batch_swap_steps: Vec<BatchSwapStep>,
    //     assets: Vec<Asset>,
    // },
}


/// ## Description -  This structure describes a migration message.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}


// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    Response Types      x----------------x----------------x--------
// ----------------x----------------x----------------x----------------x----------------x----------------


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
