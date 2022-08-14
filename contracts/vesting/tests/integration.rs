use cosmwasm_std::{to_binary, Addr, StdResult, Timestamp, Uint128};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse};
use cw_multi_test::{App, BasicApp};
use cw_multi_test::{ContractWrapper, Executor};
use dexter::vesting::{QueryMsg, VestingAccountResponse};
use dexter::{
    lp_token::InstantiateMsg as TokenInstantiateMsg,
    vesting::{
        Cw20HookMsg, ExecuteMsg, InstantiateMsg, VestingAccount, VestingSchedule,
        VestingSchedulePoint,
    },
};
use dexter_vesting::state::Config;

const OWNER1: &str = "owner1";
const USER1: &str = "user1";
const USER2: &str = "user2";
const TOKEN_INITIAL_AMOUNT: u128 = 1_000_000_000_000000;

#[test]
fn claim() {
    let user1 = Addr::unchecked(USER1);
    let owner = Addr::unchecked(OWNER1);

    let mut app = mock_app();

    let token_code_id = store_token_code(&mut app);

    let dex_token_instance =
        instantiate_token(&mut app, token_code_id, "DEX", Some(1_000_000_000_000000));

    let vesting_instance = instantiate_vesting(&mut app, &dex_token_instance);

    let msg = Cw20ExecuteMsg::Send {
        contract: vesting_instance.to_string(),
        msg: to_binary(&Cw20HookMsg::RegisterVestingAccounts {
            vesting_accounts: vec![VestingAccount {
                address: user1.to_string(),
                schedules: vec![
                    VestingSchedule {
                        start_point: VestingSchedulePoint {
                            time: Timestamp::from_seconds(100).seconds(),
                            amount: Uint128::zero(),
                        },
                        end_point: Some(VestingSchedulePoint {
                            time: Timestamp::from_seconds(101).seconds(),
                            amount: Uint128::new(200),
                        }),
                    },
                    VestingSchedule {
                        start_point: VestingSchedulePoint {
                            time: Timestamp::from_seconds(100).seconds(),
                            amount: Uint128::zero(),
                        },
                        end_point: Some(VestingSchedulePoint {
                            time: Timestamp::from_seconds(110).seconds(),
                            amount: Uint128::new(100),
                        }),
                    },
                    VestingSchedule {
                        start_point: VestingSchedulePoint {
                            time: Timestamp::from_seconds(100).seconds(),
                            amount: Uint128::zero(),
                        },
                        end_point: Some(VestingSchedulePoint {
                            time: Timestamp::from_seconds(200).seconds(),
                            amount: Uint128::new(100),
                        }),
                    },
                ],
            }],
        })
        .unwrap(),
        amount: Uint128::from(300u128),
    };

    let res = app
        .execute_contract(owner.clone(), dex_token_instance.clone(), &msg, &[])
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "Vesting schedule amount error. Schedules total amount should be equal to cw20 receive amount.");

    let msg = Cw20ExecuteMsg::Send {
        contract: vesting_instance.to_string(),
        msg: to_binary(&Cw20HookMsg::RegisterVestingAccounts {
            vesting_accounts: vec![VestingAccount {
                address: user1.to_string(),
                schedules: vec![
                    VestingSchedule {
                        start_point: VestingSchedulePoint {
                            time: Timestamp::from_seconds(100).seconds(),
                            amount: Uint128::zero(),
                        },
                        end_point: Some(VestingSchedulePoint {
                            time: Timestamp::from_seconds(101).seconds(),
                            amount: Uint128::new(100),
                        }),
                    },
                    VestingSchedule {
                        start_point: VestingSchedulePoint {
                            time: Timestamp::from_seconds(100).seconds(),
                            amount: Uint128::zero(),
                        },
                        end_point: Some(VestingSchedulePoint {
                            time: Timestamp::from_seconds(110).seconds(),
                            amount: Uint128::new(100),
                        }),
                    },
                    VestingSchedule {
                        start_point: VestingSchedulePoint {
                            time: Timestamp::from_seconds(100).seconds(),
                            amount: Uint128::zero(),
                        },
                        end_point: Some(VestingSchedulePoint {
                            time: Timestamp::from_seconds(200).seconds(),
                            amount: Uint128::new(100),
                        }),
                    },
                ],
            }],
        })
        .unwrap(),
        amount: Uint128::from(300u128),
    };

    app.execute_contract(owner.clone(), dex_token_instance.clone(), &msg, &[])
        .unwrap();

    let msg = QueryMsg::AvailableAmount {
        address: user1.to_string(),
    };

    let user1_vesting_amount: Uint128 = app
        .wrap()
        .query_wasm_smart(vesting_instance.clone(), &msg)
        .unwrap();
    assert_eq!(user1_vesting_amount.clone(), Uint128::new(300u128));

    // check owner balance
    check_token_balance(
        &mut app,
        &dex_token_instance,
        &owner.clone(),
        TOKEN_INITIAL_AMOUNT - 300u128,
    );

    // check vesting balance
    check_token_balance(
        &mut app,
        &dex_token_instance,
        &vesting_instance.clone(),
        300u128,
    );

    let msg = ExecuteMsg::Claim {
        recipient: None,
        amount: None,
    };
    let _res = app
        .execute_contract(user1.clone(), vesting_instance.clone(), &msg, &[])
        .unwrap();

    let msg = QueryMsg::VestingAccount {
        address: user1.to_string(),
    };

    let vesting_res: VestingAccountResponse = app
        .wrap()
        .query_wasm_smart(vesting_instance.clone(), &msg)
        .unwrap();
    assert_eq!(vesting_res.info.released_amount, Uint128::from(300u128));

    // check vesting balance
    check_token_balance(
        &mut app,
        &dex_token_instance,
        &vesting_instance.clone(),
        0u128,
    );

    //check user balance
    check_token_balance(&mut app, &dex_token_instance, &user1.clone(), 300u128);

    // owner balance mustn't change after claim
    check_token_balance(
        &mut app,
        &dex_token_instance,
        &owner.clone(),
        TOKEN_INITIAL_AMOUNT - 300u128,
    );

    let msg = QueryMsg::AvailableAmount {
        address: user1.to_string(),
    };

    // check user balance after claim
    let user1_vesting_amount: Uint128 = app
        .wrap()
        .query_wasm_smart(vesting_instance.clone(), &msg)
        .unwrap();

    assert_eq!(user1_vesting_amount.clone(), Uint128::new(0u128));
}

#[test]
fn register_vesting_accounts() {
    let user1 = Addr::unchecked(USER1);
    let user2 = Addr::unchecked(USER2);
    let owner = Addr::unchecked(OWNER1);

    let mut app = mock_app();

    let token_code_id = store_token_code(&mut app);

    let dex_token_instance =
        instantiate_token(&mut app, token_code_id, "DEX", Some(1_000_000_000_000000));

    let noname_token_instance = instantiate_token(
        &mut app,
        token_code_id,
        "NONAME",
        Some(1_000_000_000_000000),
    );

    mint_tokens(
        &mut app,
        &noname_token_instance,
        &owner,
        TOKEN_INITIAL_AMOUNT,
    );

    let vesting_instance = instantiate_vesting(&mut app, &dex_token_instance);

    let msg = Cw20ExecuteMsg::Send {
        contract: vesting_instance.to_string(),
        msg: to_binary(&Cw20HookMsg::RegisterVestingAccounts {
            vesting_accounts: vec![VestingAccount {
                address: user1.to_string(),
                schedules: vec![VestingSchedule {
                    start_point: VestingSchedulePoint {
                        time: Timestamp::from_seconds(150).seconds(),
                        amount: Uint128::zero(),
                    },
                    end_point: Some(VestingSchedulePoint {
                        time: Timestamp::from_seconds(100).seconds(),
                        amount: Uint128::new(100),
                    }),
                }],
            }],
        })
        .unwrap(),
        amount: Uint128::from(100u128),
    };

    let res = app
        .execute_contract(owner.clone(), dex_token_instance.clone(), &msg, &[])
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "Vesting schedule error on addr: user1. Should satisfy: (start < end and at_start < total) or (start = end and at_start = total)");

    let msg = Cw20ExecuteMsg::Send {
        contract: vesting_instance.to_string(),
        msg: to_binary(&Cw20HookMsg::RegisterVestingAccounts {
            vesting_accounts: vec![VestingAccount {
                address: user1.to_string(),
                schedules: vec![VestingSchedule {
                    start_point: VestingSchedulePoint {
                        time: Timestamp::from_seconds(100).seconds(),
                        amount: Uint128::zero(),
                    },
                    end_point: Some(VestingSchedulePoint {
                        time: Timestamp::from_seconds(150).seconds(),
                        amount: Uint128::new(100),
                    }),
                }],
            }],
        })
        .unwrap(),
        amount: Uint128::from(100u128),
    };

    let res = app
        .execute_contract(user1.clone(), dex_token_instance.clone(), &msg.clone(), &[])
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "Cannot Sub with 0 and 100");

    let res = app
        .execute_contract(owner.clone(), noname_token_instance.clone(), &msg, &[])
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "Unauthorized");

    let _res = app
        .execute_contract(owner.clone(), dex_token_instance.clone(), &msg, &[])
        .unwrap();

    let msg = QueryMsg::AvailableAmount {
        address: user1.to_string(),
    };

    let user1_vesting_amount: Uint128 = app
        .wrap()
        .query_wasm_smart(vesting_instance.clone(), &msg)
        .unwrap();

    assert_eq!(user1_vesting_amount.clone(), Uint128::new(100u128));
    check_token_balance(
        &mut app,
        &dex_token_instance,
        &owner.clone(),
        TOKEN_INITIAL_AMOUNT - 100u128,
    );
    check_token_balance(
        &mut app,
        &dex_token_instance,
        &vesting_instance.clone(),
        100u128,
    );

    // let's check user1 final vesting amount after add schedule for a new one
    let msg = Cw20ExecuteMsg::Send {
        contract: vesting_instance.to_string(),
        msg: to_binary(&Cw20HookMsg::RegisterVestingAccounts {
            vesting_accounts: vec![VestingAccount {
                address: user2.to_string(),
                schedules: vec![VestingSchedule {
                    start_point: VestingSchedulePoint {
                        time: Timestamp::from_seconds(100).seconds(),
                        amount: Uint128::zero(),
                    },
                    end_point: Some(VestingSchedulePoint {
                        time: Timestamp::from_seconds(150).seconds(),
                        amount: Uint128::new(200),
                    }),
                }],
            }],
        })
        .unwrap(),
        amount: Uint128::from(200u128),
    };

    let _res = app
        .execute_contract(owner.clone(), dex_token_instance.clone(), &msg, &[])
        .unwrap();

    let msg = QueryMsg::AvailableAmount {
        address: user2.to_string(),
    };

    let user2_vesting_amount: Uint128 = app
        .wrap()
        .query_wasm_smart(vesting_instance.clone(), &msg)
        .unwrap();

    check_token_balance(
        &mut app,
        &dex_token_instance,
        &owner.clone(),
        TOKEN_INITIAL_AMOUNT - 300u128,
    );
    check_token_balance(
        &mut app,
        &dex_token_instance,
        &vesting_instance.clone(),
        300u128,
    );
    // A new schedule have been added successfully and an old one haven't changed.
    // The new one doesn't have the same value as old one.
    assert_eq!(user2_vesting_amount, Uint128::new(200u128));
    assert_eq!(user1_vesting_amount, Uint128::from(100u128));

    // add one more vesting schedule account, final vesting amount must increase only
    let msg = Cw20ExecuteMsg::Send {
        contract: vesting_instance.to_string(),
        msg: to_binary(&Cw20HookMsg::RegisterVestingAccounts {
            vesting_accounts: vec![VestingAccount {
                address: user1.to_string(),
                schedules: vec![VestingSchedule {
                    start_point: VestingSchedulePoint {
                        time: Timestamp::from_seconds(100).seconds(),
                        amount: Uint128::zero(),
                    },
                    end_point: Some(VestingSchedulePoint {
                        time: Timestamp::from_seconds(200).seconds(),
                        amount: Uint128::new(10),
                    }),
                }],
            }],
        })
        .unwrap(),
        amount: Uint128::from(10u128),
    };

    let _res = app
        .execute_contract(owner.clone(), dex_token_instance.clone(), &msg, &[])
        .unwrap();

    let msg = QueryMsg::AvailableAmount {
        address: user1.to_string(),
    };

    let vesting_res: Uint128 = app
        .wrap()
        .query_wasm_smart(vesting_instance.clone(), &msg)
        .unwrap();

    assert_eq!(vesting_res, Uint128::new(110u128));
    check_token_balance(
        &mut app,
        &dex_token_instance,
        &owner.clone(),
        TOKEN_INITIAL_AMOUNT - 310u128,
    );
    check_token_balance(
        &mut app,
        &dex_token_instance,
        &vesting_instance.clone(),
        310u128,
    );

    let msg = ExecuteMsg::Claim {
        recipient: None,
        amount: None,
    };
    let _res = app
        .execute_contract(user1.clone(), vesting_instance.clone(), &msg, &[])
        .unwrap();

    let msg = QueryMsg::VestingAccount {
        address: user1.to_string(),
    };

    let vesting_res: VestingAccountResponse = app
        .wrap()
        .query_wasm_smart(vesting_instance.clone(), &msg)
        .unwrap();
    assert_eq!(vesting_res.info.released_amount, Uint128::from(110u128));
    check_token_balance(
        &mut app,
        &dex_token_instance,
        &vesting_instance.clone(),
        200u128,
    );
    check_token_balance(&mut app, &dex_token_instance, &user1.clone(), 110u128);
    // owner balance mustn't change after claim
    check_token_balance(
        &mut app,
        &dex_token_instance,
        &owner.clone(),
        TOKEN_INITIAL_AMOUNT - 310u128,
    );
}

fn mock_app() -> App {
    BasicApp::default()
}

fn store_token_code(app: &mut App) -> u64 {
    let dex_token_contract = Box::new(ContractWrapper::new_with_empty(
        lp_token::contract::execute,
        lp_token::contract::instantiate,
        lp_token::contract::query,
    ));

    app.store_code(dex_token_contract)
}

fn instantiate_token(app: &mut App, token_code_id: u64, name: &str, cap: Option<u128>) -> Addr {
    let name = String::from(name);

    let msg = TokenInstantiateMsg {
        name: name.clone(),
        symbol: name.clone(),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: String::from(OWNER1),
            cap: cap.map(|v| Uint128::from(v)),
        }),
        marketing: None,
    };

    app.instantiate_contract(
        token_code_id,
        Addr::unchecked(OWNER1),
        &msg,
        &[],
        name,
        None,
    )
    .unwrap()
}

fn instantiate_vesting(mut app: &mut App, dex_token_instance: &Addr) -> Addr {
    let vesting_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_vesting::contract::execute,
        dexter_vesting::contract::instantiate,
        dexter_vesting::contract::query,
    ));
    let owner = Addr::unchecked(OWNER1);
    let vesting_code_id = app.store_code(vesting_contract);

    let init_msg = InstantiateMsg {
        owner: OWNER1.to_string(),
        token_addr: dex_token_instance.to_string(),
    };

    let vesting_instance = app
        .instantiate_contract(
            vesting_code_id,
            owner.clone(),
            &init_msg,
            &[],
            "Vesting",
            None,
        )
        .unwrap();

    let res: Config = app
        .wrap()
        .query_wasm_smart(vesting_instance.clone(), &QueryMsg::Config {})
        .unwrap();
    assert_eq!(dex_token_instance.to_string(), res.token_addr.to_string());

    mint_tokens(&mut app, &dex_token_instance, &owner, TOKEN_INITIAL_AMOUNT);

    check_token_balance(&mut app, &dex_token_instance, &owner, TOKEN_INITIAL_AMOUNT);

    vesting_instance
}

fn mint_tokens(app: &mut App, token: &Addr, recipient: &Addr, amount: u128) {
    let msg = Cw20ExecuteMsg::Mint {
        recipient: recipient.to_string(),
        amount: Uint128::from(amount),
    };

    app.execute_contract(Addr::unchecked(OWNER1), token.to_owned(), &msg, &[])
        .unwrap();
}

fn check_token_balance(app: &mut App, token: &Addr, address: &Addr, expected: u128) {
    let msg = Cw20QueryMsg::Balance {
        address: address.to_string(),
    };
    let res: StdResult<BalanceResponse> = app.wrap().query_wasm_smart(token, &msg);
    assert_eq!(res.unwrap().balance, Uint128::from(expected));
}
