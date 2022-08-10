use dexter::asset::{Asset, AssetInfo};
use dexter::vault::{
    ExecuteMsg as VaultExecuteMsg, InstantiateMsg as VaultInstantiateMsg, PoolConfig, PoolType,
    QueryMsg as VaultQueryMsg, PoolInfo, FeeInfo,Cw20HookMsg,
};
use xyk_pool::contract::query_on_swap;
use dexter::pool::{
    ResponseType, ConfigResponse, AfterJoinResponse, AfterExitResponse,  FeeResponse, SwapResponse,  CumulativePricesResponse, CumulativePriceResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use cosmwasm_std::{attr, to_binary, Addr, Coin, Decimal, Uint128};
use cw20::{BalanceResponse, Cw20Coin, Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse};
use cw_multi_test::{App, ContractWrapper, Executor};

const OWNER: &str = "owner";

fn mock_app(owner: Addr, coins: Vec<Coin>) -> App {
    App::new(|router, _, storage| {
        // initialization  moved to App construction
        router.bank.init_balance(storage, &owner, coins).unwrap()
    })
}


fn store_vault_code(app: &mut App) -> u64 {
    let factory_contract = Box::new(
        ContractWrapper::new_with_empty(
            dexter_vault::contract::execute,
            dexter_vault::contract::instantiate,
            dexter_vault::contract::query,
        )
        .with_reply_empty(dexter_vault::contract::reply),
    );
    app.store_code(factory_contract)
}

fn store_xyk_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(
        ContractWrapper::new_with_empty(
            xyk_pool::contract::execute,
            xyk_pool::contract::instantiate,
            xyk_pool::contract::query,
        )
        .with_reply_empty(xyk_pool::contract::reply),
    );
    app.store_code(pool_contract)
}

fn store_token_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        lp_token::contract::execute,
        lp_token::contract::instantiate,
        lp_token::contract::query,
    ));
    app.store_code(token_contract)
}

// Mints some Tokens to "to" recipient
fn mint_some_tokens(app: &mut App, owner: Addr, token_instance: Addr, amount: Uint128, to: String) {
    let msg = cw20::Cw20ExecuteMsg::Mint {
        recipient: to.clone(),
        amount: amount,
    };
    let res = app
        .execute_contract(owner.clone(), token_instance.clone(), &msg, &[])
        .unwrap();
    assert_eq!(res.events[1].attributes[1], attr("action", "mint"));
    assert_eq!(res.events[1].attributes[2], attr("to", to));
    assert_eq!(res.events[1].attributes[3], attr("amount", amount));
}

/// Initialize a new vault and a XYK Pool with the given assets
fn instantiate_contracts_instance(app: &mut App, owner: &Addr) -> (Addr,Addr,Addr,Addr) {
    let xyk_pool_code_id = store_xyk_pool_code(app);
    let vault_code_id = store_vault_code(app);
    let token_code_id = store_token_code(app);

    let pool_configs = vec![PoolConfig {
        code_id: xyk_pool_code_id,
        pool_type: PoolType::Xyk {},
        fee_info: FeeInfo {
            total_fee_bps: 300u16,
            protocol_fee_percent: 49u16,
            dev_fee_percent: 15u16,
            developer_addr: Some(Addr::unchecked("dev".to_string())) ,
        },
        is_disabled: false,
        is_generator_disabled: false,
    }];

    let vault_init_msg = VaultInstantiateMsg {
        pool_configs: pool_configs.clone(),
        lp_token_code_id: token_code_id,
        fee_collector: Some("fee_collector".to_string()),
        owner: owner.to_string(),
        generator_address: None,
    };

    // Initialize Vault contract instance
    let vault_instance = app
        .instantiate_contract(
            vault_code_id,
            owner.to_owned(),
            &vault_init_msg,
            &[],
            "vault",
            None,
        )
        .unwrap();

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

    let asset_infos = vec![
        AssetInfo::NativeToken {
            denom: "xprt".to_string(),
        },
        AssetInfo::Token {
            contract_addr: token_instance0.clone(),
        },
    ];

    // Initialize XYK Pool contract instance
    let current_block = app.block_info();    
    let msg = VaultExecuteMsg::CreatePool {
        pool_type: PoolType::Xyk {},
        asset_infos: asset_infos.to_vec(),
        init_params: None,
        lp_token_name: None,
        lp_token_symbol: None,
    };
    let res = app
        .execute_contract(Addr::unchecked(owner), vault_instance.clone(), &msg, &[])
        .unwrap();

    assert_eq!(res.events[1].attributes[1], attr("action", "create_pool"));
    assert_eq!(res.events[1].attributes[2], attr("pool_type", "xyk"));
    let pool_res: PoolInfo = app
        .wrap()
        .query_wasm_smart(
            vault_instance.clone(),
            &VaultQueryMsg::GetPoolById {
                pool_id: Uint128::from(1u128),
            },
        )
        .unwrap();

        assert_eq!(Uint128::from(1u128), pool_res.pool_id);
        assert_eq!(PoolType::Xyk {}, pool_res.pool_type);
        assert_eq!(Some(Addr::unchecked("dev".to_string())), pool_res.developer_addr);
    
        let assets = vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_instance0.clone(),
                },
                amount: Uint128::zero(),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "xprt".to_string(),
                },
                amount: Uint128::zero(),
            },
        ];
        
    //// -----x----- Check :: ConfigResponse for XYK Pool -----x----- ////

        let pool_config_res: ConfigResponse = app.wrap().query_wasm_smart( pool_res.pool_addr.clone().unwrap(), &QueryMsg::Config { },).unwrap();               
        assert_eq!(FeeInfo {
            total_fee_bps: 300u16,
            protocol_fee_percent: 49u16,
            dev_fee_percent: 15u16,
            developer_addr: Some(Addr::unchecked("dev".to_string())) ,
        }, pool_config_res.fee_info);
        assert_eq!(Uint128::from(1u128), pool_config_res.pool_id);
        assert_eq!(pool_res.lp_token_addr.clone().unwrap(), pool_config_res.lp_token_addr.unwrap());
        assert_eq!(vault_instance, pool_config_res.vault_addr);
        assert_eq!(assets, pool_config_res.assets);
        assert_eq!(PoolType::Xyk {}, pool_config_res.pool_type);
        assert_eq!(current_block.time.seconds(), pool_config_res.block_time_last);
      
    //// -----x----- Check :: FeeResponse for XYK Pool -----x----- ////
    let pool_fee_res: FeeResponse = app.wrap().query_wasm_smart( pool_res.pool_addr.clone().unwrap(), &QueryMsg::FeeParams {},).unwrap();
    assert_eq!(300u16, pool_fee_res.total_fee_bps);
    assert_eq!(49u16, pool_fee_res.protocol_fee_percent);
    assert_eq!(15u16, pool_fee_res.dev_fee_percent);
    assert_eq!(Some(Addr::unchecked("dev".to_string())), pool_fee_res.dev_fee_collector);            


    //// -----x----- Check :: Pool-ID for XYK Pool -----x----- ////
    let pool_id_res: Uint128 = app.wrap().query_wasm_smart( pool_res.pool_addr.clone().unwrap(), &QueryMsg::PoolId {},).unwrap();
    assert_eq!(Uint128::from(1u128),pool_id_res);


    return (vault_instance, pool_res.pool_addr.unwrap(), pool_res.lp_token_addr.unwrap(), token_instance0);
}




#[test]
fn test_provide_and_withdraw_liquidity() {
    let owner = Addr::unchecked("owner");
    let alice_address = Addr::unchecked("alice");
    let mut app = mock_app( owner.clone(),
        vec![
            Coin {
                denom: "xprt".to_string(),
                amount: Uint128::new(100_000_000_000u128),
            },
        ],
    );

    // Set Alice's balances
    app.send_tokens(owner.clone(),
            alice_address.clone(),
            &[  Coin {
                    denom: "xprt".to_string(),
                    amount: Uint128::new(1000_000_000u128),
                }]).unwrap();

    let (vault_instance, pool_addr, lp_token_addr, token_instance) = instantiate_contracts_instance(&mut app, &owner);

    mint_some_tokens(
                    &mut app,
                    owner.clone(),
                    token_instance.clone(),
                    Uint128::new(900_000_000_000),
                    alice_address.to_string(),
                );

    let empty_assets = vec![Asset {
        info: AssetInfo::Token {
            contract_addr: token_instance.clone(),
        },
        amount: Uint128::zero(),
    },Asset {
        info: AssetInfo::NativeToken {
            denom: "xprt".to_string(),
        },
        amount: Uint128::zero(),
    }];
        
    let join_pool_query_res : AfterJoinResponse =  app.wrap().query_wasm_smart( pool_addr.clone(), &QueryMsg::OnJoinPool {
        assets_in:  None,
        // assets_in:  Some(vec![
        //     Asset {
        //         info: AssetInfo::Token {
        //             contract_addr: token_instance.clone(),
        //         },
        //         amount: Uint128::zero(),
        //     },
        //     Asset {
        //         info: AssetInfo::NativeToken {
        //             denom: "xprt".to_string(),
        //         },
        //         amount: Uint128::zero(),
        //     },
        // ]),
        mint_amount: None
    },).unwrap();
    assert_eq!(ResponseType::Failure {  },join_pool_query_res.response);
    assert_eq!(Uint128::zero(),join_pool_query_res.new_shares);
    assert_eq!(empty_assets,join_pool_query_res.provided_assets);
        
  



    // // Set Alice's balances
    // router
    //     .send_tokens(
    //         owner.clone(),
    //         alice_address.clone(),
    //         &[
    //             Coin {
    //                 denom: "uusd".to_string(),
    //                 amount: Uint128::new(233u128),
    //             },
    //             Coin {
    //                 denom: "uluna".to_string(),
    //                 amount: Uint128::new(200u128),
    //             },
    //             Coin {
    //                 denom: "cny".to_string(),
    //                 amount: Uint128::from(1000u16),
    //             },
    //         ],
    //     )
    //     .unwrap();

    // // Init pair
    // let pair_instance = instantiate_pair(&mut router, &owner);

    // let res: PairInfo = router
    //     .wrap()
    //     .query_wasm_smart(pair_instance.to_string(), &QueryMsg::Pair {})
    //     .unwrap();
    // let lp_token = res.liquidity_token;

    // assert_eq!(
    //     res.asset_infos,
    //     [
    //         AssetInfo::NativeToken {
    //             denom: "uusd".to_string(),
    //         },
    //         AssetInfo::NativeToken {
    //             denom: "uluna".to_string(),
    //         },
    //     ],
    // );

    // // When dealing with native tokens the transfer should happen before the contract call, which cw-multitest doesn't support
    // // Set Alice's balances
    // router
    //     .send_tokens(
    //         owner.clone(),
    //         pair_instance.clone(),
    //         &[
    //             Coin {
    //                 denom: "uusd".to_string(),
    //                 amount: Uint128::new(100u128),
    //             },
    //             Coin {
    //                 denom: "uluna".to_string(),
    //                 amount: Uint128::new(100u128),
    //             },
    //         ],
    //     )
    //     .unwrap();

    // // Provide liquidity
    // let (msg, coins) = provide_liquidity_msg(Uint128::new(100), Uint128::new(100), None, None);
    // let res = router
    //     .execute_contract(alice_address.clone(), pair_instance.clone(), &msg, &coins)
    //     .unwrap();

    // assert_eq!(
    //     res.events[1].attributes[1],
    //     attr("action", "provide_liquidity")
    // );
    // assert_eq!(res.events[1].attributes[3], attr("receiver", "alice"),);
    // assert_eq!(
    //     res.events[1].attributes[4],
    //     attr("assets", "100uusd, 100uluna")
    // );
    // assert_eq!(
    //     res.events[1].attributes[5],
    //     attr("share", 100u128.to_string())
    // );
    // assert_eq!(res.events[3].attributes[1], attr("action", "mint"));
    // assert_eq!(res.events[3].attributes[2], attr("to", "alice"));
    // assert_eq!(res.events[3].attributes[3], attr("amount", 100.to_string()));

    // // Provide liquidity for receiver
    // let (msg, coins) = provide_liquidity_msg(
    //     Uint128::new(100),
    //     Uint128::new(100),
    //     Some("bob".to_string()),
    //     None,
    // );
    // let res = router
    //     .execute_contract(alice_address.clone(), pair_instance.clone(), &msg, &coins)
    //     .unwrap();

    // assert_eq!(
    //     res.events[1].attributes[1],
    //     attr("action", "provide_liquidity")
    // );
    // assert_eq!(res.events[1].attributes[3], attr("receiver", "bob"),);
    // assert_eq!(
    //     res.events[1].attributes[4],
    //     attr("assets", "100uusd, 100uluna")
    // );
    // assert_eq!(
    //     res.events[1].attributes[5],
    //     attr("share", 50u128.to_string())
    // );
    // assert_eq!(res.events[3].attributes[1], attr("action", "mint"));
    // assert_eq!(res.events[3].attributes[2], attr("to", "bob"));
    // assert_eq!(res.events[3].attributes[3], attr("amount", 50.to_string()));

    // // Checking withdraw liquidity
    // let token_contract_code_id = store_token_code(&mut router);
    // let foo_token = router
    //     .instantiate_contract(
    //         token_contract_code_id,
    //         owner.clone(),
    //         &astroport::token::InstantiateMsg {
    //             name: "Foo token".to_string(),
    //             symbol: "FOO".to_string(),
    //             decimals: 6,
    //             initial_balances: vec![Cw20Coin {
    //                 address: alice_address.to_string(),
    //                 amount: Uint128::from(1000000000u128),
    //             }],
    //             mint: None,
    //             marketing: None,
    //         },
    //         &[],
    //         String::from("FOO"),
    //         None,
    //     )
    //     .unwrap();

    // let msg = Cw20ExecuteMsg::Send {
    //     contract: pair_instance.to_string(),
    //     amount: Uint128::from(50u8),
    //     msg: to_binary(&Cw20HookMsg::WithdrawLiquidity {}).unwrap(),
    // };
    // // Try to send withdraw liquidity with FOO token
    // let err = router
    //     .execute_contract(alice_address.clone(), foo_token.clone(), &msg, &[])
    //     .unwrap_err();
    // assert_eq!(err.root_cause().to_string(), "Unauthorized");
    // // Withdraw with LP token is successful
    // router
    //     .execute_contract(alice_address.clone(), lp_token.clone(), &msg, &[])
    //     .unwrap();

    // let err = router
    //     .execute_contract(
    //         alice_address.clone(),
    //         pair_instance.clone(),
    //         &ExecuteMsg::Swap {
    //             offer_asset: Asset {
    //                 info: AssetInfo::NativeToken {
    //                     denom: "cny".to_string(),
    //                 },
    //                 amount: Uint128::from(10u8),
    //             },
    //             belief_price: None,
    //             max_spread: None,
    //             to: None,
    //         },
    //         &[Coin {
    //             denom: "cny".to_string(),
    //             amount: Uint128::from(10u8),
    //         }],
    //     )
    //     .unwrap_err();
    // assert_eq!(
    //     err.root_cause().to_string(),
    //     "Asset mismatch between the requested and the stored asset in contract"
    // );

    // // Check pair config
    // let config: ConfigResponse = router
    //     .wrap()
    //     .query_wasm_smart(pair_instance.to_string(), &QueryMsg::Config {})
    //     .unwrap();
    // assert_eq!(
    //     config,
    //     ConfigResponse {
    //         block_time_last: router.block_info().time.seconds(),
    //         params: None
    //     }
    // )
}

// fn instantiate_pool(mut router: &mut App, owner: &Addr) -> Addr {
//     let token_contract_code_id = store_token_code(&mut router);

//     let pool_contract_code_id = store_pool_code(&mut router);
    

//     let msg = InstantiateMsg {
//         asset_infos: [
//             AssetInfo::NativeToken {
//                 denom: "uusd".to_string(),
//             },
//             AssetInfo::Token {
//                 contract_addr: token_instance0.to_string(),
//             },
//         ].to_vec(),
//         pool_id:Uint128::from(pool_contract_code_id),
//         pool_type:PoolType::Xyk {},
//         fee_info:FeeInfo { total_fee_bps:Decimal::new(Uint128::new(100)), protocol_fee_percent:2, dev_fee_percent:5, developer_addr:Some(Addr::unchecked("developer_addr"))}, 
//         lp_token_code_id:token_contract_code_id,
//         lp_token_name:Some(token_name.to_string()),
//         lp_token_symbol:Some(token_name.to_string()),
//         vault_addr:Addr::unchecked("vault"),
//         init_params: None,
//     };

//     let pool = router
//         .instantiate_contract(
//             pool_contract_code_id,
//             owner.clone(),
//             &msg,
//             &[],
//             token_name.to_string(),
//             Some(String::from("Dexter LP token"))
//             )
//            .unwrap();

//     let res: PoolInfo = router
//         .wrap()
//         .query_wasm_smart(pool.clone(), &QueryMsg::Config {})
//         .unwrap();
 

//     pool
// }

//  #[test]
//  fn test_compatibility_of_tokens_with_different_precision() {
//      let owner = Addr::unchecked(OWNER);

//      let mut app = mock_app(
//          owner.clone(),
//          vec![
//              Coin {
//                  denom: "uusd".to_string(),
//                  amount: Uint128::new(100_000_000_000000u128),
//              },
//              Coin {
//                  denom: "uluna".to_string(),
//                  amount: Uint128::new(100_000_000_000000u128),
//              },
//          ],
//      );

//      let token_code_id = store_token_code(&mut app);

//      let x_amount = Uint128::new(1000000_00000);
//      let y_amount = Uint128::new(1000000_0000000);
//      let x_offer = Uint128::new(1_00000);
//      let y_expected_return = Uint128::new(1_0000000);

//      let token_name = "Xtoken";

//      let init_msg = TokenInstantiateMsg {
//          name: token_name.to_string(),
//          symbol: token_name.to_string(),
//          decimals: 5,
//          initial_balances: vec![Cw20Coin {
//              address: OWNER.to_string(),
//              amount: x_amount + x_offer,
//          }],
//        mint: Some(MinterResponse {
//                  minter: String::from(OWNER),
//              cap: None,
//          }),
//          marketing: None,
//      };

//      let token_x_instance = app
//          .instantiate_contract(
//              token_code_id,
//              Addr::unchecked(owner.clone()),
//              &init_msg,
//              &[],
//              token_name,
//              None,
//          )
//          .unwrap();

//      let token_name = "Ytoken";

//     let init_msg = TokenInstantiateMsg {
//          name: token_name.to_string(),
//          symbol: token_name.to_string(),
//          decimals: 7,
//          initial_balances: vec![Cw20Coin {
//              address: OWNER.to_string(),
//              amount: y_amount,
//          }],
//          mint: Some(MinterResponse {
//              minter: String::from(OWNER),
//              cap: None,
//          }),
//          marketing: None,
//      };

//      let token_y_instance = app
//          .instantiate_contract(
//              token_code_id,
//              Addr::unchecked(owner.clone()),
//              &init_msg,
//             &[],
//              token_name,
//              None,
//          )
//          .unwrap();

//      let pool_code_id = store_pool_code(&mut app);
//      let vault_code_id = store_vault_code(&mut app);

//      let init_msg = VaultInstantiateMsg {
//         pool_configs: vec![PoolConfig {
//              code_id: pool_code_id,
//              pool_type: PoolType::Xyk {},
//              fee_info:FeeInfo { total_fee_bps:Decimal::new(Uint128::new(100)), protocol_fee_percent:1, dev_fee_percent:1, developer_addr:Some(Addr::unchecked("developer_addr"))},
//              is_disabled: false,
//              is_generator_disabled: false,
//          }],
//          lp_token_code_id:123,
//          fee_collector:None,
//          generator_address: Some(String::from("generator")),
//          owner: owner.to_string(),
//      };

//      let vault_instance = app
//          .instantiate_contract(
//              vault_code_id,
//              owner.clone(),
//             &init_msg,
//              &[],
//              "VAULT",
//              None,
//          )
//          .unwrap();

//      let msg = VaultExecuteMsg::CreatePool {
//          asset_infos: [
//              AssetInfo::Token {
//                  contract_addr: token_x_instance.clone(),
//              },
//              AssetInfo::Token {
//                  contract_addr: token_y_instance.clone(),
//              },
//          ].to_vec(),
//          pool_type: PoolType::Xyk {},
//          lp_token_name:Some(token_name.to_string()),
//          lp_token_symbol:Some(token_name.to_string()),
//          init_params: None,
//      };

//      app.execute_contract(owner.clone(), vault_instance.clone(), &msg, &[])
//          .unwrap();
//       /* 
//       let msg=VaultExecuteMsg::JoinPool 
//        { 
//         pool_id:Uint128::from(pool_code_id), 
//         recipient: Some(String::from("recipient")), 
//         assets:Some(assets: [
//             Asset {
//                 info: AssetInfo::Token {
//                     contract_addr: token_x_instance.clone(),
//                 },
//                 amount: x_amount,
//             },
//             Asset {
//                 info: AssetInfo::Token {
//                     contract_addr: token_y_instance.clone(),
//                 },
//                 amount: y_amount,
//             },
//         ].to_vec()),
//         lp_to_mint:Some(cosmwasm_std::Uint128::new(1000)), 
//         auto_stake:Some(false), 
//     };
//     app.execute_contract(owner.clone(), vault_instance.clone(), &msg, &[])
//     .unwrap();
//    */ 
//       let pool_instance=instantiate_pool(&mut app, &owner);

//      let msg = VaultQueryMsg::PoolConfig {
//           pool_type:PoolType::Xyk {},    
//         };
//      let res: PoolInfo = app
//          .wrap()
//          .query_wasm_smart(&pool_instance, &msg)
//          .unwrap();

//      let pool_instance=instantiate_pool(&mut app, &owner);


//     let msg = Cw20ExecuteMsg::IncreaseAllowance {
//          spender:pool_instance.to_string(),
//          expires: None,
//          amount: x_amount + x_offer,
//      };

//      app.execute_contract(owner.clone(), token_x_instance.clone(), &msg, &[])
//          .unwrap();

//      let msg = Cw20ExecuteMsg::IncreaseAllowance {
//          spender: pool_instance.to_string(),
//          expires: None,
//          amount: y_amount,
//      };

//      app.execute_contract(owner.clone(), token_y_instance.clone(), &msg, &[])
//          .unwrap();

//      let msg = ExecuteMsg::UpdateLiquidity{
//          assets: [
//              Asset {
//                  info: AssetInfo::Token {
//                      contract_addr: token_x_instance.clone(),
//                  },
//                  amount: x_amount,
//              },
//              Asset {
//                  info: AssetInfo::Token {
//                      contract_addr: token_y_instance.clone(),
//                  },
//                  amount: y_amount,
//              },
//          ].to_vec(),
//     };

//      app.execute_contract(owner.clone(), pool_instance.clone(), &msg, &[])
//          .unwrap();

//      let user = Addr::unchecked("user");

//      let swap_msg = Cw20ExecuteMsg::Send {
//          contract: pool_instance.to_string(),
//           msg: to_binary(&QueryMsg::OnSwap 
//             { 
//               swap_type: dexter::vault::SwapType::GiveIn {  }, 
//               offer_asset: AssetInfo::Token { contract_addr: token_x_instance.clone() }, 
//               ask_asset: AssetInfo::Token { contract_addr:token_y_instance.clone() },
//               amount:x_offer,  
//           })
//           .unwrap(),
//           amount: x_offer,
//        };
         
//      // try to swap after provide liquidity
//      app.execute_contract(owner.clone(), token_x_instance.clone(), &swap_msg, &[])
//          .unwrap();

//      let msg = Cw20QueryMsg::Balance {
//          address: user.to_string(),
//      };

//      let res: BalanceResponse = app
//          .wrap()
//          .query_wasm_smart(&token_y_instance, &msg)
//          .unwrap();

//      let acceptable_spread_amount = Uint128::new(10);

//      assert_eq!(res.balance, y_expected_return - acceptable_spread_amount);
//  }

/* 
 #[test]
 fn test_if_twap_is_calculated_correctly_when_pool_idles() {
     let owner = Addr::unchecked("owner");
     let user1 = Addr::unchecked("user1");

     let mut app = mock_app(
         owner.clone(),
         vec![
             Coin {
                 denom: "uusd".to_string(),
                 amount: Uint128::new(100_000_000_000000u128),
             },
             Coin {
                 denom: "uluna".to_string(),
                 amount: Uint128::new(100_000_000_000000u128),
             },
         ],
     );

     // Set Alice's balances
     app.send_tokens(
         owner.clone(),
         user1.clone(),
         &[
             Coin {
                 denom: "uusd".to_string(),
                 amount: Uint128::new(4000000_000000),
             },
             Coin {
                 denom: "uluna".to_string(),
                 amount: Uint128::new(2000000_000000),
             },
         ],
     )
     .unwrap();

     // Instantiate pool
     let pool_instance = instantiate_pool(&mut app, &user1);

     // Provide liquidity, accumulators are empty
     let (msg,coins)=QueryMsg::OnJoinPool{
            Some{Uint128::new(1000000_000000)},
     };
     app.execute_contract(user1.clone(), pool_instance.clone(), &msg, &coins)
         .unwrap();

     const BLOCKS_PER_DAY: u64 = 17280;
     const ELAPSED_SECONDS: u64 = BLOCKS_PER_DAY * 5;

     // A day later
     app.update_block(|b| {
         b.height += BLOCKS_PER_DAY;
         b.time = b.time.plus_seconds(ELAPSED_SECONDS);
     });

     // Provide liquidity, accumulators firstly filled with the same prices
     let (msg, coins) = provide_liquidity_msg(
         Uint128::new(2000000_000000),
         Uint128::new(1000000_000000),
         None,
         Some(Decimal::percent(50)),
     );
     app.execute_contract(user1.clone(), pool_instance.clone(), &msg, &coins)
         .unwrap();

     // Get current twap accumulator values
     let msg = QueryMsg::CumulativePrices {};
     let cpr_old: CumulativePricesResponse =
         app.wrap().query_wasm_smart(&pool_instance, &msg).unwrap();

     // A day later
     app.update_block(|b| {
         b.height += BLOCKS_PER_DAY;
         b.time = b.time.plus_seconds(ELAPSED_SECONDS);
     });

     // Get current cumulative price values; they should have been updated by the query method with new 2/1 ratio
     let msg = QueryMsg::CumulativePrices {};
     let cpr_new: CumulativePricesResponse =
         app.wrap().query_wasm_smart(&pool_instance, &msg).unwrap();

     let twap0 = cpr_new.price0_cumulative_new - cpr_old.price0_cumulative_last;
     let twap1 = cpr_new.price1_cumulative_last - cpr_old.price1_cumulative_last;

     // Prices weren't changed for the last day, uusd amount in pool = 3000000_000000, uluna = 2000000_000000
     // In accumulators we don't have any precision so we rely on elapsed time so we don't need to consider it
     let price_precision = Uint128::from(10u128.pow(TWAP_PRECISION.into()));
     assert_eq!(twap0 / price_precision, Uint128::new(57600)); // 0.666666 * ELAPSED_SECONDS (86400)
     assert_eq!(twap1 / price_precision, Uint128::new(129600)); //   1.5 * ELAPSED_SECONDS
 }

*/ 
 
//No use of this function
/* 
#[test]
 fn create_pool_with_same_assets() {
     let owner = Addr::unchecked("owner");
     let mut router = mock_app(
         owner.clone(),
         vec![
             Coin {
                 denom: "uusd".to_string(),
                 amount: Uint128::new(100_000_000_000u128),
             },
             Coin {
                 denom: "uluna".to_string(),
                 amount: Uint128::new(100_000_000_000u128),
             },
         ],
     );

     let token_contract_code_id = store_token_code(&mut router);
     let pool_contract_code_id = store_pool_code(&mut router);
     let token_name="XToken";
     let msg = InstantiateMsg {
         asset_infos: [
             AssetInfo::NativeToken {
                 denom: "uusd".to_string(),
             },
             AssetInfo::NativeToken {
                 denom: "uusd".to_string(),
             },
         ].to_vec(),
         pool_id:Uint128::from(pool_contract_code_id),
        pool_type:PoolType::Xyk {},
        fee_info:FeeInfo { total_fee_bps:Decimal::new(Uint128::new(100)), protocol_fee_percent:2, dev_fee_percent:5, developer_addr:Some(Addr::unchecked("developer_addr"))}, 
        lp_token_code_id:token_contract_code_id,
        lp_token_name:Some(token_name.to_string()),
        lp_token_symbol:Some(token_name.to_string()),
        vault_addr:Addr::unchecked("vault"),
        init_params:None,
     };

     let resp = router
         .instantiate_contract(
             pool_contract_code_id,
             owner.clone(),
             &msg,
             &[],
             String::from("POOL"),
             None,
         )
         .unwrap_err();

     assert_eq!(
         resp.root_cause().to_string(),
         "Doubling assets in asset infos"
     )
 }*/