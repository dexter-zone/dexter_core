import { PersistenceClient, cosmos, cosmwasm } from "persistenceonejs";
import { coins, Coin } from "@cosmjs/stargate";
import { Any } from "cosmjs-types/google/protobuf/any.js";
// import { Any } from "cosmjs-types/google/protobuf/any";
import * as Long from "long";
import { readFileSync, writeFileSync } from "fs";
import path from "path";

export const ARTIFACTS_PATH = "../artifacts";

// Reads json containing contract addresses located in /artifacts folder. Naming convention : bombay-12 / columbus-5
export function readArtifact(name: string = "artifact") {
  try {
    const data = readFileSync(
      path.join(ARTIFACTS_PATH, `${name}.json`),
      "utf8"
    );
    return JSON.parse(data);
  } catch (e) {
    return {};
  }
}

export function writeArtifact(data: object, name: string = "artifact") {
  writeFileSync(
    path.join(ARTIFACTS_PATH, `${name}.json`),
    JSON.stringify(data, null, 2)
  );
}

// --------x-------------x-------------x----- -----x---------
// --------x----- QUERY :: Helpers Functions-----x-----------
// --------x-------------x-------------x----- -----x---------

// Return list of codeIds with codeInfos having datahash for the contract
export async function getCodeIdsListWithPrintedHexHashes(
  client: PersistenceClient
) {
  let codes = await client.query.cosmwasm.wasm.v1.codes({});
  let codeInfos = codes["codeInfos"];
  let pagination = codes["pagination"];

  for (let i = 0; i < codeInfos.length; i++) {
    let hex = Buffer.from(codeInfos[i]["dataHash"]).toString("hex");
    let code_id = codeInfos[i]["codeId"];
    console.log(` code_id = ${code_id} hex = ${hex}`);
  }
  return codes;
}

// Return address of instantiated contract instance address for a given codeId
export async function getContractsByCodeId(
  client: PersistenceClient,
  codeId: number
) {
  let codes = await client.query.cosmwasm.wasm.v1.contractsByCode(
    cosmwasm.wasm.v1.QueryContractsByCodeRequest.fromPartial({ codeId: 8 })
  );
  return codes;
}

// --------x-------------x-------------x----- -----x---------
// --------x----- EXECUTE CONTRACT :: Helpers Functions-----x-----------
// --------x-------------x-------------x----- -----x---------

export async function executeContract(
  client: PersistenceClient,
  wallet_address: string,
  contract_address: string,
  msg: any,
  memo?: string,
  funds?: Coin[] | undefined
) {
  try {
    const res = await client.wasm.execute(
      wallet_address,
      contract_address,
      msg,
      { amount: coins(2_000_000, "uxprt"), gas: "200000" },
      memo,
      funds
    );
    let txhash = res["transactionHash"];
    console.log(`Tx executed -- ${txhash}`);
    return res;
  } catch (e) {
    console.log("Proposal Error has occoured => ", e);
  }
}

// --------x----- Governance Module Helpers -----x-----------
// --------x-------------x-------------x----- -----x---------

/// GOV MODULE -- SUBMIT PROPOSAL ExecuteMsg
/// Network : 512 XPRT are required as deposit to deploy the contract
/// Tokens transferred - initialDeposit amount is transferred from user a/c to the module address
export async function Gov_MsgSubmitProposal(
  client: PersistenceClient,
  proposal: { typeUrl?: string; value?: Uint8Array }
) {
  const [account] = await client.wallet.getAccounts();
  //submit proposal Msg
  const proposalMsg = {
    typeUrl: "/cosmos.gov.v1beta1.MsgSubmitProposal",
    value: {
      content: Any.fromPartial(proposal),
      initialDeposit: coins(512_000_000, "uxprt"),
      proposer: account.address,
    },
  };
  // sign & broadcast the transaction
  const res = await client.core.signAndBroadcast(
    account.address,
    [proposalMsg],
    { amount: coins(2_000_000, "uxprt"), gas: "20000000" },
    "Proposal Submitted!"
  );
  // Handle the response
  if (res.code === 0) {
    const json = JSON.parse(res.rawLog);
    console.log("Transactions executed successfully => ", res.transactionHash);
    let proposalId = json[0].events[3].attributes[1].value;
    console.log(`Proposal Id is = ${proposalId} `);
    return json; //return json response, for proposalID use json[0].events[3].attributes[1].value
  } else {
    return res.rawLog;
  }
}

/// GOV MODULE -- Deposit with Propsoal - ExecuteMsg
export async function Gov_MsgDeposit(
  client: PersistenceClient,
  proposal_id: number,
  amount: number,
  denom: string
) {
  const [account] = await client.wallet.getAccounts();
  // Deposit with Propsoal
  const proposalMsg = {
    typeUrl: "/cosmos.gov.v1beta1.MsgDeposit",
    value: {
      proposal_id: Long.fromNumber(proposal_id),
      depositor: account.address,
      amount: coins(amount, denom),
    },
  };
  // sign & broadcast the transaction
  const res = await client.core.signAndBroadcast(
    account.address,
    [proposalMsg],
    { amount: coins(2_000_000, denom), gas: "20000000" },
    "Proposal Submitted!"
  );
  // Handle the response
  if (res.code === 0) {
    const json = JSON.parse(res.rawLog);
    console.log("Transactions executed successfully => ", res.transactionHash);
    let proposalId = json[0].events[3].attributes[1].value;
    return json;
  } else {
    return res.rawLog;
  }
}

/// GOV MODULE -- VOTE on Propsoal - ExecuteMsg
export async function voteOnProposal(
  client: PersistenceClient,
  proposalid: number,
  vote: number
) {
  const [account] = await client.wallet.getAccounts();
  // Vote on Propsoal
  const sendMsg = {
    typeUrl: "/cosmos.gov.v1beta1.MsgVote",
    value: {
      proposalId: Long.fromNumber(proposalid),
      voter: account.address,
      option: cosmos.gov.v1beta1.voteOptionFromJSON(vote),
    },
  };
  // sign & broadcast the transaction
  const res = await client.core.signAndBroadcast(
    account.address,
    [sendMsg],
    { amount: coins(10_000_000, "stake"), gas: "2000000" },
    "Vote Yes!"
  );
  if (res.code === 0) {
    return res;
  } else {
    return res.rawLog;
  }
}

/// TRANSFER NATIVE COINS
export async function Send(
  client: PersistenceClient,
  from: string,
  to: string,
  amount: Coin
) {
  const wallet = client.wallet;
  const [account] = await wallet.getAccounts();
  const sendMsg = {
    typeUrl: "/cosmos.bank.v1beta1.tx.MsgSend",
    value: cosmos.bank.v1beta1.MsgSend.fromJSON({
      fromAddress: from,
      toAddress: to,
      amount: amount,
    }),
  };
  const res = await client.core.signAndBroadcast(
    account.address,
    [sendMsg],
    { amount: [{ denom: "uxprt", amount: "10000" }], gas: "100" },
    "test send"
  );
  console.log(res);
}

export async function MultiSend(
  client: PersistenceClient,
  input: [],
  output: []
) {
  /*
    example type for inputs and outputs
    const input = [
      {
        address: "persistence123...", //fromAddress
        coins: coins(300, "uxprt"),
      },
    ];
    const output = [
      {
        address: "persistence1...", //toAddress 1
        coins: coins(100, "uxprt"),
      },
      {
        address: "persistence2...", //toAddress 2
        coins: coins(100, "uxprt"),
      },
      {
        address: "persistence3...", //toAddress 3
        coins: coins(100, "uxprt"),
      },
    ];
     */
  const wallet = client.wallet;
  const [account] = await wallet.getAccounts();
  const sendMsg = {
    typeUrl: "/cosmos.bank.v1beta1.tx.MsgSend",
    value: cosmos.bank.v1beta1.MsgMultiSend.fromJSON({
      inputs: input,
      outputs: output,
    }),
  };
  const res = await client.core.signAndBroadcast(
    account.address,
    [sendMsg],
    { amount: [{ denom: "uxprt", amount: "10000" }], gas: "200000" },
    "test send"
  );
  console.log(res);
}
