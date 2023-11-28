use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};

use crate::asset::Asset;

#[cw_serde]
pub struct InstantiateMsg {
    /// lock period for any LST token that is locked for the user
    base_lock_period: u64,
}


#[cw_serde]
pub enum ExecuteMsg {

    /// Automatically starts a time-lock for the user for a defined period.
    /// In that time period, the user can only join pool using the locked LST and not withdraw it.
    /// After the completion of the time-lock, the user can withdraw the LST normally also.
    /// This message can only be executed by the whitelisted LST issuance modules like lscosmos module on the Persistence chain.
    LockLstAssetForUser {
        asset: Asset,
        user: Addr
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
}


#[cw_serde]
pub enum QueryMsg {
    
}