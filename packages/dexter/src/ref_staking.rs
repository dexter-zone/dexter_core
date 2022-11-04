use cosmwasm_schema::{cw_serde, QueryResponses};

use cosmwasm_std::{Decimal, Uint128};
use cw20::Cw20ReceiveMsg;

// ----------------x----------------x----------------x----------------x----------------x----------------
// Copied from Anchor protocol's Staking contract interface here -
// ----------------x----------------x----------------x----------------x----------------x----------------

#[cw_serde]
pub struct InstantiateMsg {
    pub anchor_token: String,
    pub staking_token: String, // lp token of ANC-UST pair contract
    pub distribution_schedule: Vec<(u64, u64, Uint128)>,
}

#[cw_serde]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    Unbond {
        amount: Uint128,
    },
    /// Withdraw pending rewards
    Withdraw {},
}

#[cw_serde]
pub enum Cw20HookMsg {
    Bond {},
}

/// We currently take no arguments for migrations
#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(StateResponse)]
    State {
        block_time: Option<u64>,
    },
    #[returns(StakerInfoResponse)]
    StakerInfo {
        staker: String,
        block_time: Option<u64>,
    },
}

// We define a custom struct for each query response
#[cw_serde]
pub struct ConfigResponse {
    pub anchor_token: String,
    pub staking_token: String,
    pub distribution_schedule: Vec<(u64, u64, Uint128)>,
}

// We define a custom struct for each query response
#[cw_serde]
pub struct StateResponse {
    pub last_distributed: u64,
    pub total_bond_amount: Uint128,
    pub global_reward_index: Decimal,
}

// We define a custom struct for each query response
#[cw_serde]
pub struct StakerInfoResponse {
    pub staker: String,
    pub reward_index: Decimal,
    pub bond_amount: Uint128,
    pub pending_reward: Uint128,
}
