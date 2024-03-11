use crate::add_wasm_execute_msg;
use crate::contract::{ContractResult, CONTRACT_NAME};
use crate::state::POOL_CREATION_REQUEST_DATA;
use const_format::concatcp;

use cosmwasm_std::{to_json_binary, Coin, CosmosMsg, DepsMut, Env, Event, MessageInfo, Response};

use dexter::asset::AssetInfo;
use dexter::governance_admin::FundsCategory;
use dexter::helper::EventExt;
use dexter::vault::ExecuteMsg as VaultExecuteMsg;

pub fn execute_resume_create_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_creation_request_id: u64,
) -> ContractResult<Response> {
    // the proposal has passed, we can now resume the pool creation in the vault directly
    // get the pool creation request
    let pool_creation_request_data =
        POOL_CREATION_REQUEST_DATA.load(deps.storage, pool_creation_request_id)?;

    let pool_creation_request = pool_creation_request_data.pool_creation_request;
    let mut messages: Vec<CosmosMsg> = vec![];

    // create a message for vault
    let vault_addr = deps.api.addr_validate(&pool_creation_request.vault_addr)?;
    let create_pool_msg = VaultExecuteMsg::CreatePoolInstance {
        pool_type: pool_creation_request.pool_type.clone(),
        fee_info: pool_creation_request.fee_info.clone(),
        native_asset_precisions: pool_creation_request.native_asset_precisions.clone(),
        init_params: pool_creation_request.init_params.clone(),
        asset_infos: pool_creation_request.asset_info.clone(),
    };

    let user_funds = pool_creation_request_data.user_deposits_detailed;
    let mut pool_creation_funds: Vec<Coin> = vec![];

    for fund in user_funds {
        if let FundsCategory::PoolCreationFee = fund.category {
            for asset in fund.assets {
                match asset.info {
                    AssetInfo::NativeToken { denom } => {
                        pool_creation_funds.push(Coin::new(asset.amount.u128(), denom));
                    }
                    AssetInfo::Token { contract_addr } => {
                        add_wasm_execute_msg!(
                            messages,
                            contract_addr,
                            cw20::Cw20ExecuteMsg::IncreaseAllowance {
                                spender: vault_addr.to_string(),
                                amount: asset.amount,
                                expires: Some(cw20::Expiration::AtHeight(env.block.height + 1)),
                            },
                            vec![]
                        );
                    }
                }
            }
        }
    }

    // add the message to the list of messages
    add_wasm_execute_msg!(messages, vault_addr, create_pool_msg, pool_creation_funds);

    // add a message to return callback to the contract post proposal creation so we can find the
    // pool id of the pool we just created. This can be just found by querying the latest pool id from the vault
    // We also need to join the pool with the bootstrapping amount
    let callback_msg = dexter::governance_admin::ExecuteMsg::ResumeJoinPool {
        pool_creation_request_id,
    };

    add_wasm_execute_msg!(messages, env.contract.address, callback_msg, vec![]);

    let event = Event::from_info(concatcp!(CONTRACT_NAME, "::resume_create_pool"), &info)
        .add_attribute(
            "pool_creation_request_id",
            pool_creation_request_id.to_string(),
        )
        .add_attribute("vault_address", vault_addr.to_string())
        .add_attribute(
            "pool_creation_request",
            serde_json_wasm::to_string(&pool_creation_request).unwrap(),
        );

    Ok(Response::new().add_messages(messages).add_event(event))
}
