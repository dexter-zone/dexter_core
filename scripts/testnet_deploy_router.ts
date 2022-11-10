// import { CosmosChainClient, cosmwasm } from "cosmoschainsjs";
import * as Pako from "pako";
import * as fs from "fs";
import * as crypto from "crypto";
import {
  Gov_MsgSubmitProposal,
  voteOnProposal,
  readArtifact,
  writeArtifact,
  executeContract,
  query_gov_params,
  query_gov_proposal,
  find_code_id_from_contract_hash,
  query_wasm_contractsByCode,
  toEncodedBinary,
  index_dexter_create_pool_tx,
  query_wasm_code,
} from "./helpers/helpers.js";
import { toBinary } from "@cosmjs/cosmwasm-stargate";
import { Slip10RawIndex, pathToString, stringToPath } from "@cosmjs/crypto";
import { CosmosChainClient, cosmwasm } from "cosmossdkjs";

// ----------- PERSISTENCE END-POINTS -------------
// testnet: https://rpc.testnet.persistence.one:443     :: test-core-1
// mainnet: https://rpc.persistence.one:443             :: core-1
// devnet :  https://rpc.devnet.core.dexter.zone/       :: dev-core-1

// This is your rpc endpoint
// DEVNET = "https://rpc.devnet.core.dexter.zone/"
// TESTNET =  "https://rpc.testnet.persistence.one:443"
// LOCALNET = "http://localhost:26657"

let CONFIGS = {
  // MAIN-NET CONFIG
  "core-1": {
    chain_id: "core-1",
    rpc_endpoint: "https://rpc.persistence.one:443",
    validator_mnemonic: "",
    validator_mnemonic_2: "",
  },
  // TEST-NET CONFIG
  "test-core-1": {
    chain_id: "test-core-1",
    rpc_endpoint: "https://rpc.testnet.persistence.one:443",
    validator_mnemonic: "",
    validator_mnemonic_2: "",
  },
  // DEV-NET CONFIG
  persistencecore: {
    chain_id: "persistencecore",
    rpc_endpoint: "https://rpc.devnet.core.dexter.zone/",
    validator_mnemonic:
      "logic help only text door wealth hurt always remove glory viable income agent olive trial female couch old offer crash menu zero pencil thrive",
    validator_mnemonic_2:
      "middle weather hip ghost quick oxygen awful library broken chicken tackle animal crunch appear fee indoor fitness enough orphan trend tackle faint eyebrow all",
  },
  // LOCAL-NET CONFIG
  testing: {
    chain_id: "testing",
    rpc_endpoint: "http://localhost:26657",
    validator_mnemonic:
      "flash tuna music boat sign image judge engage pistol reason love reform defy game ceiling basket roof clay keen hint flash buyer fancy buyer",
    validator_mnemonic_2:
      "horse end velvet train canoe walnut lottery security sure right rigid busy either sand bar palace choice extend august mystery action surround coconut online",
  },
};

// Make HD path used during wallet creation
export function makeHdPath(coinType = 118, account = 0) {
  return [
    Slip10RawIndex.hardened(44),
    Slip10RawIndex.hardened(coinType),
    Slip10RawIndex.hardened(0),
    Slip10RawIndex.normal(0),
    Slip10RawIndex.normal(account),
  ];
}

function calculateCheckSum(filePath: string): string {
  const fileBuffer = fs.readFileSync(filePath);
  const hashSum = crypto.createHash("sha256");
  hashSum.update(fileBuffer);

  const hex = hashSum.digest("hex");
  return hex;
}

async function Demo() {
  const mnemonic = process.env.WALLET_MNEMONIC;
  const chain_id = process.env.CHAIN_ID;

  // Incase the mnemonic / chain ID is not set in the environment variables
  if (!mnemonic || !chain_id) {
    throw new Error("WALLET_MNEMONIC / CHAIN_ID is not set");
  }

  // Using a random generated mnemonic
  // const devnet_mnemonic = "opinion knife other balcony surge more bamboo canoe romance ask argue teach anxiety adjust spike mystery wolf alone torch tail six decide wash alley";
  // const testnet_mnemonic =
  //   "toss hammer lazy dish they ritual suggest favorite sword alcohol enact enforce mechanic spoon gather knock giggle indicate indicate nose actor brand basket confirm";
  // const localnet_mnemonic = "gravity bus kingdom auto limit gate humble abstract reopen resemble awkward cannon maximum bread balance insane banana maple screen mimic cluster pigeon badge walnut";
  const deposit_amount = 512_000_000;
  const fee_denom = "uxprt";
  const CHAIN_ID = chain_id;

  // network : stores contract addresses
  let network = readArtifact(CHAIN_ID);

  // Create a new persistence client
  const client = await CosmosChainClient.init(
    mnemonic,
    {
      rpc: CONFIGS[CHAIN_ID].rpc_endpoint,
      chainId: CONFIGS[CHAIN_ID].chain_id,
      gasPrices: {
        denom: fee_denom,
        amount: "2000000",
      },
      gasAdjustment: "1.5",
    },
    {
      bip39Password: "",
      hdPaths: [stringToPath("m/44'/118'/0'/0/0")],
      prefix: "persistence",
    }
  );

  // Create Persistence Validators
  const validator_1 = await CosmosChainClient.init(
    CONFIGS[CHAIN_ID].validator_mnemonic,
    {
      rpc: CONFIGS[CHAIN_ID].rpc_endpoint,
      chainId: CONFIGS[CHAIN_ID].chain_id,
      gasPrices: {
        denom: fee_denom,
        amount: "2000000",
      },
      gasAdjustment: "1.5",
    }
  );
  const validator_2 = await CosmosChainClient.init(
    CONFIGS[CHAIN_ID].validator_mnemonic_2,
    {
      rpc: CONFIGS[CHAIN_ID].rpc_endpoint,
      chainId: CONFIGS[CHAIN_ID].chain_id,
      gasPrices: {
        denom: fee_denom,
        amount: "2000000",
      },
      gasAdjustment: "1.5",
    }
  );

  // Get wallet address
  const [Account] = await client.wallet.getAccounts();
  const wallet_address = Account.address;
  console.log(`WALLET ADDRESS =  ${wallet_address}`);
  const OWNER = wallet_address;

  // Get chain height
  const height = await client.wasm.getHeight();
  console.log(`Blockchain height = ${height}`);

  // Get xprt balance
  const balance_res = await client.wasm.getBalance(wallet_address, fee_denom);
  let wallet_balance = Number(balance_res["amount"]) / 10 ** 6;
  console.log(`Wallet's XPRT balance = ${wallet_balance}`);

  // Get Generator Contract Addresses if the proposal has passed
  if (!network.generator_contract_addr) {
    let res = await query_wasm_contractsByCode(
      client,
      network.generator_contract_code_id
    );
    if (res["contracts"].length > 0) {
      network.generator_contract_addr = res["contracts"][0];
      writeArtifact(network, CHAIN_ID);
    }
  }

  // let ress = await query_gov_proposal(client, 253);
  // console.log(ress);

  // let code_id_res = await find_code_id_from_contract_hash(
  //   client,
  //   "712d320459fd7ed6ef35ad00369b03957a1b23305385b3f376c7db41f68b4ca7"
  // );
  // console.log(code_id_res);
  // return;

  // -----------x-------------x-------------x------------------------------
  // ----------- MAKE STORE CODE PROPOSALS FOR ALL DEXTER CONTRACTS -------99,994
  // -----------x-------------x-------------x------------------------------

  // // CONTRACTS WHICH ARE TO BE DEPLOYED ON PERSISTENCE ONE NETWORK FOR DEXTER PROTOCOL
  let contracts: any[] = [
    {
      name: "Router contract",
      path: "../artifacts/dexter_router.wasm",
      proposal_id: 0,
    },
  ];

  for (let contract of contracts) {
    let hash = calculateCheckSum(contract.path);
    contract.hash = hash;
  }

  console.log("contracts", contracts);

  // Loop across all contracts
  if (
    !network.router_store_code_proposal_id ||
    network.router_store_code_proposal_id == 0
  ) {
    for (let i = 0; i < contracts.length; i++) {
      // TRANSATION 1. --> Make proposal on-chain
      try {
        console.log(
          `\nSubmitting Proposal to store ${contracts[i]["name"]} Contract ...`
        );
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
        const res = await Gov_MsgSubmitProposal(
          client,
          wasmStoreProposal,
          fee_denom,
          deposit_amount
        );
        contracts[i]["proposal_id"] = Number(
          res[0].events[3].attributes[1].value
        );
        console.log(
          `${contracts[i]["name"]} STORE CODE PROPOSAL ID = ${contracts[i]["proposal_id"]}`
        );
      } catch (e) {
        console.log("Proposal Error has occoured => ", e);
      }
      // TRANSACTION 2. --> Vote on proposal
      if (
        contracts[i]["proposal_id"] > 0 &&
        CHAIN_ID != "core-1" &&
        CHAIN_ID != "test-core-1"
      ) {
        try {
          await voteOnProposal(
            client,
            contracts[i]["proposal_id"],
            1,
            fee_denom
          );
          await voteOnProposal(
            validator_1,
            contracts[i]["proposal_id"],
            1,
            fee_denom
          );
          await voteOnProposal(
            validator_2,
            contracts[i]["proposal_id"],
            1,
            fee_denom
          );
          console.log("Voted successfully");
        } catch (e) {
          console.log("Error has occoured while voting => ", e);
        }
      }
    }
    // Update propsoal IDs stored
    network.router_store_code_proposal_id = contracts[0]["proposal_id"];
    writeArtifact(network, CHAIN_ID);
    console.log(
      "Proposals for storing code for dexter contracts executed successfully"
    );
  }

  // GET CODE-IDs FOR ALL CONTRACTS
  if (
    !network.router_contract_code_id ||
    network.router_contract_code_id == 0
  ) {
    let code_id_res = await find_code_id_from_contract_hash(
      client,
      contracts[0]["hash"]
    );
    console.log(code_id_res);
    network.router_contract_code_id = Number(code_id_res);
  }

  writeArtifact(network, CHAIN_ID);

  // -----------x-------------x---------x---------------
  // ----------- INSTANTIATE DEXTER ROUTER  -------------
  // -----------x-------------x---------x---------------

  // Check if Router contract code has been stored on chain
  if (network.router_contract_code_id == 0) {
    console.log("Router contract code id not found. Exiting...");
    return;
  }

  // INSTANTIATE DEXTER ROUTER CONTRACT --> If router contract has not been instantiated yet
  if (
    network.router_contract_code_id > 0 &&
    (!network.router_instantiate_proposal_id ||
      network.router_instantiate_proposal_id == 0)
  ) {
    console.log(
      `\nSubmitting Proposal to instantiate Dexter ROUTER Contract ...`
    );

    // Make proposal to instantiate Vault contract
    if (network.router_contract_code_id > 0) {
      let init_router_msg = {
        dexter_vault: network.vault_contract_address,
      };
      try {
        const wasmInstantiateProposal = {
          typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
          value: Uint8Array.from(
            cosmwasm.wasm.v1.InstantiateContractProposal.encode(
              cosmwasm.wasm.v1.InstantiateContractProposal.fromJSON({
                title: "Dexter Router",
                description:
                  "Dexter Router contract, used facilitating token swaps",
                runAs: wallet_address,
                admin: wallet_address,
                codeId: network.router_contract_code_id.toString(),
                label: "Dexter Router",
                msg: Buffer.from(JSON.stringify(init_router_msg)).toString(
                  "base64"
                ),
              })
            ).finish()
          ),
        };
        const res = await Gov_MsgSubmitProposal(
          client,
          wasmInstantiateProposal,
          fee_denom,
          deposit_amount
        );
        console.log(res);
        network.router_instantiate_proposal_id = Number(
          res[0].events[3].attributes[1].value
        );
        writeArtifact(network, CHAIN_ID);
        // const json = JSON.parse(res.rawLog?);
      } catch (e) {
        console.log("Proposal Error has occoured => ", e);
      }
      // Vote on Proposal
      if (
        network.router_instantiate_proposal_id > 0 &&
        CHAIN_ID != "core-1" &&
        CHAIN_ID != "test-core-1"
      ) {
        try {
          console.log(
            `Voting on Proposal # ${network.router_instantiate_proposal_id}`
          );
          await voteOnProposal(
            client,
            network.router_instantiate_proposal_id,
            1,
            fee_denom
          );
          await voteOnProposal(
            validator_1,
            network.router_instantiate_proposal_id,
            1,
            fee_denom
          );
          await voteOnProposal(
            validator_2,
            network.router_instantiate_proposal_id,
            1,
            fee_denom
          );
          console.log("Voted successfully");
        } catch (e) {
          console.log("Error has occoured while voting => ", e);
        }
      }
    }
  }

  // Get ROUTER Contract Address if the proposal has passed
  if (
    (!network.router_contract_address ||
      network.router_contract_address == "") &&
    network.router_instantiate_proposal_id > 0
  ) {
    let res = await query_wasm_contractsByCode(
      client,
      network.router_contract_code_id
    );
    if (res["contracts"].length > 0) {
      network.router_contract_address =
        res["contracts"][res["contracts"].length - 1];
    } else {
      console.log("Router Contract Address not found. Exiting...");
      return;
    }
    writeArtifact(network, CHAIN_ID);
  }
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

Demo().catch(console.log);
