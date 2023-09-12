use cosmwasm_std::{Addr, Coin, Uint128};
use dexter::{asset::Asset, vault::FeeInfo};
use persistence_std::types::cosmos::gov::v1::QueryParamsRequest;

mod utils;

#[test]
fn test_basic_functions() {
    let governance_params_query = QueryParamsRequest {
        params_type: String::from("deposit"),
    };

    println!("{:?}", governance_params_query);
}

#[test]
fn test_create_pool() {
    let vault_creator: Addr = Addr::unchecked("vault_creator".to_string());
    let keeper_owner: Addr = Addr::unchecked("keeper_owner".to_string());
    let _alice_address: Addr = Addr::unchecked("alice".to_string());

    let mut app = utils::mock_app(
        vault_creator.clone(),
        vec![
            Coin {
                denom: "uxprt".to_string(),
                amount: uint128_with_precision!(100_000u128, 6),
            },
            Coin {
                denom: "uatom".to_string(),
                amount: uint128_with_precision!(100_000u128, 6),
            },
        ],
    );

    let fee_info = FeeInfo {
        total_fee_bps: 1000,
        protocol_fee_percent: 20,
    };

    let (_vault_addr, _keeper_addr, _governance_admin) =
        utils::instantiate_contracts(&mut app, &vault_creator, &keeper_owner, fee_info);

    let _asset_infos_with_weights = vec![
        Asset::new_native("uxprt".to_string(), Uint128::from(1u128)),
        Asset::new_native("uatom".to_string(), Uint128::from(1u128)),
    ];

    // create a pool using governance admin
    // Currently, we'd use an account that doesn't have a private key in tests directly.
    // Ideally, during the actual run, the users trigger the Execute contract governance flow which triggers it from that address
    // but on pool creator's behalf
    // let msg = dexter::governance_admin::ExecuteMsg::CreateNewPool {
    //     vault_addr: vault_addr.to_string(),
    //     bootstrapping_amount_payer: vault_creator.to_string(),
    //     pool_type: dexter::vault::PoolType::Weighted {},
    //     fee_info: None,
    //     native_asset_precisions: vec![
    //         NativeAssetPrecisionInfo {
    //             denom: "uxprt".to_string(),
    //             precision: 6,
    //         },
    //         NativeAssetPrecisionInfo {
    //             denom: "uatom".to_string(),
    //             precision: 6,
    //         },
    //     ],
    //     assets: vec![
    //         Asset::new_native("uxprt".to_string(), uint128_with_precision!(10000u128, 6)),
    //         Asset::new_native("uatom".to_string(), uint128_with_precision!(1000u128, 6))
    //     ],
    //     init_params: Some(
    //         to_binary(&WeightedParams {
    //             weights: asset_infos_with_weights,
    //             exit_fee: Some(Decimal::from_ratio(1u128, 100u128)),
    //         })
    //         .unwrap(),
    //     ),
    // };

    // send the message along with funds. for now, they'd be directly from the non private key account
    // but ideally, funds will be sent with the governance execute contract message
}
