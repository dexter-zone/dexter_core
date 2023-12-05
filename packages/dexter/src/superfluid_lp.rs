use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};

use crate::asset::{Asset, AssetInfo};

#[cw_serde]
pub struct InstantiateMsg {
    pub vault_addr: Addr,
    /// lock period for any LST token that is locked for the user
    pub base_lock_period: u64,
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
    pub base_lock_period: u64,
    pub vault_addr: Addr,
    pub owner: Addr,
}


#[cw_serde]
pub enum ExecuteMsg {

    /// Automatically starts a time-lock for the user for a defined period.
    /// In that time period, the user can only join pool using the locked LST and not withdraw it.
    /// After the completion of the time-lock, the user can withdraw the LST normally also.
    /// This message can only be executed by the whitelisted LST issuance modules like lscosmos module on the Persistence chain.
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

    /// Available after the base lock period on the LST is over
    DirectlyUnlockBaseLst {
        asset: Asset
    },

    /// Update config
    UpdateConfig {
        base_lock_period: Option<u64>,
        vault_addr: Option<Addr>,
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
    
    /// Returns the amount of LST that is currently unlocked for the user and available to withdraw.
    /// This is amount that has served the base lock period in this contract.
    #[returns(Uint128)]
    UnlockedAmount {
        user: Addr,
        asset_info: AssetInfo
    },

    #[returns(Vec<LockInfo>)]
    TokenLocks {
        user: Addr,
        asset_info: AssetInfo
    }

}