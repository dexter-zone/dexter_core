use cosmwasm_std::{testing::mock_env, to_binary, Addr, Coin, Timestamp, Uint128};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse};
use cw_multi_test::{App, AppResponse, ContractWrapper, Executor};
use dexter::{
    asset::AssetInfo,
    multi_staking::{
        Cw20HookMsg, ExecuteMsg, InstantLpUnlockFee, InstantiateMsg, QueryMsg, TokenLock,
        TokenLockInfo, UnclaimedReward, UnlockFeeTier,
    },
};

const EPOCH_START: u64 = 1_000_000_000;

pub fn mock_app(admin: Addr, coins: Vec<Coin>) -> App {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(EPOCH_START);

    let mut app = App::new(|router, _, storage| {
        // initialization  moved to App construction
        router.bank.init_balance(storage, &admin, coins).unwrap();
    });
    app.set_block(env.block);
    app
}

pub fn instantiate_multi_staking_contract(
    app: &mut App,
    code_id: u64,
    admin: Addr,
    keeper_addr: Addr,
    minimum_reward_schedule_proposal_start_delay: u64,
    unlock_period: u64,
    instant_unbond_min_fee_bp: u64,
    instant_unbond_max_fee_bp: u64,
    fee_tier_interval: u64,
) -> Addr {
    let instantiate_msg = InstantiateMsg {
        owner: admin.clone(),
        unlock_period,
        keeper_addr,
        // 3 day delay
        minimum_reward_schedule_proposal_start_delay,
        instant_unbond_fee_bp: instant_unbond_max_fee_bp,
        instant_unbond_min_fee_bp,
        fee_tier_interval,
    };

    let multi_staking_instance = app
        .instantiate_contract(
            code_id,
            admin.to_owned(),
            &instantiate_msg,
            &[],
            "multi_staking",
            None,
        )
        .unwrap();

    return multi_staking_instance;
}

pub fn store_multi_staking_contract(app: &mut App) -> u64 {
    let multi_staking_contract = Box::new(ContractWrapper::new_with_empty(
        dexter_multi_staking::contract::execute,
        dexter_multi_staking::contract::instantiate,
        dexter_multi_staking::contract::query,
    ));
    let code_id = app.store_code(multi_staking_contract);
    return code_id;
}

pub fn store_cw20_contract(app: &mut App) -> u64 {
    let cw20_contract = Box::new(ContractWrapper::new_with_empty(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    ));
    let code_id = app.store_code(cw20_contract);
    return code_id;
}

pub fn create_dummy_cw20_token(app: &mut App, admin: &Addr, code_id: u64) -> Addr {
    let cw20_instantiate_msg = cw20_base::msg::InstantiateMsg {
        name: "dummy".to_string(),
        symbol: "dummy".to_string(),
        decimals: 6,
        initial_balances: vec![],
        marketing: None,
        mint: Some(MinterResponse {
            minter: admin.clone().to_string(),
            cap: None,
        }),
    };

    let cw20_instance = app
        .instantiate_contract(
            code_id,
            admin.to_owned(),
            &cw20_instantiate_msg,
            &[],
            "cw20",
            None,
        )
        .unwrap();

    return cw20_instance;
}

pub fn store_lp_token_contract(app: &mut App) -> u64 {
    let lp_token_contract = Box::new(ContractWrapper::new_with_empty(
        lp_token::contract::execute,
        lp_token::contract::instantiate,
        lp_token::contract::query,
    ));
    let code_id = app.store_code(lp_token_contract);
    return code_id;
}

pub fn create_lp_token(app: &mut App, code_id: u64, sender: Addr, token_name: String) -> Addr {
    let lp_token_instantiate_msg = dexter::lp_token::InstantiateMsg {
        name: token_name,
        symbol: "abcde".to_string(),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: sender.to_string(),
            cap: None,
        }),
        marketing: None,
    };

    let lp_token_instance = app
        .instantiate_contract(
            code_id,
            sender.clone(),
            &lp_token_instantiate_msg,
            &[],
            "lp_token",
            Some(sender.to_string()),
        )
        .unwrap();

    return lp_token_instance;
}

pub fn setup_generic(
    app: &mut App,
    admin_addr: Addr,
    keeper_addr: Addr,
    minimum_reward_schedule_proposal_start_delay: u64,
    unlock_time: u64,
    unbond_fee_min_bp: u64,
    unbond_fee_max_bp: u64,
    fee_tier_interval: u64,
) -> (Addr, Addr) {
    let multi_staking_code_id = store_multi_staking_contract(app);
    let multi_staking_instance = instantiate_multi_staking_contract(
        app,
        multi_staking_code_id,
        admin_addr.clone(),
        keeper_addr,
        minimum_reward_schedule_proposal_start_delay,
        unlock_time,
        unbond_fee_min_bp,
        unbond_fee_max_bp,
        fee_tier_interval,
    );

    // let cw20_code_id = store_cw20_contract(app);
    let lp_token_code_id = store_lp_token_contract(app);

    let lp_token_addr = create_lp_token(
        app,
        lp_token_code_id,
        admin_addr.clone(),
        "Dummy LP Token".to_string(),
    );

    // Allow LP token in the multi staking contract
    app.execute_contract(
        admin_addr.clone(),
        multi_staking_instance.clone(),
        &ExecuteMsg::AllowLpToken {
            lp_token: lp_token_addr.clone(),
        },
        &vec![],
    )
    .unwrap();

    return (multi_staking_instance, lp_token_addr);
}

pub fn setup(app: &mut App, admin_addr: Addr) -> (Addr, Addr) {
    let keeper_addr = Addr::unchecked("keeper");
    let (multi_staking_instance, lp_token_addr) = setup_generic(
        app,
        admin_addr,
        keeper_addr,
        3 * 24 * 60 * 60,
        // 7 days
        7 * 24 * 60 * 60,
        200,
        500,
        7 * 24 * 60 * 60,
    );

    return (multi_staking_instance, lp_token_addr);
}


pub fn create_reward_schedule(
    app: &mut App,
    proposer: &Addr,
    multistaking_contract: &Addr,
    lp_token: &Addr,
    title: String,
    description: Option<String>,
    reward_asset: AssetInfo,
    amount: Uint128,
    start_block_time: u64,
    end_block_time: u64,
) -> anyhow::Result<u64> {
    let res = match reward_asset {
        AssetInfo::NativeToken { denom } => app.execute_contract(
            proposer.clone(),
            multistaking_contract.clone(),
            &ExecuteMsg::CreateRewardSchedule {
                lp_token: lp_token.clone(),
                title,
                description,
                start_block_time,
                end_block_time,
            },
            &vec![Coin::new(amount.u128(), denom.as_str())],
        ),
        AssetInfo::Token { contract_addr } => app.execute_contract(
            proposer.clone(),
            contract_addr.clone(),
            &Cw20ExecuteMsg::Send {
                contract: multistaking_contract.to_string(),
                amount,
                msg: to_binary(&Cw20HookMsg::CreateRewardSchedule {
                    lp_token: lp_token.clone(),
                    title,
                    description,
                    start_block_time,
                    end_block_time,
                })
                .unwrap(),
            },
            &vec![],
        ),
    };

    let reward_schedule_id: anyhow::Result<u64> = res.map(|r| {
        r.events
            .iter()
            .filter(|&e| e.ty == "wasm-dexter-multi-staking::create_reward_schedule")
            .fold(Vec::new(), |acc, e| {
                let mut res = e.attributes.clone();
                res.append(&mut acc.clone());
                res
            })
            .iter()
            .find(|&a| a.key == "reward_schedule_id")
            .map(|a| a.value.parse::<u64>().unwrap())
            .unwrap()
    });

    return reward_schedule_id;
}

pub fn update_fee_tier_interval(
    app: &mut App,
    admin_addr: &Addr,
    multistaking_contract: &Addr,
    fee_tier_interval: u64,
) -> anyhow::Result<AppResponse> {
    app.execute_contract(
        admin_addr.clone(),
        multistaking_contract.clone(),
        &ExecuteMsg::UpdateConfig {
            keeper_addr: None,
            minimum_reward_schedule_proposal_start_delay: None,
            unlock_period: None,
            instant_unbond_fee_bp: None,
            instant_unbond_min_fee_bp: None,
            fee_tier_interval: Some(fee_tier_interval),
        },
        &vec![],
    )
}

pub fn mint_lp_tokens_to_addr(
    app: &mut App,
    admin_addr: &Addr,
    lp_token_addr: &Addr,
    recipient_addr: &Addr,
    amount: Uint128,
) {
    app.execute_contract(
        admin_addr.clone(),
        lp_token_addr.clone(),
        &Cw20ExecuteMsg::Mint {
            recipient: recipient_addr.to_string(),
            amount,
        },
        &vec![],
    )
    .unwrap();
}

pub fn mint_cw20_tokens_to_addr(
    app: &mut App,
    admin_addr: &Addr,
    cw20_addr: &Addr,
    recipient_addr: &Addr,
    amount: Uint128,
) {
    app.execute_contract(
        admin_addr.clone(),
        cw20_addr.clone(),
        &Cw20ExecuteMsg::Mint {
            recipient: recipient_addr.to_string(),
            amount,
        },
        &vec![],
    )
    .unwrap();
}

pub fn bond_lp_tokens(
    app: &mut App,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
    sender: &Addr,
    amount: Uint128,
) -> anyhow::Result<AppResponse> {
    app.execute_contract(
        sender.clone(),
        lp_token_addr.clone(),
        &Cw20ExecuteMsg::Send {
            contract: multistaking_contract.to_string(),
            amount,
            msg: to_binary(&Cw20HookMsg::Bond {
                beneficiary_user: None,
            })
            .unwrap(),
        },
        &vec![],
    )
}

pub fn unbond_lp_tokens(
    app: &mut App,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
    sender: &Addr,
    amount: Uint128,
) -> anyhow::Result<AppResponse> {
    app.execute_contract(
        sender.clone(),
        multistaking_contract.clone(),
        &ExecuteMsg::Unbond {
            lp_token: lp_token_addr.clone(),
            amount: Some(amount),
        },
        &vec![],
    )
}

pub fn instant_unbond_lp_tokens(
    app: &mut App,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
    sender: &Addr,
    amount: Uint128,
) -> anyhow::Result<AppResponse> {
    app.execute_contract(
        sender.clone(),
        multistaking_contract.clone(),
        &ExecuteMsg::InstantUnbond {
            lp_token: lp_token_addr.clone(),
            amount,
        },
        &vec![],
    )
}

pub fn instant_unlock_lp_tokens(
    app: &mut App,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
    sender: &Addr,
    token_locks: Vec<TokenLock>,
) -> anyhow::Result<AppResponse> {
    let msg = ExecuteMsg::InstantUnlock {
        lp_token: lp_token_addr.clone(),
        token_locks,
    };
    app.execute_contract(sender.clone(), multistaking_contract.clone(), &msg, &vec![])
}

pub fn unlock_lp_tokens(
    app: &mut App,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
    sender: &Addr,
) {
    app.execute_contract(
        sender.clone(),
        multistaking_contract.clone(),
        &ExecuteMsg::Unlock {
            lp_token: lp_token_addr.clone(),
        },
        &vec![],
    )
    .unwrap();
}

pub fn disallow_lp_token(
    app: &mut App,
    admin_addr: &Addr,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
) {
    app.execute_contract(
        admin_addr.clone(),
        multistaking_contract.clone(),
        &ExecuteMsg::RemoveLpToken {
            lp_token: lp_token_addr.clone(),
        },
        &vec![],
    )
    .unwrap();
}

pub fn query_unclaimed_rewards(
    app: &mut App,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
    user_addr: &Addr,
) -> Vec<UnclaimedReward> {
    app.wrap()
        .query_wasm_smart(
            multistaking_contract.clone(),
            &QueryMsg::UnclaimedRewards {
                lp_token: lp_token_addr.clone(),
                user: user_addr.clone(),
                block_time: None,
            },
        )
        .unwrap()
}

pub fn query_bonded_lp_tokens(
    app: &mut App,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
    user_addr: &Addr,
) -> Uint128 {
    app.wrap()
        .query_wasm_smart(
            multistaking_contract.clone(),
            &QueryMsg::BondedLpTokens {
                lp_token: lp_token_addr.clone(),
                user: user_addr.clone(),
            },
        )
        .unwrap()
}

pub fn query_token_locks(
    app: &mut App,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
    user_addr: &Addr,
    block_time: Option<u64>,
) -> TokenLockInfo {
    app.wrap()
        .query_wasm_smart(
            multistaking_contract.clone(),
            &QueryMsg::TokenLocks {
                lp_token: lp_token_addr.clone(),
                user: user_addr.clone(),
                block_time,
            },
        )
        .unwrap()
}

pub fn query_raw_token_locks(
    app: &mut App,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
    user_addr: &Addr,
) -> Vec<TokenLock> {
    app.wrap()
        .query_wasm_smart(
            multistaking_contract.clone(),
            &QueryMsg::RawTokenLocks {
                lp_token: lp_token_addr.clone(),
                user: user_addr.clone(),
            },
        )
        .unwrap()
}

pub fn withdraw_unclaimed_rewards(
    app: &mut App,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
    user_addr: &Addr,
) {
    app.execute_contract(
        user_addr.clone(),
        multistaking_contract.clone(),
        &ExecuteMsg::Withdraw {
            lp_token: lp_token_addr.clone(),
        },
        &vec![],
    )
    .unwrap();
}

pub fn claim_creator_rewards(
    app: &mut App,
    multistaking_contract: &Addr,
    reward_schedule_id: u64,
    creator_addr: &Addr,
) -> anyhow::Result<AppResponse> {
    app.execute_contract(
        creator_addr.clone(),
        multistaking_contract.clone(),
        &ExecuteMsg::ClaimUnallocatedReward { reward_schedule_id },
        &vec![],
    )
}

pub fn assert_user_lp_token_balance(
    app: &mut App,
    user_addr: &Addr,
    lp_token_addr: &Addr,
    expected_balance: Uint128,
) {
    let response: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            lp_token_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: user_addr.to_string(),
            },
        )
        .unwrap();
    let user_lp_token_balance = response.balance;
    assert_eq!(user_lp_token_balance, expected_balance);
}

pub fn assert_user_bonded_amount(
    app: &mut App,
    user_addr: &Addr,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
    expected_balance: Uint128,
) {
    let bonded_amount: Uint128 = app
        .wrap()
        .query_wasm_smart(
            multistaking_contract.clone(),
            &QueryMsg::BondedLpTokens {
                user: user_addr.clone(),
                lp_token: lp_token_addr.clone(),
            },
        )
        .unwrap();
    assert_eq!(bonded_amount, expected_balance);
}

pub fn query_cw20_balance(app: &mut App, cw20_addr: &Addr, user_addr: &Addr) -> Uint128 {
    app.wrap()
        .query_wasm_smart(
            cw20_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: user_addr.to_string(),
            },
        )
        .map(|r: BalanceResponse| r.balance)
        .unwrap()
}

pub fn query_balance(app: &mut App, user_addr: &Addr) -> Vec<Coin> {
    app.wrap().query_all_balances(user_addr).unwrap()
}

pub fn query_instant_unlock_fee_tiers(
    app: &mut App,
    multistaking_contract: &Addr,
) -> Vec<UnlockFeeTier> {
    let fee_tiers: Vec<UnlockFeeTier> = app
        .wrap()
        .query_wasm_smart(
            multistaking_contract.clone(),
            &QueryMsg::InstantUnlockFeeTiers {},
        )
        .unwrap();

    return fee_tiers;
}

pub fn query_instant_lp_unlock_fee(
    app: &mut App,
    multistaking_contract: &Addr,
    lp_token_addr: &Addr,
    user_addr: &Addr,
    token_lock: TokenLock,
) -> InstantLpUnlockFee {
    let fee: InstantLpUnlockFee = app
        .wrap()
        .query_wasm_smart(
            multistaking_contract.clone(),
            &QueryMsg::InstantUnlockFee {
                lp_token: lp_token_addr.clone(),
                user: user_addr.clone(),
                token_lock,
            },
        )
        .unwrap();

    return fee;
}
