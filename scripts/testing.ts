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
import { coins, Coin } from "@cosmjs/stargate";

// ----------- PERSISTENCE END-POINTS -------------
// testnet: https://rpc.testnet.persistence.one:443     :: test-core-1
// mainnet: https://rpc.persistence.one:443             :: core-1
// devnet :  https://rpc.devnet.core.dexter.zone/       :: dev-core-1

// This is your rpc endpoint
// DEVNET = "https://rpc.devnet.core.dexter.zone/"
// TESTNET =  "https://rpc.testnet.persistence.one:443"
// LOCALNET = "http://localhost:26657"
const rpcEndpoint = "https://rpc.testnet.persistence.one:443";

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
  // Using a random generated mnemonic
  // const devnet_mnemonic = "opinion knife other balcony surge more bamboo canoe romance ask argue teach anxiety adjust spike mystery wolf alone torch tail six decide wash alley";
  const testnet_mnemonic =
    "toss hammer lazy dish they ritual suggest favorite sword alcohol enact enforce mechanic spoon gather knock giggle indicate indicate nose actor brand basket confirm";
  // const localnet_mnemonic = "gravity bus kingdom auto limit gate humble abstract reopen resemble awkward cannon maximum bread balance insane banana maple screen mimic cluster pigeon badge walnut";
  const deposit_amount = 512_000_000;
  const fee_denom = "uxprt";
  const CHAIN_ID = "test-core-1"; // "persistencecore" "test-core-1" ; // "testing";

  // network : stores contract addresses
  let network = readArtifact(CHAIN_ID);

  // Create a new persistence client
  const client = await CosmosChainClient.init(
    testnet_mnemonic,
    {
      rpc: rpcEndpoint,
      chainId: CHAIN_ID,
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

  // console.log(client.config);
  // let params_ = await query_gov_params(client, "deposit");
  // console.log(params_.depositParams.minDeposit );

  // Create Persistence Validators
  const validator_1 = await CosmosChainClient.init(
    "logic help only text door wealth hurt always remove glory viable income agent olive trial female couch old offer crash menu zero pencil thrive",
    {
      // const validator_1 = await CosmosChainClient.init("flash tuna music boat sign image judge engage pistol reason love reform defy game ceiling basket roof clay keen hint flash buyer fancy buyer" , {
      rpc: rpcEndpoint,
      chainId: "test-core-1",
      gasPrices: {
        denom: fee_denom,
        amount: "2000000",
      },
      gasAdjustment: "1.5",
    }
  );
  const validator_2 = await CosmosChainClient.init(
    "middle weather hip ghost quick oxygen awful library broken chicken tackle animal crunch appear fee indoor fitness enough orphan trend tackle faint eyebrow all",
    {
      // const validator_2 = await CosmosChainClient.init("horse end velvet train canoe walnut lottery security sure right rigid busy either sand bar palace choice extend august mystery action surround coconut online" , {
      rpc: rpcEndpoint,
      chainId: "testing",
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

  // let codes = await getContractsByCodeId(client, 1);
  // console.log(`CODES = ${JSON.stringify(codes)}`);
  // let res = await query_gov_proposal(
  //   client,
  //   network.lp_token_instantiate_permissions_proposal_id
  // );
  // console.log(res);
  // return

  // -----------x-------------x-------------x------------------------------
  // Give allowance to Vault contract
  // for (let i = 0; i < network.test_tokens_addresses.length; i++) {
  //   let allowance_msg = {
  //     increase_allowance: {
  //       spender: network.vault_contract_address,
  //       amount: "1000000000000",
  //     },
  //   };
  //   let res = await executeContract(
  //     client,
  //     wallet_address,
  //     network.test_tokens_addresses[i],
  //     allowance_msg
  //   );
  //   let txhash = res["transactionHash"];
  //   console.log(`Test token ${i} allowance txhash = ${txhash}`);
  //   await delay(1000);
  // }

  // Add Liquidity to 1st XYK Pool
  // let mint_msg = {
  //   mint: {
  //     amount: "1000000000000",
  //     recipient: wallet_address,
  //   },
  // };

  // let res_ = await executeContract(
  //   client,
  //   wallet_address,
  //   "persistence1vguuxez2h5ekltfj9gjd62fs5k4rl2zy5hfrncasykzw08rezpfst7tmng",
  //   mint_msg
  // );
  // let txhash_ = res_["transactionHash"];
  // console.log(`Mint txhash = ${txhash_}`);
  // return;

  let POOL_ADDR =
    "persistence1k528kg8h3q56j5yazshv39fafmhjzl4540u7w36g6q2amgyrpwpsvexl2d";

  // Query the pool Id for the pool
  let quer_pool_id = await client.wasm.queryContractSmart(POOL_ADDR, {
    pool_id: {},
  });
  // console.log(`Pool Id = ${JSON.stringify(quer_pool_id)}`);
  let pool_id = quer_pool_id;
  console.log(`Pool Id = ${pool_id}`);

  // Query the pool's configuration for the pool
  let query_pool_config = await client.wasm.queryContractSmart(POOL_ADDR, {
    config: {},
  });
  let pool_assets = query_pool_config["assets"];
  let pool_lp_token = query_pool_config["lp_token_addr"];
  let pool_type = query_pool_config["pool_type"];

  console.log(`Pool Assets = ${JSON.stringify(pool_assets)}`);
  console.log(`Pool LP pool = ${JSON.stringify(pool_lp_token)}`);
  console.log(`Pool Type = ${JSON.stringify(pool_type)}`);

  // Get pool info
  let current_pool_confg = {
    pool_id: pool_id,
    pool_addr: POOL_ADDR,
    assets: pool_assets,
    lp_token_addr: pool_lp_token,
    pool_type: pool_type,
  };
  console.log(current_pool_confg);

  network.dexter_pools.push(current_pool_confg);
  writeArtifact(network, CHAIN_ID);

  // let pool_id = query["pool_id"];

  return;

  // Add Liquidity to 1st XYK Pool
  let join_pool_msg = {
    join_pool: {
      pool_id: pool_id,
      assets: [
        {
          info: {
            token: {
              contract_addr:
                "persistence1rtdulljz3dntzpu085c7mzre9dg4trgdddu4tqk7uuuvu6xrfu8s8wcs45",
            },
          },
          amount: "1000000000",
        },
        {
          info: {
            token: {
              contract_addr:
                "persistence1u2zdjcczjrenwmf57fmrpensk4the84azdm05m3unm387rm8asdsh0yf27",
            },
          },
          amount: "1000000000",
        },
      ],
    },
  };
  console.log(join_pool_msg);
  let res = await executeContract(
    client,
    wallet_address,
    network.vault_contract_address,
    join_pool_msg,
    ""
    // coins(1000000000, "uxprt")
  );
  let txhash = res["transactionHash"];
  console.log(`1st XYK Pool  JOIN POOL txhash = ${txhash}`);
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

Demo().catch(console.log);
