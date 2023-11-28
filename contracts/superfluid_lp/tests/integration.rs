use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{Addr, Coin, Timestamp};
use cw_multi_test::{App, ContractWrapper};

const EPOCH_START: u64 = 1_000_000;

fn mock_app(owner: Addr, coins: Vec<Coin>) -> App {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(EPOCH_START);

    let mut app = App::new(|router, _, storage| {
        // initialization  moved to App construction
        router.bank.init_balance(storage, &owner, coins).unwrap();
    });
    app.set_block(env.block);
    app
}


// fn store_vault_code(app: &mut App) -> u64 {
//     let factory_contract = Box::new(
//         ContractWrapper::new_with_empty(
//             dexter_vault::contract::execute,
//             dexter_vault::contract::instantiate,
//             dexter_vault::contract::query,
//         )
//         .with_reply_empty(dexter_vault::contract::reply),
//     );
//     app.store_code(factory_contract)
// }

fn store_multi_staking_code(app: &mut App) -> u64 {
    let multi_staking_contract = Box::new(
        ContractWrapper::new_with_empty(
            dexter_multi_staking::contract::execute,
            dexter_multi_staking::contract::instantiate,
            dexter_multi_staking::contract::query,
        )
    );
    app.store_code(multi_staking_contract)
}

fn store_token_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new_with_empty(
        lp_token::contract::execute,
        lp_token::contract::instantiate,
        lp_token::contract::query,
    ));
    app.store_code(token_contract)
}

fn store_superfluid_lp_code(app: &mut App) -> u64 {
    let superfluid_lp_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_superfluid_lp::contract::execute,
        dexter_superfluid_lp::contract::instantiate,
        dexter_superfluid_lp::contract::query,
    ));
    app.store_code(superfluid_lp_contract)
}



// Initialize a vault with StableSwap, Weighted pools
// fn instantiate_contract(app: &mut App, owner: &Addr) -> Addr {

//     let token_code_id = store_token_code(app);
//     let multistaking_code = store_multi_staking_code(app);

//     let superfluid_lp_code = store_superfluid_lp_code(app);

//     // instantiate multistaking contract
//     let lp_token = Addr::unchecked("lp_token");
// }

