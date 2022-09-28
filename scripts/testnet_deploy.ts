// import { CosmosChainClient, cosmwasm } from "cosmoschainsjs";
import * as Pako from "pako";
import * as fs from "fs";
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

  // -----------x-------------x-------------x------------------------------
  // ----------- MAKE STORE CODE PROPOSALS FOR ALL DEXTER CONTRACTS -------
  // -----------x-------------x-------------x------------------------------

  // // CONTRACTS WHICH ARE TO BE DEPLOYED ON PERSISTENCE ONE NETWORK FOR DEXTER PROTOCOL
  let contracts = [
    {
      name: "Dexter Vault",
      path: "../artifacts/dexter_vault.wasm",
      proposal_id: 0,
      hash: "7491d419533f35372c58562a3dfc8a9cf8252c4874aa113eb3d78ae6cb4935df",
    },
    {
      name: "Dexter Keeper",
      path: "../artifacts/dexter_keeper.wasm",
      proposal_id: 0,
      hash: "067206f9dde2ff38d9a3164c13412c1b2f480a7010cdc8b6bec2a88cb8d188d1",
    },
    {
      name: "LP Token",
      path: "../artifacts/lp_token.wasm",
      proposal_id: 0,
      hash: "48ac9688ad68b66c36184b47682c061ae2763c769e458ef190064d2013563418",
    },
    {
      name: "XYK Pool",
      path: "../artifacts/xyk_pool.wasm",
      proposal_id: 0,
      hash: "0a04a3d2bf62f9b12f2adba2835235d2c393aa5ca07c269709d64234457f1154",
    },
    {
      name: "Weighted Pool",
      path: "../artifacts/weighted_pool.wasm",
      proposal_id: 0,
      hash: "92bea1ade0540596895486a545d8b8292dbe0233126d1a70bc6ff91af14760dd",
    },
    {
      name: "Stableswap Pool",
      path: "../artifacts/stableswap_pool.wasm",
      proposal_id: 0,
      hash: "db8669b1781cc0595c841a0412d4c1175d881d99caa4516340545c9558344c15",
    },
    {
      name: "Stable5Swap Pool",
      path: "../artifacts/stable5pool.wasm",
      proposal_id: 0,
      hash: "6eb9df53c21e5de40bc4a647393e99c3815e642b95ad39cdb1c06f5b52e1751b",
    },
    {
      name: "Dexter Vesting",
      path: "../artifacts/dexter_vesting.wasm",
      proposal_id: 0,
      hash: "9fed0b82283c3881c242cc51d80c1d9b73fb8fd038da726d6f850de2736a253f",
    },
    {
      name: "Dexter Generator",
      path: "../artifacts/dexter_generator.wasm",
      proposal_id: 0,
      hash: "b34ed02bf7d57a69c90946d9503f4a58f730d8fc2772dcead2106f35bab45acd",
    },
    {
      name: "Dexter Generator : Proxy",
      path: "../artifacts/dexter_generator_proxy.wasm",
      proposal_id: 0,
      hash: "9764f035f5daa0215c7fcbcf4774403732c432077cddb34566156d17ff9dd8e2",
    },
    {
      name: "Staking contract",
      path: "../artifacts/anchor_staking.wasm",
      proposal_id: 0,
      hash: "5be7457d88f0e4c264a75ea89ae7bff16dd821cfd7f74736c5828a6d6e7f625c",
    },
  ];

  // UPLOAD CODE OF ALL CONTRACTS
  if (
    !network.contracts_store_code_proposals_executed ||
    network.contracts_store_code_proposals_executed == 0
  ) {
    // Loop across all contracts
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
    network.contracts_store_code_proposals_executed = true;

    // Update propsoal IDs stored
    network.vault_store_code_proposal_id = contracts[0]["proposal_id"];
    network.keeper_store_code_proposal_id = contracts[1]["proposal_id"];
    network.lp_token_store_code_proposal_id = contracts[2]["proposal_id"];
    network.xyk_pool_store_code_proposal_id = contracts[3]["proposal_id"];
    network.weighted_pool_store_code_proposal_id = contracts[4]["proposal_id"];
    network.stableswap_pool_store_code_proposal_id =
      contracts[5]["proposal_id"];
    network.stable5swap_store_code_proposal_id = contracts[6]["proposal_id"];
    network.vesting_store_code_proposal_id = contracts[7]["proposal_id"];
    network.generator_store_code_proposal_id = contracts[8]["proposal_id"];
    network.generator_proxy_store_code_proposal_id =
      contracts[9]["proposal_id"];
    network.eq_staking_store_code_proposal_id = contracts[10]["proposal_id"];
    writeArtifact(network, CHAIN_ID);
    console.log(
      "Proposals for storing code for dexter contracts executed successfully"
    );
  } else {
    console.log(
      "Proposals for storing code for dexter contracts have already been executed"
    );
  }

  // GET CODE-IDs FOR ALL CONTRACTS
  if (!network.vault_contract_code_id || network.vault_contract_code_id == 0) {
    let code_id_res = await find_code_id_from_contract_hash(
      client,
      contracts[0]["hash"]
    );
    console.log(code_id_res);
    network.vault_contract_code_id = Number(code_id_res);
  }
  if (
    !network.keeper_contract_code_id ||
    network.keeper_contract_code_id == 0
  ) {
    let code_id_res = await find_code_id_from_contract_hash(
      client,
      contracts[1]["hash"]
    );
    network.keeper_contract_code_id = Number(code_id_res);
  }
  if (
    !network.lp_token_contract_code_id ||
    network.lp_token_contract_code_id == 0
  ) {
    let code_id_res = await find_code_id_from_contract_hash(
      client,
      contracts[2]["hash"]
    );
    network.lp_token_contract_code_id = Number(code_id_res);
  }
  if (
    !network.xyk_pool_contract_code_id ||
    network.xyk_pool_contract_code_id == 0
  ) {
    let code_id_res = await find_code_id_from_contract_hash(
      client,
      contracts[3]["hash"]
    );
    network.xyk_pool_contract_code_id = Number(code_id_res);
  }
  if (
    !network.weighted_pool_contract_code_id ||
    network.weighted_pool_contract_code_id == 0
  ) {
    let code_id_res = await find_code_id_from_contract_hash(
      client,
      contracts[4]["hash"]
    );
    network.weighted_pool_contract_code_id = Number(code_id_res);
  }
  if (
    !network.stableswap_contract_code_id ||
    network.stableswap_contract_code_id == 0
  ) {
    let code_id_res = await find_code_id_from_contract_hash(
      client,
      contracts[5]["hash"]
    );
    network.stableswap_contract_code_id = Number(code_id_res);
  }
  if (
    !network.stable5swap_pool_contract_code_id ||
    network.stable5swap_pool_contract_code_id == 0
  ) {
    let code_id_res = await find_code_id_from_contract_hash(
      client,
      contracts[6]["hash"]
    );
    network.stable5swap_pool_contract_code_id = Number(code_id_res);
  }
  if (
    !network.vesting_contract_code_id ||
    network.vesting_contract_code_id == 0
  ) {
    let code_id_res = await find_code_id_from_contract_hash(
      client,
      contracts[7]["hash"]
    );
    network.vesting_contract_code_id = Number(code_id_res);
  }
  if (
    !network.generator_contract_code_id ||
    network.generator_contract_code_id == 0
  ) {
    let code_id_res = await find_code_id_from_contract_hash(
      client,
      contracts[8]["hash"]
    );
    network.generator_contract_code_id = Number(code_id_res);
  }
  if (
    !network.generator_proxy_contract_code_id ||
    network.generator_proxy_contract_code_id == 0
  ) {
    let code_id_res = await find_code_id_from_contract_hash(
      client,
      contracts[9]["hash"]
    );
    network.generator_proxy_contract_code_id = Number(code_id_res);
  }
  if (
    !network.staking_contract_contract_code_id ||
    network.staking_contract_contract_code_id == 0
  ) {
    let code_id_res = await find_code_id_from_contract_hash(
      client,
      contracts[10]["hash"]
    );
    network.staking_contract_contract_code_id = Number(code_id_res);
  }
  writeArtifact(network, CHAIN_ID);

  // -----------x-------------x---------x---------------
  // ----------- INSTANTIATE DEXTER VAULT  -------------
  // -----------x-------------x---------x---------------

  // Check if vault contract code has been stored on chain
  if (network.vault_contract_code_id == 0) {
    console.log("Vault contract code id not found. Exiting...");
    return;
  }
  // Check if XYK pool code has been stored on chain
  if (network.xyk_pool_contract_code_id == 0) {
    console.log("XYK pool code id not found. Exiting...");
    return;
  }
  // Check if Stableswap pool code has been stored on chain
  if (network.stableswap_contract_code_id == 0) {
    console.log("Stableswap pool code id not found. Exiting...");
    return;
  }
  // Check if Stable-5-swap pool code has been stored on chain
  if (network.stable5swap_pool_contract_code_id == 0) {
    console.log("Stable-5-swap pool code id not found. Exiting...");
    return;
  }
  // Check if Weighted pool code has been stored on chain
  if (network.weighted_pool_contract_code_id == 0) {
    console.log("Weighted pool code id not found. Exiting...");
    return;
  }
  // Check if LP Token code has been stored on chain
  if (network.lp_token_contract_code_id == 0) {
    console.log(" LP Token code id not found. Exiting...");
    return;
  }

  // INSTANTIATE DEXTER VAULT CONTRACT --> If vault contract has not been instantiated yet
  if (
    network.vault_contract_code_id > 0 &&
    (!network.vault_instantiate_proposal_id ||
      network.vault_instantiate_proposal_id == 0)
  ) {
    console.log(
      `\nSubmitting Proposal to instantiate Dexter VAULT Contract ...`
    );

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
                description:
                  "Dexter Vault contract, used facilitating token swaps and instantiating pools",
                runAs: wallet_address,
                admin: wallet_address,
                codeId: network.vault_contract_code_id.toString(),
                label: "Dexter Vault",
                msg: Buffer.from(JSON.stringify(init_vault_msg)).toString(
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
        network.vault_instantiate_proposal_id = Number(
          res[0].events[3].attributes[1].value
        );
        writeArtifact(network, CHAIN_ID);
        // const json = JSON.parse(res.rawLog?);
      } catch (e) {
        console.log("Proposal Error has occoured => ", e);
      }
      // Vote on Proposal
      if (
        network.vault_instantiate_proposal_id > 0 &&
        CHAIN_ID != "core-1" &&
        CHAIN_ID != "test-core-1"
      ) {
        try {
          console.log(
            `Voting on Proposal # ${network.vault_instantiate_proposal_id}`
          );
          await voteOnProposal(
            client,
            network.vault_instantiate_proposal_id,
            1,
            fee_denom
          );
          await voteOnProposal(
            validator_1,
            network.vault_instantiate_proposal_id,
            1,
            fee_denom
          );
          await voteOnProposal(
            validator_2,
            network.vault_instantiate_proposal_id,
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

  // Get VAULT Contract Address if the proposal has passed
  if (
    (!network.vault_contract_address || network.vault_contract_address == "") &&
    network.vault_instantiate_proposal_id > 0
  ) {
    let res = await query_wasm_contractsByCode(
      client,
      network.vault_contract_code_id
    );
    if (res["contracts"].length > 0) {
      network.vault_contract_address =
        res["contracts"][res["contracts"].length - 1];
    } else {
      console.log("Vault Contract Address not found. Exiting...");
      return;
    }
    writeArtifact(network, CHAIN_ID);
  }

  // -----------x-------------x-------------x---------x---------------
  // ----------- CONTRACT INSTIANTIATION :: TEST TOKENS --------------
  // -----------x-------------x-------------x---------x---------------

  // INSTANTIATE TEST TOKEN CONTRACT --> If test token contract has not been instantiated yet
  if (!network.dummy_tokens_instantiated) {
    let tokens_ = [
      { name: "C-LUNC", symbol: "LUNC", decimals: 6 },
      { name: "C-OSMO", symbol: "OSMO", decimals: 6 },
      { name: "C-JUNO", symbol: "JUNO", decimals: 6 },
      { name: "C-FET", symbol: "FET", decimals: 6 },
    ];
    for (let i = 0; i < tokens_.length; i++) {
      let token_init_msg = {
        name: tokens_[i]["name"],
        symbol: tokens_[i]["symbol"],
        decimals: tokens_[i]["decimals"],
        initial_balances: [
          { address: wallet_address, amount: "10000000000000" },
        ],
        mint: { minter: wallet_address, amount: "1000000000000000" },
      };
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
                msg: Buffer.from(JSON.stringify(token_init_msg)).toString(
                  "base64"
                ), // Buffer.from(JSON.stringify(init_msg)),
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
        let proposalId = res[0].events[3].attributes[1].value;
        if (proposalId > 0) {
          network.dummy_tokens_instantiated = true;
          writeArtifact(network, CHAIN_ID);
        }
        console.log(
          `Proposal Id for dummy token ${tokens_[i]["name"]} = ${proposalId}`
        );
        // Vote on Proposal
        if (
          network.vault_instantiate_proposal_id > 0 &&
          CHAIN_ID != "core-1" &&
          CHAIN_ID != "test-core-1"
        ) {
          try {
            await voteOnProposal(client, proposalId, 1, fee_denom);
            await delay(3000);
            await voteOnProposal(validator_1, proposalId, 1, fee_denom);
            await delay(3000);
            await voteOnProposal(validator_2, proposalId, 1, fee_denom);
            await delay(3000);
          } catch (e) {
            console.log(
              `Error has occoured while voting on proposal ${proposalId} => ${e}`
            );
          }
        }
      } catch (e) {
        console.log("Proposal Error has occoured => ", e);
      }
    }
  }

  // Get test tokens Contract Addresses if the proposal has passed
  if (
    !network.test_tokens_addresses ||
    network.test_tokens_addresses.length < 3
  ) {
    let res = await query_wasm_contractsByCode(
      client,
      network.lp_token_contract_code_id
    );
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
    {
      name: "LP Token Pool",
      codeId: network.lp_token_contract_code_id,
      proposal_id: 0,
    },
    {
      name: "XYK Pool",
      codeId: network.xyk_pool_contract_code_id,
      proposal_id: 0,
    },
    {
      name: "Stableswap Pool",
      codeId: network.stableswap_contract_code_id,
      proposal_id: 0,
    },
    {
      name: "Stable5swap Pool",
      codeId: network.stable5swap_pool_contract_code_id,
      proposal_id: 0,
    },
    {
      name: "Weighted Pool",
      codeId: network.weighted_pool_contract_code_id,
      proposal_id: 0,
    },
  ];
  if (
    network.vault_contract_address &&
    !network.proposals_to_update_permissions
  ) {
    // Loop
    for (let i = 0; i < contracts_to_be_updated.length; i++) {
      // TRANSATION 1. --> Make proposal on-chain
      try {
        console.log(
          `\nSubmitting Proposal to update Dexter ${contracts_to_be_updated[i]["name"]} Contract instantiation permission ...`
        );
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
                    },
                  },
                ],
              })
            ).finish()
          ),
        };
        const res = await Gov_MsgSubmitProposal(
          client,
          wasmUpdateContractInstantiationPermissionProposal,
          fee_denom,
          deposit_amount
        );
        contracts_to_be_updated[i]["proposal_id"] = Number(
          res[0].events[3].attributes[1].value
        );
      } catch (e) {
        console.log("Proposal Error has occoured => ", e);
      }
      // TRANSACTION 2. --> Vote on proposal
      if (
        contracts_to_be_updated[i]["proposal_id"] > 0 &&
        CHAIN_ID != "core-1" &&
        CHAIN_ID != "test-core-1"
      ) {
        try {
          console.log(
            `Voting on Proposal # ${contracts_to_be_updated[i]["proposal_id"]}`
          );
          await voteOnProposal(
            client,
            contracts_to_be_updated[i]["proposal_id"],
            1,
            fee_denom
          );
          await voteOnProposal(
            validator_1,
            contracts_to_be_updated[i]["proposal_id"],
            1,
            fee_denom
          );
          await voteOnProposal(
            validator_2,
            contracts_to_be_updated[i]["proposal_id"],
            1,
            fee_denom
          );
          console.log("Voted successfully");
        } catch (e) {
          console.log("Error has occoured while voting => ", e);
        }
      }
    }
    network.proposals_to_update_permissions = true;

    // Update proposal IDs stored
    network.lp_token_instantiate_permissions_proposal_id =
      contracts_to_be_updated[0]["proposal_id"];
    network.xyk_pool_instantiate_permissions_proposal_id =
      contracts_to_be_updated[1]["proposal_id"];
    network.stableswap_pool_instantiate_permissions_proposal_id =
      contracts_to_be_updated[2]["proposal_id"];
    network.stable5pool_instantiate_permissions_proposal_id =
      contracts_to_be_updated[3]["proposal_id"];
    network.weighted_instantiate_permissions_proposal_id =
      contracts_to_be_updated[4]["proposal_id"];
    writeArtifact(network, CHAIN_ID);
  }

  // -----------x-------------x-------------x---------x---------------
  // ----------- CONTRACT INSTIANTIATION :: KEEPER CONTRACT ----------
  // -----------x-------------x-------------x---------x---------------

  if (!network.keeper_contract_instantiate_proposal) {
    let init_msg = { vault_contract: network.vault_contract_address };
    try {
      const wasmInstantiateProposal = {
        typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
        value: Uint8Array.from(
          cosmwasm.wasm.v1.InstantiateContractProposal.encode(
            cosmwasm.wasm.v1.InstantiateContractProposal.fromJSON({
              title: "Dexter Keeper contract",
              description:
                "Dexter's Keeper contract which stores protocol's collected swap fees",
              runAs: wallet_address,
              admin: wallet_address,
              codeId: network.keeper_contract_code_id.toString(),
              label: "Keeper contract",
              msg: Buffer.from(JSON.stringify(init_msg)).toString("base64"),
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
      let proposalId = res[0].events[3].attributes[1].value;
      if (proposalId > 0) {
        network.keeper_contract_instantiate_proposal = proposalId;
        writeArtifact(network, CHAIN_ID);
      }
      console.log(
        `Proposal Id for instantiating Keeper contract ${proposalId}`
      );
    } catch (e) {
      console.log("Proposal Error has occoured => ", e);
    }
    // TRANSACTION 2. --> Vote on proposal
    if (
      network.keeper_contract_instantiate_proposal > 0 &&
      CHAIN_ID != "core-1" &&
      CHAIN_ID != "test-core-1"
    ) {
      try {
        console.log(
          `Voting on Proposal # ${network.keeper_contract_instantiate_proposal}`
        );
        await voteOnProposal(
          client,
          network.keeper_contract_instantiate_proposal,
          1,
          fee_denom
        );
        await voteOnProposal(
          validator_1,
          network.keeper_contract_instantiate_proposal,
          1,
          fee_denom
        );
        await voteOnProposal(
          validator_2,
          network.keeper_contract_instantiate_proposal,
          1,
          fee_denom
        );
        console.log("Voted successfully");
      } catch (e) {
        console.log("Error has occoured while voting => ", e);
      }
    }
  }

  // -----------x-------------x-------------x---------x---------------
  // ----------- CONTRACT INSTIANTIATION :: GENERATOR CONTRACT -------
  // -----------x-------------x-------------x---------x---------------

  if (!network.generator_contract_instantiate_proposal) {
    let init_msg = {
      owner: OWNER,
      vault: network.vault_contract_address,
      guardian: undefined,
      dex_token: undefined,
      tokens_per_block: "0",
      start_block: "7975290", // 7952826 + Number(24*60*60/5*1.3),
      unbonding_period: (86400 * 4) / 24,
    };
    try {
      const wasmInstantiateProposal = {
        typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
        value: Uint8Array.from(
          cosmwasm.wasm.v1.InstantiateContractProposal.encode(
            cosmwasm.wasm.v1.InstantiateContractProposal.fromJSON({
              title: "Dexter Generator contract",
              description: "Dexter's Generator contract",
              runAs: wallet_address,
              admin: wallet_address,
              codeId: network.generator_contract_code_id.toString(),
              label: "Generator contract",
              msg: Buffer.from(JSON.stringify(init_msg)).toString("base64"),
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
      // console.log(res);
      let proposalId = res[0].events[3].attributes[1].value;
      if (proposalId > 0) {
        network.generator_contract_instantiate_proposal = proposalId;
        writeArtifact(network, CHAIN_ID);
      }
      console.log(
        `Proposal Id for instantiating Generator contract ${proposalId}`
      );
    } catch (e) {
      console.log("Proposal Error has occoured => ", e);
    }
    // TRANSACTION 2. --> Vote on proposal
    if (
      network.generator_contract_instantiate_proposal > 0 &&
      CHAIN_ID != "core-1" &&
      CHAIN_ID != "test-core-1"
    ) {
      try {
        console.log(
          `Voting on Proposal # ${network.generator_contract_instantiate_proposal}`
        );
        await voteOnProposal(
          client,
          network.generator_contract_instantiate_proposal,
          1,
          fee_denom
        );
        await voteOnProposal(
          validator_1,
          network.generator_contract_instantiate_proposal,
          1,
          fee_denom
        );
        await voteOnProposal(
          validator_2,
          network.generator_contract_instantiate_proposal,
          1,
          fee_denom
        );
        console.log("Voted successfully");
      } catch (e) {
        console.log("Error has occoured while voting => ", e);
      }
    }
  }

  // Get Keeper Contract Addresses if the proposal has passed
  if (!network.keeper_contract_addr) {
    let res = await query_wasm_contractsByCode(
      client,
      network.keeper_contract_code_id
    );
    if (res["contracts"].length > 0) {
      network.keeper_contract_addr = res["contracts"][0];
      writeArtifact(network, CHAIN_ID);
    }
  }

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

  // ---------------------------
  // CREATE XYK POOL (XPRT - T1)
  // ---------------------------
  if (!network.xyk_pool_address) {
    let create_pool_exec_msg = {
      create_pool_instance: {
        pool_type: { xyk: {} },
        asset_infos: [
          { native_token: { denom: fee_denom } },
          { token: { contract_addr: network.test_tokens_addresses[0] } },
        ],
      },
    };
    // TRANSACTION 1. --> Create Pool
    try {
      let ex = await executeContract(
        client,
        wallet_address,
        network.vault_contract_address,
        create_pool_exec_msg
      );
      let events = ex?.logs[0].events;
      let addresses = index_dexter_create_pool_tx(events);
      // console.log(addresses);
      network.xyk_pool_address = addresses.pool_addr;
      network.xyk_pool_lp_token_address = addresses.lp_token_addr;
      network.xyk_pool_asset_infos =
        create_pool_exec_msg.create_pool_instance.asset_infos;
      console.log(`\nXYK Pool created! \nXYK Pool Address: ${network.xyk_pool_address}\nXYK LP token Address: ${network.xyk_pool_address}\
      \nXYK Pool : asset 1 - ${network.xyk_pool_asset_infos[0]} \nXYK Pool : asset 2 - ${network.xyk_pool_asset_infos[1]}`);
      writeArtifact(network, CHAIN_ID);
    } catch (e) {
      console.log("Error has occoured while creating XYK pool => ", e);
    }
  }

  // ---------------------------
  // CREATE STABLESWAP POOL (T0 - T1)
  // ---------------------------
  if (!network.stableswap_pool_address) {
    let create_pool_exec_msg = {
      create_pool_instance: {
        pool_type: { stable2_pool: {} },
        asset_infos: [
          { token: { contract_addr: network.test_tokens_addresses[1] } },
          { token: { contract_addr: network.test_tokens_addresses[0] } },
        ],
        init_params: toEncodedBinary({ amp: 10 }),
      },
    };
    // TRANSACTION 1. --> Create Pool
    try {
      let ex = await executeContract(
        client,
        wallet_address,
        network.vault_contract_address,
        create_pool_exec_msg
      );
      let events = ex?.logs[0].events;
      let addresses = index_dexter_create_pool_tx(events);
      // console.log(addresses);
      network.stableswap_pool_address = addresses.pool_addr;
      network.stableswap_pool_lp_token_address = addresses.lp_token_addr;
      network.stableswap_pool_asset_infos =
        create_pool_exec_msg.create_pool_instance.asset_infos;
      console.log(`\nStableswap Pool created! \nStableswap Pool Address: ${network.stableswap_pool_address}\nStableswap LP token Address: ${network.stableswap_pool_address}\
      \nStableswap Pool : asset 1 - ${network.stableswap_pool_asset_infos[0]} \nStableswap Pool : asset 2 - ${network.stableswap_pool_asset_infos[1]}`);
      writeArtifact(network, CHAIN_ID);
    } catch (e) {
      console.log("Error has occoured while creating Stableswap pool => ", e);
    }
  }

  // ---------------------------
  // CREATE STABLE-5-SWAP POOL (T0 - T1)
  // ---------------------------
  if (!network.stable5swap_pool_address) {
    let create_pool_exec_msg = {
      create_pool_instance: {
        pool_type: { stable5_pool: {} },
        asset_infos: [
          { token: { contract_addr: network.test_tokens_addresses[1] } },
          { token: { contract_addr: network.test_tokens_addresses[0] } },
          { token: { contract_addr: network.test_tokens_addresses[2] } },
        ],
        init_params: toEncodedBinary({ amp: 10 }),
      },
    };
    // TRANSACTION 1. --> Create Pool
    try {
      let ex = await executeContract(
        client,
        wallet_address,
        network.vault_contract_address,
        create_pool_exec_msg
      );
      let events = ex?.logs[0].events;
      let addresses = index_dexter_create_pool_tx(events);
      // console.log(addresses);
      network.stable5swap_pool_address = addresses.pool_addr;
      network.stable5swap_pool_lp_token_address = addresses.lp_token_addr;
      network.stable5swap_pool_asset_infos =
        create_pool_exec_msg.create_pool_instance.asset_infos;
      console.log(`\nStable-5-swap Pool created! \nStable-5-swap Pool Address: ${network.stable5swap_pool_address}\nStable-5-swap LP token Address: ${network.stable5swap_pool_address}\
      \nStable-5-swap Pool : asset 1 - ${network.stable5swap_pool_asset_infos[0]} \nStable-5-swap Pool : asset 2 - ${network.stable5swap_pool_asset_infos[1]} \nStable-5-swap Pool : asset 3 - ${network.stable5swap_pool_asset_infos[2]}`);
      writeArtifact(network, CHAIN_ID);
    } catch (e) {
      console.log(
        "Error has occoured while creating Stable-5-swap pool => ",
        e
      );
    }
  }

  // ---------------------------
  // CREATE WEIGHTED POOL
  // ---------------------------

  if (!network.weighted_pool_address) {
    let weights = [
      { info: { native_token: { denom: fee_denom } }, amount: "10" },
      {
        info: { token: { contract_addr: network.test_tokens_addresses[0] } },
        amount: "20",
      },
      {
        info: { token: { contract_addr: network.test_tokens_addresses[1] } },
        amount: "30",
      },
      {
        info: { token: { contract_addr: network.test_tokens_addresses[2] } },
        amount: "40",
      },
    ];
    let params = toEncodedBinary({
      weights: weights,
      exit_fee: "0.01",
    });
    let create_pool_exec_msg = {
      create_pool_instance: {
        pool_type: { weighted: {} },
        init_params: params,
        asset_infos: [
          { native_token: { denom: fee_denom } },
          { token: { contract_addr: network.test_tokens_addresses[0] } },
          { token: { contract_addr: network.test_tokens_addresses[1] } },
          { token: { contract_addr: network.test_tokens_addresses[2] } },
        ],
      },
    };
    // TRANSACTION 1. --> Create Pool
    try {
      let ex = await executeContract(
        client,
        wallet_address,
        network.vault_contract_address,
        create_pool_exec_msg
      );
      let events = ex?.logs[0].events;
      let addresses = index_dexter_create_pool_tx(events);
      // console.log(addresses);
      network.weighted_pool_address = addresses.pool_addr;
      network.weighted_pool_lp_token_address = addresses.lp_token_addr;
      network.weighted_pool_asset_infos =
        create_pool_exec_msg.create_pool_instance.asset_infos;
      console.log(`\nWeighted Pool created! \nWeighted Pool Address: ${network.weighted_pool_address}\nWeighted LP token Address: ${network.weighted_pool_address}\
      \nWeighted Pool : asset 1 - ${network.weighted_pool_asset_infos[0]} \nWeighted Pool : asset 2 - ${network.weighted_pool_asset_infos[1]} \nWeighted Pool : asset 3 - ${network.weighted_pool_asset_infos[2]} \nWeighted Pool : asset 4 - ${network.weighted_pool_asset_infos[3]}`);
      writeArtifact(network, CHAIN_ID);
    } catch (e) {
      console.log("Error has occoured while creating Weighted pool => ", e);
    }
  }

  // -----------x-------------x-------------x---------x---------------
  // ----------- CONTRACT INSTIANTIATION :: GENERATOR PROXY CONTRACT -------
  // -----------x-------------x-------------x---------x---------------

  // if (!network.proxy_contract_instantiate_proposal) {
  //   let init_msg = {  owner: OWNER,
  //   generator_contract_addr: network.generator_contract_addr,
  //   pair_addr: "",
  //   lp_token_addr: "",
  //   reward_contract_addr: "=",
  //   reward_token: {},
  //   };
  //   try {
  //       const wasmInstantiateProposal = {
  //         typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
  //         value: Uint8Array.from(
  //           cosmwasm.wasm.v1.InstantiateContractProposal.encode(
  //             cosmwasm.wasm.v1.InstantiateContractProposal.fromJSON({
  //               title: "Generator proxy contract",
  //               description: "Generator proxy contract",
  //               runAs: wallet_address,
  //               admin: wallet_address,
  //               codeId: network.generator_proxy_store_code_proposal_id.toString(),
  //               label: "Generator proxy contract",
  //               msg: Buffer.from(JSON.stringify(init_msg)).toString("base64"),
  //             })
  //           ).finish()
  //         ),
  //       };
  //       const res = await Gov_MsgSubmitProposal(client, wasmInstantiateProposal, fee_denom, deposit_amount);
  //       console.log(res)
  //       let proposalId = res[0].events[3].attributes[1].value;
  //       if (proposalId > 0) {
  //         network.proxy_contract_instantiate_proposal = proposalId;
  //         writeArtifact(network, CHAIN_ID);
  //       }
  //       console.log(`Proposal Id for instantiating Generator contract ${proposalId}`)
  //       // await delay(3000);
  //       // await voteOnProposal(client, proposalId, 1, fee_denom);
  //       // await delay(3000);
  //       // await voteOnProposal(validator_1, proposalId, 1, fee_denom);
  //       // await delay(3000);
  //       // await voteOnProposal(validator_2, proposalId, 1, fee_denom);
  //       // await delay(3000);
  //       // console.log(res);
  //     } catch (e) {
  //       console.log("Proposal Error has occoured => ", e);
  //     }
  // }

  // -----------x-------------x-------------x---------x---------------
  // ----------- CONTRACT INSTIANTIATION :: STAKING CONTRACT -------
  // -----------x-------------x-------------x---------x---------------

  // if (!network.eq_staking_contract_instantiate_proposal) {
  //   let init_msg = {  owner: OWNER,
  //     anchor_token: "String",
  //     staking_token: "String", // lp token of ANC-UST pair contract
  //     distribution_schedule: [{}]
  //   };
  //   try {
  //       const wasmInstantiateProposal = {
  //         typeUrl: "/cosmwasm.wasm.v1.InstantiateContractProposal",
  //         value: Uint8Array.from(
  //           cosmwasm.wasm.v1.InstantiateContractProposal.encode(
  //             cosmwasm.wasm.v1.InstantiateContractProposal.fromJSON({
  //               title: "Example Staking contract",
  //               description: "Example Staking contract",
  //               runAs: wallet_address,
  //               admin: wallet_address,
  //               codeId: network.eq_staking_store_code_proposal_id.toString(),
  //               label: "Example Staking contract",
  //               msg: Buffer.from(JSON.stringify(init_msg)).toString("base64"),
  //             })
  //           ).finish()
  //         ),
  //       };
  //       const res = await Gov_MsgSubmitProposal(client, wasmInstantiateProposal, fee_denom, deposit_amount);
  //       console.log(res)
  //       let proposalId = res[0].events[3].attributes[1].value;
  //       if (proposalId > 0) {
  //         network.eq_staking_contract_instantiate_proposal = proposalId;
  //         writeArtifact(network, CHAIN_ID);
  //       }
  //       console.log(`Proposal Id for instantiating Generator contract ${proposalId}`)
  //       // await delay(3000);
  //       // await voteOnProposal(client, proposalId, 1, fee_denom);
  //       // await delay(3000);
  //       // await voteOnProposal(validator_1, proposalId, 1, fee_denom);
  //       // await delay(3000);
  //       // await voteOnProposal(validator_2, proposalId, 1, fee_denom);
  //       // await delay(3000);
  //       // console.log(res);
  //     } catch (e) {
  //       console.log("Proposal Error has occoured => ", e);
  //     }
  // }
  // return;

  // let res = await query_gov_proposal(client, network.xyk_pool_instantiate_permissions_proposal_id);
  // let res = await query_wasm_code(client, network.xyk_pool_contract_code_id);
  // console.log(res.codeInfo.instantiatePermission);

  // return
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

Demo().catch(console.log);