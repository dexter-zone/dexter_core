use cosmwasm_std::{Addr, testing::mock_env, Timestamp, Coin};
use cw_multi_test::App;

const EPOCH_START: u64 = 1_000_000_000;

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

#[test]
fn test_staking() {
    // let mut app = mock_app(owner, coins);

}