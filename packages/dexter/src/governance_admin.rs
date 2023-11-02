use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Binary, CosmosMsg, Uint128};

use crate::{
    asset::{Asset, AssetInfo},
    vault::{FeeInfo, NativeAssetPrecisionInfo, PoolType},
};

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
    pub reward_schedules: Option<Vec<RewardScheduleCreationRequest>>,
}

#[cw_serde]
#[derive(Copy)]
pub enum PoolCreationRequestStatus {
    PendingProposalCreation,
    ProposalCreated {
        proposal_id: u64,
    },
    PoolCreated {
        proposal_id: u64,
        pool_id: Uint128,
    },
    RequestFailedAndRefunded {
        proposal_id: u64,
        refund_block_height: u64,
    },
    RequestSuccessfulAndDepositRefunded {
        proposal_id: u64,
        refund_block_height: u64,
    },
}

impl PoolCreationRequestStatus {
    pub fn proposal_id(&self) -> Option<u64> {
        match self {
            PoolCreationRequestStatus::ProposalCreated { proposal_id } => Some(*proposal_id),
            PoolCreationRequestStatus::PoolCreated { proposal_id, .. } => Some(*proposal_id),
            PoolCreationRequestStatus::RequestFailedAndRefunded { proposal_id, .. } => {
                Some(*proposal_id)
            }
            PoolCreationRequestStatus::RequestSuccessfulAndDepositRefunded {
                proposal_id, ..
            } => Some(*proposal_id),
            _ => None,
        }
    }
}

#[cw_serde]
pub struct PoolCreateRequestContextData {
    pub status: PoolCreationRequestStatus,
    pub request_sender: Addr,
    pub total_funds_acquired_from_user: Vec<Asset>,
    pub user_deposits_detailed: Vec<UserDeposit>,
    pub pool_creation_request: PoolCreationRequest,
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
#[derive(Copy)]
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
    },
    RequestSuccessfulAndDepositRefunded {
        proposal_id: u64,
        refund_block_height: u64,
    },
}

impl RewardSchedulesCreationRequestStatus {
    pub fn proposal_id(&self) -> Option<u64> {
        match self {
            RewardSchedulesCreationRequestStatus::ProposalCreated { proposal_id } => {
                Some(*proposal_id)
            }
            RewardSchedulesCreationRequestStatus::RewardSchedulesCreated { proposal_id } => {
                *proposal_id
            }
            RewardSchedulesCreationRequestStatus::RequestFailedAndRefunded {
                proposal_id, ..
            } => Some(*proposal_id),
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
    pub reward_schedule_creation_requests: Vec<RewardScheduleCreationRequest>,
}

#[cw_serde]
pub struct GovernanceProposalDescription {
    pub title: String,
    pub metadata: String,
    pub summary: String,
}

#[cw_serde]
pub enum GovAdminProposalRequestType {
    PoolCreationRequest { request_id: u64 },
    RewardSchedulesCreationRequest { request_id: u64 },
}

#[cw_serde]
pub enum FundsCategory {
    PoolCreationFee,
    ProposalDeposit,
    PoolBootstrappingAmount,
    RewardScheduleAmount,
}

#[cw_serde]
pub struct UserDeposit {
    pub category: FundsCategory,
    pub assets: Vec<Asset>,
}

#[cw_serde]
pub enum RefundReason {
    ProposalPassedDepositRefund,
    ProposalRejectedFullRefund,
    ProposalFailedFullRefund,
}

#[cw_serde]
pub struct RefundResponse {
    pub refund_reason: RefundReason,
    pub refund_receiver: Addr,
    pub refund_amount: Vec<Asset>,
    pub detailed_refund_amount: Vec<UserDeposit>,
}

#[cw_serde]
pub struct UserTotalDeposit {
    pub total_deposit: Vec<Asset>,
    pub deposit_breakdown: Vec<UserDeposit>
}

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    // User executable
    /// Creates a proposal to create a pool
    /// The proposal is created by the governance admin contract on behalf of the user to enable easy accounting of funds for pool creation
    /// Pool creation follows the following steps:
    /// 1. User calls this contract with a pool creation request and required funds and(or) approval to spend funds in case of CW20 tokens
    /// 2. This contract verifies the funds, and transfers the funds to this contract in case of CW20 tokens. The custody of the funds is transferred to the governance admin contract.
    /// 3. This contract stores the pool creation request in its state.
    /// 3. Then, this contract creates a proposal to resume the pool creation process, which returns a callback to itself with the pool creation request id.
    /// 4. If the proposal is passed, governance module of the chain will call the callback with the pool creation request id.
    /// 5. This contract will then resume the pool creation process and create the pool in the vault contract.
    /// 6. If specified, it will also bootstrap the pool with the bootstrapping amount.
    /// 7. If specified, it will also create the reward schedules for the pool in the multi-staking contract.
    /// 8. If the pool creation fails or if the proposal is rejected, the user can request all the funds back by executing the `ClaimRefund` message.
    /// 9. If the pool creation is successful, the user can request Proposal Deposit amount by the same `ClaimRefund` message.
    CreatePoolCreationProposal {
        proposal_description: GovernanceProposalDescription,
        pool_creation_request: PoolCreationRequest,
    },

    /// Creates a proposal to add one or more new reward schedule(s) to an existing pool
    /// The proposal is created by the governance admin contract on behalf of the user to enable easy accounting of funds for reward schedule creation
    /// Reward schedule creation follows the following steps:
    /// 1. User calls this contract with a reward schedule creation request and required funds and(or) approval to spend funds in case of CW20 tokens
    /// 2. This contract verifies the funds, and transfers the funds to this contract in case of CW20 tokens. The custody of the funds is transferred to the governance admin contract.
    /// 3. This contract stores the reward schedule creation request in its state.
    /// 3. Then, this contract creates a proposal to resume the reward schedule creation process, which returns a callback to itself with the reward schedule creation request id.
    /// 4. If the proposal is passed, governance module of the chain will call the callback with the reward schedule creation request id.
    /// 5. This contract will then resume the reward schedule creation process and create the reward schedule(s) in the multi-staking contract.
    /// 8. If the pool creation fails or if the proposal is rejected, the user can request all the funds back by executing the `ClaimRefund` message.
    /// 9. If the pool creation is successful, the user can request Proposal Deposit amount by the same `ClaimRefund` message.
    CreateRewardSchedulesProposal {
        proposal_description: GovernanceProposalDescription,
        multistaking_contract_addr: String,
        reward_schedule_creation_requests: Vec<RewardScheduleCreationRequest>,
    },

    /// Claims the refundable funds from the governance admin contract.
    /// Will return an error if the valid funds are already refunded to the user.
    ClaimRefund {
        request_type: GovAdminProposalRequestType,
    },

    // Gov executable
    /// Execute any messages on behalf of the governance admin contract. This is useful for configuration updates using the Governance Admin contract.
    /// As governance admin contract is designed to become owner of Vault and Multi-staking contracts, any configuration change on them can be done using this message
    /// This message can be executed only by the Governace module of the chain i.e. it is fully chain governed.
    ExecuteMsgs { msgs: Vec<CosmosMsg> },

    /// Resumes the pool creation process after the proposal is passed.
    /// This message is called by the governance module of the chain as an action of the proposal created by the `CreatePoolCreationProposal` message.
    /// This message can only be called by the governance module of the chain.
    ResumeCreatePool { pool_creation_request_id: u64 },

    /// Resumes the reward schedule creation process.
    /// This message is called by the governance module of the chain as an action of the proposal created by the `CreateRewardSchedulesProposal` message.
    /// This message can also be called by the contract itself after a new pool with reward schedules is created using the `ResumeCreatePool` message.
    ResumeCreateRewardSchedules {
        reward_schedules_creation_request_id: u64,
    },

    // Self executable
    /// Callback message to store the proposal id after the proposal is created.
    /// This message is ordered by the `CreatePoolCreationProposal` message after the `SubmitProposal` message, so it is executed right after the proposal is created in the same transaction.
    /// This message is executed by the governance admin contract itself and no-one else can execute this message.
    PostGovernanceProposalCreationCallback {
        gov_proposal_type: GovAdminProposalRequestType,
    },

    /// Callback message after the pool is created in the vault.
    /// This message is ordered by the `ResumeCreatePool` message after the `CreatePool` message, so it is executed right after the pool is created in the same transaction.
    /// This message is executed by the governance admin contract itself and no-one else can execute this message.
    /// We trigger the pool join functionality and store pool specific data in the Gov admin contract state in this message.
    ResumeJoinPool { pool_creation_request_id: u64 },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns the state of the pool creation request
    #[returns(PoolCreateRequestContextData)]
    PoolCreationRequest { pool_creation_request_id: u64 },

    /// Returns the state of the reward schedule creation request
    #[returns(RewardScheduleCreationRequestsState)]
    RewardScheduleRequest { reward_schedule_request_id: u64 },

    #[returns(UserTotalDeposit)]
    FundsForPoolCreation { request: PoolCreationRequest },

    /// Returns the refundable funds for the user.
    /// It provides total refund and also a breakdown of the refundable funds so that the user can understand the reason for the refund
    /// and the calculation involved in the refund.
    #[returns(RefundResponse)]
    RefundableFunds {
        request_type: GovAdminProposalRequestType,
    },
}
