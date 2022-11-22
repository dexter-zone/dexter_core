use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw20::Cw20ReceiveMsg;

use crate::asset::AssetInfo;

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: Addr,
}

#[cw_serde]
pub struct AssetRewardState {
    pub reward_index: Decimal,
    pub last_distributed: u64,
    pub total_bond_amount: Uint128,
}

#[cw_serde]
pub struct State {
    pub next_factory_id: u64,
}

#[cw_serde]
pub struct UnclaimedReward {
    pub asset: AssetInfo,
    pub amount: Uint128,
}

#[cw_serde]
pub struct RewardSchedule {
    pub asset: AssetInfo,
    pub amount: Uint128,
    pub staking_lp_token: Addr,
    pub start_block_time: u64,
    pub end_block_time: u64,
}

#[cw_serde]
pub struct Config {
    // TODO: check if we need this
    pub allowed_lp_tokens: Vec<Addr>,
    // Admin has privilege to add remove allowed lp tokens for reward
    pub admin: Addr,
}

#[cw_serde]
pub struct AssetStakerInfo {
    pub asset: AssetInfo,
    pub reward_index: Decimal,
    pub bond_amount: Uint128,
    pub pending_reward: Uint128,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Vec<UnclaimedReward>)]
    UnclaimedRewards {
        lp_token: Addr,
        user: Addr,
        block_time: Option<u64>,
    },
    #[returns(Vec<Addr>)]
    AllowedLPTokensForReward {},
    #[returns(Addr)]
    Admin {},
    #[returns(Vec<RewardSchedule>)]
    RewardSchedules { lp_token: Addr, asset: AssetInfo },
}

#[cw_serde]
pub enum Cw20HookMsg {
    Bond {},
    AddRewardFactory {
        lp_token: Addr,
        start_block_time: u64,
        end_block_time: u64,
    },
}

#[cw_serde]
pub enum ExecuteMsg {
    UpdateAdmin {
        new_admin: Addr,
    },
    AddRewardFactory {
        lp_token: Addr,
        denom: String,
        amount: Uint128,
        start_block_time: u64,
        end_block_time: u64,
    },
    AllowLpToken {
        lp_token: Addr,
    },
    RemoveLpToken {
        lp_token: Addr,
    },
    Receive(Cw20ReceiveMsg),
    Bond {
        lp_token: Addr,
        amount: Uint128,
    },
    Unbond {
        lp_token: Addr,
        amount: Uint128,
    },
    Withdraw {
        lp_token: Addr,
    },
}
