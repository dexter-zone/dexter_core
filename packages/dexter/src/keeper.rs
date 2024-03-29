use crate::asset::{Asset, AssetInfo};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    Instantiate, Execute Msgs and Queries      x----------------x--
// ----------------x----------------x----------------x----------------x----------------x----------------

/// This struct describes the Msg used to instantiate in this contract.
#[cw_serde]
pub struct InstantiateMsg {
    /// Owner address
    pub owner: Addr,
    /// Vault contract address
    pub vault_address: Addr,
}


/// ## Description
/// This structure stores the main parameters for the Keeper contract.
#[cw_serde]
pub struct Config {
    /// admin address
    pub owner: Addr,
    /// Vault contract address
    pub vault_address: Addr,
}

#[cw_serde]
pub struct ConfigV1 {
    pub owner: Addr
}

/// This struct describes the functions that can be executed in this contract.
#[cw_serde]
pub enum ExecuteMsg {
    /// Withdraws an asset from the contract
    /// This is used to withdraw the fees collected by the contract by the owner
    Withdraw {
        /// The asset to withdraw
        asset: AssetInfo,
        /// The amount to withdraw
        amount: Uint128,
        /// The recipient address. If None, the owner address will be used
        recipient: Option<Addr>,
    },
     /// Exit LP tokens that are received as part of instant LP unbonding fee to contain the base assets of the pool only
     ExitLPTokens {
        /// Contract address of the LP token
        lp_token_address: String,
        /// The amount of LP tokens to exit
        amount: Uint128,
        /// Slippage protection
        min_assets_received: Option<Vec<Asset>>,
    },
    /// Swap an asset contained in the keeper for a different asset using Dexter pools
    SwapAsset{
        offer_asset: Asset,
        ask_asset_info: AssetInfo,
        min_ask_amount: Option<Uint128>,
        pool_id: Uint128
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

/// This struct describes the query functions available in the contract.
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns information about the Keeper configs that contains in the [`ConfigResponse`]
    #[returns(ConfigResponse)]
    Config {},
    /// Returns the balance for each asset in the specified input parameters
    #[returns(BalancesResponse)]
    Balances { assets: Vec<AssetInfo> },
}

/// This struct describes a migration message.
/// We currently take no arguments for migrations.
#[cw_serde]
pub enum MigrateMsg {
    V2 {
        vault_address: String,
    }
}

// ----------------x----------------x----------------x----------------x----------------x----------------
// ----------------x----------------x    Response Types      x----------------x----------------x--------
// ----------------x----------------x----------------x----------------x----------------x----------------

/// A custom type that holds contract parameters and is used to retrieve them.
pub type ConfigResponse = Config;

/// A custom struct used to return multiple asset balances.
#[cw_serde]
pub struct BalancesResponse {
    pub balances: Vec<Asset>,
}
