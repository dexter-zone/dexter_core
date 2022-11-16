use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Uint128, Decimal, Addr};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;

use crate::asset::AssetInfo;


#[cw_serde]
pub struct InstantiateMsg {
    
}

#[cw_serde]
pub enum ExecuteMsg {
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
    Receive(Cw20ReceiveMsg),
    Unbond {
        lp_token: Addr,
        amount: Uint128,
    },
    Withdraw {
        lp_token: Addr,
    },
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
pub enum Cw20HookMsg {
    Bond {},
    AddRewardFactory {
        lp_token: Addr,
        start_block_time: u64,
        end_block_time: u64,
    },
}

#[cw_serde]
pub struct UnclaimedReward {
    pub asset: AssetInfo,
    pub amount: Uint128,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Vec<UnclaimedReward>)]
    UnclaimedRewards {
        lp_token: Addr,
        user: Addr
    }
}