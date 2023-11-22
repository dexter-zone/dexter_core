use cosmwasm_std::{Env, MessageInfo};
use dexter::constants::GOV_MODULE_ADDRESS;
use crate::{contract::ContractResult, error::ContractError};


pub fn validate_goverance_module_sender(info: &MessageInfo) -> ContractResult<()> {
    if info.sender != GOV_MODULE_ADDRESS {
        return Err(ContractError::Unauthorized);
    }
    Ok(())
}

pub fn validatate_goverance_module_or_self_sender(
    info: &MessageInfo,
    env: Env,
) -> ContractResult<()> {
    if info.sender != GOV_MODULE_ADDRESS && info.sender != env.contract.address {
        return Err(ContractError::Unauthorized);
    }
    Ok(())
}

pub fn validate_self_sender(info: &MessageInfo, env: Env) -> ContractResult<()> {
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized);
    }
    Ok(())
}
