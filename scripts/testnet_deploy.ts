import { PersistenceClient, cosmwasm } from "cosmoschainsjs";
import * as Pako from "pako";
import * as fs from "fs";
import { Gov_MsgSubmitProposal, voteOnProposal, readArtifact, writeArtifact, 
  query_gov_proposal, find_code_id_from_contract_hash, query_wasm_contractsByCode} from "./helpers/helpers.js";
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
        console.log(`\nSubmitting Proposal to deploy Dexter VAULT Contract ...`);
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
          console.log(`\nSubmitting Proposal to deploy Dexter XYK Pool Contract ...`);
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
                    permission: cosmwasm.wasm.v1.accessTypeFromJSON(2),
                    address: network.vault_contract_address
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
        let res = await query_gov_proposal(client, network.xyk_pool_store_code_proposal_id);
        // console.log(res);      
        network.xyk_pool_store_code_proposal_status  = Number(res["proposal"]["status"])
        writeArtifact(network, CHAIN_ID)
      }
  
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



