use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Uint128, Binary};
use cw_storage_plus::Item;
use dexter::asset::Asset;
use dexter::vault::{FeeInfo, NativeAssetPrecisionInfo};

// ## Description
// Stores the contract configuration at the given key
// pub const CONFIG: Item<Config> = Item::new("config");

// Ownership Proposal currently active in the Vault in a [`OwnershipProposal`] struc
// pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");

#[cw_serde]
pub struct CreatePoolTempData {
    pub assumed_pool_id: Uint128,
    pub vault_addr: String,
    pub bootstrapping_amount_payer: String,
    pub pool_type: String,
    pub fee_info: Option<FeeInfo>,
    pub native_asset_precisions: Vec<NativeAssetPrecisionInfo>,
    pub assets: Vec<Asset>,
    pub init_params: Option<Binary>
}


pub const CREATE_POOL_TEMP_DATA: Item<CreatePoolTempData> = Item::new("join_pool_reference");