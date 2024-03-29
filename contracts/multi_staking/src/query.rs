use cosmwasm_std::Decimal;
use dexter::multi_staking::{InstantUnbondConfig, UnbondConfig, UnlockFeeTier};

use crate::error::ContractError;

pub fn query_instant_unlock_fee_tiers(
    config: UnbondConfig,
) -> Result<Vec<UnlockFeeTier>, ContractError> {
    // Fee tiers exist on day boundaries linearly interpolating the values from min_fee to max_fee
    let mut fee_tiers: Vec<UnlockFeeTier> = vec![];

    let unlock_period = config.unlock_period;

    // if the ILPU is disabled, throw error saying that unlock is not supported
    match config.instant_unbond_config {
        InstantUnbondConfig::Disabled => {
            // panic!("Instant unlock is not supported");
            Err(ContractError::InstantUnbondDisabled {})
        }
        InstantUnbondConfig::Enabled {
            min_fee,
            max_fee,
            fee_tier_interval,
        } => {
            // if the unlock period is less than tier interval then there's only one tier equal to max fee
            if unlock_period <= fee_tier_interval {
                fee_tiers.push(UnlockFeeTier {
                    seconds_till_unlock_end: unlock_period,
                    seconds_till_unlock_start: 0,
                    unlock_fee_bp: max_fee,
                });
            } else {
                // num tiers is the ceiling of unlock period in terms of tier interval
                let num_tiers = (Decimal::from_ratio(unlock_period, fee_tier_interval))
                    .to_uint_ceil()
                    .u128();
                // fee increment per tier
                let fee_increment: Decimal =
                    Decimal::from_ratio(max_fee - min_fee, (num_tiers - 1) as u64);

                let mut tier_start_time = 0;
                let mut tier_end_time = fee_tier_interval;

                for tier in 0..num_tiers {
                    fee_tiers.push(UnlockFeeTier {
                        seconds_till_unlock_end: tier_end_time,
                        seconds_till_unlock_start: tier_start_time,
                        // unlock_fee_bp: min_fee + (fee_increment * tier)
                        unlock_fee_bp: min_fee
                            + fee_increment
                                .checked_mul(Decimal::from_ratio(tier, 1u64))
                                .unwrap()
                                .to_uint_ceil()
                                .u128() as u64,
                    });

                    tier_start_time = tier_end_time;
                    // if this is the last tier then set the end time to the unlock period
                    if tier == num_tiers - 2 {
                        tier_end_time = unlock_period;
                    } else {
                        tier_end_time += fee_tier_interval;
                    }
                }
            }

            Ok(fee_tiers)
        }
    }
}
