import { PersistenceClient, cosmos, cosmwasm } from "cosmoschainsjs";
import { coins, Coin } from "@cosmjs/stargate";
import { Any } from "cosmjs-types/google/protobuf/any.js";
import Long  from "long";
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


// --------x-------------x-------------x----- -----x-----------------x-----------
// --------x----- WASM MODULE :::: QUERIES :: Helpers Functions -----x-----------
// --------x-------------x-------------x----- -----x-----------------x-----------


// ContractInfo gets the contract meta data
export async function query_wasm_contractInfo(
  client: PersistenceClient,
  contract_addr: string
) {
  let response = await client.query.cosmwasm.wasm.v1.contractInfo(
    cosmwasm.wasm.v1.QueryContractInfoRequest.fromPartial({ address: contract_addr })
  );
  return response;
}

// ContractHistory gets the contract code history
export async function query_wasm_contractHistory(
  client: PersistenceClient,
  contract_addr: string,
  offset?: number,
  limit?: number
) {
  let response = await client.query.cosmwasm.wasm.v1.contractHistory(
    cosmwasm.wasm.v1.QueryContractHistoryRequest.fromPartial({ address: contract_addr,
    pagination: cosmos.base.query.v1beta1.PageRequest.fromPartial({offset: offset, limit: limit}) })
  );
  return response;
}

// ContractsByCode lists all smart contracts for a code id
export async function query_wasm_contractsByCode(
  client: PersistenceClient,
  codeId: number,
  offset?: number,
  limit?: number,
) {
  let codes = await client.query.cosmwasm.wasm.v1.contractsByCode(
    cosmwasm.wasm.v1.QueryContractsByCodeRequest.fromPartial({ codeId: codeId ,
    pagination: cosmos.base.query.v1beta1.PageRequest.fromPartial({offset: offset, limit: limit})} )
  );
  return codes;
}

// AllContractState gets all raw store data for a single contract
export async function query_wasm_allContractState(
  client: PersistenceClient,
  address: string,
  offset?: number,
  limit?: number,
) {
  let response = await client.query.cosmwasm.wasm.v1.allContractState(
    cosmwasm.wasm.v1.QueryAllContractStateRequest.fromPartial({ address: address,
    pagination: cosmos.base.query.v1beta1.PageRequest.fromPartial({offset: offset, limit: limit})} )
  );
  return response;
}

// RawContractState gets single key from the raw store data of a contract
export async function query_wasm_rawContractState(
  client: PersistenceClient,
  address: string,
  queryData: Uint8Array
) {
  let response = await client.query.cosmwasm.wasm.v1.rawContractState(
    cosmwasm.wasm.v1.QueryRawContractStateRequest.fromPartial({ address: address, queryData: queryData })
  );
  return response;
}

// SmartContractState get smart query result from the contract
export async function query_wasm_smartContractState(
  client: PersistenceClient,
  address: string,
  queryData: Uint8Array
) {
  let response = await client.query.cosmwasm.wasm.v1.smartContractState(
    cosmwasm.wasm.v1.QuerySmartContractStateRequest.fromPartial({ address: address, queryData: queryData })
  );
  return response;
}

// Code gets the binary code and metadata for a singe wasm code
export async function query_wasm_code(
  client: PersistenceClient,
  code_id: number
) {
  let response = await client.query.cosmwasm.wasm.v1.code(
    cosmwasm.wasm.v1.QueryCodeRequest.fromPartial({ codeId: code_id })
  );
  return response;
}

// Code gets the binary code and metadata for a singe wasm code
export async function query_wasm_codes(
  client: PersistenceClient,
  offset?: number,
  limit?: number
) {
  let response = await client.query.cosmwasm.wasm.v1.codes(
    cosmwasm.wasm.v1.QueryCodesRequest.fromPartial({ pagination: cosmos.base.query.v1beta1.PageRequest.fromPartial({offset: offset, limit: limit}) })
  );
  return response;
}

// PinnedCodes gets the pinned code ids
export async function query_wasm_pinnedCodes(
  client: PersistenceClient,
  offset?: number,
  limit?: number
) {
  let response = await client.query.cosmwasm.wasm.v1.pinnedCodes(
    cosmwasm.wasm.v1.QueryPinnedCodesRequest.fromPartial({ pagination: cosmos.base.query.v1beta1.PageRequest.fromPartial({offset: offset, limit: limit}) })
  );
  return response;
}


// Return list of codeIds with codeInfos having datahash for the contract
export async function find_code_id_from_contract_hash(
  client: PersistenceClient,
  hash: string,
  offset?: number,
  limit?: number
) {
  let codes = await query_wasm_codes(client, offset, limit);
  let codeInfos = codes["codeInfos"];

  for (let i = 0; i < codeInfos.length; i++) {
    let hex = Buffer.from(codeInfos[i]["dataHash"]).toString("hex");
    let code_id = codeInfos[i]["codeId"];
    console.log(` code_id = ${code_id} hex = ${hex}`);
    if (hash == hex) {
      return code_id;
    }

  }
  return 0;
}


// --------x-------------x-------------x----- -----x-----------------x-----------
// --------x----- GOV MODULE :::: QUERIES :: Helpers Functions -----x-----------
// --------x-------------x-------------x----- -----x-----------------x-----------


// Proposal queries proposal details based on ProposalID
  // PROPOSAL_STATUS_UNSPECIFIED = 0
  // PROPOSAL_STATUS_DEPOSIT_PERIOD = 1
  // PROPOSAL_STATUS_VOTING_PERIOD = 2
  // PROPOSAL_STATUS_PASSED = 3
  // PROPOSAL_STATUS_REJECTED = 4
  // PROPOSAL_STATUS_FAILED = 5
export async function query_gov_proposal(
  client: PersistenceClient,
  proposalId: number
) {
  let response = await client.query.cosmos.gov.v1beta1.proposal(
    cosmos.gov.v1beta1.QueryProposalRequest.fromPartial({proposalId: proposalId}) 
  );
  return response;
}

// Proposals queries all proposals based on given status.
export async function query_gov_proposals(
  client: PersistenceClient,
  proposalStatus: number,
  voter: string,
  depositor: string,
  limit?: number,
  offset?:number 
) {
  let response = await client.query.cosmos.gov.v1beta1.proposals(
    cosmos.gov.v1beta1.QueryProposalsRequest.fromPartial({ 
      proposalStatus: cosmos.gov.v1beta1.proposalStatusFromJSON(proposalStatus),
      voter: voter,
      depositor: depositor,
      pagination: cosmos.base.query.v1beta1.PageRequest.fromPartial({offset: offset, limit: limit})
     })
  );
  return response;
}

// Vote queries voted information based on proposalID, voterAddr
export async function query_gov_vote(
  client: PersistenceClient,
  proposalId: number,
  voter: string
) {
  let response = await client.query.cosmos.gov.v1beta1.vote(
    cosmos.gov.v1beta1.QueryVoteRequest.fromPartial({ proposalId: proposalId, voter: voter })
  );
  return response;
}

// Votes queries votes of a given proposal.
export async function query_gov_votes(
  client: PersistenceClient,
  proposalId: number,
  offset?: number,
  limit?: number
) {
  let response = await client.query.cosmos.gov.v1beta1.votes(
    cosmos.gov.v1beta1.QueryVotesRequest.fromPartial({ proposalId: proposalId, 
      pagination: cosmos.base.query.v1beta1.PageRequest.fromPartial({offset: offset, limit: limit})
     })
  );
  return response;
}

// Params queries all parameters of the gov module.
// params_type defines which parameters to query for, can be one of "voting", "tallying" or "deposit".
export async function query_gov_params(
  client: PersistenceClient,
  paramsType: string
) {
  let response = await client.query.cosmos.gov.v1beta1.params(
    cosmos.gov.v1beta1.QueryParamsRequest.fromPartial({ paramsType: paramsType })
  );
  return response;
}

// Deposit queries single deposit information based proposalID, depositAddr.
export async function query_gov_deposit(
  client: PersistenceClient,
  proposalId: number,
  depositor: string
) {
  let response = await client.query.cosmos.gov.v1beta1.deposit(
    cosmos.gov.v1beta1.QueryDepositRequest.fromPartial({ proposalId: proposalId, depositor: depositor })
  );
  return response;
}

// Deposits queries all deposits of a single proposal.
export async function query_gov_deposits(
  client: PersistenceClient,
  proposalId: number,
  offset?: number,
  limit?: number
) {
  let response = await client.query.cosmos.gov.v1beta1.deposits(
    cosmos.gov.v1beta1.QueryDepositsRequest.fromPartial({  proposalId: proposalId, 
      pagination: cosmos.base.query.v1beta1.PageRequest.fromPartial({offset: offset, limit: limit}) })
  );
  return response;
}

// TallyResult queries the tally of a proposal vote.
export async function query_gov_tallyresult(
  client: PersistenceClient,
  proposalId: number,
) {
  let response = await client.query.cosmos.gov.v1beta1.tallyResult(
    cosmos.gov.v1beta1.QueryTallyResultRequest.fromPartial({ proposalId: proposalId })
  );
  return response;
}


// --------x-------------x-------------x----- -----x--------------x-----------
// --------x----- EXECUTE CONTRACT :: Helpers Functions      -----x-----------
// --------x-------------x-------------x----- -----x--------------x-----------

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

// --------x-------------x-------------x----- -----x---------
// --------x----- Governance Module Helpers -----x-----------
// --------x-------------x-------------x----- -----x---------

/// GOV MODULE -- SUBMIT PROPOSAL ExecuteMssg
/// Network : 512 XPRT are required as deposit to deploy the contract
/// Tokens transferred - initialDeposit amount is transferred from user a/c to the module address
export async function Gov_MsgSubmitProposal(
  client: PersistenceClient,
  proposal: { typeUrl?: string; value?: Uint8Array },
  denom: string,
  deposit: number
) {
  let signer = client.config;
  const [account] = await client.wallet.getAccounts();
  //submit proposal Msg
  const proposalMsg = {
    typeUrl: "/cosmos.gov.v1beta1.MsgSubmitProposal",
    value: {
      content: Any.fromPartial(proposal),
      initialDeposit: coins(deposit, denom),
      proposer: account.address,
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
    console.log(`Proposal Id is = ${proposalId} `);
    return json; //return json response, for proposalID use json[0].events[3].attributes[1].value
  } else {
    return res.rawLog;
  }
}

/// GOV MODULE -- Deposit with Propsoal - ExecuteMssg
export async function Gov_MsgDeposit(
  client: PersistenceClient,
  proposal_id: number,
  amount: number,
  denom: string
) {
  const [account] = await client.wallet.getAccounts();
  // Deposit with Propsoal
  const depositMsg = {
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
    [depositMsg],
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

/// GOV MODULE -- VOTE on Propsoal - ExecuteMssg
export async function voteOnProposal(
  client: PersistenceClient,
  proposalid: number,
  vote: number,
  denom: string
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
    { amount: coins(10_000_000, denom), gas: "2000000" }
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









