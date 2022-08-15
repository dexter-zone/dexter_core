import { PersistenceClient } from "persistenceonejs";
import { coins } from "@cosmjs/stargate";

async function Demo() {
  const alice = await PersistenceClient.init(
    "obtain door word season wealth inspire tobacco shallow thumb tip walk forum someone verb pistol bright mutual nest fog valley tiny section sauce typical"
  ); //persistence1ht0tun4u5uj4f4z83p9tncjerwu27ycsm52txm

  const codes = await alice.query.cosmwasm.wasm.v1.codes({});
  console.log(codes);

  const [account] = await alice.wallet.getAccounts();
  const aliceaddress = account.address; //persistence1ht0tun4u5uj4f4z83p9tncjerwu27ycsm52txm

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
