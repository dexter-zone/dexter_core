use cosmwasm_std::Addr;
use dexter::multi_staking::Config;

use crate::{contract::ContractResult, error::ContractError};

pub mod unbond;
pub mod unlock;
pub mod create_reward_schedule;

pub const NO_PKEY_ALLOWED_ADDR: &str = "persistence1pqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqzw5qx7";

pub fn check_if_lp_token_allowed(config: &Config, lp_token: &Addr) -> ContractResult<()> {
    if !config.allowed_lp_tokens.contains(lp_token) {
        return Err(ContractError::LpTokenNotAllowed);
    }
    Ok(())
}