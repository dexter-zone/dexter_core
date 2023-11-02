use cosmwasm_std::{Deps, Coin, Uint128};
use dexter::{governance_admin::{PoolCreationRequest, UserDeposit, UserTotalDeposit, FundsCategory}, querier::query_vault_config, asset::{AssetInfo, Asset}, vault::PoolCreationFee};

use crate::{contract::ContractResult, utils::queries::query_proposal_min_deposit_amount};


/// Sums up the requirements in terms of pool creation fee, pool bootstrapping amount and reward schedule
/// amounts and returns it
/// This can later be used to validate if the user has sent enough funds to create the pool and
/// transfer Cw20 token to this contract for further processing
pub fn find_total_funds_needed(
    deps: Deps,
    gov_proposal_min_deposit_amount: &Vec<Coin>,
    pool_creation_request_proposal: &dexter::governance_admin::PoolCreationRequest,
) -> ContractResult<UserTotalDeposit> {
    let mut total_funds_map = std::collections::HashMap::new();
    let mut user_deposits_detailed = vec![];

    let vault_addr = deps
        .api
        .addr_validate(&pool_creation_request_proposal.vault_addr)?;

    // find the pool creation fee by querying the vault contract currently
    let vault_config = query_vault_config(&deps.querier, vault_addr.to_string())?;
    let pool_creation_fee = vault_config.pool_creation_fee;

    // add the proposal deposit to the total funds.
    // We need to query the gov module to figure this out
    let mut proposal_deposit_assets = vec![];

    for coin in gov_proposal_min_deposit_amount {
        let asset_info = AssetInfo::native_token(coin.denom.clone());
        let amount: Uint128 = total_funds_map
            .get(&asset_info)
            .cloned()
            .unwrap_or_default();

        let c_amount = coin.amount;
        total_funds_map.insert(asset_info.clone(), amount.checked_add(c_amount)?);
        proposal_deposit_assets.push(Asset {
            info: asset_info,
            amount: c_amount,
        });
    }

    user_deposits_detailed.push(UserDeposit {
        category: FundsCategory::ProposalDeposit,
        assets: proposal_deposit_assets,
    });

    // add the pool creation fee to the total funds
    if let PoolCreationFee::Enabled { fee } = pool_creation_fee {
        let amount = total_funds_map.get(&fee.info).cloned().unwrap_or_default();
        total_funds_map.insert(fee.clone().info, amount.checked_add(fee.amount)?);

        user_deposits_detailed.push(UserDeposit {
            category: FundsCategory::PoolCreationFee,
            assets: vec![fee],
        });
    }

    // add the bootstrapping amount to the total funds
    if let Some(bootstrapping_amount) = &pool_creation_request_proposal.bootstrapping_amount {
        for asset in bootstrapping_amount {
            let amount = total_funds_map
                .get(&asset.info)
                .cloned()
                .unwrap_or_default();
            total_funds_map.insert(asset.info.clone(), amount.checked_add(asset.amount)?);
        }

        user_deposits_detailed.push(UserDeposit {
            category: FundsCategory::PoolBootstrappingAmount,
            assets: bootstrapping_amount.clone(),
        });
    }

    // add the reward schedule amounts to the total funds
    if let Some(reward_schedules) = &pool_creation_request_proposal.reward_schedules {
        for reward_schedule in reward_schedules {
            let amount = total_funds_map
                .get(&reward_schedule.asset)
                .cloned()
                .unwrap_or_default();
            total_funds_map.insert(
                reward_schedule.asset.clone(),
                amount.checked_add(reward_schedule.amount)?,
            );

            user_deposits_detailed.push(UserDeposit {
                category: FundsCategory::RewardScheduleAmount,
                assets: vec![Asset::new(reward_schedule.asset.clone(), reward_schedule.amount)]
            });
        }
    }

    let total_funds = total_funds_map
        .into_iter()
        .map(|(k, v)| Asset { info: k, amount: v })
        .collect();

    Ok(UserTotalDeposit { total_deposit: total_funds, deposit_breakdown: user_deposits_detailed })
}

/// returns the total funds needed for creating a pool
/// This includes the funds needed for:
/// 1. Gov proposal deposit
/// 2. Pool creation fee
/// 3. Bootstrapping amount
/// 4. Reward schedule amount
pub fn query_funds_for_pool_creation_request(deps: Deps, pool_creation_request: &PoolCreationRequest) -> ContractResult<UserTotalDeposit> {
    // query gov module for the proposal deposit
    let gov_proposal_min_deposit_amount = query_proposal_min_deposit_amount(deps)?;
    let user_total_deposit = find_total_funds_needed(deps, &gov_proposal_min_deposit_amount, &pool_creation_request)?;
    Ok(user_total_deposit)
}