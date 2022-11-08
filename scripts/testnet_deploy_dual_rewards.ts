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

  // Setting up dual rewards for dexter generator
  // 1. Instantiate staking contract with lp token, rewards token and distribution schedule
  // 2. Instantiate proxy contract for the staking contract
  // 3. Send reward tokens to staking contract for distribution
  // 4. Allow proxy in generator and initialize the lp generator pool

  // 1. Setup vesting schedule for generator in vesting contract
  // 2. Set tokens_per_block for generator

  const DEX_TOKEN =
    "persistence14pca2wuhufwpe4hsuka6ue2fmg0ffl5uumaa4p45l009mjw7r0pqtnz2f5";

  const LP_POOL =
    "persistence1nldrmkjqskmpjzkmefmvt4k9xh6rd7z68vf9hggjfnwh5uhgtwrqlsxx0j";
  const LP_TOKEN =
    "persistence14ay0q9mpn4qrz9wtc8wyms6pq5lchcynfjn7hskh2u95t4437wmsdnfwv9";
  const REWARD_TOKEN =
    "persistence1da9krw7mn7cp2p74sus6x0ckfd5c9q5vhqe92yx8cf5dyqu8q8gq7mg5uk";
  const distribution: [number, number, String] = [
    1667566020,
    1675342020,
    "1000000000000",
  ];

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

  // INSTANTIATE VESTING CONTRACT
  // ----------------------------
  // ----------------------------
  if (
    !network.vesting_instantiate_proposal_id &&
    (!network.vesting_contract_addr || network.vesting_contract_addr === "")
  ) {
    console.log(`\nSubmitting Proposal to instantiate Vesting Contract ...`);

    // Make proposal to instantiate Vesting contract
    if (network.vesting_contract_code_id > 0) {
      let init_vesting_msg = {
        owner: wallet_address,
        token_addr: DEX_TOKEN,
      };
      try {
        const wasmInstantiateProposal = {
          typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
          value: Uint8Array.from(
            cosmwasm.wasm.v1.InstantiateContractProposal.encode(
              cosmwasm.wasm.v1.InstantiateContractProposal.fromJSON({
                title: "Vesting Contract",
                description: "Vesting contract",
                runAs: wallet_address,
                admin: wallet_address,
                codeId: network.vesting_contract_code_id.toString(),
                label: "Vesting Contract",
                msg: Buffer.from(JSON.stringify(init_vesting_msg)).toString(
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
        network.vesting_instantiate_proposal_id = Number(
          res[0].events[3].attributes[1].value
        );
        writeArtifact(network, CHAIN_ID);
        // const json = JSON.parse(res.rawLog?);
      } catch (e) {
        console.log("Proposal Error has occoured => ", e);
      }
      // Vote on Proposal
      if (
        network.vesting_instantiate_proposal_id > 0 &&
        CHAIN_ID != "core-1" &&
        CHAIN_ID != "test-core-1"
      ) {
        try {
          console.log(
            `Voting on Proposal # ${network.vesting_instantiate_proposal_id}`
          );
          await voteOnProposal(
            client,
            network.vesting_instantiate_proposal_id,
            1,
            fee_denom
          );
          await voteOnProposal(
            validator_1,
            network.vesting_instantiate_proposal_id,
            1,
            fee_denom
          );
          await voteOnProposal(
            validator_2,
            network.vesting_instantiate_proposal_id,
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

  // Get Vesting contract address if the proposal has passed
  if (!network.vesting_contract_addr) {
    let res = await query_wasm_contractsByCode(
      client,
      network.vesting_contract_code_id
    );
    console.log(res);
    if (res["contracts"].length > 0) {
      network.vesting_contract_addr = res["contracts"][0];
      writeArtifact(network, CHAIN_ID);
    }
  }
  console.log(`Vesting Contract address = ${network.vesting_contract_addr}`);
  return;

  // INSTANTIATE STAKING CONTRACT
  // ----------------------------
  // ----------------------------
  if (
    !network.staking_contract_addr ||
    network.staking_contract_addr === "" ||
    !network.eq_staking_store_code_proposal_id ||
    network.eq_staking_store_code_proposal_id == 0
  ) {
    console.log(`\nSubmitting Proposal to instantiate Staking Contract ...`);

    // Make proposal to instantiate Staking contract
    if (network.staking_contract_contract_code_id > 0) {
      let init_staking_msg = {
        anchor_token: REWARD_TOKEN,
        staking_token: LP_TOKEN,
        distribution_schedule: [distribution],
      };
      try {
        const wasmInstantiateProposal = {
          typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
          value: Uint8Array.from(
            cosmwasm.wasm.v1.InstantiateContractProposal.encode(
              cosmwasm.wasm.v1.InstantiateContractProposal.fromJSON({
                title: "Staking Contract",
                description: "Staking contract",
                runAs: wallet_address,
                admin: wallet_address,
                codeId: network.staking_contract_contract_code_id.toString(),
                label: "Staking Contract",
                msg: Buffer.from(JSON.stringify(init_staking_msg)).toString(
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
        network.staking_instantiate_proposal_id = Number(
          res[0].events[3].attributes[1].value
        );
        writeArtifact(network, CHAIN_ID);
        // const json = JSON.parse(res.rawLog?);
      } catch (e) {
        console.log("Proposal Error has occoured => ", e);
      }
      // Vote on Proposal
      if (
        network.staking_instantiate_proposal_id > 0 &&
        CHAIN_ID != "core-1" &&
        CHAIN_ID != "test-core-1"
      ) {
        try {
          console.log(
            `Voting on Proposal # ${network.staking_instantiate_proposal_id}`
          );
          await voteOnProposal(
            client,
            network.staking_instantiate_proposal_id,
            1,
            fee_denom
          );
          await voteOnProposal(
            validator_1,
            network.staking_instantiate_proposal_id,
            1,
            fee_denom
          );
          await voteOnProposal(
            validator_2,
            network.staking_instantiate_proposal_id,
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

  // Get Staking contract address if the proposal has passed
  if (!network.staking_contract_addr) {
    let res = await query_wasm_contractsByCode(
      client,
      network.staking_contract_contract_code_id
    );
    console.log(res);
    if (res["contracts"].length > 0) {
      network.staking_contract_addr = res["contracts"][0];
      writeArtifact(network, CHAIN_ID);
    }
  }
  console.log(`Staking Contract address = ${network.staking_contract_addr}`);

  // INSTANTIATE PROXY REWARDS CONTRACT
  // ----------------------------
  // ----------------------------
  if (
    network.staking_contract_addr &&
    network.generator_proxy_contract_code_id &&
    network.generator_proxy_contract_code_id != 0 &&
    !network.proxy_instantiate_proposal_id
  ) {
    console.log(`\nSubmitting Proposal to instantiate Proxy Contract ...`);

    // Make proposal to instantiate Proxy contract
    if (network.generator_proxy_contract_code_id > 0) {
      let init_proxy_msg = {
        generator_contract_addr: network.generator_contract_addr,
        pair_addr: LP_POOL,
        lp_token_addr: LP_TOKEN,
        reward_contract_addr: network.staking_contract_addr,
        reward_token: { token: { contract_addr: REWARD_TOKEN } },
      };
      try {
        const wasmInstantiateProposal = {
          typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
          value: Uint8Array.from(
            cosmwasm.wasm.v1.InstantiateContractProposal.encode(
              cosmwasm.wasm.v1.InstantiateContractProposal.fromJSON({
                title: "Proxy Contract",
                description: "Proxy contract",
                runAs: wallet_address,
                admin: wallet_address,
                codeId: network.generator_proxy_contract_code_id.toString(),
                label: "Proxy Contract",
                msg: Buffer.from(JSON.stringify(init_proxy_msg)).toString(
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
        network.proxy_instantiate_proposal_id = Number(
          res[0].events[3].attributes[1].value
        );
        writeArtifact(network, CHAIN_ID);
        // const json = JSON.parse(res.rawLog?);
      } catch (e) {
        console.log("Proposal Error has occoured => ", e);
      }
      // Vote on Proposal
      if (
        network.proxy_instantiate_proposal_id > 0 &&
        CHAIN_ID != "core-1" &&
        CHAIN_ID != "test-core-1"
      ) {
        try {
          console.log(
            `Voting on Proposal # ${network.proxy_instantiate_proposal_id}`
          );
          await voteOnProposal(
            client,
            network.proxy_instantiate_proposal_id,
            1,
            fee_denom
          );
          await voteOnProposal(
            validator_1,
            network.proxy_instantiate_proposal_id,
            1,
            fee_denom
          );
          await voteOnProposal(
            validator_2,
            network.proxy_instantiate_proposal_id,
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

  // Get Proxy contract address if the proposal has passed
  if (!network.proxy_contract_addr) {
    let res = await query_wasm_contractsByCode(
      client,
      network.generator_proxy_contract_code_id
    );
    console.log(res);
    if (res["contracts"].length > 0) {
      network.proxy_contract_addr = res["contracts"][0];
      writeArtifact(network, CHAIN_ID);
    }
  }
  console.log(`Proxy Contract address = ${network.proxy_contract_addr}`);
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

Demo().catch(console.log);
