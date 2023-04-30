use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{to_binary, Addr, Coin, Timestamp, Uint128};
use cw20::{BalanceResponse, Cw20QueryMsg};
use cw_multi_test::{App, ContractWrapper, Executor};
use dexter::asset::Asset;
use dexter::{asset::AssetInfo, vault::FeeInfo};

use dexter::vault::{
    InstantiateMsg as VaultInstantiateMsg, NativeAssetPrecisionInfo, PauseInfo, PoolCreationFee,
    PoolInfoResponse, PoolType, PoolTypeConfig,
};

use dexter::keeper::InstantiateMsg as KeeperInstantiateMsg;
use weighted_pool::state::WeightedParams;

pub const EPOCH_START: u64 = 1_000_000;

#[macro_export]
macro_rules! uint128_with_precision {
    ($value:expr, $precision:expr) => {
        Uint128::from($value)
            .checked_mul(Uint128::from(10u64).pow($precision as u32))
            .unwrap()
    };
}

pub fn mock_app(owner: Addr, coins: Vec<Coin>) -> App {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(EPOCH_START);

    let mut app = App::new(|router, _, storage| {
        // initialization  moved to App construction
        router.bank.init_balance(storage, &owner, coins).unwrap();
    });
    app.set_block(env.block);
    app
}

pub fn assert_cw20_balance(app: &App, contract: &Addr, address: &Addr, expected_balance: Uint128) {
    let query_msg = Cw20QueryMsg::Balance {
        address: address.to_string(),
    };
    let balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(contract, &query_msg)
        .unwrap();

    assert_eq!(balance.balance, expected_balance);
}

pub fn instantiate_contracts(
    app: &mut App,
    vault_owner: &Addr,
    keeper_owner: &Addr,
    fee_info: FeeInfo,
    asset_infos: Vec<AssetInfo>,
    native_asset_precisions: Vec<(String, u8)>,
) -> (Addr, Addr, Uint128, Addr, Addr) {
    let weighted_pool_code_id: u64 = store_weighted_pool_code(app);
    let vault_code_id: u64 = store_vault_code(app);
    let keeper_code_id: u64 = store_keeper_code(app);
    let lp_token_code_id: u64 = store_token_code(app);

    let pool_configs = vec![PoolTypeConfig {
        pool_type: PoolType::Weighted {},
        code_id: weighted_pool_code_id,
        default_fee_info: fee_info.clone(),
        allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
        paused: PauseInfo::default(),
    }];

    let vault_init_msg = VaultInstantiateMsg {
        pool_configs,
        lp_token_code_id: Some(lp_token_code_id),
        fee_collector: None,
        owner: vault_owner.to_string(),
        pool_creation_fee: PoolCreationFee::Disabled,
        auto_stake_impl: dexter::vault::AutoStakeImpl::None,
    };

    // Initialize Vault contract instance
    let vault_instance = app
        .instantiate_contract(
            vault_code_id,
            vault_owner.to_owned(),
            &vault_init_msg,
            &[],
            "vault",
            None,
        )
        .unwrap();

    // initialize the keeper
    let keeper_init_msg = KeeperInstantiateMsg {
        owner: keeper_owner.clone(),
        vault_address: vault_instance.clone(),
    };

    let keeper_instance = app
        .instantiate_contract(
            keeper_code_id,
            keeper_owner.to_owned(),
            &keeper_init_msg,
            &[],
            "keeper",
            None,
        )
        .unwrap();

    // register keeper in the vault
    let register_msg = dexter::vault::ExecuteMsg::UpdateConfig {
        fee_collector: Some(keeper_instance.to_string()),
        lp_token_code_id: None,
        pool_creation_fee: None,
        auto_stake_impl: None,
        paused: None,
    };

    // send the message
    app.execute_contract(
        vault_owner.to_owned(),
        vault_instance.clone(),
        &register_msg,
        &[],
    )
    .unwrap();

    // Create a weighted pool of uxprt and uatom
    let pool_init_msg = dexter::vault::ExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::Weighted {},
        asset_infos: asset_infos.clone(),
        native_asset_precisions: native_asset_precisions
            .iter()
            .map(|(denom, precision)| NativeAssetPrecisionInfo {
                denom: denom.clone(),
                precision: *precision,
            })
            .collect(),
        fee_info: Some(fee_info.clone()),
        init_params: Some(
            to_binary(&WeightedParams {
                weights: asset_infos
                    .iter()
                    .map(|info| Asset {
                        amount: Uint128::from(1u64),
                        info: info.clone(),
                    })
                    .collect(),
                exit_fee: None,
            })
            .unwrap(),
        ),
    };

    // send the message
    app.execute_contract(
        vault_owner.to_owned(),
        vault_instance.clone(),
        &pool_init_msg,
        &[],
    )
    .unwrap();

    // validate that pool with id 1 was created
    let pool_id = Uint128::from(1u64);

    let pool_info = dexter::vault::QueryMsg::GetPoolById { pool_id };
    let pool_info_res: PoolInfoResponse = app
        .wrap()
        .query_wasm_smart(vault_instance.clone(), &pool_info)
        .unwrap();

    assert_eq!(pool_info_res.pool_id, pool_id);
    assert_eq!(pool_info_res.pool_type, PoolType::Weighted {});

    // return vault_address, keeper_address, pool_id, pool_address, lp_token_address
    (
        vault_instance,
        keeper_instance,
        pool_id,
        pool_info_res.pool_addr,
        pool_info_res.lp_token_addr,
    )
}

fn store_weighted_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(ContractWrapper::new_with_empty(
        weighted_pool::contract::execute,
        weighted_pool::contract::instantiate,
        weighted_pool::contract::query,
    ));
    app.store_code(pool_contract)
}

fn store_vault_code(app: &mut App) -> u64 {
    let vault_contract = Box::new(
        ContractWrapper::new_with_empty(
            dexter_vault::contract::execute,
            dexter_vault::contract::instantiate,
            dexter_vault::contract::query,
        )
        .with_reply_empty(dexter_vault::contract::reply),
    );
    app.store_code(vault_contract)
}

fn store_keeper_code(app: &mut App) -> u64 {
    let keeper_contract: Box<ContractWrapper<_, _, _, _, _, _>> =
        Box::new(ContractWrapper::new_with_empty(
            dexter_keeper::contract::execute,
            dexter_keeper::contract::instantiate,
            dexter_keeper::contract::query,
        ));
    app.store_code(keeper_contract)
}

fn store_token_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        lp_token::contract::execute,
        lp_token::contract::instantiate,
        lp_token::contract::query,
    ));
    app.store_code(token_contract)
}
