import { PersistenceClient, cosmos } from "persistenceonejs";
import { coins, Coin } from "@cosmjs/stargate";
import { Any } from "cosmjs-types/google/protobuf/any";
import * as Long from "long";

/// SUBMIT CONTRACT PROPOSAL
export async function contractProposal(
  client: PersistenceClient,
  proposal: { typeUrl?: string; value?: Uint8Array }
) {
  const [account] = await client.wallet.getAccounts();
  //submit proposal
  const proposalMsg = {
    typeUrl: "/cosmos.gov.v1beta1.MsgSubmitProposal",
    value: {
      content: Any.fromPartial(proposal),
      initialDeposit: coins(600_000_000, "stake"),
      proposer: account.address,
    },
  };
  const res = await client.core.signAndBroadcast(
    account.address,
    [proposalMsg],
    { amount: coins(20_000_000, "stake"), gas: "20000000" },
    "Proposal Submitted!"
  );
  if (res.code === 0) {
    // const json = JSON.parse(res.rawLog);
    return res.rawLog; //return json response, for proposalID use json[0].events[3].attributes[1].value
  } else {
    return res.rawLog;
  }
}

// /// TRANSFER NATIVE COINS
// export async function Send(
//   client: PersistenceClient,
//   from: string,
//   to: string,
//   amount: Coin
// ) {
//   const wallet = client.wallet;
//   const [account] = await wallet.getAccounts();
//   const sendMsg = {
//     typeUrl: "/cosmos.bank.v1beta1.tx.MsgSend",
//     value: cosmos.bank.v1beta1.MsgSend.fromJSON({
//       fromAddress: from,
//       toAddress: to,
//       amount: amount,
//     }),
//   };
//   const res = await client.core.signAndBroadcast(
//     account.address,
//     [sendMsg],
//     { amount: [{ denom: "uxprt", amount: "10000" }], gas: "100" },
//     "test send"
//   );
//   console.log(res);
// }

// /// VOTE ON PROPOSAL
// export async function voteOnProposal(
//   client: PersistenceClient,
//   proposalid: number,
//   vote: number
// ) {
//   const [account] = await client.wallet.getAccounts();
//   const sendMsg = {
//     typeUrl: "/cosmos.gov.v1beta1.MsgVote",
//     value: {
//       proposalId: Long.fromNumber(proposalid),
//       voter: account.address,
//       option: cosmos.gov.v1beta1.voteOptionFromJSON(vote),
//     },
//   };
//   const res = await client.core.signAndBroadcast(
//     account.address,
//     [sendMsg],
//     { amount: coins(10_000_000, "stake"), gas: "2000000" },
//     "Vote Yes!"
//   );
//   if (res.code === 0) {
//     return res;
//   } else {
//     return res.rawLog;
//   }
// }

// export async function MultiSend(
//   client: PersistenceClient,
//   input: [],
//   output: []
// ) {
//   /*
//     example type for inputs and outputs
//     const input = [
//       {
//         address: "persistence123...", //fromAddress
//         coins: coins(300, "uxprt"),
//       },
//     ];
//     const output = [
//       {
//         address: "persistence1...", //toAddress 1
//         coins: coins(100, "uxprt"),
//       },
//       {
//         address: "persistence2...", //toAddress 2
//         coins: coins(100, "uxprt"),
//       },
//       {
//         address: "persistence3...", //toAddress 3
//         coins: coins(100, "uxprt"),
//       },
//     ];
//      */
//   const wallet = client.wallet;
//   const [account] = await wallet.getAccounts();
//   const sendMsg = {
//     typeUrl: "/cosmos.bank.v1beta1.tx.MsgSend",
//     value: cosmos.bank.v1beta1.MsgMultiSend.fromJSON({
//       inputs: input,
//       outputs: output,
//     }),
//   };
//   const res = await client.core.signAndBroadcast(
//     account.address,
//     [sendMsg],
//     { amount: [{ denom: "uxprt", amount: "10000" }], gas: "200000" },
//     "test send"
//   );
//   console.log(res);
// }
