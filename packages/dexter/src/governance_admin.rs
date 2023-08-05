use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Coin, Binary, CosmosMsg, Uint128};

use crate::{vault::{PoolType, NativeAssetPrecisionInfo, FeeInfo}, asset::{AssetInfo, Asset}};

#[cw_serde]
pub struct InstantiateMsg {}


#[cw_serde]
pub enum ExecuteMsg {

   ExecuteMsgs {
        msgs: Vec<CosmosMsg>
   },

   // Create new pool with funds
   CreateNewPool {
      vault_addr: String,
      bootstrapping_amount_payer: String,
      pool_type: PoolType,
      fee_info: Option<FeeInfo>,
      native_asset_precisions: Vec<NativeAssetPrecisionInfo>,
      assets: Vec<Asset>,
      init_params: Option<Binary>
   },

   // Resume join pool operation after pool is created successfully
   ResumeJoinPool {}

}

#[cw_serde]
pub enum QueryMsg {}