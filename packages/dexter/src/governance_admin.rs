use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Coin, Binary, CosmosMsg, Uint128};

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
      proposal_creation_request_id: Uint128,
   },

   ResumeCreatePool {
      pool_creation_request_id: Uint128,
   },

   ResumeJoinPool {
      pool_creation_request_id: Uint128,
   },

   // Create new pool with funds
   CreateNewPool {
      vault_addr: String,
      bootstrapping_amount_payer: String,
      pool_type: PoolType,
      fee_info: Option<FeeInfo>,
      native_asset_precisions: Vec<NativeAssetPrecisionInfo>,
      assets: Vec<Asset>,
      init_params: Option<Binary>
   },

}

#[cw_serde]
pub enum QueryMsg {}