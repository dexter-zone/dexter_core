// For simulation we use the following parameters:
// Fees: 0% to ensure that the swap rate is 1:1 and the spread is equal to the difference between the offer and ask amounts.
// Amplication factor: From 1 to 1000 finding the optimal value for the swap rate.

use std::{fs::File, io::Write};

use anyhow::Result;
use cosmwasm_std::{Uint128, Addr, Coin, Decimal, Decimal256};
use cw_multi_test::{App, Executor};
use dexter::{asset::{AssetInfo, Asset}, vault::{FeeInfo, ExecuteMsg, SwapType, SingleSwapRequest}, pool::{SwapResponse, ConfigResponse}};
use itertools::Itertools;
use stable5pool::state::AssetScalingFactor;
use dexter::pool::QueryMsg as PoolQueryMsg;

use crate::utils::*;

pub mod utils;

#[allow(dead_code)]
struct StableSwapSimulation {
    amplification_coefficient: u64,
    assets_with_bootstrapping_amount: Vec<Asset>,
    scaling_factors: Vec<AssetScalingFactor>,
    swap_in_amount: Uint128,
    swap_in_asset: AssetInfo,
    swap_out_asset: AssetInfo,
    app: App,
    owner: Addr,
    user: Addr,
    vault: Addr,
    pool: Addr,
    lp_token: Addr,
}

impl StableSwapSimulation {

    fn init(
        amplification_coefficient: u64,
        assets_with_bootstrapping_amount: Vec<Asset>,
        native_asset_precisions: Vec<(String, u8)>,
        scaling_factors: Vec<AssetScalingFactor>,
        swap_in_amount: Uint128,
        swap_in_asset: AssetInfo,
        swap_out_asset: AssetInfo,
    ) -> Self {

        let fee_info = FeeInfo {
            total_fee_bps: 0,
            protocol_fee_percent: 0,
        };

        let owner = Addr::unchecked("owner");
        let user = Addr::unchecked("user");

        let asset_infos = assets_with_bootstrapping_amount
            .iter()
            .map(|asset| asset.info.clone())
            .collect();

        let native_assets: Vec<AssetInfo> = assets_with_bootstrapping_amount
            .iter()
            .filter(|asset| asset.info.is_native_token())
            .map(|asset| asset.info.clone())
            .collect();

        let coins = native_assets.iter()
        .map(|a| {
            Coin {
                denom: a.to_string(),
                amount: Uint128::new(100_000_000_000_000_000_000u128),
            }
        }).collect_vec();
        
        let mut app = mock_app(
            owner.clone(),
            coins,
        );

        // Send tokens to user
        let coins_to_send = native_assets.iter()
            .map(|a| {
                Coin {
                    denom: a.to_string(),
                    amount: Uint128::new(10_000_000_000_000_000_000u128),
                }
            }).collect_vec();

        app.send_tokens(owner.clone(),user.clone(),&coins_to_send).unwrap();


        let (vault_addr, pool_addr, lp_token_addr, _current_block_time) = instantiate_contract_generic(
            &mut app,
            &owner,
            fee_info,
            asset_infos,
            native_asset_precisions,
            scaling_factors.clone(),
            amplification_coefficient,
        );

        // Let's add liquidity corresponding to the bootstrapping amount to the pool
        add_liquidity_to_pool(&mut app, &owner, &user,  vault_addr.clone(), Uint128::from(1u64), pool_addr.clone(), assets_with_bootstrapping_amount.clone());

        Self {
            amplification_coefficient,
            assets_with_bootstrapping_amount,
            scaling_factors,
            swap_in_amount,
            swap_in_asset,
            swap_out_asset,
            app,
            owner,
            user,
            vault: vault_addr,
            pool: pool_addr,
            lp_token: lp_token_addr,
        }
    }

    fn perform_swap(&mut self) -> Result<(Uint128, Uint128, Uint128)> {
        let max_spread = Some(Decimal::from_ratio(50u128, 100u128));
        let swap_query_msg = PoolQueryMsg::OnSwap { 
            swap_type: SwapType::GiveIn {}, 
            offer_asset: self.swap_in_asset.clone(), 
            ask_asset: self.swap_out_asset.clone(),
            amount: self.swap_in_amount,
            max_spread: max_spread.clone(),
            belief_price: None
        };

        let swap_response: SwapResponse = self.app.wrap().query_wasm_smart(&self.pool, &swap_query_msg).unwrap();
        if !swap_response.response.is_success() {
            return Err(anyhow::format_err!("Swap failed: {:?}", swap_response.response));
        }

        let trade = swap_response.trade_params;

        let swap_msg = ExecuteMsg::Swap {
            swap_request: SingleSwapRequest {
                pool_id: Uint128::from(1u128),
                swap_type: SwapType::GiveIn {},
                asset_in: self.swap_in_asset.clone(),
                asset_out: self.swap_out_asset.clone(),
                amount: self.swap_in_amount,
                max_spread: max_spread.clone(),
                belief_price: None,
            },
            recipient: None,
            min_receive: None,
            max_spend: None,
        };

        self.app
            .execute_contract(
                self.user.clone(),
                self.vault.clone(),
                &swap_msg,
                &[Coin {
                    denom: self.swap_in_asset.to_string(),
                    amount: self.swap_in_amount,
                }],
            )
            .unwrap();

       Ok((trade.amount_in, trade.amount_out, trade.spread))
    }

    fn find_pool_balances_by_asset(&mut self) -> Vec<(AssetInfo, Uint128)> {
        // Pool balances after swap
        let pool_info: ConfigResponse = self
            .app
            .wrap()
            .query_wasm_smart(&self.pool, &PoolQueryMsg::Config {})
            .unwrap();

        let pool_balances_by_asset = pool_info
            .assets
            .iter()
            .map(|asset| {
                (asset.info.clone(), asset.amount)
            })
            .collect_vec();

        pool_balances_by_asset
    }

    fn run_simulation(&mut self, max_trades: Option<usize>) -> (Vec<Vec<(AssetInfo, Uint128)>>, Vec<(Uint128, Uint128, Decimal, Uint128)>) {

        let mut historical_pool_balances: Vec<Vec<(AssetInfo, Uint128)>> = vec![];
        let mut historical_trade_prices: Vec<(Uint128, Uint128, Decimal, Uint128)> = vec![];
        let pool_balances_before_swap = self.find_pool_balances_by_asset();
        historical_pool_balances.push(pool_balances_before_swap);

        loop {
            if max_trades.is_some() && historical_trade_prices.len() >= max_trades.unwrap() {
                break;
            }

            // Peform swap
            let result = self.perform_swap();
            if result.is_err() {
                println!("Swap failed: {:?}", result.err());
                break;

            } else {
                let (amount_in, amount_out, spread) = result.unwrap();
                
                let trade_price = Decimal::new(amount_out).checked_div(Decimal::new(amount_in)).unwrap();
                historical_trade_prices.push((amount_in, amount_out, trade_price, spread))
            }

            // Pool balances after swap
            let pool_balances = self.find_pool_balances_by_asset();
            historical_pool_balances.push(pool_balances);
        }

        (historical_pool_balances, historical_trade_prices)
    }

    
}

#[test]
fn run_stableswap_simulation() {

    let write_to_file = false;

    let mut simulation_runner = StableSwapSimulation::init(
        10,
        vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusdc".to_string(),
                },
                amount: Uint128::new(1_000_000_000_000u128),
            },
            // Native token 2
            Asset {
                info: AssetInfo::NativeToken { 
                    denom: "uusdt".to_string(),
                },
                amount: Uint128::new(1_000_000_000_000u128),
            },
        ],
        vec![("uusdc".to_string(), 6), ("uusdt".to_string(), 6)],
        vec![
            AssetScalingFactor {
                asset_info: AssetInfo::NativeToken {
                    denom: "uusdc".to_string(),
                },
                scaling_factor: Decimal256::from_ratio(1u128, 1u128),
            },
            AssetScalingFactor {
                asset_info: AssetInfo::NativeToken {
                    denom: "uusdt".to_string(),
                },
                scaling_factor: Decimal256::from_ratio(1u128, 1u128),
            },
        ],
        Uint128::new(100_000_000u128),
        AssetInfo::NativeToken {
            denom: "uusdc".to_string(),
        },
        AssetInfo::NativeToken {
            denom: "uusdt".to_string(),
        },
    );

    if !write_to_file {
        return;
    }

    // Write the simulation results to a file
    let mut file = File::create("simulation_results.csv").unwrap();
    file.write(b"reserves_uusdc,reserves_uusdt,amount_in,amount_out,trade_price,spread\n").unwrap();

    let simulation_results = simulation_runner.run_simulation(Some(100000));
    let zipped = simulation_results.0.iter().zip(simulation_results.1.iter());
    for (pool_balances, trade) in zipped {
        let (amount_in, amount_out, trade_price, spread) = (trade.0, trade.1, trade.2, trade.3);
        let (reserves_uusdc, reserves_uusdt) = (pool_balances[0].1, pool_balances[1].1);
        let line = format!("{},{},{},{},{},{}\n", reserves_uusdc, reserves_uusdt, amount_in, amount_out, trade_price, spread);
        file.write(line.as_bytes()).unwrap();
    }

}


#[test]
fn run_metastable_simulation() {

    let write_to_file = false;
    // Considering current rate:
    // 1 ATOM = 0.9800000 stkATOM
    // 1 stkATOM ~= 1.020408163265306122448979591836 uATOM
    let amp = 100;
    let trade_size = Uint128::new(18_000_000_000u128);
    // let trade_size = Uint128::new(100_000_000_000u128);
    // let trade_size = Uint128::new(100_000_000_000u128);

    let assets_with_bootstrapping_amount = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "uatom".to_string(),
            },
            amount: Uint128::new(180_000_000_000u128),
        },
        // Native token 2
        Asset {
            info: AssetInfo::NativeToken { 
                denom: "ustkatom".to_string(),
            },
            amount: Uint128::new(176_400_000_000u128),
        },
    ];

    let native_asset_precisions = vec![
        ("uatom".to_string(), 6),
        ("ustkatom".to_string(), 6),
    ];

    let scaling_factors = vec![
        AssetScalingFactor {
            asset_info: AssetInfo::NativeToken {
                denom: "uatom".to_string(),
            },
            scaling_factor: Decimal256::from_ratio(1u128, 1u128),
        },
        AssetScalingFactor {
            asset_info: AssetInfo::NativeToken {
                denom: "ustkatom".to_string(),
            },
            scaling_factor: Decimal256::from_ratio(1u128, 1u128),
        },
    ];

    let mut simulation_runner = StableSwapSimulation::init(
        amp,
        assets_with_bootstrapping_amount.clone(),
        native_asset_precisions.clone(),
        scaling_factors.clone(),
        trade_size,
        AssetInfo::NativeToken {
            denom: "uatom".to_string(),
        },
        AssetInfo::NativeToken {
            denom: "ustkatom".to_string(),
        },
    );

    if !write_to_file {
        return;
    }

    // Write the simulation results to a file
    let mut file = File::create(format!("simulation_results_metastable_{}|{}_amp={}_trade_size={}.csv", "uatom", "ustkatom", amp, trade_size)).unwrap();
    file.write(b"reserves_uatom,reserves_ustkatom,amount_in,amount_out,stkatom_price_vs_atom,trade_price,spread\n").unwrap();

    let simulation_results = simulation_runner.run_simulation(Some(100000));

    // Run reverse trade simulation
    let mut simulation_runner = StableSwapSimulation::init(
        amp,
        assets_with_bootstrapping_amount,
        native_asset_precisions,
        scaling_factors,
        trade_size,
        AssetInfo::NativeToken {
            denom: "ustkatom".to_string(),
        },
        AssetInfo::NativeToken {
            denom: "uatom".to_string(),
        },
    );

    let reverse_trade_simulation = simulation_runner.run_simulation(Some(100000));

    let zipped = simulation_results.0.iter().zip(simulation_results.1.iter()).rev();
    for (pool_balances, trade) in zipped {
        let (amount_in, amount_out, trade_price, spread) = (trade.0, trade.1, trade.2, trade.3);
        let (reserves_uatom, reserves_ustkatom) = (pool_balances[0].1, pool_balances[1].1);
        let line = format!("{},{},{},{},{},{},{}\n", reserves_uatom, reserves_ustkatom, amount_in, amount_out, trade_price, trade_price, spread);
        file.write(line.as_bytes()).unwrap();
    }

    // Write the reverse trade simulation results to the same file
    let zipped = reverse_trade_simulation.0.iter().zip(reverse_trade_simulation.1.iter());
    for (pool_balances, trade) in zipped {
        let (amount_in, amount_out, trade_price, spread) = (trade.0, trade.1, trade.2, trade.3);
        let (reserves_uatom, reserves_ustkatom) = (pool_balances[0].1, pool_balances[1].1);
        let stkatom_price_vs_atom = Decimal::one().checked_div(trade_price).unwrap();
        let line = format!("{},{},{},{},{},{},{}\n", reserves_uatom, reserves_ustkatom, amount_in, amount_out, stkatom_price_vs_atom, trade_price, spread);
        file.write(line.as_bytes()).unwrap();
    }

}