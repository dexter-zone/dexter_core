import { PersistenceClient, cosmwasm } from "cosmoschainsjs";
import * as Pako from "pako";
import * as fs from "fs";
import { Gov_MsgSubmitProposal, voteOnProposal, readArtifact, writeArtifact, 
  query_gov_proposal, find_code_id_from_contract_hash, query_wasm_contractsByCode, query_wasm_codes} from "./helpers/helpers.js";
import { toBinary } from "@cosmjs/cosmwasm-stargate";
import { Slip10RawIndex, pathToString, stringToPath } from '@cosmjs/crypto';
import { DirectSecp256k1HdWallet, decodePubkey } from "@cosmjs/proto-signing";
import { fromBase64, toBase64, fromHex , toHex} from "@cosmjs/encoding";

// ----------- PERSISTENCE END-POINTS -------------
// testnet: https://rpc.testnet.persistence.one:443     :: test-core-1
// mainnet: https://rpc.persistence.one:443             :: core-1

// This is your rpc endpoint
const rpcEndpoint = "http://localhost:26657";

const VAULT_CODE_HASH = "14e717077b44530459c66607788069428b7bd15dd248bf1ff72b0c4d17a203d5";


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

    // -----------x-------------x-------------x---------------------
    // ----------- UPLOAD VAULT CONTRACT CODE TO THE NETWORK -------
    // -----------x-------------x-------------x---------------------

    // UPLOAD CODE --> If proposal to store VAULT's code on-chain has not been executed yet
    if (!network.vault_store_code_proposal_id || network.vault_store_code_proposal_id == 0) {  
      // TRANSATION 1. --> Make proposal on-chain
      try {
        console.log(`\nSubmitting Proposal to store Dexter VAULT Contract ...`);
        const wasm = fs.readFileSync("../artifacts/dexter_vault.wasm");
        const wasmStoreProposal = {
          typeUrl: "/cosmwasm.wasm.v1.StoreCodeProposal",
          value: Uint8Array.from(
            cosmwasm.wasm.v1.StoreCodeProposal.encode(
              cosmwasm.wasm.v1.StoreCodeProposal.fromPartial({
                title: "Dexter Vault",
                description: `Add wasm code for Dexter Vault contract.`,
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
        network.vault_store_code_proposal_id = Number(res[0].events[3].attributes[1].value);
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
      if (network.vault_store_code_proposal_id > 0 && CHAIN_ID == "testing") {
        try {
          console.log(`Voting on Proposal # ${network.vault_store_code_proposal_id}`);
          await voteOnProposal(client, network.vault_store_code_proposal_id, 1, fee_denom);
          await voteOnProposal(validator_1, network.vault_store_code_proposal_id, 1, fee_denom);
          await voteOnProposal(validator_2, network.vault_store_code_proposal_id, 1, fee_denom);
          console.log("Voted successfully")
        } catch (e) {
          console.log("Error has occoured while voting => ", e);
        }
      }
    }  

    // Get Vault contract code store proposal status <if its not passed already>
    if (network.vault_store_code_proposal_status != 3) {

      console.log('waiting for proposal voting period to complete');
      await new Promise(resolve => setTimeout(resolve, 31000));
      console.log('voting period over');

      let res = await query_gov_proposal(client, network.vault_store_code_proposal_id);
      console.log(res);      
      network.vault_store_code_proposal_status  = Number(res["proposal"]["status"])
      writeArtifact(network, CHAIN_ID)
    }


    // INSTANTIATE DEXTER VAULT CONTRACT --> If vault contract has not been instantiated yet 
    if (network.vault_store_code_proposal_status == 3 && (!network.vault_instantiate_proposal_id || network.vault_instantiate_proposal_id == 0 )) {  

      // Get VAULT contract Code ID first
      let res = await find_code_id_from_contract_hash(client, VAULT_CODE_HASH);
      // console.log(res)
      network.vault_contract_code_id = Number(res);

      // Make proposal to instantiate Vault contract
      if (network.vault_contract_code_id > 0) {
        let init_vault_msg = {
          pool_configs: [],
          lp_token_code_id: 0,
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
          console.log('Submitting proposal to instantiate vault contract');
          const res = await Gov_MsgSubmitProposal(client, wasmInstantiateProposal, fee_denom, deposit_amount);
          console.log(res);
          network.vault_instantiate_proposal_id = res[0].events[3].attributes[1].value;
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
      console.log('waiting for proposal voting period to complete');
      await new Promise(resolve => setTimeout(resolve, 31000));
      console.log('voting period over');

      let res = await query_wasm_contractsByCode(client, network.vault_contract_code_id );
      // console.log(res);
      network.vault_contract_address = res["contracts"][  res["contracts"].length - 1 ];
      writeArtifact(network, CHAIN_ID);
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
                    permission: cosmwasm.wasm.v1.accessTypeFromJSON(1),
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
  
      // Get XYK Pool contract code store proposal status <if its not passed already>
      if (network.xyk_pool_store_code_proposal_status != 3) {
        console.log('waiting for proposal voting period to complete');
        await new Promise(resolve => setTimeout(resolve, 31000));
        console.log('voting period over');

        let res = await query_gov_proposal(client, network.xyk_pool_store_code_proposal_id);
        // console.log(res);      
        network.xyk_pool_store_code_proposal_status  = Number(res["proposal"]["status"])
        writeArtifact(network, CHAIN_ID)
      }

      
      if (network.xyk_pool_store_code_proposal_status == 3) {
        let codes = await query_wasm_codes(client)
        // console.log('codes', codes);

        let codeInfos = codes.codeInfos;
        let latestCode = codeInfos[codeInfos.length-1];

        let latestCodeId = Number(latestCode.codeId);
        let latestCodePermission = latestCode.instantiatePermission;

        console.log('code id', latestCodeId);
        console.log('code permission', latestCodePermission);

        network.xyk_pool_code_id = latestCodeId;
        writeArtifact(network, CHAIN_ID)
      }

      if (network.xyk_pool_code_id > 0) {
           // TRANSATION 1. --> Make proposal on-chain
          try {
            console.log(`\nSubmitting Proposal to update Dexter XYK Pool Contract instantiation permission ...`);
            console.log('network', network);
            const wasmUpdateContractInstantiationPermissionProposal = {
              typeUrl: "/cosmwasm.wasm.v1.UpdateInstantiateConfigProposal",
              value: Uint8Array.from(
                cosmwasm.wasm.v1.UpdateInstantiateConfigProposal.encode(
                  cosmwasm.wasm.v1.UpdateInstantiateConfigProposal.fromPartial({
                    title: "Dexter :: XYK Pool update instantiation config",
                    description: `Update Dexter XYK Pool contract instantiation permission to vault addresss`,
                    // runAs: wallet_address,
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
            console.log('submit proposal response', res);
            // Update artifact json
            network.xyk_pool_update_config_proposal_id = Number(res[0].events[3].attributes[1].value);
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
        
    
        // Get XYK Pool contract code store proposal status <if its not passed already>
        if (network.xyk_pool_update_config_proposal_status != 3) {
          console.log('waiting for proposal voting period to complete');
          await new Promise(resolve => setTimeout(resolve, 31000));
          console.log('voting period over');
  
          let res = await query_gov_proposal(client, network.xyk_pool_update_config_proposal_id);
          // console.log(res);      
          network.xyk_pool_update_config_proposal_status  = Number(res["proposal"]["status"])
          writeArtifact(network, CHAIN_ID)
        }
  
        if (network.xyk_pool_update_config_proposal_status == 3) {
          let codes = await query_wasm_codes(client)
          // console.log('codes', codes);
  
          let codeInfos = codes.codeInfos;
          let latestCode = codeInfos[codeInfos.length-1];
  
          let latestCodeId = Number(latestCode.codeId);
          let latestCodePermission = latestCode.instantiatePermission;
  
          console.log('code id', latestCodeId);
          console.log('code permission', latestCodePermission);
        }
      }
    }


    return;

}

Demo().catch(console.log);



