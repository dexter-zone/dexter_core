use std::fmt::Display;
use std::fs::File;
use std::io::Read;
use std::process::Command;

use cosmwasm_std::{to_json_binary, Addr, Coin, CosmosMsg, Event, Uint128, WasmMsg};
use cw20::MinterResponse;
use dexter::constants::GOV_MODULE_ADDRESS;
use dexter::vault::FeeInfo;

use dexter::vault::{PauseInfo, PoolCreationFee, PoolType, PoolTypeConfig};


use persistence_std::types::cosmos::bank::v1beta1::QueryBalanceRequest;
use persistence_std::types::cosmos::gov::v1::{
    MsgSubmitProposal, MsgVote, ProposalStatus, QueryProposalRequest, VoteOption,
};
use persistence_std::types::cosmwasm::wasm::v1::MsgExecuteContract;
use persistence_test_tube::{Account, Bank, Gov, Module, PersistenceTestApp, SigningAccount, Wasm};

#[macro_export]
macro_rules! uint128_with_precision {
    ($value:expr, $precision:expr) => {
        cosmwasm_std::Uint128::from($value)
            .checked_mul(cosmwasm_std::Uint128::from(10u64).pow($precision as u32))
            .unwrap()
    };
}

#[allow(dead_code)]
fn compile_current_contract_without_symbols() {
    let _output = Command::new("cargo")
        .env("RUSTFLAGS", "-C link-arg=-s")
        .args(&["wasm"])
        .output()
        .unwrap();

    // println!("output: {:?}", output);
}

#[allow(dead_code)]
fn move_compiled_contract_to_artifacts() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let _output = Command::new("cp")
        .args(&[
            format!(
                "{}/../../target/wasm32-unknown-unknown/release/dexter_governance_admin.wasm",
                manifest_dir
            ),
            format!(
                "{}/../../artifacts/dexter_governance_admin.wasm",
                manifest_dir
            ),
            // "target/wasm32-unknown-unknown/release/dexter_governance_admin.wasm",
            // "artifacts/dexter_governance_admin.wasm",
        ])
        .output()
        .unwrap();

    // println!("output: {:?}", output);
}

fn read_wasm_byte_code_at_path(path: &str) -> Vec<u8> {
    let test_base_path = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = format!("{}/../../{}", test_base_path, path);

    let mut wasm_byte_code = Vec::new();
    let mut file = File::open(path).unwrap();
    file.read_to_end(&mut wasm_byte_code).unwrap();
    wasm_byte_code
}

pub struct GovAdminTestSetup {
    pub accs: Vec<SigningAccount>,

    pub persistence_test_app: PersistenceTestApp,

    pub gov_admin_instance: Addr,
    pub vault_instance: Addr,
    pub keeper_instance: Addr,
    pub multi_staking_instance: Addr,

    pub cw20_token_1: Addr,
    pub cw20_token_2: Addr,
}

impl Display for GovAdminTestSetup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "
            Gov Admin: {}
            Vault: {}
            Keeper: {}
            Multi Staking: {}
            CW20 Token 1: {}
            CW20 Token 2: {}
        ",
            self.gov_admin_instance,
            self.vault_instance,
            self.keeper_instance,
            self.multi_staking_instance,
            self.cw20_token_1,
            self.cw20_token_2
        )
    }
}

pub fn setup_test_contracts() -> GovAdminTestSetup {
    // compile_current_contract_without_symbols();
    // move_compiled_contract_to_artifacts();

    let persistence_test_app = PersistenceTestApp::new();
    let accs = persistence_test_app
        .init_accounts(
            &[
                Coin::new(1_000_000_000_000, "uxprt"),
                // Coin::new(1_000_000_000_000, "uosmo"),
            ],
            1,
        )
        .unwrap();

    let user = &accs[0];
    let address = user.address();
    println!("admin address: {}", address);

    let wasm = Wasm::new(&persistence_test_app);

    let gov_admin_wasm_code =
        read_wasm_byte_code_at_path("artifacts/dexter_governance_admin-aarch64.wasm");
    let vault_wasm_code = read_wasm_byte_code_at_path("artifacts/dexter_vault-aarch64.wasm");
    let keeper_wasm_code = read_wasm_byte_code_at_path("artifacts/dexter_keeper-aarch64.wasm");
    let stable_pool_wasm_code = read_wasm_byte_code_at_path("artifacts/stable_pool-aarch64.wasm");
    let weighted_pool_wasm_code =
        read_wasm_byte_code_at_path("artifacts/weighted_pool-aarch64.wasm");
    let multi_staking_wasm_code =
        read_wasm_byte_code_at_path("artifacts/dexter_multi_staking-aarch64.wasm");
    let lp_token_wasm_code = read_wasm_byte_code_at_path("artifacts/lp_token-aarch64.wasm");

    let gov_admin_code_id = wasm
        .store_code(&gov_admin_wasm_code, None, &user)
        .unwrap()
        .data
        .code_id;
    let vault_code_id = wasm
        .store_code(&vault_wasm_code, None, &user)
        .unwrap()
        .data
        .code_id;
    let keeper_code_id = wasm
        .store_code(&keeper_wasm_code, None, &user)
        .unwrap()
        .data
        .code_id;
    let stable_pool_code_id = wasm
        .store_code(&stable_pool_wasm_code, None, &user)
        .unwrap()
        .data
        .code_id;
    let weighted_pool_code_id = wasm
        .store_code(&weighted_pool_wasm_code, None, &user)
        .unwrap()
        .data
        .code_id;
    let lp_token_code_id = wasm
        .store_code(&lp_token_wasm_code, None, &user)
        .unwrap()
        .data
        .code_id;
    let multi_staking_code_id = wasm
        .store_code(&multi_staking_wasm_code, None, &user)
        .unwrap()
        .data
        .code_id;

    // instantiate gov admin first
    let gov_admin_instantiate_msg = dexter::governance_admin::InstantiateMsg {};
    let gov_admin_instance = wasm
        .instantiate(
            gov_admin_code_id,
            &gov_admin_instantiate_msg,
            None,
            Some("Dexter Gov Admin"),
            &[],
            &user,
        )
        .unwrap()
        .data
        .address;

    // instante the multistaking contract
    let multi_staking_instantiate = dexter::multi_staking::InstantiateMsg {
        owner: Addr::unchecked(gov_admin_instance.clone()),
        keeper_addr: Addr::unchecked(gov_admin_instance.clone()),
        minimum_reward_schedule_proposal_start_delay: 0,
        unbond_config: dexter::multi_staking::UnbondConfig {
            unlock_period: 86400u64,
            instant_unbond_config: dexter::multi_staking::InstantUnbondConfig::Enabled {
                min_fee: 200u64,
                max_fee: 500u64,
                fee_tier_interval: 86400u64,
            },
        },
    };

    let multi_staking_instance = wasm
        .instantiate(
            multi_staking_code_id,
            &multi_staking_instantiate,
            None,
            Some("Dexter Multi Staking"),
            &[],
            &user,
        )
        .unwrap()
        .data
        .address;

    let pool_configs = vec![
        PoolTypeConfig {
            code_id: stable_pool_code_id,
            pool_type: PoolType::StableSwap {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            paused: PauseInfo::default(),
        },
        PoolTypeConfig {
            code_id: weighted_pool_code_id,
            pool_type: PoolType::Weighted {},
            default_fee_info: FeeInfo {
                total_fee_bps: 300u16,
                protocol_fee_percent: 49u16,
            },
            allow_instantiation: dexter::vault::AllowPoolInstantiation::Everyone,
            paused: PauseInfo::default(),
        },
    ];

    // instantiate the vault with gov admin
    let vault_instantiate_msg = dexter::vault::InstantiateMsg {
        owner: gov_admin_instance.clone(),
        pool_configs: pool_configs,
        lp_token_code_id: Some(lp_token_code_id),
        fee_collector: None,
        pool_creation_fee: PoolCreationFee::default(),
        auto_stake_impl: dexter::vault::AutoStakeImpl::Multistaking {
            contract_addr: Addr::unchecked(multi_staking_instance.clone()),
        },
    };

    let vault_instance = wasm
        .instantiate(
            vault_code_id,
            &vault_instantiate_msg,
            None,
            Some("Dexter Vault"),
            &[],
            &user,
        )
        .unwrap()
        .data
        .address;

    // instantiate keeper contract
    let keeper_instantiate_msg = dexter::keeper::InstantiateMsg {
        owner: Addr::unchecked(user.address()),
        vault_address: Addr::unchecked(vault_instance.clone()),
    };

    let keeper_instance = wasm
        .instantiate(
            keeper_code_id,
            &keeper_instantiate_msg,
            None,
            Some("Dexter Keeper"),
            &[],
            &user,
        )
        .unwrap()
        .data
        .address;

    // update keeper contract address in vault
    let vault_update_keeper_msg = dexter::vault::ExecuteMsg::UpdateConfig {
        lp_token_code_id: None,
        fee_collector: Some(keeper_instance.clone()),
        pool_creation_fee: None,
        auto_stake_impl: None,
        paused: None,
    };

    let msg_update_keeper_in_vault = dexter::governance_admin::ExecuteMsg::ExecuteMsgs {
        msgs: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: vault_instance.clone(),
            msg: to_json_binary(&vault_update_keeper_msg).unwrap(),
            funds: vec![],
        })],
    };

    let wasm_msg = MsgExecuteContract {
        sender: GOV_MODULE_ADDRESS.to_owned(),
        contract: gov_admin_instance.to_string(),
        msg: to_json_binary(&msg_update_keeper_in_vault).unwrap().0,
        funds: vec![],
    };

    let msg_submit_proposal = MsgSubmitProposal {
        messages: vec![wasm_msg.to_any()],
        initial_deposit: vec![persistence_std::types::cosmos::base::v1beta1::Coin {
            denom: "uxprt".to_string(),
            amount: Uint128::new(1000000000).to_string(),
        }],
        proposer: user.address().to_string(),
        metadata: "Update vault config".to_string(),
        title: "Update vault config".to_string(),
        summary: "EMPTY".to_string(),
    };

    let gov = Gov::new(&persistence_test_app);
    let proposal_id = gov
        .submit_proposal(msg_submit_proposal, user)
        .unwrap()
        .data
        .proposal_id;

    // vote as the validator
    let validator_signing_account = persistence_test_app
        .get_first_validator_signing_account()
        .unwrap();
    gov.vote(
        MsgVote {
            proposal_id,
            voter: validator_signing_account.address(),
            option: VoteOption::Yes as i32,
            metadata: "pass kardo bhai".to_string(),
        },
        &validator_signing_account,
    )
    .unwrap();

    // make time pass
    // wait for the proposal to pass
    let proposal = gov
        .query_proposal(&QueryProposalRequest {
            proposal_id: proposal_id,
        })
        .unwrap()
        .proposal
        .unwrap();

    // find the proposal voting end time and increase chain time to that
    let proposal_end_time = proposal.voting_end_time.unwrap();
    let proposal_start_time = proposal.voting_start_time.unwrap();

    let difference_seconds = proposal_end_time.seconds - proposal_start_time.seconds;

    persistence_test_app.increase_time(difference_seconds as u64);
    // query proposal again
    // let proposal = gov.query_proposal(&QueryProposalRequest {
    //     proposal_id,
    // }).unwrap().proposal.unwrap();

    // query vault config
    // let vault_config: ConfigResponse = wasm
    //     .query(
    //         &vault_instance,
    //         &dexter::vault::QueryMsg::Config {},
    //     )
    //     .unwrap();

    // wasm.execute(
    //     &vault_instance.clone(),
    //     &vault_update_keeper_msg,
    //     &[],
    //     &admin,
    // ).unwrap();

    // create 2 CW20 tokens
    let cw20_test_token_1_address = wasm
        .instantiate(
            lp_token_code_id,
            &cw20_base::msg::InstantiateMsg {
                name: "Test Token".to_string(),
                symbol: "TTT".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: user.address().to_string(),
                    cap: None,
                }),
                marketing: None,
            },
            None,
            Some("Test Token"),
            &[],
            &user,
        )
        .unwrap()
        .data
        .address;

    let cw20_test_token_2_address = wasm
        .instantiate(
            lp_token_code_id,
            &cw20_base::msg::InstantiateMsg {
                name: "Test Token 2".to_string(),
                symbol: "TTTT".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: user.address().to_string(),
                    cap: None,
                }),
                marketing: None,
            },
            None,
            Some("Test Token 2"),
            &[],
            &user,
        )
        .unwrap()
        .data
        .address;

    GovAdminTestSetup {
        persistence_test_app,
        accs,
        gov_admin_instance: Addr::unchecked(gov_admin_instance),
        vault_instance: Addr::unchecked(vault_instance),
        keeper_instance: Addr::unchecked(keeper_instance),

        cw20_token_1: Addr::unchecked(cw20_test_token_1_address),
        cw20_token_2: Addr::unchecked(cw20_test_token_2_address),
        multi_staking_instance: Addr::unchecked(multi_staking_instance),
    }
    // persistence_test_app.sed
}

pub fn vote_on_proposal(
    gov_admin_test_setup: &GovAdminTestSetup,
    proposal_id: u64,
    vote_option: VoteOption,
) {
    let validator = gov_admin_test_setup
        .persistence_test_app
        .get_first_validator_signing_account()
        .unwrap();

    // vote on the proposal
    let vote_msg = persistence_std::types::cosmos::gov::v1::MsgVote {
        proposal_id: proposal_id.clone(),
        voter: validator.address(),
        option: vote_option.into(),
        metadata: "".to_string(),
    };

    let gov = Gov::new(&gov_admin_test_setup.persistence_test_app);
    gov.vote(vote_msg, &validator).unwrap();

    // make time fast forward for the proposal to pass
    let proposal = gov
        .query_proposal(&QueryProposalRequest { proposal_id })
        .unwrap()
        .proposal
        .unwrap();

    // find the proposal voting end time and increase chain time to that
    let proposal_end_time = proposal.voting_end_time.unwrap();
    let proposal_start_time = proposal.voting_start_time.unwrap();

    let difference_seconds = proposal_end_time.seconds - proposal_start_time.seconds;
    gov_admin_test_setup
        .persistence_test_app
        .increase_time(difference_seconds as u64);

    // query proposal again
    let proposal = gov
        .query_proposal(&QueryProposalRequest { proposal_id })
        .unwrap()
        .proposal
        .unwrap();

    // assert that the proposal has passed or rejected based on the vote
    match vote_option {
        VoteOption::Yes => assert_eq!(proposal.status, ProposalStatus::Passed as i32),
        VoteOption::No => assert_eq!(proposal.status, ProposalStatus::Rejected as i32),
        VoteOption::NoWithVeto => assert_eq!(proposal.status, ProposalStatus::Rejected as i32),
        _ => panic!("Invalid vote option"),
    }
}

pub fn find_event_attr(events: &Vec<Event>, event: &str, attr: &str) -> String {
    return events
        .iter()
        .find(|e| e.ty == event)
        .unwrap()
        .attributes
        .iter()
        .find(|a| a.key == attr)
        .unwrap()
        .value
        .clone();
}

pub fn query_balance(
    gov_admin_test_setup: &GovAdminTestSetup,
    address: String,
    denom: String,
) -> Uint128 {
    let bank = Bank::new(&gov_admin_test_setup.persistence_test_app);
    return bank
        .query_balance(&QueryBalanceRequest { address, denom })
        .unwrap()
        .balance
        .unwrap()
        .amount
        .parse()
        .unwrap();
}
