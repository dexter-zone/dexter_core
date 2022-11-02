
use crate::helper::{is_valid_name, is_valid_symbol};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{StdError, StdResult, Uint128};
use cw20::{Cw20Coin, Logo, MinterResponse};

/// ## Description -  This structure describes the basic settings for creating a token contract.
#[cw_serde]
pub struct InstantiateMsg {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub initial_balances: Vec<Cw20Coin>,
    pub mint: Option<MinterResponse>,
    pub marketing: Option<InstantiateMarketingInfo>,
}

/// ## Description -  This structure describes the marketing info settings such as project, description, and token logo.
#[cw_serde]
pub struct InstantiateMarketingInfo {
    /// The project name
    pub project: Option<String>,
    /// The project description
    pub description: Option<String>,
    /// The address of an admin who is able to update marketing info
    pub marketing: Option<String>,
    /// The token logo
    pub logo: Option<Logo>,
}

/// ## Description -  This structure describes a migration message.
#[cw_serde]
pub struct MigrateMsg {}

impl InstantiateMsg {
    pub fn get_cap(&self) -> Option<Uint128> {
        self.mint.as_ref().and_then(|v| v.cap)
    }

    pub fn validate(&self) -> StdResult<()> {
        // Check name, symbol, decimals
        if !is_valid_name(&self.name) {
            return Err(StdError::generic_err(
                "Name is not in the expected format (3-50 UTF-8 bytes)",
            ));
        }
        if !is_valid_symbol(&self.symbol) {
            return Err(StdError::generic_err(
                "Ticker symbol is not in expected format [a-zA-Z\\-]{3,12}",
            ));
        }
        if self.decimals > 18 {
            return Err(StdError::generic_err("Decimals must not exceed 18"));
        }
        Ok(())
    }
}
