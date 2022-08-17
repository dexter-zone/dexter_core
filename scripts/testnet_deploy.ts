import { PersistenceClient, cosmwasm } from "persistenceonejs";
// import {
//   SigningCosmWasmClient,
//   Secp256k1HdWallet,
//   setupWebKeplr,
//   CosmWasmClient,
// } from "cosmwasm";
// import { coins, Coin } from "@cosmjs/stargate";
// import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import * as Pako from "pako";
import * as fs from "fs";
import { InitiateContractProposal } from "./helpers/helpers.js";

// ----------- PERSISTENCE END-POINTS -------------
// testnet: https://rpc.testnet.persistence.one:443     :: test-core-1
// mainnet: https://rpc.persistence.one:443             :: core-1

// This is your rpc endpoint
const rpcEndpoint = "https://rpc.testnet.persistence.one:443";

// Using a random generated mnemonic
const mnemonic = "";

async function Demo() {
  // Create a new persistence client
  const client = await PersistenceClient.init(mnemonic, {
    rpc: rpcEndpoint,
    chainId: "test-core-1",
    gasPrices: {
      denom: "uxprt",
      amount: "2000000",
    },
    gasAdjustment: "1.5",
  });
  const [Account] = await client.wallet.getAccounts();
  const wallet_address = Account.address;
  console.log(` WALLET ADDRESS =  ${wallet_address}`);

  // -----------x-------------x-------------x-----
  // ----------- CONTRACT DEPLOYMENT -------------
  // -----------x-------------x-------------x-----

  // CONTRACTS WHICH ARE TO BE DEPLOYED ON PERSISTENCE ONE NETWORK FOR DEXTER PROTOCOL
  let contracts = [
    { name: "Dexter Vault", path: "../artifacts/dexter_vault.wasm" },
    { name: "Dexter Keeper", path: "../artifacts/dexter_keeper.wasm" },
    { name: "LP Token", path: "../artifacts/lp_token.wasm" },
    { name: "XYK Pool", path: "../artifacts/xyk_pool.wasm" },
    { name: "Weighted Pool", path: "../artifacts/weighted_pool.wasm" },
    { name: "Stableswap Pool", path: "../artifacts/stableswap_pool.wasm" },
    { name: "Dexter Vesting", path: "../artifacts/dexter_vesting.wasm" },
    { name: "Dexter Generator", path: "../artifacts/dexter_generator.wasm" },
    {
      name: "Dexter Generator : Proxy",
      path: "../artifacts/dexter_generator_proxy.wasm",
    },
    { name: "Staking contract", path: "../artifacts/anchor_staking.wasm" },
  ];

  // LOOP -::- CREATE PROTOCOLS FOR EACH CONTRACT ON-CHAIN
  for (let i = 0; i < contracts.length; i++) {
    let contract_name = contracts[i]["name"];
    let contract_path = contracts[i]["path"];

    try {
      console.log(
        `\nSubmitting Proposal to deploy ${contract_name} Contract ...`
      );
      const wasm = fs.readFileSync(contract_path);
      //wasm proposl of type StoreCodeProposal
      const wasmStoreProposal = {
        typeUrl: "/cosmwasm.wasm.v1.StoreCodeProposal",
        value: Uint8Array.from(
          cosmwasm.wasm.v1.StoreCodeProposal.encode(
            cosmwasm.wasm.v1.StoreCodeProposal.fromPartial({
              title: contract_name,
              description: `Add wasm code for ${contract_name} contract.`,
              runAs: wallet_address,
              wasmByteCode: Pako.gzip(wasm, { level: 9 }),
              instantiatePermission: {
                permission: cosmwasm.wasm.v1.accessTypeFromJSON(1),
              },
            })
          ).finish()
        ),
      };
      const res = await InitiateContractProposal(client, wasmStoreProposal);
      let proposalId = res[0].events[3].attributes[1].value;
      // const json = JSON.parse(res.rawLog?);
      // console.log(res);
    } catch (e) {
      console.log("Proposal Error has occoured => ", e);
    }
  }

  // const config = {
  //   chainId: "test-core-1",
  //   rpcEndpoint: rpcEndpoint,
  //   prefix: "persistence",
  // };
  // const client = await CosmWasmClient.connect(rpcEndpoint);
  // console.log(client);

  // // Create a wallet
  // const wallet = await Secp256k1HdWallet.fromMnemonic(mnemonic);
  // console.log(wallet);

  // This is your contract address
  // const contractAddr =
  //   "wasm19qws2lfd8pskyn0cfgpl5yjjyq3msy5402qr8nkzff9kdnkaepyqycedfh";
  // const conswasm_client = await CosmWasmClient.connect(rpcEndpoint);
  // const q_config = await client.queryContractSmart(contractAddr, {
  //   config: {},
  // });
  // const alice = await PersistenceClient.init(
  //   "obtain door word season wealth inspire tobacco shallow thumb tip walk forum someone verb pistol bright mutual nest fog valley tiny section sauce typical"
  // ); //persistence1ht0tun4u5uj4f4z83p9tncjerwu27ycsm52txm
  // const codes = await alice.query.cosmwasm.wasm.v1.codes({});
  // console.log(codes);
  // const [account] = await alice.wallet.getAccounts();

  // const aliceaddress = account.address; //persistence1ht0tun4u5uj4f4z83p9tncjerwu27ycsm52txm
  // const pstake =
  //   "persistence14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9sjvz4fk"; //cw20 token address on chain
  // const res = await alice.wasm.execute(
  //   aliceaddress,
  //   pstake,
  //   {
  //     transfer: {
  //       recipient: "persistence123em6jp7y96rtylp6tjk9r0dcescl0k4ccqvpu", //recipient address
  //       amount: "10",
  //     },
  //   },
  //   { amount: coins(2_000_000, "stake"), gas: "200000" }
  // );
  // console.log(res);
  // const balance = await alice.wasm.queryContractSmart(pstake, {
  //   balance: { address: "persistence123em6jp7y96rtylp6tjk9r0dcescl0k4ccqvpu" },
  // });
  // console.log(balance);
}

Demo().catch(console.log);
