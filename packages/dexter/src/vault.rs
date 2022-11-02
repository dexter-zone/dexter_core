use crate::asset::{Asset, AssetInfo};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Binary, Decimal, Uint128};
use cw20::Cw20ReceiveMsg;
use std::fmt::{Display, Formatter, Result};

// TWAP PRECISION is 9 decimal places
pub const TWAP_PRECISION: u16 = 9u16;

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    {{PoolType}} enum Type       x----------------x----------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// This enum describes the key for the different Pool types supported by Dexter
#[cw_serde]
pub enum PoolType {
    /// XYK pool type
    Xyk {},
    /// Stable pool type
    Stable2Pool {},
    /// Stable pool type
    Stable5Pool {},
    /// Weighted pool type
    Weighted {},
    /// Custom pool type
    Custom(String),
}

// Return a raw encoded string representing the name of each pool type
impl Display for PoolType {
    fn fmt(&self, fmt: &mut Formatter) -> Result {
        match self {
            PoolType::Xyk {} => fmt.write_str("xyk"),
            PoolType::Stable2Pool {} => fmt.write_str("stable-2-pool"),
            PoolType::Weighted {} => fmt.write_str("weighted"),
            PoolType::Stable5Pool {} => fmt.write_str("stable-5-pool"),
            PoolType::Custom(pool_type) => fmt.write_str(format!("custom-{}", pool_type).as_str()),
        }
    }
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    {{SwapType}} enum Type    x----------------x-------------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// This enum describes available Swap types.
#[cw_serde]
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

// FEE PRECISION is 4 decimal places
pub const FEE_PRECISION: u16 = 10_000u16;
// Maximum total commission in bps that can be charged on any supported pool by Dexter
// If MAX_TOTAL_FEE_BPS / FEE_PRECISION is 1, then the maximum total commission that can be charged on any supported pool by Dexter is 1%
const MAX_TOTAL_FEE_BPS: u16 = 10_000u16;
// Maximum total protocol fee as % of the commission fee that can be charged on any supported pool by Dexter
const MAX_PROTOCOL_FEE_PERCENT: u16 = 50u16;
// Maximum dev protocol fee as % of the commission fee that can be charged on any supported pool by Dexter
const MAX_DEV_FEE_PERCENT: u16 = 25u16;

/// ## Description - This struct describes the Fee configuration supported by a particular pool type.
#[cw_serde]
pub struct FeeInfo {
    pub total_fee_bps: u16,
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
    pub fn calculate_total_fee_breakup(&self, total_fee: Uint128) -> (Uint128, Uint128) {
        // println!(
        //     "calculate_total_fee_breakup:: protocol_fee_percent = {}, dev_fee_percent = {}",
        //     self.protocol_fee_percent, self.dev_fee_percent
        // );
        let protocol_fee: Uint128 =
            total_fee * Decimal::from_ratio(self.protocol_fee_percent, Uint128::from(100u128));
        let dev_fee: Uint128 =
            total_fee * Decimal::from_ratio(self.dev_fee_percent, Uint128::from(100u128));
        (protocol_fee, dev_fee)
    }
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    Generic struct Types      x----------------x-------------------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// ## Description - This struct describes the main control config of Vault.
#[cw_serde]
pub struct Config {
    /// The admin address that controls settings for factory, pools and tokenomics contracts
    pub owner: Addr,
    /// The Contract ID that is used for instantiating LP tokens for new pools
    pub lp_token_code_id: u64,
    /// The contract address to which protocol fees are sent
    pub fee_collector: Option<Addr>,
    /// The contract where users can stake LP tokens for 3rd party rewards. Used for `auto-stake` feature
    pub generator_address: Option<Addr>,
    /// The next pool ID to be used for creating new pools
    pub next_pool_id: Uint128,
}

/// This struct stores a pool type's configuration.
#[cw_serde]
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
    pub is_generator_disabled: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        PoolConfig {
            code_id: 0u64,
            pool_type: PoolType::Xyk {},
            fee_info: FeeInfo {
                total_fee_bps: 0,
                protocol_fee_percent: 0,
                dev_fee_percent: 0,
                developer_addr: None,
            },
            is_disabled: false,
            is_generator_disabled: false,
        }
    }
}

/// ## Description - This is an intermediate struct for storing the key of a pair and used in reply of submessage.
#[cw_serde]
pub struct TmpPoolInfo {
    pub pool_id: Uint128,
    pub assets: Vec<AssetInfo>,
}

/// This struct stores a pool type's configuration.
#[cw_serde]
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
}

#[cw_serde]
pub struct SingleSwapRequest {
    pub pool_id: Uint128,
    pub asset_in: AssetInfo,
    pub asset_out: AssetInfo,
    pub swap_type: SwapType,
    pub amount: Uint128,
    pub max_spread: Option<Decimal>,
    pub belief_price: Option<Decimal>,
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    Instantiate, Execute Msgs and Queries      x----------------x--
// ----------------x----------------x----------------x----------------x----------------x----------------

/// This struct describes the Msg used to instantiate in this contract.
#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    /// IDs and configs of contracts that are allowed to instantiate pools
    pub pool_configs: Vec<PoolConfig>,
    pub lp_token_code_id: u64,
    pub fee_collector: Option<String>,
    pub generator_address: Option<String>,
}

/// This struct describes the functions that can be executed in this contract.
#[cw_serde]
pub enum ExecuteMsg {
    // Receives LP Tokens when removing Liquidity
    Receive(Cw20ReceiveMsg),
    /// Executable only by `config.owner`. Facilitates updating `config.fee_collector`, `config.generator_address`,
    /// `config.lp_token_code_id` parameters.       
    UpdateConfig {
        lp_token_code_id: Option<u64>,
        fee_collector: Option<String>,
        generator_address: Option<String>,
    },
    ///  Executable only by `pool_config.fee_info.developer_addr` or `config.owner` if its not set.
    /// Facilitates enabling / disabling new pool instances creation (`pool_config.is_disabled`) ,
    /// and updating Fee (` pool_config.fee_info`) for new pool instances
    UpdatePoolConfig {
        pool_type: PoolType,
        is_disabled: Option<bool>,
        new_fee_info: Option<FeeInfo>,
        is_generator_disabled: Option<bool>,
    },
    ///  Adds a new pool with a new [`PoolType`] Key.                                                                       
    AddToRegistry {
        new_pool_config: PoolConfig,
    },
    /// Creates a new pool with the specified parameters in the `asset_infos` variable.                               
    CreatePoolInstance {
        pool_type: PoolType,
        asset_infos: Vec<AssetInfo>,
        lp_token_name: Option<String>,
        lp_token_symbol: Option<String>,
        init_params: Option<Binary>,
    },
    // Entry point for a user to Join a pool supported by the Vault. User can join by providing the pool id and
    // either the number of assets to be provided or the LP tokens to be minted to the user (as defined by the Pool Contract).                        |
    JoinPool {
        pool_id: Uint128,
        recipient: Option<String>,
        assets: Option<Vec<Asset>>,
        lp_to_mint: Option<Uint128>,
        slippage_tolerance: Option<Decimal>,
        auto_stake: Option<bool>,
    },
    // Entry point for a swap tx between offer and ask assets. The swap request details are passed in
    // [`SingleSwapRequest`] Type parameter.
    Swap {
        swap_request: SingleSwapRequest,
        recipient: Option<String>,
        min_receive: Option<Uint128>,
        max_spend: Option<Uint128>,
    },
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
/// This struct describes a CW20 hook message.
#[cw_serde]
pub enum Cw20HookMsg {
    /// Withdrawing liquidity from the pool
    ExitPool {
        pool_id: Uint128,
        recipient: Option<String>,
        assets: Option<Vec<Asset>>,
        burn_amount: Option<Uint128>,
    },
}

/// Returns the [`PoolType`]'s Configuration settings  in custom [`PoolConfigResponse`] struct
    

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Config returns controls settings that specified in custom [`ConfigResponse`] struct
    #[returns[ConfigResponse]]
    Config {},
    /// Return PoolConfig
    #[returns(PoolConfigResponse)]
    QueryRegistry { pool_type: PoolType },
    /// Returns boolean value indicating if the genarator is disabled or not for the pool
    #[returns(bool)]
    IsGeneratorDisabled { lp_token_addr: String },
    /// Returns the current stored state of the Pool in custom [`PoolInfoResponse`] struct
    #[returns(PoolInfoResponse)]
    GetPoolById { pool_id: Uint128 },
    /// Returns the current stored state of the Pool in custom [`PoolInfoResponse`] struct
    #[returns(PoolInfoResponse)]
    GetPoolByAddress { pool_addr: String },
}

/// ## Description -  This struct describes a migration message.
#[cw_serde]
pub struct MigrateMsg {}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    Response Types      x----------------x----------------x--------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// ## Description -  A custom struct for each query response that returns controls settings of contract.
#[cw_serde]
pub struct ConfigResponse {
    pub owner: Addr,
    pub lp_token_code_id: u64,
    pub fee_collector: Option<Addr>,
    pub generator_address: Option<Addr>,
    /// The next pool ID to be used for creating new pools
    pub next_pool_id: Uint128,
}

#[derive(Serialize, Deserialize)]
pub struct AssetFeeBreakup {
    pub asset: AssetInfo,
    pub total_fee: Uint128,
    pub protocol_fee: Uint128,
    pub dev_fee: Uint128
}

pub type PoolConfigResponse = PoolConfig;

/// ## Description -  A custom struct for query response that returns the
/// current stored state of a Pool Instance identified by either pool_id or pool_address.
/// Parameters -::-
/// `pool_id` - The ID of the pool instance
/// `pool_address` - The address of the pool instance
/// lp_token_address - The address of the LP token contract
/// assets - The current asset balances of the pool
/// pool_type - The type of the pool
pub type PoolInfoResponse = PoolInfo;
