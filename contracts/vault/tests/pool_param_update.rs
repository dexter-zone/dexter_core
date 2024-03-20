
pub mod utils;

use std::vec;

use cosmwasm_std::{attr, coin, Addr, Coin, Uint128, to_json_binary, from_json};
use cw20::MinterResponse;
use cw_multi_test::Executor;
use dexter::asset::{Asset, AssetInfo};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;

use dexter::pool::ConfigResponse;
use dexter::vault::{ExecuteMsg, PoolInfo, PoolType, QueryMsg};
use dexter_stable_pool::state::{StablePoolParams, StablePoolUpdateParams};

use crate::utils::{instantiate_contract, mock_app, store_token_code};

#[test]
fn update_pool_params() {
    let owner = String::from("owner");
    let mut app = mock_app(
        Addr::unchecked(owner.clone()),
        vec![Coin {
            denom: "uxprt".to_string(),
            amount: Uint128::from(1_000_000_000u64),
        }],
    );

    let owner_addr = Addr::unchecked(owner.clone());
    let user_addr = Addr::unchecked("user".to_string());

    // Send some funds from owner to user
    app.send_tokens(
        owner_addr.clone(),
        user_addr.clone(),
        &[coin(200_000_000u128, "uxprt")],
    )
    .unwrap();

    let token_code_id = store_token_code(&mut app);
    let vault_instance = instantiate_contract(&mut app, &Addr::unchecked(owner.clone()));

    // Create Token X
    let init_msg = TokenInstantiateMsg {
        name: "x_token".to_string(),
        symbol: "X-Tok".to_string(),
        decimals: 18,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
        marketing: None,
    };
    let token_instance0 = app
        .instantiate_contract(
            token_code_id,
            Addr::unchecked(owner.clone()),
            &init_msg,
            &[],
            "x_token",
            None,
        )
        .unwrap();

    // Create Token Y
    let init_msg = TokenInstantiateMsg {
        name: "y_token".to_string(),
        symbol: "Y-Tok".to_string(),
        decimals: 18,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
        marketing: None,
    };
    let token_instance1 = app
        .instantiate_contract(
            token_code_id,
            Addr::unchecked(owner.clone()),
            &init_msg,
            &[],
            "y_token",
            None,
        )
        .unwrap();

    let asset_infos = vec![
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
        AssetInfo::Token {
            contract_addr: token_instance1.clone(),
        },
    ];

    let msg = ExecuteMsg::CreatePoolInstance {
        pool_type: PoolType::StableSwap {},
        asset_infos: asset_infos.to_vec(),
        native_asset_precisions: vec![],
        init_params: Some(to_json_binary(&StablePoolParams {
            amp: 100u64,
            scaling_factor_manager: Some(Addr::unchecked("manager1".to_string())),
            supports_scaling_factors_update: true,
            scaling_factors: vec![],
        }).unwrap()),
        fee_info: None,
    };

    let res = app
        .execute_contract(
            Addr::unchecked(owner.clone()),
            vault_instance.clone(),
            &msg,
            &[],
        )
        .unwrap();

    assert_eq!(res.events[1].attributes[2], attr("pool_type", "stable-swap"));

    let pool_res: PoolInfo = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &QueryMsg::GetPoolById {
                pool_id: Uint128::from(1u128),
            },
        )
        .unwrap();

    let assets = vec![
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance0.clone(),
            },
            amount: Uint128::zero(),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: token_instance1.clone(),
            },
            amount: Uint128::zero(),
        },
    ];

    assert_eq!(Uint128::from(1u128), pool_res.pool_id);
    assert_eq!(
        Addr::unchecked("contract3".to_string()),
        pool_res.lp_token_addr
    );
    assert_eq!(
        Addr::unchecked("contract4".to_string()),
        pool_res.pool_addr
    );
    assert_eq!(assets, pool_res.assets);
    assert_eq!(PoolType::StableSwap {}, pool_res.pool_type);

    let pool_addr = Addr::unchecked("contract4".to_string());

    // Let's update the pool params: scaling factor manager
    let msg = ExecuteMsg::UpdatePoolParams {
        pool_id: Uint128::from(1u128),
        params: to_json_binary(&StablePoolUpdateParams::UpdateScalingFactorManager {
            manager: Addr::unchecked("manager2".to_string()),
        }).unwrap(),
    };

    app
        .execute_contract(
            Addr::unchecked(owner.clone()),
            vault_instance.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // Fetch the pool config from the pool contract directly
    let pool_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(
            pool_addr.clone(),
            &dexter::pool::QueryMsg::Config {},
        )
        .unwrap();

    // unmarshal the pool params
    let pool_params: StablePoolParams = from_json(&pool_res.additional_params.unwrap()).unwrap();
    assert_eq!(Addr::unchecked("manager2".to_string()), pool_params.scaling_factor_manager.unwrap());

    // Try to update the pool params with a non owner
    let msg = ExecuteMsg::UpdatePoolParams {
        pool_id: Uint128::from(1u128),
        params: to_json_binary(&StablePoolUpdateParams::UpdateScalingFactorManager { 
            manager: Addr::unchecked("manager3".to_string())
        }).unwrap(),
    };

    let res = app
        .execute_contract(
            Addr::unchecked("user".to_string()),
            vault_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();

    assert_eq!(res.root_cause().to_string(), "Unauthorized");
    
}
