use cosmwasm_std::Decimal;
use dexter::multi_staking::UnlockFeeTier;

const SECOND_IN_DAY: u64 = 86400;

pub fn query_instant_unlock_fee_tiers(
    unlock_period: u64,
    min_fee_bp: u64,
    max_fee_bp: u64,
) -> Vec<UnlockFeeTier> {
    // Fee tiers exist on day boundaries linearly interpolating the values from min_fee to max_fee
    let mut fee_tiers: Vec<UnlockFeeTier> = vec![];

    // if the unlock period is less than 24 hours then there's only one tier equal to max fee
    if unlock_period < 86400 {
        fee_tiers.push(UnlockFeeTier {
            seconds_till_unlock_end: 0,
            seconds_till_unlock_start: unlock_period,
            unlock_fee_bp: max_fee_bp
        });
    } else {
        // num tiers is the ceiling of unlock period in days
        let num_tiers = (unlock_period as f64 / 86400.0).ceil() as u64;

        println!("num tiers: {}", num_tiers);

        // fee increment per tier
        // let fee_increment = (max_fee - min_fee) / (num_tiers - 1);
        let fee_increment: Decimal = Decimal::from_ratio(max_fee_bp - min_fee_bp, (num_tiers - 1) as u64);

        let mut tier_start_time = 0;
        let mut tier_end_time = SECOND_IN_DAY;

        for tier in 0..num_tiers {
            fee_tiers.push(UnlockFeeTier {
                seconds_till_unlock_end: tier_end_time,
                seconds_till_unlock_start: tier_start_time,
                // unlock_fee_bp: min_fee + (fee_increment * tier)
                unlock_fee_bp: min_fee_bp + fee_increment.checked_mul(Decimal::from_ratio(tier, 1u64)).unwrap().to_uint_ceil().u128() as u64
            });

            tier_start_time = tier_end_time;
            tier_end_time += SECOND_IN_DAY;
        }
    }

    fee_tiers
}