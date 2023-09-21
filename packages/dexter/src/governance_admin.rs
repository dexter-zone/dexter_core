use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Binary, CosmosMsg, Uint128, Addr};

use crate::{vault::{PoolType, NativeAssetPrecisionInfo, FeeInfo}, asset::{AssetInfo, Asset}, multi_staking::RewardSchedule};

#[cw_serde]
pub struct InstantiateMsg {}


#[cw_serde]
pub struct PoolCreationRequest {
   pub vault_addr: String,
   pub pool_type: PoolType,
   pub fee_info: Option<FeeInfo>,
   pub native_asset_precisions: Vec<NativeAssetPrecisionInfo>,
   pub asset_info: Vec<AssetInfo>,
   pub init_params: Option<Binary>,
   // this address will be the owner of the bootsrapping liquidity
   pub bootstrapping_liquidity_owner: String,
   // Optional fields depending on the fact if user wants to bootstrap liquidty to the pool
   pub bootstrapping_amount: Option<Vec<Asset>>,
   // Optional field to specify if the user wants to create reward schedule(s) for this pool
   pub reward_schedules: Option<Vec<RewardSchedule>>
}

#[cw_serde]
pub enum ExecuteMsg {

   ExecuteMsgs {
        msgs: Vec<CosmosMsg>
   },

   CreatePoolCreationProposal {
      title: String,
      description: String,
      pool_creation_request: PoolCreationRequest,
   },

   PostGovernanceProposalCreationCallback {
      pool_creation_request_id: u64,
   },

   ResumeCreatePool {
      pool_creation_request_id: u64,
   },

   ResumeJoinPool {
      pool_creation_request_id: u64,
   },

   CreateRewardSchedulesProposal {
      title: String,
      description: String,
      reward_schedules: Vec<RewardSchedule>,
   },

   PostRewardSchedulesProposalCreationCallback {
      reward_schedules_creation_request_id: u64,
   },

   ResumeCreateRewardSchedules {
      reward_schedules_creation_request_id: u64,
   },

}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {

}