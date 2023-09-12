use cosmwasm_std::{Addr, Coin};
use cw_multi_test::{App, ContractWrapper, Executor};
use std::vec;

use dexter::vault::{FeeInfo, PauseInfo, PoolCreationFee, PoolType, PoolTypeConfig};

fn mock_app(owner: Addr, coins: Vec<Coin>) -> App {
    App::new(|router, _, storage| {
        // initialization moved to App construction
        router.bank.init_balance(storage, &owner, coins).unwrap()
    })
}

fn instantiate_contracts(router: &mut App, owner: Addr, keeper_admin: Addr) -> (Addr, Addr) {
    let vault_contract = Box::new(
        ContractWrapper::new_with_empty(
            dexter_vault::contract::execute,
            dexter_vault::contract::instantiate,
            dexter_vault::contract::query,
        )
        .with_reply_empty(dexter_vault::contract::reply),
    );
    let vault_code_id = router.store_code(vault_contract);

    let stable5_contract = Box::new(ContractWrapper::new_with_empty(
        stable_pool::contract::execute,
        stable_pool::contract::instantiate,
        stable_pool::contract::query,
    ));
    let stable5_code_id = router.store_code(stable5_contract);

    // Instantiate Vault Contract
    let msg = dexter::vault::InstantiateMsg {
        pool_configs: vec![PoolTypeConfig {
            code_id: stable5_code_id,
            pool_type: PoolType::StableSwap {},
            default_fee_info: FeeInfo {
                total_fee_bps: 0u16,
                protocol_fee_percent: 50u16,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            paused: PauseInfo::default(),
        }],
        lp_token_code_id: Some(1u64),
        fee_collector: None,
        owner: owner.to_string(),
        pool_creation_fee: PoolCreationFee::default(),
        auto_stake_impl: dexter::vault::AutoStakeImpl::None,
    };
    let vault_instance = router
        .instantiate_contract(
            vault_code_id,
            owner.clone(),
            &msg,
            &[],
            String::from("VAULT"),
            None,
        )
        .unwrap();

    // Instantiate Keeper Contract
    let keeper_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_keeper::contract::execute,
        dexter_keeper::contract::instantiate,
        dexter_keeper::contract::query,
    ));
    let keeper_code_id = router.store_code(keeper_contract);
    let k_msg = dexter::keeper::InstantiateMsg {
        owner: keeper_admin,
        vault_address: vault_instance.clone(),
    };
    let keeper_instance = router
        .instantiate_contract(
            keeper_code_id,
            Addr::unchecked("instantiator"),
            &k_msg,
            &[],
            String::from("KEEPER"),
            None,
        )
        .unwrap();

    (vault_instance, keeper_instance)
}
