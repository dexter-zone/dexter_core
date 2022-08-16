import { PersistenceClient } from "persistenceonejs";
import {
  SigningCosmWasmClient,
  Secp256k1HdWallet,
  setupWebKeplr,
  CosmWasmClient,
} from "cosmwasm";
import { coins, Coin } from "@cosmjs/stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";

// ----------- PERSISTENCE END-POINTS -------------
// testnet: https://rpc.testnet.persistence.one:443     :: test-core-1
// mainnet: https://rpc.persistence.one:443             :: core-1

// This is your rpc endpoint
const rpcEndpoint = "https://rpc.testnet.persistence.one:443";

// Using a random generated mnemonic
const mnemonic =
  "rifle same bitter control garage duck grab spare mountain doctor rubber cook";

async function Demo() {
  const val1 = await PersistenceClient.init(
    "flash tuna music boat sign image judge engage pistol reason love reform defy game ceiling basket roof clay keen hint flash buyer fancy buyer",
    {
      rpc: rpcEndpoint,
      chainId: "test-core-1",
      gasPrices: { denom: "", amount: "0" },
      gasAdjustment: "1.5",
    }
  );
  const [val1Account] = await val1.wallet.getAccounts();
  const val1Address = val1Account.address;
  console.log(val1Address);
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
