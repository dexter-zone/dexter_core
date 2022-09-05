import { PersistenceClient, cosmwasm } from "persistenceonejs";
import * as Pako from "pako";
import * as fs from "fs";
import {
  Gov_MsgSubmitProposal,
  getContractsByCodeId,
  getCodeIdsListWithPrintedHexHashes,
  readArtifact,
  executeContract,
  writeArtifact,
} from "./helpers/helpers.js";
import { QueryContractsByCodeRequest } from "persistenceonejs/src/proto/cosmwasm/wasm/v1/query.js";

import sha256 from "crypto-js/sha256.js";
import { toBinary } from "@cosmjs/cosmwasm-stargate";
import { coins, Coin } from "@cosmjs/stargate";

// ------------------------- PERSISTENCE END-POINTS --------------------
// testnet: https://rpc.testnet.persistence.one:443     :: test-core-1
// mainnet: https://rpc.persistence.one:443             :: core-1
// ---------------------------------------------------------------------

// Persistence testnet rpc endpoint
const rpcEndpoint = "https://rpc.testnet.persistence.one:443";

// Testnet Wallet Mnemonic
const mnemonic =
  "toss hammer lazy dish they ritual suggest favorite sword alcohol enact enforce mechanic spoon gather knock giggle indicate indicate nose actor brand basket confirm";

// network : stores contract addresses
let network = readArtifact("test_core");
// console.log(network);

async function DexterDeploymentOnTestnetPipeline() {
  // Initialize a persistence Client for interacting with the chain
  const client = await PersistenceClient.init(mnemonic, {
    rpc: rpcEndpoint,
    chainId: "test-core-1",
    gasPrices: {
      denom: "uxprt",
      amount: "2000000",
    },
    gasAdjustment: "1.5",
  });

  // Get Wallet Address
  const [Account] = await client.wallet.getAccounts();
  const wallet_address = Account.address;
  console.log(` WALLET ADDRESS =  ${wallet_address}`);

  // CONTRACT IDs
  let vault_code_id = 6;
  let lp_token_code_id = 8;
  let xyk_pool_code_id = 9;
  let stableswap_pool_code_id = 11;
  // let stable5swap_pool_code_id = 16;
  let weighted_pool_code_id = 10;
  let keeper_code_id = 7;

  // let codes = await getCodeIdsListWithPrintedHexHashes();

  // let codes = await getContractsByCodeId(client, vault_code_id);
  // console.log(`CODES = ${JSON.stringify(codes)}`);

  // -----------x--------------x-------------x-----
  // ----------- CONTRACTS DEPLOYMENT -------------
  // -----------x--------------x-------------x-----

  // CONTRACTS WHICH ARE TO BE DEPLOYED ON PERSISTENCE ONE NETWORK FOR DEXTER PROTOCOL
  let contracts = [
    { name: "Dexter Vault", path: "../artifacts/dexter_vault.wasm" },
    { name: "Dexter Keeper", path: "../artifacts/dexter_keeper.wasm" },
    { name: "LP Token", path: "../artifacts/lp_token.wasm" },
    { name: "XYK Pool", path: "../artifacts/xyk_pool.wasm" },
    { name: "Weighted Pool", path: "../artifacts/weighted_pool.wasm" },
    { name: "Stableswap Pool", path: "../artifacts/stableswap_pool.wasm" },
    { name: "Stable5Swap Pool", path: "../artifacts/stable5pool.wasm" },
    { name: "Dexter Vesting", path: "../artifacts/dexter_vesting.wasm" },
    { name: "Dexter Generator", path: "../artifacts/dexter_generator.wasm" },
    {
      name: "Dexter Generator : Proxy",
      path: "../artifacts/dexter_generator_proxy.wasm",
    },
    { name: "Staking contract", path: "../artifacts/anchor_staking.wasm" },
  ];

  // LOOP -::- CREATE PROTOCOLS FOR EACH CONTRACT ON-CHAIN
  // for (let i = 0; i < contracts.length; i++) {
  //   let contract_name = contracts[i]["name"];
  //   let contract_path = contracts[i]["path"];

  //   try {
  //     console.log(
  //       `\nSubmitting Proposal to deploy ${contract_name} Contract ...`
  //     );
  //     const wasm = fs.readFileSync(contract_path);
  //     const wasmStoreProposal = {
  //       typeUrl: "/cosmwasm.wasm.v1.StoreCodeProposal",
  //       value: Uint8Array.from(
  //         cosmwasm.wasm.v1.StoreCodeProposal.encode(
  //           cosmwasm.wasm.v1.StoreCodeProposal.fromPartial({
  //             title: contract_name,
  //             description: `Add wasm code for ${contract_name} contract.`,
  //             runAs: wallet_address,
  //             wasmByteCode: Pako.gzip(wasm, { level: 9 }),
  //             instantiatePermission: {
  //               permission: cosmwasm.wasm.v1.accessTypeFromJSON(1),
  //             },
  //           })
  //         ).finish()
  //       ),
  //     };
  //     const res = await Gov_MsgSubmitProposal(client, wasmStoreProposal);
  //     let proposalId = res[0].events[3].attributes[1].value;
  //     // const json = JSON.parse(res.rawLog?);
  //     console.log(res);
  //   } catch (e) {
  //     console.log("Proposal Error has occoured => ", e);
  //   }
  // }

  // -----------x-------------x-------------x---------x---------------
  // ----------- CONTRACT INSTIANTIATION :: DEXTER VAULT -------------
  // -----------x-------------x-------------x---------x---------------

  // INSTIANTIATING CONTRACTS -::- DEXTER VAULT
  // let init_msg = {
  //   pool_configs: [
  //     {
  //       code_id: xyk_pool_code_id,
  //       pool_type: { xyk: {} },
  //       fee_info: {
  //         total_fee_bps: 300,
  //         protocol_fee_percent: 49,
  //         dev_fee_percent: 15,
  //         developer_addr: wallet_address,
  //       },
  //       is_disabled: false,
  //       is_generator_disabled: false,
  //     },
  //     {
  //       code_id: stableswap_pool_code_id,
  //       pool_type: { stable2_pool: {} },
  //       fee_info: {
  //         total_fee_bps: 300,
  //         protocol_fee_percent: 49,
  //         dev_fee_percent: 15,
  //         developer_addr: null,
  //       },
  //       is_disabled: false,
  //       is_generator_disabled: false,
  //     },
  //     {
  //       code_id: weighted_pool_code_id,
  //       pool_type: { weighted: {} },
  //       fee_info: {
  //         total_fee_bps: 300,
  //         protocol_fee_percent: 49,
  //         dev_fee_percent: 15,
  //         developer_addr: null,
  //       },
  //       is_disabled: false,
  //       is_generator_disabled: false,
  //     },
  //   ],
  //   lp_token_code_id: lp_token_code_id,
  //   fee_collector: null,
  //   owner: wallet_address,
  //   generator_address: null,
  // };
  // try {
  //   const wasmInstantiateProposal = {
  //     typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
  //     value: Uint8Array.from(
  //       cosmwasm.wasm.v1.InstantiateContractProposal.encode(
  //         cosmwasm.wasm.v1.InstantiateContractProposal.fromJSON({
  //           title: "Dexter Vault",
  //           description:
  //             "Dexter Vault contract, used facilitating token swaps and instantiating pools",
  //           runAs: wallet_address,
  //           admin: wallet_address,
  //           codeId: vault_code_id.toString(),
  //           label: "Dexter Vault",
  //           msg: Buffer.from(JSON.stringify(init_msg)).toString("base64"), // Buffer.from(JSON.stringify(init_msg)),
  //         })
  //       ).finish()
  //     ),
  //   };
  //   const res = await Gov_MsgSubmitProposal(client, wasmInstantiateProposal);
  //   console.log(res);
  //   let proposalId = res[0].events[3].attributes[1].value;
  //   // const json = JSON.parse(res.rawLog?);
  //   console.log(res);
  // } catch (e) {
  //   console.log("Proposal Error has occoured => ", e);
  // }

  // -----------x-------------x-------------x---------x---------------
  // ----------- CONTRACT INSTIANTIATION :: TEST TOKENS --------------
  // -----------x-------------x-------------x---------x---------------

  // let token_init_msg = {
  //   name: "Test3",
  //   symbol: "atDEX",
  //   decimals: 6,
  //   initial_balances: [{ address: wallet_address, amount: "10000000000000" }],
  //   mint: { minter: wallet_address, amount: "1000000000000000" },
  // };
  // try {
  //   const wasmInstantiateProposal = {
  //     typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
  //     value: Uint8Array.from(
  //       cosmwasm.wasm.v1.InstantiateContractProposal.encode(
  //         cosmwasm.wasm.v1.InstantiateContractProposal.fromJSON({
  //           title: "Test token",
  //           description: "Test token for testing dexter DEX",
  //           runAs: wallet_address,
  //           admin: wallet_address,
  //           codeId: lp_token_code_id.toString(),
  //           label: "Instiantiate dummy token 1",
  //           msg: Buffer.from(JSON.stringify(token_init_msg)).toString("base64"), // Buffer.from(JSON.stringify(init_msg)),
  //         })
  //       ).finish()
  //     ),
  //   };
  //   const res = await Gov_MsgSubmitProposal(client, wasmInstantiateProposal);
  //   console.log(res);
  //   let proposalId = res[0].events[3].attributes[1].value;
  //   // const json = JSON.parse(res.rawLog?);
  //   console.log(res);
  // } catch (e) {
  //   console.log("Proposal Error has occoured => ", e);
  // }

  // -----------x-------------x-------------x---------x---------------
  // ----------- CONTRACT INSTIANTIATION :: DEXTER KEEPER -------------
  // -----------x-------------x-------------x---------x---------------

  // INSTIANTIATING CONTRACTS -::- DEXTER KEEPER
  // let keeper_init_msg = {
  //   vault_contract: network.dexter_vault,
  // };
  // try {
  //   const wasmInstantiateProposal = {
  //     typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
  //     value: Uint8Array.from(
  //       cosmwasm.wasm.v1.InstantiateContractProposal.encode(
  //         cosmwasm.wasm.v1.InstantiateContractProposal.fromJSON({
  //           title: "Test token",
  //           description: "Dexter Keeper contract",
  //           runAs: wallet_address,
  //           admin: wallet_address,
  //           codeId: keeper_code_id.toString(),
  //           label: "Instiantiate Dexter Keeper contract",
  //           msg: Buffer.from(JSON.stringify(keeper_init_msg)).toString(
  //             "base64"
  //           ), // Buffer.from(JSON.stringify(init_msg)),
  //         })
  //       ).finish()
  //     ),
  //   };
  //   const res = await Gov_MsgSubmitProposal(client, wasmInstantiateProposal);
  //   console.log(res);
  //   let proposalId = res[0].events[3].attributes[1].value;
  //   console.log(proposalId);
  // } catch (e) {
  //   console.log("Proposal Error has occoured => ", e);
  // }

  // -----------x-------------x-------------x---------------
  // ----------- DEXTER VAULT :: ADD NEW POOL TYPE ---------
  // -----------x-------------x-------------x---------------

  // let add_pool_exec_msg = {
  //   add_to_registery: {
  //     new_pool_config: {
  //       code_id: 16,
  //       pool_type: { stable5_pool: {} },
  //       fee_info: {
  //         total_fee_bps: 300,
  //         protocol_fee_percent: 49,
  //         dev_fee_percent: 15,
  //         developer_addr: null,
  //       },
  //       is_disabled: false,
  //       is_generator_disabled: false,
  //     },
  //   },
  // };

  // const res = await executeContract(
  //   client,
  //   wallet_address,
  //   network.dexter_vault,
  //   add_pool_exec_msg
  // );
  // console.log(res);

  // -----------x-------------x-------------x---------------
  // ----------- DEXTER VAULT :: INITIALIZE NEW POOL ---
  // -----------x-------------x-------------x---------------

  // CREATE NEW DEXTER POOLS VIA GOVERNANCE
  let add_create_pool_exec_msg = {
    create_pool_instance: {
      pool_type: { xyk: {} },
      asset_infos: [
        { native_token: { denom: "uxprt" } },
        { token: { contract_addr: network.test_token_1 } },
      ],
    },
  };
  // Submit `ExecuteContractProposal` Proposal
  try {
    const wasmExecuteProposal = {
      typeUrl: "/cosmwasm.wasm.v1.ExecuteContractProposal",
      value: Uint8Array.from(
        cosmwasm.wasm.v1.ExecuteContractProposal.encode(
          cosmwasm.wasm.v1.ExecuteContractProposal.fromJSON({
            title: "Dexter - XYK Pool Instantiation",
            description: "Create dexter XYK Pool",
            runAs: wallet_address,
            contract: network.dexter_vault,
            msg: Buffer.from(JSON.stringify(add_create_pool_exec_msg)).toString(
              "base64"
            ), // Buffer.from(JSON.stringify(init_msg)),
          })
        ).finish()
      ),
    };
    const res = await Gov_MsgSubmitProposal(client, wasmExecuteProposal);
    console.log(res);
    let proposalId = res[0].events[3].attributes[1].value;
    console.log(proposalId);
  } catch (e) {
    console.log("Proposal Error has occoured => ", e);
  }

  // const res = await executeContract(
  //   client,
  //   wallet_address,
  //   network.dexter_vault,
  //   add_create_pool_exec_msg
  // );
  // console.log(res);
}

DexterDeploymentOnTestnetPipeline().catch(console.log);

// Instantiate Keeper contract - DONE
// Transactions executed successfully =>  03793CEE703FDE173CD5F2A5A11C2BDBE4C3B2ABF0ED138205CA4133CEAF462A
// Proposal Id is = 42

// Add stable5pool to dexter Vault - DONE
// TxHash - B721E52C8DAB1A05A05875BB63E3969AECBA56652E43DAA8E6BE40C48852B7EC

// Initialize XYK pool
// Initialize Stablepool pool
// Initialize WEIGHTED pool

// Initialize Dexter Generator
