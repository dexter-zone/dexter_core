use std::collections::HashSet;

use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Binary, CosmosMsg, Uint128, Addr, StdError};

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
   pub reward_schedules: Option<Vec<RewardScheduleCreationRequest>>
}


#[cw_serde]
pub struct RewardScheduleCreationRequest {
   /// This is null when it is being used within a new pool creation request
   /// This is not null when it is being used as a reward schedule creation request
   pub lp_token_addr: Option<Addr>,
   pub title: String,
   pub asset: AssetInfo,
   pub amount: Uint128,
   pub start_block_time: u64,
   pub end_block_time: u64,
}

#[cw_serde]
pub enum RewardSchedulesCreationRequestStatus {
   PendingProposalCreation,
   NonProposalRewardSchedule,
   ProposalCreated {
      proposal_id: u64,
   },
   RewardSchedulesCreated {
      proposal_id: Option<u64>,
   },
   RequestFailedAndRefunded {
      proposal_id: u64,
      refund_block_height: u64,
   }
}

impl RewardSchedulesCreationRequestStatus {

   pub fn proposal_id(&self) -> Option<u64> {
      match self {
         RewardSchedulesCreationRequestStatus::ProposalCreated { proposal_id } => Some(*proposal_id),
         RewardSchedulesCreationRequestStatus::RewardSchedulesCreated { proposal_id } => *proposal_id,
         RewardSchedulesCreationRequestStatus::RequestFailedAndRefunded { proposal_id, .. } => Some(*proposal_id),
         _ => None,
      }
   }
}

#[cw_serde]
pub struct RewardScheduleCreationRequestsState {
   pub multistaking_contract_addr: Addr,
   pub status: RewardSchedulesCreationRequestStatus,
   pub request_sender: Addr,
   /// this field is only set if the request is linked to a governance proposal
   pub total_funds_acquired_from_user: Vec<Asset>,
   pub user_deposits_detailed: Vec<UserDeposit>,
   pub reward_schedule_creation_requests: Vec<RewardScheduleCreationRequest>
}

#[cw_serde]
pub struct GovernanceProposalDescription {
   pub title: String,
   pub metadata: String,
   pub summary: String,
}

#[cw_serde]
pub enum GovAdminProposalType {
   PoolCreationRequest {
      request_id: u64,
   },
   RewardSchedulesCreationRequest {
      request_id: u64,
   },
}

#[cw_serde]
pub enum ExecuteMsg {

   // User executable
   CreatePoolCreationProposal {
      proposal_description: GovernanceProposalDescription,
      pool_creation_request: PoolCreationRequest,
   },

   CreateRewardSchedulesProposal {
      proposal_description: GovernanceProposalDescription,
      multistaking_contract_addr: String,
      reward_schedule_creation_requests: Vec<RewardScheduleCreationRequest>,
   },

   ClaimFailedCreatePoolProposalFunds {
      pool_creation_request_id: u64,
   },

   ClaimFailedRewardScheduleProposalFunds {
      reward_schedule_creation_request_id: u64,
   },

   // Gov executable
   ExecuteMsgs {
      msgs: Vec<CosmosMsg>
   },

   ResumeCreatePool {
      pool_creation_request_id: u64,
   },

   ResumeCreateRewardSchedules {
      reward_schedules_creation_request_id: u64,
   },

   // Self executable
   PostGovernanceProposalCreationCallback {
      gov_proposal_type: GovAdminProposalType,
   },

   ResumeJoinPool {
      pool_creation_request_id: u64,
   }
}


#[cw_serde]
pub enum FundsCategory {
   PoolCreationFee,
   ProposalDeposit,
   PoolBootstrappingAmount,
   RewardScheduleAmount
}

#[cw_serde]
pub struct UserDeposit {
   pub category: FundsCategory,
   pub assets: Vec<Asset>,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {

   #[returns(PoolCreationRequest)]
   PoolCreationRequest { pool_creation_request_id: u64 },
   
   #[returns(Vec<RewardSchedule>)]
   RewardScheduleRequest { reward_schedule_request_id: u64 },

   #[returns(Uint128)]
   PoolCreationRequestProposalId { pool_creation_request_id: u64 },
   
   #[returns(Uint128)]
   RewardScheduleRequestProposalId { reward_schedule_request_id: u64 },

   #[returns(Vec<UserDeposit>)]
   RefundableFunds { pool_creation_request_id: u64 },

}