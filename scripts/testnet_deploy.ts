import { PersistenceClient, cosmwasm } from "cosmoschainsjs";
import * as Pako from "pako";
import * as fs from "fs";
import { Gov_MsgSubmitProposal, voteOnProposal, readArtifact, writeArtifact, executeContract,
  query_gov_proposal, find_code_id_from_contract_hash, query_wasm_contractsByCode, query_wasm_code} from "./helpers/helpers.js";
import { toBinary } from "@cosmjs/cosmwasm-stargate";
import { Slip10RawIndex, pathToString, stringToPath } from '@cosmjs/crypto';

// ----------- PERSISTENCE END-POINTS -------------
// testnet: https://rpc.testnet.persistence.one:443     :: test-core-1
// mainnet: https://rpc.persistence.one:443             :: core-1

// This is your rpc endpoint
const rpcEndpoint = "http://localhost:26657";

// Make HD path used during wallet creation
export function makeHdPath(coinType = 118, account = 0) {
  return [
    Slip10RawIndex.hardened(44),
    Slip10RawIndex.hardened(coinType),
    Slip10RawIndex.hardened(0),
    Slip10RawIndex.normal(0),
    Slip10RawIndex.normal(account)
  ];
}

async function Demo() {

  // Using a random generated mnemonic
  const mnemonic = "gravity bus kingdom auto limit gate humble abstract reopen resemble awkward cannon maximum bread balance insane banana maple screen mimic cluster pigeon badge walnut";
  const deposit_amount = 512_000_000;
  const fee_denom = "uxprt";
  const CHAIN_ID = "testing";

  // network : stores contract addresses
  let network = readArtifact(CHAIN_ID);
  console.log("network")

  // Create a new persistence client
  const client = await PersistenceClient.init(mnemonic , {
    rpc: rpcEndpoint,
    chainId: CHAIN_ID,
    gasPrices: {
      denom: fee_denom,
      amount: "2000000",
    },
    gasAdjustment: "1.5",
  });

  // Create Persistence Validators
  const validator_1 = await PersistenceClient.init("flash tuna music boat sign image judge engage pistol reason love reform defy game ceiling basket roof clay keen hint flash buyer fancy buyer" , {
    rpc: rpcEndpoint,
    chainId: "testing",
    gasPrices: {
      denom: fee_denom,
      amount: "2000000",
    },
    gasAdjustment: "1.5",
  });
  const validator_2 = await PersistenceClient.init("horse end velvet train canoe walnut lottery security sure right rigid busy either sand bar palace choice extend august mystery action surround coconut online" , {
    rpc: rpcEndpoint,
    chainId: "testing",
    gasPrices: {
      denom: fee_denom,
      amount: "2000000",
    },
    gasAdjustment: "1.5",
  });
  
  // Get wallet address
  const [Account] = await client.wallet.getAccounts();
  const wallet_address = Account.address;
  console.log(`WALLET ADDRESS =  ${wallet_address}`);

  // Get chain height
  const height = await client.wasm.getHeight();
  console.log(`Blockchain height = ${height}`);

  // Get xprt balance
  const balance_res = await client.wasm.getBalance( wallet_address, fee_denom);
  let wallet_balance = Number(balance_res["amount"])/10**6;
  console.log(`Wallet's XPRT balance = ${wallet_balance}`);

  // let codes = await getContractsByCodeId(client, 1);
  // console.log(`CODES = ${JSON.stringify(codes)}`);

  // return;

  // -----------x-------------x-------------x------------------------------
  // ----------- MAKE STORE CODE PROPOSALS FOR ALL DEXTER CONTRACTS -------
  // -----------x-------------x-------------x------------------------------

  // // CONTRACTS WHICH ARE TO BE DEPLOYED ON PERSISTENCE ONE NETWORK FOR DEXTER PROTOCOL
  let contracts = [
  { name: "Dexter Vault", path: "../artifacts/dexter_vault.wasm", proposal_id: 0, hash:"8c90ca57c3624d4676a0f63c898ff15dc709ef6066359b6e9c3dc094b3012774"  }, 
  { name: "Dexter Keeper", path: "../artifacts/dexter_keeper.wasm", proposal_id: 0, hash:"51de835121fcfa4fd772d68889b98294c789bc2a50a80a9132fead99b755e0c3"  }, 
  { name: "LP Token", path: "../artifacts/lp_token.wasm", proposal_id: 0, hash:"48ac9688ad68b66c36184b47682c061ae2763c769e458ef190064d2013563418"  }, 
  { name: "XYK Pool", path: "../artifacts/xyk_pool.wasm", proposal_id: 0, hash:"34f5f23b815105bb76da42efaaae31e682501560a291100c96c4a94d99f2c96a"  }, 
  { name: "Weighted Pool", path: "../artifacts/weighted_pool.wasm", proposal_id: 0, hash:"c2a9fbb275327831fdec61bffb694014613ed010bfd8448c8116abb2248a64f5"  }, 
  { name: "Stableswap Pool", path: "../artifacts/stableswap_pool.wasm", proposal_id: 0, hash:"4ec84d214a1403addf1460013c08b0316699a51bd85e343692b01f5a482b1bb4"  }, 
  { name: "Stable5Swap Pool", path: "../artifacts/stable5pool.wasm", proposal_id: 0, hash:"b2c73f4f3633db13f9bef932e6dd0f00acb36128b34d292e81e142282adb4664"  }, 
  { name: "Dexter Vesting", path: "../artifacts/dexter_vesting.wasm", proposal_id: 0, hash:"9fed0b82283c3881c242cc51d80c1d9b73fb8fd038da726d6f850de2736a253f"  }, 
  { name: "Dexter Generator", path: "../artifacts/dexter_generator.wasm", proposal_id: 0, hash:"d83433369379a5cec32b7ea4de7574222964ddec9f70d64c7775acbb1e008747"  }, 
  {
    name: "Dexter Generator : Proxy",
    path: "../artifacts/dexter_generator_proxy.wasm",
    proposal_id: 0 ,
    hash: "3d0d2eb1b6b8ba699ec2a0fbf2677da2d7b481adf84d19dc1c0641dbfd346289"
  },
  { name: "Staking contract", path: "../artifacts/anchor_staking.wasm", proposal_id: 0, hash:"e119395d04dafdaf6f87cf38c1fb6cf81ddbd69b7874d7751bb7be66bd3a9883"  },
  ];

  // UPLOAD CODE OF ALL CONTRACTS
  if (!network.contracts_store_code_proposals_executed || network.contracts_store_code_proposals_executed == 0) {        
    console.log(network.contracts_store_code_proposals_executed);
    console.log("GETGRTGE")
      // Loop across all contracts
      for (let i = 0; i < contracts.length; i++) {
        // TRANSATION 1. --> Make proposal on-chain      
        try {
          console.log(`\nSubmitting Proposal to store ${contracts[i]["name"]} Contract ...`);
          const wasm = fs.readFileSync(contracts[i]["path"]);
          const wasmStoreProposal = {
            typeUrl: "/cosmwasm.wasm.v1.StoreCodeProposal",
            value: Uint8Array.from(
              cosmwasm.wasm.v1.StoreCodeProposal.encode(
                cosmwasm.wasm.v1.StoreCodeProposal.fromPartial({
                  title: contracts[i]["name"],
                  description: `Add wasm code for ${contracts[i]["name"]} contract.`,
                  runAs: wallet_address,
                  wasmByteCode: Pako.gzip(wasm, { level: 9 }),
                  instantiatePermission: {                  
                    permission: cosmwasm.wasm.v1.accessTypeFromJSON(1),
                  },
                })
              ).finish()
            ),
          };
          const res = await Gov_MsgSubmitProposal(client, wasmStoreProposal, fee_denom, deposit_amount);
          contracts[i]["proposal_id"] = Number(res[0].events[3].attributes[1].value);          
          console.log(`${contracts[i]["name"]} STORE CODE PROPOSAL ID = ${contracts[i]["proposal_id"]}`)
        } catch (e) {
          console.log("Proposal Error has occoured => ", e);
        }  
        // TRANSACTION 2. --> Vote on proposal
        if (contracts[i]["proposal_id"] > 0 && CHAIN_ID == "testing") {
          try {
            await voteOnProposal(client, contracts[i]["proposal_id"], 1, fee_denom);
            await voteOnProposal(validator_1, contracts[i]["proposal_id"], 1, fee_denom);
            await voteOnProposal(validator_2, contracts[i]["proposal_id"], 1, fee_denom);
            console.log("Voted successfully")
          } catch (e) {
            console.log("Error has occoured while voting => ", e);
          }
        }
      }
      network.contracts_store_code_proposals_executed = true;

      // Update propsoal IDs stored 
      network.vault_store_code_proposal_id = contracts[0]["proposal_id"];
      network.keeper_store_code_proposal_id = contracts[1]["proposal_id"];
      network.lp_token_store_code_proposal_id = contracts[2]["proposal_id"];
      network.xyk_pool_store_code_proposal_id = contracts[3]["proposal_id"];
      network.weighted_pool_store_code_proposal_id = contracts[4]["proposal_id"];
      network.stableswap_pool_store_code_proposal_id = contracts[5]["proposal_id"];
      network.stable5swap_store_code_proposal_id = contracts[6]["proposal_id"];
      network.vesting_store_code_proposal_id = contracts[7]["proposal_id"];
      network.generator_store_code_proposal_id = contracts[8]["proposal_id"];
      network.generator_proxy_store_code_proposal_id = contracts[9]["proposal_id"];
      network.eq_staking_store_code_proposal_id = contracts[10]["proposal_id"];
      writeArtifact(network, CHAIN_ID);
  }  

  // GET CODE-IDs FOR ALL CONTRACTS
  if (!network.vault_contract_code_id || network.vault_contract_code_id == 0) {
    let code_id_res = await find_code_id_from_contract_hash(client, contracts[0]["hash"]);
    network.vault_contract_code_id = Number(code_id_res);  
  }
  if (!network.keeper_contract_code_id || network.keeper_contract_code_id == 0) {
    let code_id_res = await find_code_id_from_contract_hash(client, contracts[1]["hash"]);
    network.keeper_contract_code_id = Number(code_id_res);  
  }
  if (!network.lp_token_contract_code_id || network.lp_token_contract_code_id == 0) {
    let code_id_res = await find_code_id_from_contract_hash(client, contracts[2]["hash"]);
    network.lp_token_contract_code_id = Number(code_id_res);  
  }
  if (!network.xyk_pool_contract_code_id || network.xyk_pool_contract_code_id == 0) {
    let code_id_res = await find_code_id_from_contract_hash(client, contracts[3]["hash"]);
    network.xyk_pool_contract_code_id = Number(code_id_res);  
  }
  if (!network.weighted_pool_contract_code_id || network.weighted_pool_contract_code_id == 0) {
    let code_id_res = await find_code_id_from_contract_hash(client, contracts[4]["hash"]);
    network.weighted_pool_contract_code_id = Number(code_id_res);  
  }
  if (!network.stableswap_contract_code_id || network.stableswap_contract_code_id == 0) {
    let code_id_res = await find_code_id_from_contract_hash(client, contracts[5]["hash"]);
    network.stableswap_contract_code_id = Number(code_id_res);  
  }
  if (!network.stable5swap_pool_contract_code_id || network.stable5swap_pool_contract_code_id == 0) {
    let code_id_res = await find_code_id_from_contract_hash(client, contracts[6]["hash"]);
    network.stable5swap_pool_contract_code_id = Number(code_id_res);  
  }
  if (!network.vesting_contract_code_id || network.vesting_contract_code_id == 0) {
    let code_id_res = await find_code_id_from_contract_hash(client, contracts[7]["hash"]);
    network.vesting_contract_code_id = Number(code_id_res);  
  }
  if (!network.generator_contract_code_id || network.generator_contract_code_id == 0) {
    let code_id_res = await find_code_id_from_contract_hash(client, contracts[8]["hash"]);
    network.generator_contract_code_id = Number(code_id_res);  
  }
  if (!network.generator_proxy_contract_code_id || network.generator_proxy_contract_code_id == 0) {
    let code_id_res = await find_code_id_from_contract_hash(client, contracts[9]["hash"]);
    network.generator_proxy_contract_code_id = Number(code_id_res);  
  }
  if (!network.staking_contract_contract_code_id || network.staking_contract_contract_code_id == 0) {
    let code_id_res = await find_code_id_from_contract_hash(client, contracts[10]["hash"]);
    network.staking_contract_contract_code_id = Number(code_id_res);  
  }
  writeArtifact(network, CHAIN_ID);


  // -----------x-------------x---------x---------------
  // ----------- INSTANTIATE DEXTER VAULT  -------------
  // -----------x-------------x---------x---------------

  // INSTANTIATE DEXTER VAULT CONTRACT --> If vault contract has not been instantiated yet 
  if (network.vault_contract_code_id > 0 && (!network.vault_instantiate_proposal_id || network.vault_instantiate_proposal_id == 0 )) {  
      console.log(`\nSubmitting Proposal to instantiate Dexter VAULT Contract ...`);
      // Make proposal to instantiate Vault contract
      if (network.vault_contract_code_id > 0) {
        let init_vault_msg = {
          pool_configs: [
                  {
                    code_id: network.xyk_pool_contract_code_id,
                    pool_type: { xyk: {} },
                    fee_info: {
                      total_fee_bps: 300,
                      protocol_fee_percent: 49,
                      dev_fee_percent: 15,
                      developer_addr: wallet_address,
                    },
                    is_disabled: false,
                    is_generator_disabled: false,
                  },
                  {
                    code_id: network.stableswap_contract_code_id,
                    pool_type: { stable2_pool: {} },
                    fee_info: {
                      total_fee_bps: 300,
                      protocol_fee_percent: 49,
                      dev_fee_percent: 15,
                      developer_addr: null,
                    },
                    is_disabled: false,
                    is_generator_disabled: false,
                  },
                  {
                    code_id: network.stable5swap_pool_contract_code_id,
                    pool_type: { stable5_pool: {} },
                    fee_info: {
                      total_fee_bps: 300,
                      protocol_fee_percent: 49,
                      dev_fee_percent: 15,
                      developer_addr: null,
                    },
                    is_disabled: false,
                    is_generator_disabled: false,
                  },
                  {
                    code_id: network.weighted_pool_contract_code_id,
                    pool_type: { weighted: {} },
                    fee_info: {
                      total_fee_bps: 300,
                      protocol_fee_percent: 49,
                      dev_fee_percent: 15,
                      developer_addr: null,
                    },
                    is_disabled: false,
                    is_generator_disabled: false,
                  },
                ],          
          lp_token_code_id: network.lp_token_contract_code_id,
          fee_collector: null,
          owner: wallet_address,
          generator_address: null,
        };
        try {
          const wasmInstantiateProposal = {
            typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
            value: Uint8Array.from(
              cosmwasm.wasm.v1.InstantiateContractProposal.encode(
                cosmwasm.wasm.v1.InstantiateContractProposal.fromJSON({
                  title: "Dexter Vault",
                  description:  "Dexter Vault contract, used facilitating token swaps and instantiating pools",
                  runAs: wallet_address,
                  admin: wallet_address,
                  codeId: network.vault_contract_code_id.toString(),
                  label: "Dexter Vault",
                  msg: Buffer.from(JSON.stringify(init_vault_msg)).toString("base64"),
                })
              ).finish()
            ),
          };
          const res = await Gov_MsgSubmitProposal(client, wasmInstantiateProposal, fee_denom, deposit_amount);
          console.log(res);
          network.vault_instantiate_proposal_id = Number(res[0].events[3].attributes[1].value);
          writeArtifact(network, CHAIN_ID);
          // const json = JSON.parse(res.rawLog?);
        } catch (e) {
          console.log("Proposal Error has occoured => ", e);
        }
        // Vote on Proposal
        if (network.vault_instantiate_proposal_id > 0  && CHAIN_ID == "testing") {
          try {
            console.log(`Voting on Proposal # ${network.vault_instantiate_proposal_id}`);
            await voteOnProposal(client, network.vault_instantiate_proposal_id, 1, fee_denom);
            await voteOnProposal(validator_1, network.vault_instantiate_proposal_id, 1, fee_denom);
            await voteOnProposal(validator_2, network.vault_instantiate_proposal_id, 1, fee_denom);      
            console.log("Voted successfully")
          } catch (e) {
            console.log("Error has occoured while voting => ", e);
          }
        }
      }
  }

  // Get VAULT Contract Address if the proposal has passed
  if (!network.vault_contract_address || network.vault_contract_address == "") {
    let res = await query_wasm_contractsByCode(client, network.vault_contract_code_id );
    // console.log(res);
    network.vault_contract_address = res["contracts"][  res["contracts"].length - 1 ];
    writeArtifact(network, CHAIN_ID);
  }

  // -----------x-------------x-------------x---------x---------------
  // ----------- CONTRACT INSTIANTIATION :: TEST TOKENS --------------
  // -----------x-------------x-------------x---------x---------------

  if (!network.dummy_tokens_instantiated) {
    let tokens_ = [
      {name:"C-OSMO", symbol:"OSMO", decimals:6},
      {name:"C-JUNO", symbol:"JUNO", decimals:6},
      {name:"C-FET", symbol:"FET", decimals:6}
    ]
    for (let i=0;i<tokens_.length;i++) {
      let token_init_msg = {
        name: tokens_[i]["name"],
        symbol: tokens_[i]["symbol"],
        decimals: tokens_[i]["decimals"],
        initial_balances: [{ address: wallet_address, amount: "10000000000000" }],
        mint: { minter: wallet_address, amount: "1000000000000000" },
      };
      console.log(token_init_msg);
      try {
        const wasmInstantiateProposal = {
          typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
          value: Uint8Array.from(
            cosmwasm.wasm.v1.InstantiateContractProposal.encode(
              cosmwasm.wasm.v1.InstantiateContractProposal.fromJSON({
                title: "Test token",
                description: "Test token for testing dexter DEX",
                runAs: wallet_address,
                admin: wallet_address,
                codeId: network.lp_token_contract_code_id.toString(),
                label: "Dummy token",
                msg: Buffer.from(JSON.stringify(token_init_msg)).toString("base64"), // Buffer.from(JSON.stringify(init_msg)),
              })
            ).finish()
          ),
        };
        const res = await Gov_MsgSubmitProposal(client, wasmInstantiateProposal, fee_denom, deposit_amount);
        let proposalId = res[0].events[3].attributes[1].value;
        if (proposalId > 0) {
          network.dummy_tokens_instantiated = true;
          writeArtifact(network, CHAIN_ID);
        } 
        console.log(`Proposal Id for dummy token ${tokens_[i]["name"]} = ${proposalId}`)
        await voteOnProposal(client, proposalId, 1, fee_denom);
        await voteOnProposal(validator_1, proposalId, 1, fee_denom);
        await voteOnProposal(validator_2, proposalId, 1, fee_denom);       
        console.log(res);
      } catch (e) {
        console.log("Proposal Error has occoured => ", e);
      }    
    }
  }

  // Get VAULT Contract Address if the proposal has passed
  if (!network.test_tokens_addresses || network.test_tokens_addresses.length < 3 ) {
    let res = await query_wasm_contractsByCode(client, network.lp_token_contract_code_id );
    if (res["contracts"].length > 0) {
      network.test_tokens_addresses = res["contracts"];
      writeArtifact(network, CHAIN_ID);  
    }
  }

  // -----------x-------------x--------------x---------------x---------------x-----------------------
  // ----------- MAKE PROPOSALS TO UPDATE INSTANTIATION PERMISSIONS FOR POOL CONTRACTS  -------------
  // -----------x-------------x--------------x---------------x---------------x-----------------------


  // MAKE PROPOSALS TO UPDATE INSTANTIATE PERMISSION FOR CONTRACTS WHICH ARE TO BE INSTANTIATED BY THE VAULT CONTRACT
  let contracts_to_be_updated = [
    { name:"LP Token Pool", codeId: network.xyk_pool_contract_code_id, proposal_id:0 },
    { name:"XYK Pool", codeId: network.xyk_pool_contract_code_id, proposal_id:0 },
    { name:"Stableswap Pool", codeId: network.xyk_pool_contract_code_id, proposal_id:0 },
    { name:"Stable5swap Pool", codeId: network.xyk_pool_contract_code_id, proposal_id:0 },
    { name:"Weighted Pool", codeId: network.xyk_pool_contract_code_id, proposal_id:0 }
  ]
  if (!network.proposals_to_update_permissions || network.proposals_to_update_permissions == 0 ) {
    // Loop
    for (let i=0;i<contracts_to_be_updated.length;i++) {
      // TRANSATION 1. --> Make proposal on-chain
      try {
        console.log(`\nSubmitting Proposal to update Dexter ${contracts_to_be_updated[i]["name"]} Contract instantiation permission ...`);
        const wasmUpdateContractInstantiationPermissionProposal = {
          typeUrl: "/cosmwasm.wasm.v1.UpdateInstantiateConfigProposal",
          value: Uint8Array.from(
            cosmwasm.wasm.v1.UpdateInstantiateConfigProposal.encode(
              cosmwasm.wasm.v1.UpdateInstantiateConfigProposal.fromPartial({
                title: `Dexter :: ${contracts_to_be_updated[i]["name"]} update instantiation config`,
                description: `Update Dexter ${contracts_to_be_updated[i]["name"]} contract instantiation permission to vault addresss`,
                accessConfigUpdates: [
                  {
                    codeId: contracts_to_be_updated[i].codeId,
                    instantiatePermission: {
                      address: network.vault_contract_address,
                      permission: cosmwasm.wasm.v1.accessTypeFromJSON(2),
                    }
                  }
                ]
              })
            ).finish()
          ),
        };
        const res = await Gov_MsgSubmitProposal(client, wasmUpdateContractInstantiationPermissionProposal, fee_denom, deposit_amount);
        contracts_to_be_updated[i]["proposal_id"] = Number(res[0].events[3].attributes[1].value);
      } catch (e) {
        console.log("Proposal Error has occoured => ", e);
      }  
      // TRANSACTION 2. --> Vote on proposal
      if (contracts_to_be_updated[i]["proposal_id"] > 0 && CHAIN_ID == "testing") {
        try {
          console.log(`Voting on Proposal # ${contracts_to_be_updated[i]["proposal_id"]}`);
          await voteOnProposal(client, contracts_to_be_updated[i]["proposal_id"], 1, fee_denom);
          await voteOnProposal(validator_1, contracts_to_be_updated[i]["proposal_id"], 1, fee_denom);
          await voteOnProposal(validator_2, contracts_to_be_updated[i]["proposal_id"], 1, fee_denom);
          console.log("Voted successfully")
        } catch (e) {
          console.log("Error has occoured while voting => ", e);
        }
      }
    }
    network.proposals_to_update_permissions = true;

    // Update propsoal IDs stored 
    network.lp_token_instantiate_permissions_proposal_id = contracts_to_be_updated[0]["proposal_id"];
    network.xyk_pool_instantiate_permissions_proposal_id = contracts_to_be_updated[1]["proposal_id"];
    network.stableswap_pool_instantiate_permissions_proposal_id = contracts_to_be_updated[2]["proposal_id"];
    network.stable5pool_instantiate_permissions_proposal_id = contracts_to_be_updated[3]["proposal_id"];
    network.weighted_instantiate_permissions_proposal_id = contracts_to_be_updated[4]["proposal_id"];
    writeArtifact(network, CHAIN_ID);    
  }

  // let res = await query_gov_proposal(client, network.xyk_pool_instantiate_permissions_proposal_id);
  // let res = await query_wasm_code(client, network.xyk_pool_contract_code_id);
  // console.log(res);      

  let add_create_pool_exec_msg = {
    create_pool_instance: {
      pool_type: { xyk: {} },
      asset_infos: [
        { native_token: { denom: fee_denom } },
        { token: { contract_addr: network.test_tokens_addresses[0] } },
      ],
    },
  };
  let ex = await executeContract(client, wallet_address, network.vault_contract_address, add_create_pool_exec_msg );
  console.log(ex)

  return;
    // // Get Vault contract code store proposal status <if its not passed already>
    // if (network.vault_store_code_proposal_status != 3) {
    //   let res = await query_gov_proposal(client, network.vault_store_code_proposal_id);
    //   // console.log(res);      
    //   network.vault_store_code_proposal_status  = Number(res["proposal"]["status"])
    //   writeArtifact(network, CHAIN_ID)
    // }
    // if (network.vault_store_code_proposal_status != 3 ) {
    //   console.log("Vault store code proposal has not passed yet. Terminating deployment script...")
    // }


    // Get Vault contract instantiate proposal status <if its not passed already>
    if (network.vault_instantiate_proposal_status != 3) {
      let res = await query_gov_proposal(client, network.vault_instantiate_proposal_id);
      // console.log(res);      
      network.vault_instantiate_proposal_status  = Number(res["proposal"]["status"])
      writeArtifact(network, CHAIN_ID)
    }
    if (network.vault_instantiate_proposal_status != 3 ) {
      console.log("Vault instantiate proposal has not passed yet. Terminating deployment script...")
    }



    // Once the VAULT Contract is instantiated, we need to do the following - 
    // - Make proposal to store XYK pool code with permissions given to VAULT Contract
    // - Make proposal to store WEIGHTED pool code with permissions given to VAULT Contract
    // - Make proposal to store STABLE-SWAP pool code with permissions given to VAULT Contract
    // - Make proposal to store STABLE-5-SWAP pool code with permissions given to VAULT Contract
    // - Make proposal to store LP Token code with permissions given to VAULT Contract

    if (network.vault_contract_address && network.vault_contract_address != "") {
      // ---------------------------------
      // Make proposal to store XYK pool code with permissions given to VAULT Contract
      if (!network.xyk_pool_store_code_proposal_id || network.xyk_pool_store_code_proposal_id == 0) {  
        // TRANSATION 1. --> Make proposal on-chain
        try {
          console.log(`\nSubmitting Proposal to store Dexter XYK Pool Contract ...`);
          const wasm = fs.readFileSync("../artifacts/xyk_pool.wasm");
          const wasmStoreProposal = {
            typeUrl: "/cosmwasm.wasm.v1.StoreCodeProposal",
            value: Uint8Array.from(
              cosmwasm.wasm.v1.StoreCodeProposal.encode(
                cosmwasm.wasm.v1.StoreCodeProposal.fromPartial({
                  title: "Dexter :: XYK Pool",
                  description: `Add wasm code for Dexter XYK Pool contract.`,
                  runAs: wallet_address,
                  wasmByteCode: Pako.gzip(wasm, { level: 9 }),
                  instantiatePermission: {                  
                    permission: cosmwasm.wasm.v1.accessTypeFromJSON(1)
                  },
                })
              ).finish()
            ),
          };
          const res = await Gov_MsgSubmitProposal(client, wasmStoreProposal, fee_denom, deposit_amount);
          console.log(res)
          // Update artifact json
          network.xyk_pool_store_code_proposal_id = Number(res[0].events[3].attributes[1].value);
          writeArtifact(network, CHAIN_ID)
        } catch (e) {
          console.log("Proposal Error has occoured => ", e);
        }  
        // TRANSACTION 2. --> Vote on proposal
              // 0 == - VOTE_OPTION_UNSPECIFIED defines a no-op vote option
              // 1 == - VOTE_OPTION_YES defines a yes vote option.
              // 2 == - VOTE_OPTION_ABSTAIN defines an abstain vote option.
              // 3 == - VOTE_OPTION_NO defines a no vote option.
              // 4 == - VOTE_OPTION_NO_WITH_VETO defines a no with veto vote option.
        if (network.xyk_pool_store_code_proposal_id > 0 && CHAIN_ID == "testing") {
          try {
            console.log(`Voting on Proposal # ${network.xyk_pool_store_code_proposal_id}`);
            await voteOnProposal(client, network.xyk_pool_store_code_proposal_id, 1, fee_denom);
            await voteOnProposal(validator_1, network.xyk_pool_store_code_proposal_id, 1, fee_denom);
            await voteOnProposal(validator_2, network.xyk_pool_store_code_proposal_id, 1, fee_denom);
            console.log("Voted successfully")
          } catch (e) {
            console.log("Error has occoured while voting => ", e);
          }
        }
      }    
    }

    // Get XYK Pool contract code store proposal status <if its not passed already>
    if (network.xyk_pool_store_code_proposal_status != 3) {
      let res = await query_gov_proposal(client, network.xyk_pool_store_code_proposal_id);
      // console.log(res);      
      network.xyk_pool_store_code_proposal_status  = Number(res["proposal"]["status"])
      writeArtifact(network, CHAIN_ID)
    }
    if (network.vault_instantiate_proposal_status != 3 ) {
      console.log("Vault instantiate proposal has not passed yet. Terminating deployment script...")
    }      

    // GET Code Id for XYK Pool
    // if (network.xyk_pool_store_code_proposal_status==3 && (!network.xyk_pool_code_id || network.xyk_pool_code_id == 0)) {
    //   // Get XYK contract Code ID first
    //   let res = await find_code_id_from_contract_hash(client, XYK_CODE_HASH);
    //   network.xyk_pool_code_id = Number(res);
    //   writeArtifact(network, CHAIN_ID);
    // }

    // XYK POOL --> Update Instantiation Permission
    if (network.xyk_pool_code_id > 0 && (!network.xyk_pool_update_config_proposal_id || network.xyk_pool_update_config_proposal_id==0)) {
      // TRANSATION 1. --> Make proposal on-chain
     try {
       console.log(`\nSubmitting Proposal to update Dexter XYK Pool Contract instantiation permission ...`);
       const wasmUpdateContractInstantiationPermissionProposal = {
         typeUrl: "/cosmwasm.wasm.v1.UpdateInstantiateConfigProposal",
         value: Uint8Array.from(
           cosmwasm.wasm.v1.UpdateInstantiateConfigProposal.encode(
             cosmwasm.wasm.v1.UpdateInstantiateConfigProposal.fromPartial({
               title: "Dexter :: XYK Pool update instantiation config",
               description: `Update Dexter XYK Pool contract instantiation permission to vault addresss`,
               accessConfigUpdates: [
                 {
                   codeId: network.xyk_pool_code_id,
                   instantiatePermission: {
                     address: network.vault_contract_address,
                     permission: cosmwasm.wasm.v1.accessTypeFromJSON(2),
                   }
                 }
               ]
             })
           ).finish()
         ),
       };
       const res = await Gov_MsgSubmitProposal(client, wasmUpdateContractInstantiationPermissionProposal, fee_denom, deposit_amount);
       // Update artifact json
       network.xyk_pool_update_config_proposal_id = Number(res[0].events[3].attributes[1].value);
       writeArtifact(network, CHAIN_ID)
     } catch (e) {
       console.log("Proposal Error has occoured => ", e);
     }  
     // TRANSACTION 2. --> Vote on proposal
     if (network.xyk_pool_update_config_proposal_id > 0 && CHAIN_ID == "testing") {
       try {
         console.log(`Voting on Proposal # ${network.xyk_pool_update_config_proposal_id}`);
         await voteOnProposal(client, network.xyk_pool_update_config_proposal_id, 1, fee_denom);
         await voteOnProposal(validator_1, network.xyk_pool_update_config_proposal_id, 1, fee_denom);
         await voteOnProposal(validator_2, network.xyk_pool_update_config_proposal_id, 1, fee_denom);
         console.log("Voted successfully")
       } catch (e) {
         console.log("Error has occoured while voting => ", e);
       }
     }
    }


   // Get XYK Pool contract code update instantiate permissions proposal status <if its not passed already>
   if (network.xyk_pool_update_config_proposal_status != 3) {
     let res = await query_gov_proposal(client, network.xyk_pool_update_config_proposal_id);
     network.xyk_pool_update_config_proposal_status  = Number(res["proposal"]["status"])
     writeArtifact(network, CHAIN_ID)
   }
   if (network.xyk_pool_update_config_proposal_status != 3) { 
    console.log("XYK Pool contract code update instantiate permissions proposal has not passed yet. Terminating... ");
    return;
   }


   // Get instantiation permissions for the XYK pool's codeId
    // if (network.xyk_pool_update_config_proposal_status == 3) {
    //   let latestCode = await query_wasm_code(client, network.xyk_pool_code_id);
    //   console.log(latestCode)

    //   let latestCodeId = Number(latestCode.codeInfo.codeId);
    //   let latestCodePermission = latestCode.codeInfo.instantiatePermission;

    //   console.log('code id', latestCodeId);
    //   console.log('code permission', latestCodePermission);

    //   network.xyk_pool_code_id = latestCodeId;
    //   writeArtifact(network, CHAIN_ID)
    // }

    // Try instantiating the XYK Pool
    // 1. Add XYK pool to Vault contract 
    // 2. 
    // let add_xyk_pool_msg = {"add_to_registery": { "new_pool_config" : {
    //                             code_id: network.xyk_pool_code_id,
    //                             pool_type: { xyk: {} },
    //                             fee_info: {
    //                               total_fee_bps: 300,
    //                               protocol_fee_percent: 49,
    //                               dev_fee_percent: 15,
    //                               developer_addr: wallet_address,
    //                             },
    //                             is_disabled: false,
    //                             is_generator_disabled: false,
    //                           },        
    //                         }};
    // let res = await executeContract(client, wallet_address, network.vault_contract_address, add_xyk_pool_msg);
    // console.log(res);

    // let r4s = await client.wasm.queryContractSmart(network.vault_contract_address, {"query_rigistery":{ "pool_type": {"xyk":{}} }});
    // console.log(r4s);

    // let add_xyk_pool_msg = {"add_to_registery": { "new_pool_config" : {
    //                             code_id: network.xyk_pool_code_id,
    //                             pool_type: { xyk: {} },
    //                             fee_info: {
    //                               total_fee_bps: 300,
    //                               protocol_fee_percent: 49,
    //                               dev_fee_percent: 15,
    //                               developer_addr: wallet_address,
    //                             },
    //                             is_disabled: false,
    //                             is_generator_disabled: false,
    //                           },        
    //                         }};
    // let res = await executeContract(client, wallet_address, network.vault_contract_address, add_xyk_pool_msg);
    // console.log(res);


      // TRANSATION 1. --> Make proposal on-chain
      try {
        console.log(`\nSubmitting Proposal to store LP Token Contract ...`);
        const wasm = fs.readFileSync("../artifacts/lp_token.wasm");
        const wasmStoreProposal = {
          typeUrl: "/cosmwasm.wasm.v1.StoreCodeProposal",
          value: Uint8Array.from(
            cosmwasm.wasm.v1.StoreCodeProposal.encode(
              cosmwasm.wasm.v1.StoreCodeProposal.fromPartial({
                title: "LP Token",
                description: `Add wasm code for LP Token contract.`,
                runAs: wallet_address,
                wasmByteCode: Pako.gzip(wasm, { level: 9 }),
                instantiatePermission: {                  
                  permission: cosmwasm.wasm.v1.accessTypeFromJSON(1),
                },
              })
            ).finish()
          ),
        };
        const res = await Gov_MsgSubmitProposal(client, wasmStoreProposal, fee_denom, deposit_amount);
        // Update artifact json
        network.lp_token_store_code_proposal_id = Number(res[0].events[3].attributes[1].value);
        writeArtifact(network, CHAIN_ID)
      } catch (e) {
        console.log("Proposal Error has occoured => ", e);
      }  
      // TRANSACTION 2. --> Vote on proposal
      if (network.lp_token_store_code_proposal_id > 0 && CHAIN_ID == "testing") {
        try {
          console.log(`Voting on Proposal # ${network.lp_token_store_code_proposal_id}`);
          await voteOnProposal(client, network.lp_token_store_code_proposal_id, 1, fee_denom);
          await voteOnProposal(validator_1, network.lp_token_store_code_proposal_id, 1, fee_denom);
          await voteOnProposal(validator_2, network.lp_token_store_code_proposal_id, 1, fee_denom);
          console.log("Voted successfully")
        } catch (e) {
          console.log("Error has occoured while voting => ", e);
        }
      }
    

    // Get Vault contract code store proposal status <if its not passed already>
    if (network.lp_token_store_code_proposal_id != 3) {
      let res = await query_gov_proposal(client, network.lp_token_store_code_proposal_id);
      // console.log(res);      
      network.vault_store_code_proposal_status  = Number(res["proposal"]["status"])
      writeArtifact(network, CHAIN_ID)
    }
    if (network.vault_store_code_proposal_status != 3 ) {
      console.log("Vault store code proposal has not passed yet. Terminating deployment script...")
    }



    return;



  // // CONTRACTS WHICH ARE TO BE DEPLOYED ON PERSISTENCE ONE NETWORK FOR DEXTER PROTOCOL
  // let contracts = [
  // // { name: "Dexter Vault", path: "../artifacts/dexter_vault.wasm" }, 
  // // { name: "Dexter Keeper", path: "../artifacts/dexter_keeper.wasm" }, 
  // // { name: "LP Token", path: "../artifacts/lp_token.wasm" },
  // // { name: "XYK Pool", path: "../artifacts/xyk_pool_1.wasm" },
  // // { name: "Weighted Pool", path: "../artifacts/weighted_pool.wasm" },
  // // { name: "Stableswap Pool", path: "../artifacts/stableswap_pool.wasm" },
  // // { name: "Stable5Swap Pool", path: "../artifacts/stable5pool.wasm" },
  // // { name: "Dexter Vesting", path: "../artifacts/dexter_vesting.wasm" },
  // // { name: "Dexter Generator", path: "../artifacts/dexter_generator.wasm" },
  // // {
  // //   name: "Dexter Generator : Proxy",
  // //   path: "../artifacts/dexter_generator_proxy.wasm",
  // // },
  // // { name: "Staking contract", path: "../artifacts/anchor_staking.wasm" },
  // ];

  // // LOOP -::- CREATE PROTOCOLS FOR EACH CONTRACT ON-CHAIN
  // if (!PROPOSALS_CREATED) {
  //   for (let i = 0; i < contracts.length; i++) {
  //     let contract_name = contracts[i]["name"];
  //     let contract_path = contracts[i]["path"];
  
  //     // TRANSATION 1. --> Make proposal on-chain
  //     let proposalId = 0;
  //     try {
  //       console.log(
  //         `\nSubmitting Proposal to deploy ${contract_name} Contract ...`
  //       );
  //       const wasm = fs.readFileSync(contract_path);
  //       const wasmStoreProposal = {
  //         typeUrl: "/cosmwasm.wasm.v1.StoreCodeProposal",
  //         value: Uint8Array.from(
  //           cosmwasm.wasm.v1.StoreCodeProposal.encode(
  //             cosmwasm.wasm.v1.StoreCodeProposal.fromPartial({
  //               title: contract_name,
  //               description: `Add wasm code for ${contract_name} contract.`,
  //               runAs: wallet_address,
  //               wasmByteCode: Pako.gzip(wasm, { level: 9 }),
  //               instantiatePermission: {                  
  //                 permission: cosmwasm.wasm.v1.accessTypeFromJSON(2),
  //                 address: ""
  //               },
  //             })
  //           ).finish()
  //         ),
  //       };
  //       const res = await Gov_MsgSubmitProposal(client, wasmStoreProposal, fee_denom, deposit_amount);
  //       proposalId = Number(res[0].events[3].attributes[1].value);
  //     } catch (e) {
  //       console.log("Proposal Error has occoured => ", e);
  //     }
  
  //     // TRANSACTION 2. --> Vote on proposal
  //           // 0 == - VOTE_OPTION_UNSPECIFIED defines a no-op vote option
  //           // 1 == - VOTE_OPTION_YES defines a yes vote option.
  //           // 2 == - VOTE_OPTION_ABSTAIN defines an abstain vote option.
  //           // 3 == - VOTE_OPTION_NO defines a no vote option.
  //           // 4 == - VOTE_OPTION_NO_WITH_VETO defines a no with veto vote option.
  //     if (proposalId > 0) {
  //       try {
  //         console.log(`Voting on Proposal # ${proposalId}`);
  //         await voteOnProposal(client, proposalId, 1, fee_denom);
  //         await voteOnProposal(validator_1, proposalId, 1, fee_denom);
  //         await voteOnProposal(validator_2, proposalId, 1, fee_denom);
  //         console.log("Voted successfully")
  //       } catch (e) {
  //         console.log("Error has occoured while voting => ", e);
  //       }
  //     }
  //   }  
  //   return;
  // }

  // {
  //   code_id: xyk_pool_1_code_id,
  //   pool_type: { xyk: {} },
  //   fee_info: {
  //     total_fee_bps: 300,
  //     protocol_fee_percent: 49,
  //     dev_fee_percent: 15,
  //     developer_addr: wallet_address,
  //   },
  //   is_disabled: false,
  //   is_generator_disabled: false,
  // },

          // {
        //   code_id: stableswap_pool_code_id,
        //   pool_type: { stable2_pool: {} },
        //   fee_info: {
        //     total_fee_bps: 300,
        //     protocol_fee_percent: 49,
        //     dev_fee_percent: 15,
        //     developer_addr: null,
        //   },
        //   is_disabled: false,
        //   is_generator_disabled: false,
        // },
        // {
        //   code_id: stable5swap_pool_code_id,
        //   pool_type: { stable5_pool: {} },
        //   fee_info: {
        //     total_fee_bps: 300,
        //     protocol_fee_percent: 49,
        //     dev_fee_percent: 15,
        //     developer_addr: null,
        //   },
        //   is_disabled: false,
        //   is_generator_disabled: false,
        // },
        // {
        //   code_id: weighted_pool_code_id,
        //   pool_type: { weighted: {} },
        //   fee_info: {
        //     total_fee_bps: 300,
        //     protocol_fee_percent: 49,
        //     dev_fee_percent: 15,
        //     developer_addr: null,
        //   },
        //   is_disabled: false,
        //   is_generator_disabled: false,
        // },



  // // -----------x-------------x-------------x---------x---------------
  // // ----------- CONTRACT INSTIANTIATION :: DEXTER VAULT -------------
  // // -----------x-------------x-------------x---------x---------------

  // // CONTRACT IDs
  // let vault_code_id = 1;
  // let keeper_code_id = 2;
  // let lp_token_code_id = 3;
  // let xyk_pool_code_id = 4;
  // let xyk_pool_1_code_id = 12;
  // let weighted_pool_code_id = 5;
  // let stableswap_pool_code_id = 6;
  // let stable5swap_pool_code_id = 7;

  // let VAULT_INSTIANTIATED = true;
  // let VAULT_ADDRESS = "persistence1ltd0maxmte3xf4zshta9j5djrq9cl692ctsp9u5q0p9wss0f5lmsa3cn0m"; // "persistence14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9sjvz4fk";

  // if (!VAULT_INSTIANTIATED) {
  //   let vault_proposal_id = 0;
  //   // INSTIANTIATING CONTRACTS -::- DEXTER VAULT
  //   let init_vault_msg = {
  //     pool_configs: [
  //       {
  //         code_id: xyk_pool_1_code_id,
  //         pool_type: { xyk: {} },
  //         fee_info: {
  //           total_fee_bps: 300,
  //           protocol_fee_percent: 49,
  //           dev_fee_percent: 15,
  //           developer_addr: wallet_address,
  //         },
  //         is_disabled: false,
  //         is_generator_disabled: false,
  //       },
  //       // {
  //       //   code_id: stableswap_pool_code_id,
  //       //   pool_type: { stable2_pool: {} },
  //       //   fee_info: {
  //       //     total_fee_bps: 300,
  //       //     protocol_fee_percent: 49,
  //       //     dev_fee_percent: 15,
  //       //     developer_addr: null,
  //       //   },
  //       //   is_disabled: false,
  //       //   is_generator_disabled: false,
  //       // },
  //       // {
  //       //   code_id: stable5swap_pool_code_id,
  //       //   pool_type: { stable5_pool: {} },
  //       //   fee_info: {
  //       //     total_fee_bps: 300,
  //       //     protocol_fee_percent: 49,
  //       //     dev_fee_percent: 15,
  //       //     developer_addr: null,
  //       //   },
  //       //   is_disabled: false,
  //       //   is_generator_disabled: false,
  //       // },
  //       // {
  //       //   code_id: weighted_pool_code_id,
  //       //   pool_type: { weighted: {} },
  //       //   fee_info: {
  //       //     total_fee_bps: 300,
  //       //     protocol_fee_percent: 49,
  //       //     dev_fee_percent: 15,
  //       //     developer_addr: null,
  //       //   },
  //       //   is_disabled: false,
  //       //   is_generator_disabled: false,
  //       // },
  //     ],
  //     lp_token_code_id: lp_token_code_id,
  //     fee_collector: null,
  //     owner: wallet_address,
  //     generator_address: null,
  //   };
  //   try {
  //     const wasmInstantiateProposal = {
  //       typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
  //       value: Uint8Array.from(
  //         cosmwasm.wasm.v1.InstantiateContractProposal.encode(
  //           cosmwasm.wasm.v1.InstantiateContractProposal.fromJSON({
  //             title: "Dexter Vault",
  //             description:
  //               "Dexter Vault contract, used facilitating token swaps and instantiating pools",
  //             runAs: wallet_address,
  //             admin: wallet_address,
  //             codeId: vault_code_id.toString(),
  //             label: "Dexter Vault",
  //             msg: Buffer.from(JSON.stringify(init_vault_msg)).toString("base64"), // Buffer.from(JSON.stringify(init_msg)),
  //           })
  //         ).finish()
  //       ),
  //     };
  //     const res = await Gov_MsgSubmitProposal(client, wasmInstantiateProposal, fee_denom, deposit_amount);
  //     vault_proposal_id = res[0].events[3].attributes[1].value;
  //     // const json = JSON.parse(res.rawLog?);
  //     console.log(res);
  //   } catch (e) {
  //     console.log("Proposal Error has occoured => ", e);
  //   }
  //   // Vote on Proposal
  //   if (vault_proposal_id > 0) {
  //     try {
  //       console.log(`Voting on Proposal # ${vault_proposal_id}`);
  //       await voteOnProposal(client, vault_proposal_id, 1, fee_denom);
  //       await voteOnProposal(validator_1, vault_proposal_id, 1, fee_denom);
  //       await voteOnProposal(validator_2, vault_proposal_id, 1, fee_denom);      
  //       console.log("Voted successfully")
  //     } catch (e) {
  //       console.log("Error has occoured while voting => ", e);
  //     }
  //   }
  //   return;
  // }



  // // -----------x-------------x-------------x---------x---------------
  // // ----------- CONTRACT INSTIANTIATION :: TEST TOKENS --------------
  // // -----------x-------------x-------------x---------x---------------

  // let test_token_1 = "persistence17p9rzwnnfxcjp32un9ug7yhhzgtkhvl9jfksztgw5uh69wac2pgsxzejz5";
  // let test_token_2 = "persistence1unyuj8qnmygvzuex3dwmg9yzt9alhvyeat0uu0jedg2wj33efl5qascd8u";
  // let test_token_3 = "persistence1qwlgtx52gsdu7dtp0cekka5zehdl0uj3fhp9acg325fvgs8jdzkspueg4j";

  // // let token_init_msg = {
  // //   name: "Test3",
  // //   symbol: "atDEX",
  // //   decimals: 6,
  // //   initial_balances: [{ address: wallet_address, amount: "10000000000000" }],
  // //   mint: { minter: wallet_address, amount: "1000000000000000" },
  // // };
  // // try {
  // //   const wasmInstantiateProposal = {
  // //     typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
  // //     value: Uint8Array.from(
  // //       cosmwasm.wasm.v1.InstantiateContractProposal.encode(
  // //         cosmwasm.wasm.v1.InstantiateContractProposal.fromJSON({
  // //           title: "Test token",
  // //           description: "Test token for testing dexter DEX",
  // //           runAs: wallet_address,
  // //           admin: wallet_address,
  // //           codeId: lp_token_code_id.toString(),
  // //           label: "Instiantiate dummy token 1",
  // //           msg: Buffer.from(JSON.stringify(token_init_msg)).toString("base64"), // Buffer.from(JSON.stringify(init_msg)),
  // //         })
  // //       ).finish()
  // //     ),
  // //   };
  // //   const res = await Gov_MsgSubmitProposal(client, wasmInstantiateProposal, fee_denom, deposit_amount);
  // //   console.log(res);
  // //   let proposalId = res[0].events[3].attributes[1].value;
  // //   await voteOnProposal(client, proposalId, 1, fee_denom);
  // //   await voteOnProposal(validator_1, proposalId, 1, fee_denom);
  // //   await voteOnProposal(validator_2, proposalId, 1, fee_denom);       
  // //   // const json = JSON.parse(res.rawLog?);
  // //   console.log(res);
  // // } catch (e) {
  // //   console.log("Proposal Error has occoured => ", e);
  // // }


  // // -----------x-------------x-------------x---------x---------------
  // // ----------- CONTRACT INSTIANTIATION :: DEXTER KEEPER -------------
  // // -----------x-------------x-------------x---------x---------------


  // let KEEPER_INSTIANTIATED = true;
  // let KEEPER_ADDRESS = "persistence1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrq7e8hts";

  // if (!KEEPER_INSTIANTIATED) {
  //   let keeper_proposal_id = 0;
  //   // INSTIANTIATING CONTRACTS -::- DEXTER KEEPER
  //   let keeper_init_msg = {
  //     vault_contract: VAULT_ADDRESS,
  //   };
  //   try {
  //     const wasmInstantiateProposal = {
  //       typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
  //       value: Uint8Array.from(
  //         cosmwasm.wasm.v1.InstantiateContractProposal.encode(
  //           cosmwasm.wasm.v1.InstantiateContractProposal.fromPartial({
  //             title: "Dexter Keeper",
  //             description:
  //               "Dexter Keeper contract",
  //             runAs: wallet_address,
  //             admin: wallet_address,            
  //             codeId: keeper_code_id.toString(),
  //             label: "Dexter Keeper",
  //             msg: Buffer.from(JSON.stringify(keeper_init_msg)),
  //           })
  //         ).finish()
  //       ),
  //     };
  //     const res = await Gov_MsgSubmitProposal(client, wasmInstantiateProposal, fee_denom, deposit_amount);
  //     keeper_proposal_id = res[0].events[3].attributes[1].value;
  //       // const json = JSON.parse(res.rawLog?);
  //   } catch (e) {
  //       console.log("Proposal Error has occoured => ", e);
  //   }
  //   // Vote on Proposal
  //   if (keeper_proposal_id > 0) {
  //     try {
  //       console.log(`Voting on Proposal # ${keeper_proposal_id}`);
  //       await voteOnProposal(client, keeper_proposal_id, 1, fee_denom);
  //       await voteOnProposal(validator_1, keeper_proposal_id, 1, fee_denom);
  //       await voteOnProposal(validator_2, keeper_proposal_id, 1, fee_denom);      
  //       console.log("Voted successfully")
  //     } catch (e) {
  //       console.log("Error has occoured while voting => ", e);
  //     }
  //   }    
  // }

  // // -----------x-------------x-------------x---------------
  // // ----------- DEXTER VAULT :: INITIALIZE NEW POOL -------
  // // -----------x-------------x-------------x---------------

  // const res = await client.wasm.queryContractSmart(VAULT_ADDRESS, {"query_rigistery":{ "pool_type": {"xyk": {}} }});
  // console.log(res);

  // let keeper_proposal_id = 0
  // let keeper_init_msg = {
  //   vault_contract: VAULT_ADDRESS,
  // };
  // // try {
  // //   const wasmInstantiateProposal = {
  // //     typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
  // //     value: Uint8Array.from(
  // //       cosmwasm.wasm.v1.InstantiateContractProposal.encode(
  // //         cosmwasm.wasm.v1.InstantiateContractProposal.fromPartial({
  // //           title: "Dexter Keeper",
  // //           description:
  // //             "Dexter Keeper contract",
  // //           runAs: VAULT_ADDRESS,
  // //           admin: wallet_address,            
  // //           codeId: xyk_pool_1_code_id.toString(),
  // //           label: "Dexter Keeper",
  // //           msg: Buffer.from(JSON.stringify(keeper_init_msg)),
  // //         })
  // //       ).finish()
  // //     ),
  // //   };
  // //   const res = await Gov_MsgSubmitProposal(client, wasmInstantiateProposal, fee_denom, deposit_amount);
  // //   keeper_proposal_id = res[0].events[3].attributes[1].value;
  // //     // const json = JSON.parse(res.rawLog?);
  // // } catch (e) {
  // //     console.log("Proposal Error has occoured => ", e);
  // // }
  // // // Vote on Proposal
  // // if (keeper_proposal_id > 0) {
  // //   try {
  // //     console.log(`Voting on Proposal # ${keeper_proposal_id}`);
  // //     await voteOnProposal(client, keeper_proposal_id, 1, fee_denom);
  // //     await voteOnProposal(validator_1, keeper_proposal_id, 1, fee_denom);
  // //     await voteOnProposal(validator_2, keeper_proposal_id, 1, fee_denom);      
  // //     console.log("Voted successfully")
  // //   } catch (e) {
  // //     console.log("Error has occoured while voting => ", e);
  // //   }
  // // }   

  // // return

  // // CREATE NEW DEXTER POOLS VIA GOVERNANCE
  // let add_create_pool_exec_msg = {
  //   create_pool_instance: {
  //     pool_type: { xyk: {} },
  //     asset_infos: [
  //       { native_token: { denom: fee_denom } },
  //       { token: { contract_addr: test_token_1 } },
  //     ],
  //   },
  // };
  // // Submit `ExecuteContractProposal` Proposal
  // try {
  //   const wasmExecuteProposal = {
  //     typeUrl: "/cosmwasm.wasm.v1.ExecuteContractProposal",
  //     value: Uint8Array.from(
  //       cosmwasm.wasm.v1.ExecuteContractProposal.encode(
  //         cosmwasm.wasm.v1.ExecuteContractProposal.fromJSON({
  //           title: "Dexter - XYK Pool Instantiation",
  //           description: "Create dexter XYK Pool",
  //           runAs: VAULT_ADDRESS,
  //           contract: VAULT_ADDRESS,
  //           msg: Buffer.from(JSON.stringify(add_create_pool_exec_msg)).toString(
  //             "base64"
  //           ), // Buffer.from(JSON.stringify(init_msg)),
  //         })
  //       ).finish()
  //     ),
  //   };
  //   const res = await Gov_MsgSubmitProposal(client, wasmExecuteProposal, fee_denom, deposit_amount);
  //   console.log(res);
  //   let proposalId = res[0].events[3].attributes[1].value;
  //   console.log(proposalId);
  // } catch (e) {
  //   console.log("Proposal Error has occoured => ", e);
  // }

}

Demo().catch(console.log);



