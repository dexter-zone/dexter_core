use cosmwasm_std::{MessageInfo, Env,};

use crate::{error::ContractError, contract::ContractResult};

use super::constants::GOV_MODULE_ADDRESS;

pub fn validate_goverance_module_sender(info: &MessageInfo) -> ContractResult<()> {
    if info.sender != GOV_MODULE_ADDRESS {
        return Err(ContractError::Unauthorized)
    }
    Ok(())
}

pub fn validatate_goverance_module_or_self_sender(info: &MessageInfo, env: Env) -> ContractResult<()> {
    if info.sender != GOV_MODULE_ADDRESS && info.sender != env.contract.address {
        return Err(ContractError::Unauthorized)
    }
    Ok(())
}

pub fn validate_self_sender(info: &MessageInfo, env: Env) -> ContractResult<()> {
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized)
    }
    Ok(())
}