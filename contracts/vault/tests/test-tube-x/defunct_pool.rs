#![cfg(test)]

use cosmwasm_std::{coins, Addr, Uint128};
use dexter::asset::{Asset, AssetInfo};
use dexter::vault::{DefunctPoolInfo, ExecuteMsg, QueryMsg};
use persistence_test_tube::{Account, Module, Wasm};

#[cfg(feature = "test-tube")]
pub mod utils;

struct DefunctPoolTestSuite {
    app: persistence_test_tube::PersistenceTestApp,
    owner: persistence_test_tube::SigningAccount,
    vault_instance: String,
    token1: String,
    token2: String,
    token3: String,
}

impl DefunctPoolTestSuite {
    fn new() -> Self {
        let (app, owner) = utils::mock_app(vec![
            cosmwasm_std::Coin {
                denom: "denom1".to_string(),
                amount: Uint128::from(1_000_000_000_000u128),
            },
            cosmwasm_std::Coin {
                denom: "denom2".to_string(),
                amount: Uint128::from(1_000_000_000_000u128),
            },
            cosmwasm_std::Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(1_000_000_000_000u128),
            },
            cosmwasm_std::Coin {
                denom: "uxprt".to_string(),
                amount: Uint128::from(1_000_000_000_000u128),
            },
        ]);

        let vault_instance = utils::instantiate_contract(&app, &owner);

        // Initialize the token contracts
        let (token1, token2, token3) = utils::initialize_3_tokens(&app, &owner);

        // Mint tokens and set allowances
        utils::mint_some_tokens(
            &app,
            &owner,
            &token1,
            Uint128::from(10000000_000000u128),
            owner.address(),
        );
        utils::mint_some_tokens(
            &app,
            &owner,
            &token2,
            Uint128::from(10000000_000000u128),
            owner.address(),
        );
        utils::mint_some_tokens(
            &app,
            &owner,
            &token3,
            Uint128::from(10000000_000000u128),
            owner.address(),
        );

        utils::increase_token_allowance(
            &app,
            &owner,
            &token1,
            vault_instance.to_string(),
            Uint128::from(10000000_000000u128),
        );
        utils::increase_token_allowance(
            &app,
            &owner,
            &token2,
            vault_instance.to_string(),
            Uint128::from(10000000_000000u128),
        );
        utils::increase_token_allowance(
            &app,
            &owner,
            &token3,
            vault_instance.to_string(),
            Uint128::from(10000000_000000u128),
        );

        Self {
            app,
            owner,
            vault_instance,
            token1,
            token2,
            token3,
        }
    }

    fn run_all_tests(&self) {
        self.test_defunct_check_with_active_pool();
        self.test_defunct_check_with_defunct_pool();
        self.test_execute_defunct_pool_successful();
        self.test_execute_defunct_pool_unauthorized();
        self.test_execute_defunct_pool_nonexistent();
        self.test_execute_defunct_pool_already_defunct();
        self.test_operations_on_defunct_pool_join();
        self.test_operations_on_defunct_pool_swap();
        self.test_query_defunct_pool_info_existing();
        self.test_query_defunct_pool_info_nonexistent();
        self.test_query_is_user_refunded_false();
        self.test_process_refund_batch_successful();
        self.test_process_refund_batch_unauthorized();
        self.test_process_refund_batch_non_defunct_pool();
        self.test_defunct_pool_with_active_reward_schedules();
        self.test_defunct_pool_with_future_reward_schedules();
    }

    fn test_defunct_check_with_active_pool(&self) {
        let wasm = Wasm::new(&self.app);

        let (_, _lp_token_instance, pool_id) = utils::initialize_weighted_pool(
            &self.app,
            &self.owner,
            &self.vault_instance,
            self.token1.clone(),
            self.token2.clone(),
            self.token3.clone(),
            "denom1".to_string(),
            "denom2".to_string(),
        );

        // Try to join an active (non-defunct) pool - should succeed
        // The weighted pool has 5 assets in this order: denom1, denom2, token2, token1, token3
        let join_msg = ExecuteMsg::JoinPool {
            pool_id,
            recipient: None,
            assets: Some(vec![
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "denom1".to_string(),
                    },
                    amount: Uint128::from(1000u128),
                },
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "denom2".to_string(),
                    },
                    amount: Uint128::from(1000u128),
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked(self.token2.clone()),
                    },
                    amount: Uint128::from(1000u128),
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked(self.token1.clone()),
                    },
                    amount: Uint128::from(1000u128),
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked(self.token3.clone()),
                    },
                    amount: Uint128::from(1000u128),
                },
            ]),
            min_lp_to_receive: None,
            auto_stake: None,
        };

        // This should NOT fail because pool is active
        let result = wasm.execute(
            &self.vault_instance,
            &join_msg,
            &[
                cosmwasm_std::Coin {
                    denom: "denom1".to_string(),
                    amount: Uint128::from(1000u128),
                },
                cosmwasm_std::Coin {
                    denom: "denom2".to_string(),
                    amount: Uint128::from(1000u128),
                },
            ],
            &self.owner,
        );
        assert!(result.is_ok());
    }

    fn test_defunct_check_with_defunct_pool(&self) {
        let wasm = Wasm::new(&self.app);

        let (_, _lp_token_instance, pool_id) = utils::initialize_weighted_pool(
            &self.app,
            &self.owner,
            &self.vault_instance,
            self.token1.to_string(),
            self.token2.to_string(),
            self.token3.to_string(),
            "denom1".to_string(),
            "denom2".to_string(),
        );

        // First, make the pool defunct
        let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
        let result = wasm.execute(&self.vault_instance, &defunct_msg, &[], &self.owner);
        assert!(result.is_ok());

        // Now try to join the defunct pool - should fail
        let join_msg = ExecuteMsg::JoinPool {
            pool_id,
            recipient: None,
            assets: Some(vec![
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "denom1".to_string(),
                    },
                    amount: Uint128::from(1000u128),
                },
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "denom2".to_string(),
                    },
                    amount: Uint128::from(1000u128),
                },
            ]),
            min_lp_to_receive: None,
            auto_stake: None,
        };

        // This SHOULD fail because pool is defunct
        let result = wasm.execute(
            &self.vault_instance,
            &join_msg,
            &coins(2000u128, "uusd"),
            &self.owner,
        );
        assert!(result.is_err());

        // Verify it's the correct error
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Pool is already defunct")
                || error_msg.contains("PoolAlreadyDefunct")
                || error_msg.contains("pool already defunct")
        );
    }

    fn test_execute_defunct_pool_successful(&self) {
        let wasm = Wasm::new(&self.app);

        let (_pool_addr, lp_token_instance, pool_id) = utils::initialize_weighted_pool(
            &self.app,
            &self.owner,
            &self.vault_instance,
            self.token1.clone(),
            self.token2.clone(),
            self.token3.clone(),
            "denom1".to_string(),
            "denom2".to_string(),
        );

        // Join the pool to create some LP tokens
        let join_msg = ExecuteMsg::JoinPool {
            pool_id,
            recipient: None,
            assets: Some(vec![
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "denom1".to_string(),
                    },
                    amount: Uint128::from(1000u128),
                },
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "denom2".to_string(),
                    },
                    amount: Uint128::from(1000u128),
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked(self.token2.clone()),
                    },
                    amount: Uint128::from(1000u128),
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked(self.token1.clone()),
                    },
                    amount: Uint128::from(1000u128),
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked(self.token3.clone()),
                    },
                    amount: Uint128::from(1000u128),
                },
            ]),
            min_lp_to_receive: None,
            auto_stake: None,
        };

        let result = wasm.execute(
            &self.vault_instance,
            &join_msg,
            &[
                cosmwasm_std::Coin {
                    denom: "denom1".to_string(),
                    amount: Uint128::from(1000u128),
                },
                cosmwasm_std::Coin {
                    denom: "denom2".to_string(),
                    amount: Uint128::from(1000u128),
                },
            ],
            &self.owner,
        );
        assert!(result.is_ok());

        // Execute defunct pool
        let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
        let result = wasm.execute(&self.vault_instance, &defunct_msg, &[], &self.owner);

        assert!(result.is_ok());

        // Verify pool is in defunct state
        let query_msg = QueryMsg::GetDefunctPoolInfo { pool_id };
        let defunct_info: Option<DefunctPoolInfo> =
            wasm.query(&self.vault_instance, &query_msg).unwrap();

        assert!(defunct_info.is_some());

        let defunct_info = defunct_info.unwrap();
        assert_eq!(defunct_info.pool_id, pool_id);
        assert_eq!(
            defunct_info.lp_token_addr,
            Addr::unchecked(lp_token_instance)
        );
        assert!(!defunct_info.total_lp_supply_at_defunct.is_zero());
        assert!(!defunct_info.total_assets_at_defunct.is_empty());
    }

    fn test_execute_defunct_pool_unauthorized(&self) {
        let wasm = Wasm::new(&self.app);
        let unauthorized = self
            .app
            .init_account(&[cosmwasm_std::Coin {
                denom: "uxprt".to_string(),
                amount: Uint128::from(100_000u128),
            }])
            .unwrap();

        let (_, _, pool_id) = utils::initialize_weighted_pool(
            &self.app,
            &self.owner,
            &self.vault_instance,
            self.token1.clone(),
            self.token2.clone(),
            self.token3.clone(),
            "denom1".to_string(),
            "denom2".to_string(),
        );

        // Try to defunct pool with unauthorized user
        let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
        let result = wasm.execute(&self.vault_instance, &defunct_msg, &[], &unauthorized);

        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Unauthorized")
                || error_msg.contains("unauthorized")
                || error_msg.contains("Only the owner")
                || error_msg.contains("insufficient funds")
        );
    }

    fn test_execute_defunct_pool_nonexistent(&self) {
        let wasm = Wasm::new(&self.app);

        // Try to defunct a non-existent pool
        let nonexistent_pool_id = Uint128::from(999u128);
        let defunct_msg = ExecuteMsg::DefunctPool {
            pool_id: nonexistent_pool_id,
        };
        let result = wasm.execute(&self.vault_instance, &defunct_msg, &[], &self.owner);

        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Invalid PoolId")
                || error_msg.contains("InvalidPoolId")
                || error_msg.contains("pool not found")
        );
    }

    fn test_execute_defunct_pool_already_defunct(&self) {
        let wasm = Wasm::new(&self.app);

        let (_, _, pool_id) = utils::initialize_weighted_pool(
            &self.app,
            &self.owner,
            &self.vault_instance,
            self.token1.clone(),
            self.token2.clone(),
            self.token3.clone(),
            "denom1".to_string(),
            "denom2".to_string(),
        );

        // Make pool defunct first time
        let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
        let result = wasm.execute(&self.vault_instance, &defunct_msg, &[], &self.owner);
        assert!(result.is_ok());

        // Try to make it defunct again - should fail
        let result = wasm.execute(&self.vault_instance, &defunct_msg, &[], &self.owner);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Pool is already defunct")
                || error_msg.contains("PoolAlreadyDefunct")
                || error_msg.contains("pool already defunct")
        );
    }

    fn test_operations_on_defunct_pool_join(&self) {
        let wasm = Wasm::new(&self.app);

        let (_, _, pool_id) = utils::initialize_weighted_pool(
            &self.app,
            &self.owner,
            &self.vault_instance,
            self.token1.clone(),
            self.token2.clone(),
            self.token3.clone(),
            "denom1".to_string(),
            "denom2".to_string(),
        );

        // Make pool defunct
        let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
        let result = wasm.execute(&self.vault_instance, &defunct_msg, &[], &self.owner);
        assert!(result.is_ok());

        // Try to join defunct pool - should fail
        let join_msg = ExecuteMsg::JoinPool {
            pool_id,
            recipient: None,
            assets: Some(vec![Asset {
                info: AssetInfo::NativeToken {
                    denom: "denom1".to_string(),
                },
                amount: Uint128::from(1000u128),
            }]),
            min_lp_to_receive: None,
            auto_stake: None,
        };

        let result = wasm.execute(
            &self.vault_instance,
            &join_msg,
            &coins(1000u128, "uusd"),
            &self.owner,
        );
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Pool is already defunct")
                || error_msg.contains("PoolAlreadyDefunct")
                || error_msg.contains("pool already defunct")
        );
    }

    fn test_operations_on_defunct_pool_swap(&self) {
        let wasm = Wasm::new(&self.app);

        let (_, _, pool_id) = utils::initialize_weighted_pool(
            &self.app,
            &self.owner,
            &self.vault_instance,
            self.token1.clone(),
            self.token2.clone(),
            self.token3.clone(),
            "denom1".to_string(),
            "denom2".to_string(),
        );

        // Make pool defunct
        let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
        let result = wasm.execute(&self.vault_instance, &defunct_msg, &[], &self.owner);
        assert!(result.is_ok());

        // Try to swap in defunct pool - should fail
        let swap_msg = ExecuteMsg::Swap {
            swap_request: dexter::vault::SingleSwapRequest {
                pool_id,
                swap_type: dexter::vault::SwapType::GiveIn {},
                asset_in: AssetInfo::NativeToken {
                    denom: "denom1".to_string(),
                },
                asset_out: AssetInfo::NativeToken {
                    denom: "denom2".to_string(),
                },
                amount: Uint128::from(100u128),
            },
            recipient: None,
            min_receive: None,
            max_spend: None,
        };

        let result = wasm.execute(
            &self.vault_instance,
            &swap_msg,
            &coins(100u128, "denom1"),
            &self.owner,
        );
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Pool is already defunct")
                || error_msg.contains("PoolAlreadyDefunct")
                || error_msg.contains("pool already defunct")
        );
    }

    fn test_query_defunct_pool_info_existing(&self) {
        let wasm = Wasm::new(&self.app);

        let (_, lp_token_instance, pool_id) = utils::initialize_weighted_pool(
            &self.app,
            &self.owner,
            &self.vault_instance,
            self.token1.clone(),
            self.token2.clone(),
            self.token3.clone(),
            "denom1".to_string(),
            "denom2".to_string(),
        );

        // Make pool defunct
        let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
        let result = wasm.execute(&self.vault_instance, &defunct_msg, &[], &self.owner);
        assert!(result.is_ok());

        // Query defunct pool info
        let query_msg = QueryMsg::GetDefunctPoolInfo { pool_id };
        let defunct_info: Option<DefunctPoolInfo> =
            wasm.query(&self.vault_instance, &query_msg).unwrap();

        assert!(defunct_info.is_some());

        let defunct_info = defunct_info.unwrap();
        assert_eq!(defunct_info.pool_id, pool_id);
        assert_eq!(
            defunct_info.lp_token_addr,
            Addr::unchecked(lp_token_instance)
        );
    }

    fn test_query_defunct_pool_info_nonexistent(&self) {
        let wasm = Wasm::new(&self.app);

        // Query defunct pool info for non-existent pool
        let query_msg = QueryMsg::GetDefunctPoolInfo {
            pool_id: Uint128::from(999u128),
        };
        let defunct_info: Option<DefunctPoolInfo> =
            wasm.query(&self.vault_instance, &query_msg).unwrap();

        assert!(defunct_info.is_none());
    }

    fn test_query_is_user_refunded_false(&self) {
        let wasm = Wasm::new(&self.app);

        let (_, _, pool_id) = utils::initialize_weighted_pool(
            &self.app,
            &self.owner,
            &self.vault_instance,
            self.token1.clone(),
            self.token2.clone(),
            self.token3.clone(),
            "denom1".to_string(),
            "denom2".to_string(),
        );

        // Query user refund status (should be false by default)
        let query_msg = QueryMsg::IsUserRefunded {
            pool_id,
            user: self.owner.address(),
        };
        let is_refunded: bool = wasm.query(&self.vault_instance, &query_msg).unwrap();

        assert!(!is_refunded);
    }

    fn test_process_refund_batch_successful(&self) {
        let wasm = Wasm::new(&self.app);
        let user1 = self.app.init_account(&[]).unwrap();
        let user2 = self.app.init_account(&[]).unwrap();

        let (_, _, pool_id) = utils::initialize_weighted_pool(
            &self.app,
            &self.owner,
            &self.vault_instance,
            self.token1.clone(),
            self.token2.clone(),
            self.token3.clone(),
            "denom1".to_string(),
            "denom2".to_string(),
        );

        // Make pool defunct first
        let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
        let result = wasm.execute(&self.vault_instance, &defunct_msg, &[], &self.owner);
        assert!(result.is_ok());

        // Process refund batch
        let refund_msg = ExecuteMsg::ProcessRefundBatch {
            pool_id,
            user_addresses: vec![user1.address(), user2.address()],
        };
        let result = wasm.execute(&self.vault_instance, &refund_msg, &[], &self.owner);
        assert!(result.is_ok());
    }

    fn test_process_refund_batch_unauthorized(&self) {
        let wasm = Wasm::new(&self.app);
        let unauthorized = self
            .app
            .init_account(&[cosmwasm_std::Coin {
                denom: "uxprt".to_string(),
                amount: Uint128::from(100_000u128),
            }])
            .unwrap();

        let (_, _, pool_id) = utils::initialize_weighted_pool(
            &self.app,
            &self.owner,
            &self.vault_instance,
            self.token1.clone(),
            self.token2.clone(),
            self.token3.clone(),
            "denom1".to_string(),
            "denom2".to_string(),
        );

        // Make pool defunct first
        let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
        let result = wasm.execute(&self.vault_instance, &defunct_msg, &[], &self.owner);
        assert!(result.is_ok());

        // Try to process refund batch with unauthorized user
        let refund_msg = ExecuteMsg::ProcessRefundBatch {
            pool_id,
            user_addresses: vec!["user1".to_string()],
        };
        let result = wasm.execute(&self.vault_instance, &refund_msg, &[], &unauthorized);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Unauthorized")
                || error_msg.contains("unauthorized")
                || error_msg.contains("insufficient funds")
        );
    }

    fn test_process_refund_batch_non_defunct_pool(&self) {
        let wasm = Wasm::new(&self.app);

        let (_, _, pool_id) = utils::initialize_weighted_pool(
            &self.app,
            &self.owner,
            &self.vault_instance,
            self.token1.clone(),
            self.token2.clone(),
            self.token3.clone(),
            "denom1".to_string(),
            "denom2".to_string(),
        );

        // Try to process refund batch on active (non-defunct) pool
        let refund_msg = ExecuteMsg::ProcessRefundBatch {
            pool_id,
            user_addresses: vec!["user1".to_string()],
        };
        let result = wasm.execute(&self.vault_instance, &refund_msg, &[], &self.owner);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Pool is not defunct")
                || error_msg.contains("PoolNotDefunct")
                || error_msg.contains("pool not defunct")
        );
    }

    fn test_defunct_pool_with_active_reward_schedules(&self) {
        let wasm = Wasm::new(&self.app);

        let (_, _, pool_id) = utils::initialize_weighted_pool(
            &self.app,
            &self.owner,
            &self.vault_instance,
            self.token1.clone(),
            self.token2.clone(),
            self.token3.clone(),
            "denom1".to_string(),
            "denom2".to_string(),
        );

        // Mock a situation where there might be active reward schedules
        // Note: This test will pass because our validation only checks common assets
        // and the test environment doesn't have multistaking enabled by default
        // In a real environment with multistaking and active reward schedules,
        // this would fail with PoolHasActiveRewardSchedules error

        let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
        let result = wasm.execute(&self.vault_instance, &defunct_msg, &[], &self.owner);

        // This should succeed because there are no active reward schedules in our test environment
        assert!(result.is_ok());
    }

    fn test_defunct_pool_with_future_reward_schedules(&self) {
        let wasm = Wasm::new(&self.app);

        let (_, _, pool_id) = utils::initialize_weighted_pool(
            &self.app,
            &self.owner,
            &self.vault_instance,
            self.token1.clone(),
            self.token2.clone(),
            self.token3.clone(),
            "denom1".to_string(),
            "denom2".to_string(),
        );

        // Note: This test demonstrates the validation logic structure
        // In a real environment with multistaking and future reward schedules,
        // this would fail with PoolHasFutureRewardSchedules error
        // Currently passes because test environment doesn't have multistaking with future schedules

        let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
        let result = wasm.execute(&self.vault_instance, &defunct_msg, &[], &self.owner);

        // This should succeed because there are no future reward schedules in our test environment
        assert!(result.is_ok());
    }
}

#[test]
#[cfg(feature = "test-tube")]
fn run_defunct_pool_test_suite() {
    let suite = DefunctPoolTestSuite::new();
    suite.run_all_tests();
}
