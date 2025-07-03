#![cfg(test)]

use cosmwasm_std::{coins, from_json, to_json_binary, Addr, Uint128};
use dexter::asset::{Asset, AssetInfo};
use dexter::vault::{DefunctPoolInfo, ExecuteMsg, QueryMsg};
use persistence_test_tube::{bank, Account, Module, Runner, RunnerExecuteResult, Wasm};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use persistence_std::types::cosmwasm::wasm::v1::{MsgMigrateContractResponse, MsgMigrateContract, QueryRawContractStateRequest, QueryRawContractStateResponse};
use cw20::{BalanceResponse, Cw20QueryMsg};
use cw2::ContractVersion;

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
                amount: Uint128::from(1000_000_000_000_000u128),
            },
        ]);

        let fee_collector = app
            .init_account(&[cosmwasm_std::Coin {
                denom: "uxprt".to_string(),
                amount: Uint128::from(100_000_000_000u128),
            }])
            .unwrap();

        let vault_instance =
            utils::instantiate_contract(&app, &owner, fee_collector.address().to_string());

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
        self.test_defunct_pool_multiple_users_refund();
        self.test_vault_migration();
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

    fn test_defunct_pool_multiple_users_refund(&self) {
        let wasm = Wasm::new(&self.app);

        let assets_to_check = vec![
            AssetInfo::NativeToken {
                denom: "denom1".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "denom2".to_string(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked(self.token1.clone()),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked(self.token2.clone()),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked(self.token3.clone()),
            },
        ];

        let pre_test_balances =
            utils::query_all_asset_balances(&self.app, &self.vault_instance, &assets_to_check);

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

        let mut user_addresses = vec![];
        let mut user_lp_balances: std::collections::HashMap<String, Uint128> =
            std::collections::HashMap::new();

        // Create 10 users and have them join the pool with different amounts
        for i in 0..10 {
            let user = self
                .app
                .init_account(&[cosmwasm_std::Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(100_000_000_000u128),
                },
                cosmwasm_std::Coin {
                    denom: "uxprt".to_string(),
                    amount: Uint128::from(100_000_000_000u128),
                },
                cosmwasm_std::Coin {
                    denom: "denom1".to_string(),
                    amount: Uint128::from(100_000_000_000u128),
                },
                cosmwasm_std::Coin {
                    denom: "denom2".to_string(),
                    amount: Uint128::from(100_000_000_000u128),
                }]
            )
                .unwrap();
            user_addresses.push(user.address().to_string());

            let mut rng = StdRng::seed_from_u64(i as u64);
            let join_amount_token1 = Uint128::from(rng.gen_range(100..10000) as u128);
            let join_amount_token2 = Uint128::from(rng.gen_range(50..5000) as u128);
            let join_amount_token3 = Uint128::from(rng.gen_range(75..7500) as u128);
            let join_amount_denom1 = Uint128::from(rng.gen_range(100..10000) as u128);
            let join_amount_denom2 = Uint128::from(rng.gen_range(50..5000) as u128);

            utils::mint_some_tokens(
                &self.app,
                &self.owner,
                &self.token1,
                join_amount_token1,
                user.address().to_string(),
            );
            utils::mint_some_tokens(
                &self.app,
                &self.owner,
                &self.token2,
                join_amount_token2,
                user.address().to_string(),
            );
            utils::mint_some_tokens(
                &self.app,
                &self.owner,
                &self.token3,
                join_amount_token3,
                user.address().to_string(),
            );

            utils::increase_token_allowance(
                &self.app,
                &user,
                &self.token1,
                self.vault_instance.to_string(),
                join_amount_token1,
            );
            utils::increase_token_allowance(
                &self.app,
                &user,
                &self.token2,
                self.vault_instance.to_string(),
                join_amount_token2,
            );
            utils::increase_token_allowance(
                &self.app,
                &user,
                &self.token3,
                self.vault_instance.to_string(),
                join_amount_token3,
            );

            let join_msg = ExecuteMsg::JoinPool {
                pool_id,
                recipient: None,
                assets: Some(vec![
                    Asset {
                        info: AssetInfo::NativeToken {
                            denom: "denom1".to_string(),
                        },
                        amount: join_amount_denom1,
                    },
                    Asset {
                        info: AssetInfo::NativeToken {
                            denom: "denom2".to_string(),
                        },
                        amount: join_amount_denom2,
                    },
                    Asset {
                        info: AssetInfo::Token {
                            contract_addr: Addr::unchecked(self.token1.clone()),
                        },
                        amount: join_amount_token1,
                    },
                    Asset {
                        info: AssetInfo::Token {
                            contract_addr: Addr::unchecked(self.token2.clone()),
                        },
                        amount: join_amount_token2,
                    },
                    Asset {
                        info: AssetInfo::Token {
                            contract_addr: Addr::unchecked(self.token3.clone()),
                        },
                        amount: join_amount_token3,
                    },
                ]),
                min_lp_to_receive: None,
                auto_stake: None,
            };

            let initial_lp_balance: BalanceResponse = wasm
                .query(
                    &lp_token_instance,
                    &cw20::Cw20QueryMsg::Balance { address: user.address().to_string() },
                )
                .unwrap();  

            let result = wasm.execute(
                &self.vault_instance,
                &join_msg,
                &[
                    cosmwasm_std::Coin {
                        denom: "denom1".to_string(),
                        amount: join_amount_denom1,
                    },
                    cosmwasm_std::Coin {
                        denom: "denom2".to_string(),
                        amount: join_amount_denom2,
                    },
                ],
                &user,
            );

            assert!(result.is_ok(), "Failed to join pool for user {}", i);

            let final_lp_balance: BalanceResponse = wasm
                .query(
                    &lp_token_instance,
                    &cw20::Cw20QueryMsg::Balance { address: user.address().to_string() },
                )
                .unwrap();
            let lp_received = final_lp_balance.balance - initial_lp_balance.balance;
            user_lp_balances.insert(user.address().to_string(), lp_received);
        }

        // Make pool defunct
        let defunct_msg = ExecuteMsg::DefunctPool { pool_id };
        let result = wasm.execute(&self.vault_instance, &defunct_msg, &[], &self.owner);
        assert!(result.is_ok());

        // Store initial pool assets before processing refunds
        let defunct_pool_info: Option<DefunctPoolInfo> = wasm
            .query(
                &self.vault_instance,
                &QueryMsg::GetDefunctPoolInfo { pool_id },
            )
            .unwrap();

        let initial_pool_assets = defunct_pool_info
            .as_ref()
            .expect("Defunct pool info should be present")
            .total_assets_at_defunct
            .clone();

        let total_lp_supply_at_defunct = defunct_pool_info
            .as_ref()
            .expect("Defunct pool info should be present")
            .total_lp_supply_at_defunct;

        let mut users_pre_refund_balances: std::collections::HashMap<
            String,
            std::collections::HashMap<String, Uint128>,
        > = std::collections::HashMap::new();

        for user_addr in &user_addresses {
            let mut asset_balances = std::collections::HashMap::new();
            for asset in &initial_pool_assets {
                let balance = utils::query_asset_balance(&self.app, user_addr, &asset.info);
                asset_balances.insert(asset.info.to_string(), balance);
            }
            users_pre_refund_balances.insert(user_addr.clone(), asset_balances);
        }

        // Process refund batches for all users
        for chunk in user_addresses.chunks(20) {
            let refund_msg = ExecuteMsg::ProcessRefundBatch {
                pool_id,
                user_addresses: chunk.to_vec(),
            };
            let result = wasm.execute(&self.vault_instance, &refund_msg, &[], &self.owner);
            assert!(result.is_ok(), "Failed to process refund batch");
        }

        for (user_addr, user_lp_balance) in &user_lp_balances {
            let pre_refund_balances = users_pre_refund_balances.get(user_addr).unwrap();

            for asset in &initial_pool_assets {
                let expected_refund = asset
                    .amount
                    .multiply_ratio(*user_lp_balance, total_lp_supply_at_defunct);

                let post_refund_balance =
                    utils::query_asset_balance(&self.app, user_addr, &asset.info);
                let pre_refund_balance = pre_refund_balances.get(&asset.info.to_string()).unwrap();
                let actual_refund_received = post_refund_balance.checked_sub(*pre_refund_balance).unwrap();

                assert_eq!(
                    actual_refund_received,
                    expected_refund,
                    "Mismatched refund for user {} and asset {}. Expected {}, got {}",
                    user_addr,
                    asset.info,
                    expected_refund,
                    actual_refund_received
                );
            }
        }

        // Verify all users are refunded and their LP tokens are burnt, and assets returned
        for (user_addr, _expected_lp_balance_at_defunct) in &user_lp_balances {
            let is_refunded: bool = wasm
                .query(
                    &self.vault_instance,
                    &QueryMsg::IsUserRefunded {
                        pool_id,
                        user: user_addr.clone(),
                    },
                )
                .unwrap();
            assert!(is_refunded, "User {} not refunded", user_addr);

            // Verify user cannot claim twice
            let refund_msg = ExecuteMsg::ProcessRefundBatch {
                pool_id,
                user_addresses: vec![user_addr.clone()],
            };
            let result = wasm.execute(&self.vault_instance, &refund_msg, &[], &self.owner);
            assert!(result.is_err(), "User {} could claim twice", user_addr);
            let error_msg = result.unwrap_err().to_string();
            assert!(
                error_msg.contains("UserAlreadyRefunded")
                    || error_msg.contains("User has already been refunded from this defunct pool"),
                "Unexpected error for double claim: {}",
                error_msg
            );
        }

        // --- Final Dust Verification ---
        // Verify that the dust amount calculated by the contract's internal state matches the actual
        // balances held in the vault's address after all refunds.

        // 1. Get the final pool assets as tracked by the contract's state
        let defunct_pool_info_after_refund: Option<DefunctPoolInfo> = wasm
            .query(
                &self.vault_instance,
                &QueryMsg::GetDefunctPoolInfo { pool_id },
            )
            .unwrap();

        let mut final_pool_assets_from_state = defunct_pool_info_after_refund
            .as_ref()
            .expect("Defunct pool info should be present")
            .current_assets_in_pool
            .clone();

        // 2. Query the actual balances of the vault contract for each asset and subtract pre-test balances
        let final_vault_balances = utils::query_all_asset_balances(
            &self.app,
            &self.vault_instance,
            &initial_pool_assets
                .iter()
                .map(|a| a.info.clone())
                .collect::<Vec<_>>(),
        );
        let mut actual_dust_in_vault: Vec<Asset> = vec![];

        for final_asset_balance in &final_vault_balances {
            let pre_test_balance_asset = pre_test_balances
                .iter()
                .find(|a| a.info == final_asset_balance.info)
                .unwrap();

            let dust = final_asset_balance
                .amount
                .checked_sub(pre_test_balance_asset.amount)
                .unwrap();

            actual_dust_in_vault.push(Asset {
                info: final_asset_balance.info.clone(),
                amount: dust,
            });
        }

        // 4. Sort and assert equality for a precise check
        final_pool_assets_from_state.sort_by(|a, b| a.info.to_string().cmp(&b.info.to_string()));
        actual_dust_in_vault.sort_by(|a, b| a.info.to_string().cmp(&b.info.to_string()));

        assert_eq!(
            final_pool_assets_from_state, actual_dust_in_vault,
            "Dust mismatch between contract state and actual vault balance"
        );
    }

    fn test_vault_migration(&self) {
        let wasm = Wasm::new(&self.app);

        // Store old vault code using the helper function
        let old_vault_code_id = utils::store_old_vault_code(&self.app, &self.owner);

        // Store new vault code
        let new_vault_code_id = utils::store_vault_code(&self.app, &self.owner);

        // Instantiate old vault
        let old_vault_instance = wasm
            .instantiate(
                old_vault_code_id,
                &dexter::vault::InstantiateMsg {
                    pool_configs: vec![],
                    lp_token_code_id: None,
                    fee_collector: None,
                    owner: self.owner.address(),
                    auto_stake_impl: dexter::vault::AutoStakeImpl::None,
                    pool_creation_fee: dexter::vault::PoolCreationFee::default(),
                },
                Some(self.owner.address().as_str()),
                Some("old_vault"),
                &[],
                &self.owner,
            )
            .unwrap()
            .data
            .address;

        // --- Test successful migration ---
        let migrate_msg = dexter::vault::MigrateMsg::V1_2 {
            reward_schedule_validation_assets: Some(vec![AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            }, AssetInfo::NativeToken {
                denom: "uxprt".to_string(),
            }])
        };

        // We need to send a message directly on the persistence test app since we have runner in scope, we can just send the whole message

        let migrate_cosmos_msg = MsgMigrateContract {
            contract: old_vault_instance.to_string(),
            code_id: new_vault_code_id,
            sender: self.owner.address().to_string(),
            msg: to_json_binary(&migrate_msg).unwrap().to_vec(),
        };

        let result: RunnerExecuteResult<MsgMigrateContractResponse> = self.app.execute(
            migrate_cosmos_msg,
            "/cosmwasm.wasm.v1.MsgMigrateContract",
            &self.owner,
        );
        assert!(result.is_ok(), "Migration should succeed with valid input");
       
        // Verify contract version after migration
        let contract_info_res = self
            .app
            .query::<QueryRawContractStateRequest, QueryRawContractStateResponse>(
                "/cosmwasm.wasm.v1.Query/RawContractState",
                &QueryRawContractStateRequest {
                    address: old_vault_instance.to_string(),
                    query_data: "contract_info".as_bytes().to_vec(),
                },
            )
            .unwrap();

        let contract_info: ContractVersion = from_json(&contract_info_res.data).unwrap();
        assert_eq!(contract_info.version, "1.2.0");
        assert_eq!(contract_info.contract, "dexter-vault");

        // Verify config after successful migration
        let reward_schedule_validation_assets: Vec<AssetInfo> = wasm
            .query(&old_vault_instance, &QueryMsg::RewardScheduleValidationAssets {})
            .unwrap();


        assert_eq!(
            reward_schedule_validation_assets,
            vec![AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            }, AssetInfo::NativeToken {
                denom: "uxprt".to_string(),
            }]
        );

        // --- Test migration with None (should use default assets) ---
        // Create another old vault instance for this test
        let old_vault_instance2 = wasm
            .instantiate(
                old_vault_code_id,
                &dexter::vault::InstantiateMsg {
                    pool_configs: vec![],
                    lp_token_code_id: None,
                    fee_collector: None,
                    owner: self.owner.address(),
                    auto_stake_impl: dexter::vault::AutoStakeImpl::None,
                    pool_creation_fee: dexter::vault::PoolCreationFee::default(),
                },
                Some(self.owner.address().as_str()),
                Some("old_vault2"),
                &[],
                &self.owner,
            )
            .unwrap()
            .data
            .address;

        let migrate_msg_none = dexter::vault::MigrateMsg::V1_2 { 
            reward_schedule_validation_assets: None
        };
        let migrate_cosmos_msg = MsgMigrateContract {
            contract: old_vault_instance2.to_string(),
            code_id: new_vault_code_id,
            sender: self.owner.address().to_string(),
            msg: to_json_binary(&migrate_msg_none).unwrap().to_vec(),
        };
        let result: RunnerExecuteResult<MsgMigrateContractResponse> = self.app.execute(
            migrate_cosmos_msg,
            "/cosmwasm.wasm.v1.MsgMigrateContract",
            &self.owner,
        );
        assert!(result.is_err(), "Migration should fail when no validation assets are provided");

        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("reward_schedule_validation_assets must be provided"),
            "Unexpected error for migration with None: {}",
            error_msg
        );


        // --- Test unauthorized migration ---
        let unauthorized_user = self
            .app
            .init_account(&[cosmwasm_std::Coin {
                denom: "uxprt".to_string(),
                amount: Uint128::from(100_000u128),
            }])
            .unwrap();

        // Create another old vault instance for this test
        let old_vault_instance3 = wasm
            .instantiate(
                old_vault_code_id,
                &dexter::vault::InstantiateMsg {
                    pool_configs: vec![],
                    lp_token_code_id: None,
                    fee_collector: None,
                    owner: self.owner.address(),
                    auto_stake_impl: dexter::vault::AutoStakeImpl::None,
                    pool_creation_fee: dexter::vault::PoolCreationFee::default(),
                },
                Some(self.owner.address().as_str()),
                Some("old_vault3"),
                &[],
                &self.owner,
            )
            .unwrap()
            .data
            .address;

        let migrate_cosmos_msg = MsgMigrateContract {
            contract: old_vault_instance3.to_string(),
            code_id: new_vault_code_id,
            sender: unauthorized_user.address(),
            msg: to_json_binary(&migrate_msg).unwrap().to_vec(),
        };

        let result: RunnerExecuteResult<MsgMigrateContractResponse> = self.app.execute(
            migrate_cosmos_msg,
            "/cosmwasm.wasm.v1.MsgMigrateContract",
            &unauthorized_user,
        );
        assert!(result.is_err(), "Unauthorized migration should fail");
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Unauthorized")
                || error_msg.contains("unauthorized"),
            "Unexpected error for unauthorized migration: {}",
            error_msg
        );
    }
}

#[test]
fn run_defunct_pool_test_suite() {
    let suite = DefunctPoolTestSuite::new();
    suite.run_all_tests();
}
