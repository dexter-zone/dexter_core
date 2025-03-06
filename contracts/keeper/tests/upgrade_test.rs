use cosmwasm_std::{to_json_binary, Addr};
use dexter::keeper::{Config, MigrateMsg, QueryMsg};
use persistence_std::types::{cosmos::gov::v1::{MsgSubmitProposal, MsgVote, VoteOption}, cosmwasm::wasm::v1::MsgMigrateContract};
use persistence_test_tube::{Account, Gov, Module, PersistenceTestApp, SigningAccount, Wasm};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Temporary struct to match the V1 contract's InstantiateMsg format
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
struct KeeperV1InstantiateMsg {
    /// Owner address
    pub owner: Addr,
}

#[test]
fn test_contract_upgrade() {
    println!("=== Starting contract upgrade test ===");
    
    // Initialize the test app
    let app = PersistenceTestApp::new();
    println!("✅ Test app initialized");
    
    let accs = app.init_accounts(&[cosmwasm_std::Coin::new(1_000_000_000_000, "uxprt")], 1).unwrap();
    let user = &accs[0];
    println!("✅ Test account created with address: {}", user.address());
    
    let wasm = Wasm::new(&app);
    
    // 1. Store V1 code
    println!("📦 Storing V1 code...");
    let v1_code_id = store_contract_code(&app, user, "../../artifacts/dexter_keeper_v1.wasm");
    println!("✅ V1 code stored with code ID: {}", v1_code_id);
    
    // 2. Instantiate keeper using V1 artifact with the correct V1 message format
    println!("🚀 Instantiating keeper V1...");
    let keeper_instance = instantiate_keeper_v1(&app, user, v1_code_id);
    println!("✅ Keeper V1 instantiated at address: {}", keeper_instance);
    
    // 3. Store V2 code
    println!("📦 Storing V2 code...");
    let v2_code_id = store_contract_code(&app, user, "../../artifacts/dexter_keeper-aarch64.wasm");
    println!("✅ V2 code stored with code ID: {}", v2_code_id);
    
    // 4. Store and instantiate vault
    println!("📦 Storing vault code...");
    let vault_code_id = store_contract_code(&app, user, "../../artifacts/dexter_vault-aarch64.wasm");
    println!("✅ Vault code stored with code ID: {}", vault_code_id);
    
    println!("🚀 Instantiating vault...");
    let vault_instance = instantiate_vault(&app, user, vault_code_id);
    println!("✅ Vault instantiated at address: {}", vault_instance);
    
    // 5. Migrate keeper contract from V1 to V2 with the vault address using governance
    println!("🔄 Preparing migration from V1 to V2...");
    let migrate_msg = MigrateMsg::V2 {
        vault_address: vault_instance.to_string(),
    };
    println!("✅ Migration message created: {:?}", migrate_msg);
    
    // Create a migrate message
    println!("📝 Creating MsgMigrateContract...");
    let migrate_msg_wasm = MsgMigrateContract {
        msg: to_json_binary(&migrate_msg).unwrap().to_vec(),
        sender: "persistence10d07y265gmmuvt4z0w9aw880jnsr700j5w4kch".to_string(),
        contract: keeper_instance.to_string(),
        code_id: v2_code_id,
    };
    println!("✅ MsgMigrateContract created with:");
    println!("   - Contract: {}", keeper_instance);
    println!("   - New code ID: {}", v2_code_id);
    println!("   - Sender: {}", "persistence10d07y265gmmuvt4z0w9aw880jnsr700j5w4kch");
    
    // Submit and execute the migration through governance
    println!("🏛️ Submitting governance proposal...");
    let proposal_id = submit_governance_proposal(&app, user, migrate_msg_wasm);
    println!("✅ Governance proposal submitted with ID: {}", proposal_id);
    
    println!("🗳️ Voting on proposal...");
    vote_on_proposal(&app, proposal_id);
    println!("✅ Proposal passed");
    
    // Verify the config was updated with the new vault address
    println!("🔍 Verifying contract configuration after migration...");
    let config: Config = wasm
        .query(&keeper_instance.to_string(), &QueryMsg::Config {})
        .unwrap();
    
    println!("📊 Current config:");
    println!("   - Vault address: {}", config.vault_address);
    println!("   - Expected vault address: {}", vault_instance);
    
    assert_eq!(config.vault_address, vault_instance, "Vault address was not updated correctly");
    println!("✅ Migration successful! Vault address correctly updated.");
    println!("=== Contract upgrade test completed successfully ===");
}

fn store_contract_code(app: &PersistenceTestApp, user: &SigningAccount, path: &str) -> u64 {
    println!("   Reading WASM file from: {}", path);
    let wasm_byte_code = match std::fs::read(path) {
        Ok(code) => {
            println!("   ✅ WASM file read successfully, size: {} bytes", code.len());
            code
        },
        Err(e) => {
            println!("   ❌ Failed to read WASM file: {}", e);
            panic!("Failed to read WASM file: {}", e);
        }
    };
    
    let wasm = Wasm::new(app);
    println!("   Storing code on chain...");
    let result = wasm.store_code(&wasm_byte_code, None, user);
    
    match &result {
        Ok(res) => {
            println!("   ✅ Code stored successfully with code ID: {}", res.data.code_id);
            res.data.code_id
        },
        Err(e) => {
            println!("   ❌ Failed to store code: {}", e);
            panic!("Failed to store code: {}", e);
        }
    }
}

fn instantiate_keeper_v1(app: &PersistenceTestApp, user: &SigningAccount, code_id: u64) -> Addr {
    let wasm = Wasm::new(app);
    
    // Use the V1-compatible instantiate message
    let instantiate_msg = KeeperV1InstantiateMsg {
        owner: Addr::unchecked(user.address()),
    };
    println!("   Instantiating keeper V1 with owner: {}", user.address());
    
    let res = match wasm.instantiate(
        code_id,
        &instantiate_msg,
        None,
        Some("Keeper V1"),
        &[],
        user,
    ) {
        Ok(res) => {
            println!("   ✅ Keeper V1 instantiated successfully at: {}", res.data.address);
            res
        },
        Err(e) => {
            println!("   ❌ Failed to instantiate keeper V1: {}", e);
            panic!("Failed to instantiate keeper V1: {}", e);
        }
    };
    
    Addr::unchecked(res.data.address)
}

fn instantiate_vault(app: &PersistenceTestApp, user: &SigningAccount, code_id: u64) -> Addr {
    let wasm = Wasm::new(app);
    
    // Simplified vault instantiation for testing purposes
    let instantiate_msg = dexter::vault::InstantiateMsg {
        owner: user.address().to_string(),
        pool_configs: vec![],
        lp_token_code_id: None,
        fee_collector: None,
        pool_creation_fee: dexter::vault::PoolCreationFee::Disabled {},
        auto_stake_impl: dexter::vault::AutoStakeImpl::Multistaking { 
            contract_addr: Addr::unchecked("persistence1k8re7jwz6rnnwrktnejdwkwnncte7ek7gt29gvnl3sdrg9mtnqkstujtpg") 
        },
    };
    println!("   Instantiating vault with owner: {}", user.address());
    
    let res = match wasm.instantiate(
        code_id,
        &instantiate_msg,
        None,
        Some("Vault"),
        &[],
        user,
    ) {
        Ok(res) => {
            println!("   ✅ Vault instantiated successfully at: {}", res.data.address);
            res
        },
        Err(e) => {
            println!("   ❌ Failed to instantiate vault: {}", e);
            panic!("Failed to instantiate vault: {}", e);
        }
    };
    
    Addr::unchecked(res.data.address)
}

fn submit_governance_proposal(
    app: &PersistenceTestApp, 
    user: &SigningAccount, 
    msg: MsgMigrateContract
) -> u64 {
    println!("   Creating governance proposal for contract migration");
    
    // Create a governance proposal
    let msg_submit_proposal = MsgSubmitProposal {
        messages: vec![msg.to_any()],
        initial_deposit: vec![persistence_std::types::cosmos::base::v1beta1::Coin {
            denom: "uxprt".to_string(),
            amount: cosmwasm_std::Uint128::new(1000000000).to_string(),
        }],
        proposer: user.address().to_string(),
        metadata: "Contract Upgrade".to_string(),
        title: "Upgrade Keeper Contract".to_string(),
        summary: "Proposal to upgrade the Keeper contract to a new version".to_string(),
    };
    println!("   Proposal created with:");
    println!("   - Title: {}", msg_submit_proposal.title);
    println!("   - Proposer: {}", msg_submit_proposal.proposer);
    println!("   - Initial deposit: {} {}", 1000000000, "uxprt");
    
    // Submit the proposal
    let gov = Gov::new(app);
    println!("   Submitting proposal to governance module...");
    let result = gov.submit_proposal(msg_submit_proposal, user);
    
    let proposal_id = match result {
        Ok(res) => {
            println!("   ✅ Proposal submitted successfully with ID: {}", res.data.proposal_id);
            res.data.proposal_id
        },
        Err(e) => {
            println!("   ❌ Failed to submit proposal: {}", e);
            panic!("Failed to submit proposal: {}", e);
        }
    };
    
    proposal_id
}

fn vote_on_proposal(app: &PersistenceTestApp, proposal_id: u64) {
    let gov = Gov::new(app);
    
    // Get the validator to vote on the proposal
    let validator = match app.get_first_validator_signing_account() {
        Ok(v) => {
            println!("   ✅ Got validator account: {}", v.address());
            v
        },
        Err(e) => {
            println!("   ❌ Failed to get validator account: {}", e);
            panic!("Failed to get validator account: {}", e);
        }
    };
    
    // Vote yes on the proposal
    println!("   Validator voting YES on proposal {}...", proposal_id);
    let vote_result = gov.vote(
        MsgVote {
            proposal_id,
            voter: validator.address(),
            option: VoteOption::Yes as i32,
            metadata: "".to_string(),
        },
        &validator,
    );
    
    match vote_result {
        Ok(_) => println!("   ✅ Vote cast successfully"),
        Err(e) => {
            println!("   ❌ Failed to cast vote: {}", e);
            panic!("Failed to cast vote: {}", e);
        }
    }
    
    // Find the proposal voting end time and increase chain time to that
    println!("   Querying proposal details...");
    let proposal = match gov.query_proposal(&persistence_std::types::cosmos::gov::v1::QueryProposalRequest { 
        proposal_id 
    }) {
        Ok(res) => {
            println!("   ✅ Proposal details retrieved");
            res.proposal.unwrap()
        },
        Err(e) => {
            println!("   ❌ Failed to query proposal: {}", e);
            panic!("Failed to query proposal: {}", e);
        }
    };
    
    let proposal_end_time = proposal.voting_end_time.unwrap();
    let proposal_start_time = proposal.voting_start_time.unwrap();
    
    let difference_seconds = proposal_end_time.seconds - proposal_start_time.seconds;
    println!("   Voting period: {} seconds", difference_seconds);
    println!("   Fast-forwarding chain time to end of voting period...");
    app.increase_time(difference_seconds as u64);
    println!("   ✅ Chain time increased");
    
    // Verify the proposal has passed
    println!("   Verifying proposal status...");
    let proposal = match gov.query_proposal(&persistence_std::types::cosmos::gov::v1::QueryProposalRequest { 
        proposal_id 
    }) {
        Ok(res) => {
            println!("   ✅ Proposal details retrieved");
            res.proposal.unwrap()
        },
        Err(e) => {
            println!("   ❌ Failed to query proposal: {}", e);
            panic!("Failed to query proposal: {}", e);
        }
    };
    
    println!("   Proposal status: {}", proposal.status);
    
    assert_eq!(
        proposal.status, 
        persistence_std::types::cosmos::gov::v1::ProposalStatus::Passed as i32,
        "Proposal did not pass"
    );
    println!("   ✅ Proposal has passed successfully");
} 