use cosmwasm_std::Uint128;
use dexter::multi_staking::{InstantUnbondConfig, TokenLock, UnbondConfig};

use crate::query::query_instant_unlock_fee_tiers;

/// Find the difference between two lock vectors.
/// This must take into account that same looking lock can coexist, for example, there can be 2 locks for unlocking
/// 100 tokens at block 100 both.
/// In this case, the difference calculation must only remove one occurances of the lock if one is present in the locks_to_be_unlocked vector.
/// Locks are by default stored by unlock time in ascending order by design, but we can sort it once more to be sure.
/// Return both locks to keep and valid locks to be unlocked since the locks_to_be_unlocked vector can actually contain invalid locks.
pub fn find_lock_difference(
    all_locks: Vec<TokenLock>,
    locks_to_be_unlocked: Vec<TokenLock>,
) -> (Vec<TokenLock>, Vec<TokenLock>) {
    let mut all_locks = all_locks;
    // sort by unlock time
    all_locks.sort_by(|a, b| a.unlock_time.cmp(&b.unlock_time));

    let mut locks_to_be_unlocked = locks_to_be_unlocked;

    // sort by unlock time
    locks_to_be_unlocked.sort_by(|a, b| a.unlock_time.cmp(&b.unlock_time));

    let mut difference = vec![];

    let mut i = 0;
    let mut j = 0;

    let mut valid_locks_to_be_unlocked = vec![];

    while i < all_locks.len() && j < locks_to_be_unlocked.len() {
        if all_locks[i] == locks_to_be_unlocked[j] {
            valid_locks_to_be_unlocked.push(locks_to_be_unlocked[j].clone());
            i += 1;
            j += 1;
        } else if all_locks[i].unlock_time < locks_to_be_unlocked[j].unlock_time {
            difference.push(all_locks[i].clone());
            i += 1;
        } else {
            j += 1;
        }
    }

    while i < all_locks.len() {
        difference.push(all_locks[i].clone());
        i += 1;
    }

    return (difference, valid_locks_to_be_unlocked);
}

/// Calculate the instant unlock fee for a given token lock.
/// The fee is calculated as a percentage of the locked amount.
/// It is linearly interpolated between the start and end time of the lock at tier_interval granularity.
pub fn calculate_unlock_fee(
    token_lock: &TokenLock,
    current_block_time: u64,
    unbond_config: &UnbondConfig,
) -> (u64, Uint128) {
    let lock_end_time = token_lock.unlock_time;

    if current_block_time >= lock_end_time {
        return (0, Uint128::zero());
    }

    // check if ILPU is enabled
    match unbond_config.instant_unbond_config {
        InstantUnbondConfig::Disabled => {
            panic!("Instant unlock is not supported");
        }
        InstantUnbondConfig::Enabled {
            min_fee: _,
            max_fee,
            fee_tier_interval: _,
        } => {
            let tiers = query_instant_unlock_fee_tiers(unbond_config.clone());

            // find applicable tier based on second left to unlock
            let seconds_left_to_unlock = lock_end_time - current_block_time;

            let mut fee_bp = max_fee;
            for tier in tiers {
                // the tier is applicable if the seconds fall in tiers range, end non-inclusive
                if seconds_left_to_unlock >= tier.seconds_till_unlock_start
                    && seconds_left_to_unlock < tier.seconds_till_unlock_end
                {
                    fee_bp = tier.unlock_fee_bp;
                    break;
                }
            }

            let fee = token_lock.amount.multiply_ratio(fee_bp, 10000u64);
            (fee_bp, fee)
        }
    }
}
