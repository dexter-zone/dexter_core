use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::Addr;

use crate::contract::instantiate;
use crate::state::{Config, CONFIG};
use dexter::keeper::InstantiateMsg;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies();
    let info = mock_info("addr0000", &[]);

    let env = mock_env();
    let vault = Addr::unchecked("vault");

    let instantiate_msg = InstantiateMsg {
        vault_contract: vault.to_string(),
    };
    let res = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap();
    assert_eq!(0, res.messages.len());

    let state = CONFIG.load(deps.as_mut().storage).unwrap();
    assert_eq!(
        state,
        Config {
            vault_contract: Addr::unchecked("vault"),
            staking_contract: None,
            dex_token_contract: None,
        }
    )
}
