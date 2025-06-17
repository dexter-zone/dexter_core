# Defunct Pool Implementation Plan

## Overview
This plan implements defunct pool functionality directly in the vault contract. A defunct pool is completely shut down (unlike paused which allows some operations) and users can be refunded their proportional share of pool assets.

## Architecture Decision
- ✅ **Add to Vault Contract** (vs new contract)
- Reasons: Direct access to pool state, atomic operations, simpler architecture

---

## Phase 1: Core Data Structures & Types

### Task 1.1: Add Defunct Pool Types to packages/dexter/src/vault.rs
- [x] Add `DefunctPoolInfo` struct
- [x] Add `RefundBatchEntry` struct  
- [x] Add new ExecuteMsg variants: `DefunctPool`, `ProcessRefundBatch`
- [x] Add new QueryMsg variants: `GetDefunctPoolInfo`, `IsUserRefunded`
- [x] **Test**: Verify types compile correctly

### Task 1.2: Add Storage Items to contracts/vault/src/state.rs
- [x] Add `DEFUNCT_POOLS: Map<Uint128, DefunctPoolInfo>`
- [x] Add `REFUNDED_USERS: Map<(Uint128, &str), bool>`
- [x] Update imports to include `DefunctPoolInfo`
- [x] **Test**: Verify storage compiles correctly

### Task 1.3: Add Error Types to contracts/vault/src/error.rs
- [x] Add `PoolAlreadyDefunct`
- [x] Add `PoolNotDefunct` 
- [x] Add `UserAlreadyRefunded`
- [x] Add `PoolHasActiveRewardSchedules`
- [x] Add `LpTokenBalanceMismatch`
- [x] Add `DefunctPoolOperationDisabled`
- [x] **Test**: Verify error types compile correctly

### Task 1.4: Add Temporary Stubs to contracts/vault/src/contract.rs
- [x] Add temporary match arms for `DefunctPool` and `ProcessRefundBatch` in execute function
- [x] Add temporary match arms for `GetDefunctPoolInfo` and `IsUserRefunded` in query function
- [x] Add `DefunctPoolInfo` to imports
- [x] **Test**: Verify entire contract compiles with temporary stubs

✅ **Phase 1 Complete** - All core data structures and types are implemented and tested

---

## Phase 2: Helper Functions & Validations

### Task 2.1: Add Defunct Check Helper Function
- [ ] Implement `check_pool_not_defunct(deps: &Deps, pool_id: Uint128)` in contract.rs
- [ ] Function should return `ContractError::PoolIsDefunct` if pool is defunct
- [ ] **Test**: Unit test for defunct check with defunct and active pools

### Task 2.2: Add Reward Schedule Validation Function  
- [ ] Implement `validate_no_active_reward_schedules(querier: &QuerierWrapper, lp_token: &Addr, current_time: u64)`
- [ ] Query multistaking contract for active reward schedules
- [ ] Return error if any active or future schedules found
- [ ] **Test**: Unit test with mock multistaking responses

### Task 2.3: Add LP Token Holdings Calculator
- [ ] Implement `calculate_user_lp_holdings(querier: &QuerierWrapper, lp_token: &Addr, user: &Addr, auto_stake_impl: &AutoStakeImpl)`
- [ ] Query direct LP balance from CW20
- [ ] Query bonded, locked, and unlocked amounts from multistaking
- [ ] Return `RefundBatchEntry` with all LP token states
- [ ] **Test**: Unit test with various LP token state combinations

### Task 2.4: Add Asset Share Calculator
- [ ] Implement `calculate_user_asset_share(defunct_info: &DefunctPoolInfo, user_lp_amount: Uint128)`
- [ ] Calculate proportional share of each pool asset
- [ ] Handle edge cases (zero LP supply, zero user LP)
- [ ] **Test**: Unit test with different LP amounts and pool compositions

---

## Phase 3: Core Defunct Pool Logic

### Task 3.1: Implement execute_defunct_pool Function
- [ ] Add function signature in contract.rs
- [ ] Validate sender is owner
- [ ] Load pool from ACTIVE_POOLS 
- [ ] Validate no active reward schedules
- [ ] Query LP token total supply
- [ ] Create DefunctPoolInfo struct
- [ ] Save to DEFUNCT_POOLS storage
- [ ] Remove from ACTIVE_POOLS storage (atomic operation)
- [ ] Return success response with events
- [ ] **Test**: Unit test for successful defunct operation
- [ ] **Test**: Unit test for unauthorized access
- [ ] **Test**: Unit test for non-existent pool
- [ ] **Test**: Unit test with active reward schedules

### Task 3.2: Implement execute_process_refund_batch Function
- [ ] Add function signature in contract.rs
- [ ] Validate sender is owner
- [ ] Load defunct pool info
- [ ] Iterate through user addresses
- [ ] Skip already refunded users
- [ ] Calculate user LP holdings (all states)
- [ ] Calculate user asset share
- [ ] Create transfer messages for assets
- [ ] Mark user as refunded
- [ ] Update total LP refunded counter
- [ ] Return response with transfer messages
- [ ] **Test**: Unit test for successful batch processing
- [ ] **Test**: Unit test skipping already refunded users
- [ ] **Test**: Unit test with zero LP holdings
- [ ] **Test**: Unit test with various asset combinations

---

## Phase 4: Integrate Defunct Checks into Existing Operations

### Task 4.1: Add Defunct Checks to execute_join_pool
- [ ] Add `check_pool_not_defunct(&deps.as_ref(), pool_id)?` at start of function
- [ ] **Test**: Unit test joining defunct pool (should fail)
- [ ] **Test**: Unit test joining active pool (should succeed)

### Task 4.2: Add Defunct Checks to execute_exit_pool  
- [ ] Add `check_pool_not_defunct(&deps.as_ref(), pool_id)?` at start of function
- [ ] **Test**: Unit test exiting defunct pool (should fail)
- [ ] **Test**: Unit test exiting active pool (should succeed)

### Task 4.3: Add Defunct Checks to execute_swap
- [ ] Add `check_pool_not_defunct(&deps.as_ref(), swap_request.pool_id)?` at start of function
- [ ] **Test**: Unit test swapping in defunct pool (should fail)
- [ ] **Test**: Unit test swapping in active pool (should succeed)

### Task 4.4: Add Defunct Checks to Pool Config Updates
- [ ] Add defunct checks to `execute_update_pool_config`
- [ ] Add defunct checks to `execute_update_pool_params`
- [ ] **Test**: Unit test updating defunct pool config (should fail)
- [ ] **Test**: Unit test updating active pool config (should succeed)

---

## Phase 5: Query Functions

### Task 5.1: Add query_defunct_pool_info Function
- [ ] Implement query function in contract.rs
- [ ] Load from DEFUNCT_POOLS storage
- [ ] Return Option<DefunctPoolInfo>
- [ ] **Test**: Unit test querying existing defunct pool
- [ ] **Test**: Unit test querying non-existent defunct pool

### Task 5.2: Add query_is_user_refunded Function
- [ ] Implement query function in contract.rs
- [ ] Check REFUNDED_USERS storage
- [ ] Return boolean
- [ ] **Test**: Unit test for refunded user
- [ ] **Test**: Unit test for non-refunded user

### Task 5.3: Update query Router in contract.rs
- [ ] Add new query message handlers to query() function
- [ ] **Test**: Integration test for all query functions

---

## Phase 6: Integration Tests

### Task 6.1: Create Defunct Pool Integration Test
- [ ] Create test file: `contracts/vault/tests/defunct_pool.rs`
- [ ] Test complete defunct flow:
  - Create pool with liquidity
  - Add some users with LP tokens  
  - Defunct the pool
  - Process refund batches
  - Verify users receive correct assets
- [ ] **Test**: End-to-end defunct pool scenario

### Task 6.2: Create Multistaking Integration Test
- [ ] Test defunct pool with multistaking:
  - Users have bonded LP tokens
  - Users have unbonding LP tokens
  - Users have unlocked but unclaimed LP tokens
  - Process refunds for all states
- [ ] **Test**: Complex multistaking refund scenario

### Task 6.3: Create Error Scenarios Test
- [ ] Test all error conditions:
  - Defunct pool with active rewards
  - Operations on defunct pools
  - Double refunds
  - Unauthorized access
- [ ] **Test**: Comprehensive error testing

---

## Phase 7: Documentation & Final Testing

### Task 7.1: Update Contract Documentation
- [ ] Update contracts/vault/README.md with new functionality
- [ ] Document new ExecuteMsg and QueryMsg variants
- [ ] Add examples of defunct pool usage
- [ ] **Review**: Documentation completeness

### Task 7.2: Add Schema Generation
- [ ] Ensure new types are included in schema generation
- [ ] Run `cargo schema` to update JSON schemas
- [ ] **Test**: Schema generation succeeds

### Task 7.3: Final Integration Testing
- [ ] Run all existing vault tests to ensure no regressions
- [ ] Run new defunct pool tests
- [ ] Test with different pool types (weighted, stable)
- [ ] **Test**: Full test suite passes

---

## Implementation Notes

### Key Files to Modify:
1. `packages/dexter/src/vault.rs` - Add types
2. `contracts/vault/src/state.rs` - Add storage
3. `contracts/vault/src/error.rs` - Add errors  
4. `contracts/vault/src/contract.rs` - Add functions
5. `contracts/vault/tests/defunct_pool.rs` - Add tests

### Critical Requirements:
- **Atomicity**: Defunct operation must be atomic (remove from ACTIVE_POOLS and add to DEFUNCT_POOLS)
- **Safety**: All existing operations must check defunct status
- **Accuracy**: LP token calculations must account for all states (direct, bonded, locked, unlocked)
- **Prevention**: Cannot defunct pools with active reward schedules

### Testing Strategy:
- **Unit Tests**: Test each function in isolation
- **Integration Tests**: Test complete workflows
- **Error Tests**: Test all error conditions
- **Regression Tests**: Ensure existing functionality unchanged

### Code Quality:
- Follow existing code patterns and style
- Add comprehensive documentation
- Use consistent error handling
- Include detailed events for indexing

---

## Progress Tracking

**Phase 1: Core Data Structures & Types**
- [x] Task 1.1: Add types to vault.rs
- [x] Task 1.2: Add storage items  
- [x] Task 1.3: Add error types
- [x] Task 1.4: Add temporary stubs

**Phase 2: Helper Functions & Validations**
- [ ] Task 2.1: Defunct check helper
- [ ] Task 2.2: Reward schedule validation
- [ ] Task 2.3: LP holdings calculator
- [ ] Task 2.4: Asset share calculator

**Phase 3: Core Defunct Pool Logic**
- [ ] Task 3.1: execute_defunct_pool
- [ ] Task 3.2: execute_process_refund_batch

**Phase 4: Integrate Defunct Checks**  
- [ ] Task 4.1: Join pool checks
- [ ] Task 4.2: Exit pool checks
- [ ] Task 4.3: Swap checks
- [ ] Task 4.4: Config update checks

**Phase 5: Query Functions**
- [ ] Task 5.1: query_defunct_pool_info
- [ ] Task 5.2: query_is_user_refunded  
- [ ] Task 5.3: Update query router

**Phase 6: Integration Tests**
- [ ] Task 6.1: Basic defunct flow test
- [ ] Task 6.2: Multistaking integration test
- [ ] Task 6.3: Error scenarios test

**Phase 7: Documentation & Final Testing**
- [ ] Task 7.1: Update documentation
- [ ] Task 7.2: Schema generation
- [ ] Task 7.3: Final integration testing

**Implementation Complete**: [ ]
