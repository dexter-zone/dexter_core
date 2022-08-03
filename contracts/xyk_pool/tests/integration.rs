use dexter::asset::{Asset, AssetInfo};
use dexter::vault::{
    ExecuteMsg as VaultExecuteMsg, InstantiateMsg as VaultInstantiateMsg, PoolConfig, PoolType,
    QueryMsg as VaultQueryMsg, PoolInfo, FeeInfo,Cw20HookMsg
};
use xyk_pool::contract::query_on_swap;
use dexter::pool::{
    ConfigResponse, CumulativePricesResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
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

fn store_token_code(app: &mut App) -> u64 {
     let token_contract = Box::new(ContractWrapper::new_with_empty(
         dexter_vault::contract::execute,
         dexter_vault::contract::instantiate,
         dexter_vault::contract::query,
             ));

     app.store_code(token_contract)
 }

fn store_pool_code(app: &mut App) -> u64 {
    let pool_contract = Box::new(
        ContractWrapper::new_with_empty(
            dexter_vault::contract::execute,
            dexter_vault::contract::instantiate,
            dexter_vault::contract::query,
        )
        .with_reply_empty(xyk_pool::contract::reply),
    );

    app.store_code(pool_contract)
}

fn store_vault_code(app: &mut App) -> u64 {
    let vault_contract = Box::new(
        ContractWrapper::new_with_empty(
            dexter_vault::contract::execute,
            dexter_vault::contract::instantiate,
            dexter_vault::contract::query,
        )
        .with_reply_empty(xyk_pool::contract::reply),
    );

    app.store_code(vault_contract)
}

fn instantiate_pool(mut router: &mut App, owner: &Addr) -> Addr {
    let token_contract_code_id = store_token_code(&mut router);

    let pool_contract_code_id = store_pool_code(&mut router);
    let token_name = "Xtoken";
    
    let msg = InstantiateMsg {
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        ].to_vec(),
        pool_id:Uint128::from(pool_contract_code_id),
        pool_type:PoolType::Xyk {},
        fee_info:FeeInfo { total_fee_bps:Decimal::new(Uint128::new(100)), protocol_fee_percent:2, dev_fee_percent:5, developer_addr:Some(Addr::unchecked("developer_addr"))}, 
        lp_token_code_id:token_contract_code_id,
        lp_token_name:Some(token_name.to_string()),
        lp_token_symbol:Some(token_name.to_string()),
        vault_addr:Addr::unchecked("vault"),
        init_params: None,
    };

    let pool = router
        .instantiate_contract(
            pool_contract_code_id,
            owner.clone(),
            &msg,
            &[],
            token_name.to_string(),
            Some(String::from("Dexter LP token"))
            )
           .unwrap();

    let res: PoolInfo = router
        .wrap()
        .query_wasm_smart(pool.clone(), &QueryMsg::Config {})
        .unwrap();
 

    pool
}

 #[test]
 fn test_compatibility_of_tokens_with_different_precision() {
     let owner = Addr::unchecked(OWNER);

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

     let token_code_id = store_token_code(&mut app);

     let x_amount = Uint128::new(1000000_00000);
     let y_amount = Uint128::new(1000000_0000000);
     let x_offer = Uint128::new(1_00000);
     let y_expected_return = Uint128::new(1_0000000);

     let token_name = "Xtoken";

     let init_msg = TokenInstantiateMsg {
         name: token_name.to_string(),
         symbol: token_name.to_string(),
         decimals: 5,
         initial_balances: vec![Cw20Coin {
             address: OWNER.to_string(),
             amount: x_amount + x_offer,
         }],
       mint: Some(MinterResponse {
                 minter: String::from(OWNER),
             cap: None,
         }),
         marketing: None,
     };

     let token_x_instance = app
         .instantiate_contract(
             token_code_id,
             owner.clone(),
             &init_msg,
             &[],
             token_name,
             None,
         )
         .unwrap();

     let token_name = "Ytoken";

    let init_msg = TokenInstantiateMsg {
         name: token_name.to_string(),
         symbol: token_name.to_string(),
         decimals: 7,
         initial_balances: vec![Cw20Coin {
             address: OWNER.to_string(),
             amount: y_amount,
         }],
         mint: Some(MinterResponse {
             minter: String::from(OWNER),
             cap: None,
         }),
         marketing: None,
     };

     let token_y_instance = app
         .instantiate_contract(
             token_code_id,
            owner.clone(),
             &init_msg,
            &[],
             token_name,
             None,
         )
         .unwrap();

     let pool_code_id = store_pool_code(&mut app);
     let vault_code_id = store_vault_code(&mut app);

     let init_msg = VaultInstantiateMsg {
        pool_configs: vec![PoolConfig {
             code_id: pool_code_id,
             pool_type: PoolType::Xyk {},
             fee_info:FeeInfo { total_fee_bps:Decimal::new(Uint128::new(100)), protocol_fee_percent:2, dev_fee_percent:5, developer_addr:Some(Addr::unchecked("developer_addr"))},
             is_disabled: false,
             is_generator_disabled: false,
         }],
         lp_token_code_id:vault_code_id,
         fee_collector:Some(String::from("Fee_Collector")),
         generator_address: Some(String::from("generator")),
         owner: owner.to_string(),
     };

     let vault_instance = app
         .instantiate_contract(
             vault_code_id,
             owner.clone(),
            &init_msg,
             &[],
             "VAULT",
             None,
         )
         .unwrap();

     let msg = VaultExecuteMsg::CreatePool {
         asset_infos: [
             AssetInfo::Token {
                 contract_addr: token_x_instance.clone(),
             },
             AssetInfo::Token {
                 contract_addr: token_y_instance.clone(),
             },
         ].to_vec(),
         pool_type: PoolType::Xyk {},
         lp_token_name:Some(token_name.to_string()),
         lp_token_symbol:Some(token_name.to_string()),
         init_params: None,
     };

     app.execute_contract(owner.clone(), vault_instance.clone(), &msg, &[])
         .unwrap();

     let msg = VaultQueryMsg::PoolConfig {
         asset_infos: [
             AssetInfo::Token {
                 contract_addr: token_x_instance.clone(),
             },
             AssetInfo::Token {
                 contract_addr: token_y_instance.clone(),
             },
         ].to_vec(),
          pool_type:PoolType::Xyk {},
            
        };
     let res: PoolInfo = app
         .wrap()
         .query_wasm_smart(&vault_instance, &msg)
         .unwrap();

     let pool_instance=instantiate_pool(&mut app, &owner);


    let msg = Cw20ExecuteMsg::IncreaseAllowance {
         spender:pool_instance.to_string(),
         expires: None,
         amount: x_amount + x_offer,
     };

     app.execute_contract(owner.clone(), token_x_instance.clone(), &msg, &[])
         .unwrap();

     let msg = Cw20ExecuteMsg::IncreaseAllowance {
         spender: pool_instance.to_string(),
         expires: None,
         amount: y_amount,
     };

     app.execute_contract(owner.clone(), token_y_instance.clone(), &msg, &[])
         .unwrap();

     let msg = ExecuteMsg::UpdateLiquidity{
         assets: [
             Asset {
                 info: AssetInfo::Token {
                     contract_addr: token_x_instance.clone(),
                 },
                 amount: x_amount,
             },
             Asset {
                 info: AssetInfo::Token {
                     contract_addr: token_y_instance.clone(),
                 },
                 amount: y_amount,
             },
         ].to_vec(),
    };

     app.execute_contract(owner.clone(), pool_instance.clone(), &msg, &[])
         .unwrap();

     let user = Addr::unchecked("user");

     let swap_msg = Cw20ExecuteMsg::Send {
         contract: pool_instance.to_string(),
          msg: to_binary(&QueryMsg::OnSwap 
            { 
              swap_type: dexter::vault::SwapType::GiveIn {  }, 
              offer_asset: AssetInfo::Token { contract_addr: token_x_instance.clone() }, 
              ask_asset: AssetInfo::Token { contract_addr:token_y_instance.clone() },
              amount:x_offer,  
          })
          .unwrap(),
          amount: x_offer,
       };

     // try to swap after provide liquidity
     app.execute_contract(owner.clone(), token_x_instance.clone(), &swap_msg, &[])
         .unwrap();

     let msg = Cw20QueryMsg::Balance {
         address: user.to_string(),
     };

     let res: BalanceResponse = app
         .wrap()
         .query_wasm_smart(&token_y_instance, &msg)
         .unwrap();

     let acceptable_spread_amount = Uint128::new(10);

     assert_eq!(res.balance, y_expected_return - acceptable_spread_amount);
 }

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
     let (msg, Coins) =OnJoinPool(
         Uint128::new(1000000_000000),
         Uint128::new(1000000_000000),
         None,
         Option::from(Decimal::one()),
     );
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

     let twap0 = cpr_new.price0_cumulative_last - cpr_old.price0_cumulative_last;
     let twap1 = cpr_new.price1_cumulative_last - cpr_old.price1_cumulative_last;

     // Prices weren't changed for the last day, uusd amount in pool = 3000000_000000, uluna = 2000000_000000
     // In accumulators we don't have any precision so we rely on elapsed time so we don't need to consider it
     let price_precision = Uint128::from(10u128.pow(TWAP_PRECISION.into()));
     assert_eq!(twap0 / price_precision, Uint128::new(57600)); // 0.666666 * ELAPSED_SECONDS (86400)
     assert_eq!(twap1 / price_precision, Uint128::new(129600)); //   1.5 * ELAPSED_SECONDS
 }


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