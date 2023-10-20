# Dexter - Governance admin Contract

Dexter Governance Admin contract assumed the `owner` role of the Vault and the Multistaking contract.
Actions in the governance admin contract can be triggered by Chain governance making Vault and Multistaking contract entirely governed by Chain's native Gov module.

## Supported state transition functions

### User executable

Following transition functions can be executed by any user.

#### 1. _**CreatePoolCreationProposal**_

Used to submit pool creation request, which will create a governance proposal and when passed the pool will be created by `ResumeCreatePool`.

- _Parameters_:

  - <a name="pd"></a>**proposal_description**: provide proposal title, metadata & summary here.

      ```rs
      GovernanceProposalDescription {
          title: String,
          metadata: String,
          summary: String,
      }
      ```

  - **pool_creation_request**: this contains vault contract address & params for `vault::CreatePoolInstance` msg. Optionally you can provide bootstrap liquidity amount and schedule rewards along with pool creation.

      ```rs
      PoolCreationRequest {
          // vault contract address
          vault_addr: String,

          // vault::CreatePoolInstance params
          pool_type: PoolType,
          fee_info: Option<FeeInfo>,
          native_asset_precisions: Vec<NativeAssetPrecisionInfo>,
          asset_info: Vec<AssetInfo>,
          init_params: Option<Binary>,

          // Optional fields depending on the fact if user wants to bootstrap liquidty to the pool
          bootstrapping_amount: Option<Vec<Asset>>,
          // this address will be the owner of the bootsrapping liquidity
          bootstrapping_liquidity_owner: String,

          // Optional field to specify if the user wants to create reward schedule(s) for this pool
          reward_schedules: Option<Vec<RewardScheduleCreationRequest>>
      }
      ```

- _Execution Flow_:

  - Validates provided `asset_info`, precision must be specified in `native_asset_precisions` for every native asset provided.
  - Validates if bootstrap amount (if present) includes all the assets of the pool.
  - Validate reward schedules inputs (if present).
  - Validates if total deposited funds match the proposal deposit amount + bootstrap amount + pool creation fee + reward schedules amount (if present).
  - Saves pool creation request with status `PendingProposalCreation`.
  - Governance proposal is submitted to executed `ResumeCreatePool` msg with pool creation request id.
  - `PostGovernanceProposalCreationCallback` msg is executed, where the status of pool creation request is set to `ProposalCreated`.

- _Events_:

  - Event: `dexter-governance-admin::create_pool_creation_proposal`  
    Attributes: `pool_creation_request_id`

#### 2. _**CreateRewardSchedulesProposal**_

Used to submit reward schedules request, which will create a governance proposal and when passed the reward schedules will be created by `ResumeCreateRewardSchedules`.

- _Parameters_:

  - **proposal_description**: Same as [above](#pd).

  - **multistaking_contract_addr**: multistaking contract address as `String`.

  - **reward_schedule_creation_requests**: Accepts array of reward schedules.

      ```rs
      reward_schedule_creation_requests: Vec<RewardScheduleCreationRequest>

      RewardScheduleCreationRequest {
          /// This is null when it is being used within a new pool creation request
          /// This is not null when it is being used as a reward schedule creation request
          lp_token_addr: Option<Addr>,
          title: String,
          asset: AssetInfo,
          amount: Uint128,
          start_block_time: u64,
          end_block_time: u64,
      }
      ```

- _Execution Flow_:

  - Validates provided `lp_token_addr` is allowed in multistaking contract.
  - Validate reward schedules inputs.
  - Validates if total deposited funds match the proposal deposit amount + reward schedules amount.
  - Saves reward schedule creation request with status `PendingProposalCreation`.
  - Governance proposal is submitted to executed `ResumeCreateRewardSchedules` msg with pool creation request id.
  - `PostGovernanceProposalCreationCallback` msg is executed, where the status of reward schedules creation request is set to `ProposalCreated`.

- _Events_:

  - Event: `dexter-governance-admin::create_reward_schedule_proposal`  
    Attributes: `reward_schedules_creation_request_id`

#### 3. _**ClaimRefund**_

Used to claim funds submitted in either `CreatePoolCreationProposal` or `CreateRewardSchedulesProposal` msg.
Claims the proposal deposit amount or the total deposited funds in case proposal is rejected / failed.

- _Parameters_:

  - **request_type**: type of request for which to claim funds, and request_id for the same.

    ```rs
    enum GovAdminProposalRequestType {
      PoolCreationRequest {
          request_id: u64,
      },
      RewardSchedulesCreationRequest {
          request_id: u64,
      },
    }
    ```

- _Execution Flow_: -- TODO --

- _Events_: -- None --

### Gov executable

Following transition functions can only be executed via governance.

#### 1. _**ExecuteMsgs**_

- _Parameters_: -- TOOD --

- _Execution Flow_: -- TODO --

- _Events_: -- TODO --

#### 2. _**ResumeCreatePool**_

This is executed by gov module once the proposal is passed for pool creation request.

- _Parameters_:

  - **pool_creation_request_id**: Pool creation request id as `u64`.

- _Execution Flow_:

  - Executes `CreatePoolInstance` on vault contract with the requested parameters.
  - Executes `ResumeJoinPool` where the bootstrap amount is deposited in the pool.

- _Events_:

  - Event: `dexter-governance-admin::resume_create_pool`  
    Attributes: `pool_creation_request_id`

#### 3. _**ResumeCreateRewardSchedules**_

This is executed by gov module once the proposal is passed for reward schedules creation request.

- _Parameters_:

  - **reward_schedules_creation_request_id**: Reward schedules creation request id as `u64`.

- _Execution Flow_:

  - Updates status of reward schedules creation request to `RewardSchedulesCreated`.
  - Executes `CreateRewardSchedule` on multistaking contract with the requested parameters.

- _Events_: -- None --

### Self executable

Following functions are only executable by the contract itself.

#### 1. _**PostGovernanceProposalCreationCallback**_

This is executed by gov admin contract when the proposal is submitted from `CreatePoolCreationProposal` or `CreateRewardSchedulesProposal`.

- _Parameters_:

  - **gov_proposal_type**: request type for which the proposal has been submitted.
  
    ```rs
    enum GovAdminProposalRequestType {
      PoolCreationRequest {
          request_id: u64,
      },
      RewardSchedulesCreationRequest {
          request_id: u64,
      },
    }
    ```

- _Execution Flow_:

  - Fetches proposal id of the latest proposal submitted.
  - Sanity check for request id with proposal content.
  - Status of the request is updated to `ProposalCreated` with the proposal id.

- _Events_:

  - If param `gov_proposal_type` is `PoolCreationRequest`
    Event: `dexter-governance-admin::post_governance_proposal_creation_callback`  
    Attributes: `pool_creation_request_id`, `proposal_id`
  
  - If param `gov_proposal_type` is `RewardSchedulesCreationRequest`
    Event: `dexter-governance-admin::post_governance_proposal_creation_callback`  
    Attributes: `reward_schedules_creation_request_id`, `proposal_id`

#### 2. _**ResumeJoinPool**_

This is executed by gov admin contract when pool is created from `ResumeCreatePool` msg.
Here the bootrapping amount is deposited in the pool.

- _Parameters_:

  - **pool_creation_request_id**: Pool creation request id as `u64`.

- _Execution Flow_:

  - Fetches pool id & info from vault contract.
  - Updates request status to `PoolCreated`.
  - Executes `JoinPool` on vault contract with requested bootstrap amount.
  - Registers lp token in multistaking contract.
  - If requested, submits a request to create reward schedules, which follows the flow of `CreateRewardSchedulesProposal`.

- _Events_:

  - Event: `dexter-governance-admin::resume_join_pool`  
    Attributes: `pool_creation_request_id`, `pool_id`
