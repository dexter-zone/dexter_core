use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::Addr;

use crate::contract::instantiate;
use crate::state::CONFIG;
use dexter::keeper::{Config, InstantiateMsg};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies();
    let info = mock_info("addr0000", &[]);

    let env = mock_env();
    let admin = Addr::unchecked("admin");

    let instantiate_msg = InstantiateMsg {
        owner: admin.clone(),
        vault_address: Addr::unchecked("vault"),
    };
    let res = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap();
    assert_eq!(0, res.messages.len());

    let state = CONFIG.load(deps.as_mut().storage).unwrap();
    assert_eq!(
        state,
        Config {
            owner: admin,
            vault_address: Addr::unchecked("vault"),
        }
    )
}
