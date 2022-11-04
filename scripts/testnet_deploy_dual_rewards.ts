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

  // // CONTRACTS WHICH ARE TO BE DEPLOYED ON PERSISTENCE ONE NETWORK FOR DEXTER PROTOCOL
  let contracts: any[] = [
    {
      name: "Dexter Generator : Proxy",
      path: "../artifacts/dexter_generator_proxy.wasm",
      proposal_id: 0,
    },
    {
      name: "Staking contract",
      path: "../artifacts/anchor_staking.wasm",
      proposal_id: 0,
    },
  ];

  // UPLOAD CODE IDs
  // --- Staking Contract ---
  let staking_contract_code_id = 0;
  try {
    console.log(`\nSubmitting Proposal to store Staking Contract ...`);
    const wasm = fs.readFileSync("../artifacts/anchor_staking.wasm");
    const wasmStoreProposal = {
      typeUrl: "/cosmwasm.wasm.v1.StoreCodeProposal",
      value: Uint8Array.from(
        cosmwasm.wasm.v1.StoreCodeProposal.encode(
          cosmwasm.wasm.v1.StoreCodeProposal.fromPartial({
            title: "Staking contract",
            description: `Add wasm code for Staking contract`,
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
    staking_contract_code_id = Number(res[0].events[3].attributes[1].value);
    console.log(
      `Staking contract -::- STORE CODE PROPOSAL ID = ${staking_contract_code_id}`
    );
  } catch (e) {
    console.log("Proposal Error has occoured => ", e);
  }
  // TRANSACTION 2. --> Vote on proposal
  if (
    staking_contract_code_id > 0 &&
    CHAIN_ID != "core-1" &&
    CHAIN_ID != "test-core-1"
  ) {
    try {
      await voteOnProposal(client, contracts[i]["proposal_id"], 1, fee_denom);
      await voteOnProposal(validator_1, staking_contract_code_id, 1, fee_denom);
      await voteOnProposal(validator_2, staking_contract_code_id, 1, fee_denom);
      console.log("Voted successfully");
    } catch (e) {
      console.log("Error has occoured while voting => ", e);
    }
  }
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

Demo().catch(console.log);
