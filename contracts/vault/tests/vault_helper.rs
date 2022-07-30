use anyhow::Result as AnyResult;
use dexter::asset::{Asset, AssetInfo};
use dexter::vault::{PoolConfig, PoolType, QueryMsg};
use cosmwasm_std::{Addr, Binary};
use cw20::MinterResponse;
use cw_multi_test::{App, AppResponse, ContractWrapper, Executor};

pub struct VaultyHelper {
    pub owner: Addr,
    pub astro_token: Addr,
    pub vault: Addr,
    pub cw20_token_code_id: u64,
}

impl VaultyHelper {
    pub fn init(router: &mut App, owner: &Addr) -> Self {


        let pool_contract = Box::new(
            ContractWrapper::new_with_empty(
                xyk_pool::contract::execute, 
                xyk_pool::contract::instantiate,
                xyk_pool::contract::query,
            )
            .with_reply_empty(xyk_pool::contract::reply),
        );

        let pool_code_id = router.store_code(pool_contract);

        let vault_contract = Box::new(
            ContractWrapper::new_with_empty(
                dexter_vault::contract::execute,
                dexter_vault::contract::instantiate,
                dexter_vault::contract::query,
            )
            .with_reply_empty(dexter_vault::contract::reply),
        );

        let vault_code_id = router.store_code(vault_contract);

        let msg = dexter::vault::InstantiateMsg {
            pool_configs: vec![PoolConfig {
                code_id: pool_code_id,
                pool_type: PoolType::Xyk {},
                total_fee_bps: 100,
                maker_fee_bps: 10,
                is_disabled: false,
                is_generator_disabled: false,
            }],
            token_code_id: cw20_token_code_id,
            fee_address: None,
            generator_address: None,
            owner: owner.to_string(),
            whitelist_code_id: 0,
        };

        let vault = router
            .instantiate_contract(
                vault_code_id,
                owner.clone(),
                &msg,
                &[],
                String::from("ASTRO"),
                None,
            )
            .unwrap();

        Self {
            owner: owner.clone(),
            astro_token,
            vault,
            cw20_token_code_id,
        }
    }

    pub fn update_config(
        &mut self,
        router: &mut App,
        sender: &Addr,
        token_code_id: Option<u64>,
        fee_address: Option<String>,
        generator_address: Option<String>,
        whitelist_code_id: Option<u64>,
    ) -> AnyResult<AppResponse> {
        let msg = dexter::vault::ExecuteMsg::UpdateConfig {
            token_code_id,
            fee_address,
            generator_address,
            whitelist_code_id,
        };

        router.execute_contract(sender.clone(), self.vault.clone(), &msg, &[])
    }

    pub fn create_pair(
        &mut self,
        router: &mut App,
        sender: &Addr,
        pool_type: PoolType,
        tokens: [&Addr; 2],
        init_params: Option<Binary>,
    ) -> AnyResult<AppResponse> {
        let asset_infos = [
            AssetInfo::Token {
                contract_addr: tokens[0].clone(),
            },
            AssetInfo::Token {
                contract_addr: tokens[1].clone(),
            },
        ];

        let msg = dexter::vault::ExecuteMsg::CreatePair {
            pool_type,
            asset_infos,
            init_params,
        };

        router.execute_contract(sender.clone(), self.vault.clone(), &msg, &[])
    }

    pub fn create_pool_with_addr(
        &mut self,
        router: &mut App,
        sender: &Addr,
        pool_type: PoolType,
        tokens: [&Addr; 2],
        init_params: Option<Binary>,
    ) -> AnyResult<Addr> {
        self.create_pair(router, sender, pool_type, tokens, init_params)?;

        let asset_infos = [
            AssetInfo::Token {
                contract_addr: tokens[0].clone(),
            },
            AssetInfo::Token {
                contract_addr: tokens[1].clone(),
            },
        ];

        let res: PoolInfo = router.wrap().query_wasm_smart(
            self.vault.clone(),
            &QueryMsg::Pair {
                asset_infos: asset_infos.clone(),
            },
        )?;

        Ok(res.contract_addr)
    }
}

pub fn instantiate_token(
    app: &mut App,
    token_code_id: u64,
    owner: &Addr,
    token_name: &str,
    decimals: Option<u8>,
) -> Addr {
    let init_msg = dexter::token::InstantiateMsg {
        name: token_name.to_string(),
        symbol: token_name.to_string(),
        decimals: decimals.unwrap_or(6),
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
        marketing: None,
    };

    app.instantiate_contract(
        token_code_id,
        owner.clone(),
        &init_msg,
        &[],
        token_name,
        None,
    )
    .unwrap()
}
