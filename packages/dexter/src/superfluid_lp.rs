use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};

use crate::asset::{Asset, AssetInfo};

#[cw_serde]
pub struct InstantiateMsg {
    pub vault_addr: Addr,
    // allowed lockable tokens
    pub allowed_lockable_tokens: Vec<AssetInfo>,
    /// owner of the contract
    pub owner: Addr,
}

#[cw_serde]
#[derive(Eq)]
pub struct LockInfo {
    pub amount: Uint128,
    pub unlock_time: u64,
}

#[cw_serde]
pub struct Config {
    pub vault_addr: Addr,
    pub owner: Addr,
    pub allowed_lockable_tokens: Vec<AssetInfo>
}


#[cw_serde]
pub enum ExecuteMsg {

    /// Locks an LST asset for the user, which can only be used to join a pool on Dexter
    LockLstAsset {
        asset: Asset,
    },

    /// Join pool on behalf of the user using the locked LST and any extra tokens that the user is willing to spend.
    /// This function will also bond the LP position for the user so they immediately start earning the LP incentive rewards.
    JoinPoolAndBondUsingLockedLst {
        pool_id: Uint128,
        /// Represents the total amount of each token that the user wants to spend for joining the pool
        /// This includes the amount of tokens that are locked for the user + any extra token that he is willing to send along and spend
        total_assets: Vec<Asset>,
        min_lp_to_receive: Option<Uint128>
    },

    /// Update config
    UpdateConfig {
        vault_addr: Option<Addr>,
    },

    // add a new token to the list of allowed lockable tokens
    AddAllowedLockableToken {
        asset_info: AssetInfo
    },

    /// Allows the owner to transfer ownership to a new address.
    /// Ownership transfer is done in two steps:
    /// 1. The owner proposes a new owner.
    /// 2. The new owner accepts the ownership.
    /// The proposal expires after a certain period of time within which the new owner must accept the ownership.
    ProposeNewOwner {
        owner: Addr,
        expires_in: u64,
    },
    /// Allows the new owner to accept ownership.
    ClaimOwnership {},
    /// Allows the owner to drop the ownership transfer proposal.
    DropOwnershipProposal {}
}


#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Uint128)]
    TotalAmountLocked {
        user: Addr,
        asset_info: AssetInfo
    },
    
    #[returns(Config)]
    Config {}

}