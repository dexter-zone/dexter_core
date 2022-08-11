use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{attr, from_binary, to_binary, Addr, Coin, Decimal, Timestamp, Uint128};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse};
use cw_multi_test::{App, ContractWrapper, Executor};

use dexter::asset::{Asset, AssetExchangeRate, AssetInfo};
use dexter::lp_token::InstantiateMsg as TokenInstantiateMsg;
use dexter::pool::{
    AfterExitResponse, AfterJoinResponse, ConfigResponse, CumulativePriceResponse,
    CumulativePricesResponse, ExecuteMsg, FeeResponse, QueryMsg, ResponseType, SwapResponse,
};
use dexter::vault::{
    Cw20HookMsg, ExecuteMsg as VaultExecuteMsg, FeeInfo, InstantiateMsg as VaultInstantiateMsg,
    PoolConfig, PoolInfo, PoolInfoResponse, PoolType, QueryMsg as VaultQueryMsg, SingleSwapRequest,
    SwapType,
};

use stableswap_pool::math::{MAX_AMP, MAX_AMP_CHANGE, MIN_AMP_CHANGING_TIME};
use stableswap_pool::state::{MathConfig, StablePoolParams, StablePoolUpdateParams};
