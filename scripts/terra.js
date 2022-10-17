import * as Pako from "pako";
import * as fs from "fs";
// import {
//   Gov_MsgSubmitProposal,
//   voteOnProposal,
//   readArtifact,
//   writeArtifact,
//   executeContract,
//   query_gov_params,
//   query_gov_proposal,
//   find_code_id_from_contract_hash,
//   query_wasm_contractsByCode,
//   toEncodedBinary,
//   index_dexter_create_pool_tx,
//   query_wasm_code,
// } from "./helpers/helpers.js";
// import { toBinary } from "@cosmjs/cosmwasm-stargate";
// import { Slip10RawIndex, pathToString, stringToPath } from "@cosmjs/crypto";
// import { CosmosChainClient, cosmwasm } from "cosmossdkjs";
import { LCDClient, Coin, MnemonicKey } from "@terra-money/terra.js";

// type Proposal = {
//   id: Number,
//   content: Dictionary,
//   status: proposal.status,
//   final_tally_result: proposal.final_tally_result,
//   submit_time: proposal.submit_time,
//   deposit_end_time: proposal.deposit_end_time,
//   total_deposit: proposal.total_deposit,
//   voting_start_time: proposal.voting_start_time,
//   voting_end_time: proposal.voting_end_time,
// };

async function Demo() {
  // Using a random generated mnemonic
  // const devnet_mnemonic = "opinion knife other balcony surge more bamboo canoe romance ask argue teach anxiety adjust spike mystery wolf alone torch tail six decide wash alley";
  const testnet_mnemonic =
    "toss hammer lazy dish they ritual suggest favorite sword alcohol enact enforce mechanic spoon gather knock giggle indicate indicate nose actor brand basket confirm";
  // const localnet_mnemonic = "gravity bus kingdom auto limit gate humble abstract reopen resemble awkward cannon maximum bread balance insane banana maple screen mimic cluster pigeon badge walnut";
  const deposit_amount = 512_000_000;
  const fee_denom = "uluna";
  const CHAIN_ID = "columbus-5"; // "persistencecore" "test-core-1" ; // "testing";

  // network : stores contract addresses
  // let network = readArtifact(CHAIN_ID);
  // let testnetWalletOptions = {
  //   bip39Password: "",
  //   hdPaths: [stringToPath("m/44'/118'/0'/0/0")],
  //   prefix: "persistence",
  // };

  // connect to columbus-5 terra classic network
  const terra = new LCDClient({
    URL: "https://columbus-lcd.terra.dev",
    chainID: "columbus-5",
    isClassic: true, // *set to true to connect terra-classic chain*
  });
  const mk = new MnemonicKey({
    mnemonic:
      "notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius",
  });
  const wallet = terra.wallet(mk);
  // Get wallet address
  // const [Account] = await terra.wallet.getAccounts();
  // const wallet_address = Account.address;
  // console.log(`WALLET ADDRESS =  ${wallet_address}`);
  // const OWNER = wallet_address;

  // Get chain height
  // const balance = await terra.bank.balance(
  //   "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v"
  // );
  let cur_id = 1;
  let propsoals_list = [];

  // fs.createReadStream("./final_astro_airdrop_numbers.csv")
  //   .pipe(csv())
  //   .on("data", (data) => airdrop_recepients.push(data))
  //   .on("end", () => {
  while (cur_id < 5000) {
    try {
      let proposal = await terra.gov.proposal(cur_id);
      cur_id++;
      // let proposal_json = JSON.stringify(proposal);
      console.log(proposal.id);
      // console.log(proposal.content);
      let proposal_json = {
        id: proposal.id,
        content: proposal.content,
        status: proposal.status,
        final_tally_result: proposal.final_tally_result,
        submit_time: proposal.submit_time,
        deposit_end_time: proposal.deposit_end_time,
        total_deposit: proposal.total_deposit,
        voting_start_time: proposal.voting_start_time,
        voting_end_time: proposal.voting_end_time,
      };
      console.log(proposal_json);
      propsoals_list.push(proposal_json);
      // console.log("\n");
      // let json = JSON.stringify(propsoals_list);
      // fs.writeFile("proposals.json", json, function (err) {});
    } catch (e) {
      console.log(`Error: ${cur_id}`);
      cur_id++;
      continue;
    }

    fs.writeFileSync("final_proposals.json", JSON.stringify(propsoals_list));
  }

  fs.writeFileSync("final_proposals.json", JSON.stringify(propsoals_list));
  // });

  // console.log(proposal);
  // console.log(`Blockchain height = ${height}`);
}

Demo().catch(console.log);
