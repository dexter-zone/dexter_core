use cosmwasm_std::{Uint128, Coin, Deps};
use dexter::{governance_admin::{FundsCategory, UserDeposit, RewardScheduleCreationRequest, UserTotalDeposit}, asset::{AssetInfo, Asset}};

use crate::{contract::ContractResult, utils::queries::query_proposal_min_deposit_amount};

pub fn find_total_needed_funds(
    requests: &Vec<RewardScheduleCreationRequest>,
    gov_proposal_min_deposit_amount: &Vec<Coin>,
) -> ContractResult<(Vec<UserDeposit>, Vec<Asset>)> {
    let mut total_funds_map = std::collections::HashMap::new();
    let mut user_deposits_detailed = vec![];

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

    for reward_schedule in requests {
        let amount: Uint128 = total_funds_map
            .get(&reward_schedule.asset)
            .cloned()
            .unwrap_or_default();

        total_funds_map.insert(
            reward_schedule.asset.clone(),
            amount.checked_add(reward_schedule.amount)?,
        );

        user_deposits_detailed.push(UserDeposit {
            category: FundsCategory::RewardScheduleAmount,
            assets: vec![Asset {
                info: reward_schedule.asset.clone(),
                amount: reward_schedule.amount,
            }],
        });
    }

    let total_funds: Vec<Asset> = total_funds_map
        .into_iter()
        .map(|(k, v)| Asset { info: k, amount: v })
        .collect();

    Ok((user_deposits_detailed, total_funds))
}

pub fn query_funds_for_reward_schedule_creation(deps: Deps, requests: &Vec<RewardScheduleCreationRequest>) -> ContractResult<UserTotalDeposit> {
    let gov_proposal_min_deposit_amount = query_proposal_min_deposit_amount(deps)?;
    let (user_deposits_detailed, total_needed_funds) = find_total_needed_funds(requests, &gov_proposal_min_deposit_amount)?;

    Ok(UserTotalDeposit {
        deposit_breakdown: user_deposits_detailed,
        total_deposit: total_needed_funds,
    })
}