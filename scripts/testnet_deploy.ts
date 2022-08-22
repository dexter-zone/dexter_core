import { PersistenceClient, cosmwasm } from "persistenceonejs";
import * as Pako from "pako";
import * as fs from "fs";
import { Gov_MsgSubmitProposal } from "./helpers/helpers.js";
import sha256 from "crypto-js/sha256.js";
import { toBinary } from "@cosmjs/cosmwasm-stargate";

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

  let codes = await client.query.cosmwasm.wasm.v1.codes({});
  // console.log(`CODES = ${JSON.stringify(codes)}`);
  // console.log(codes[0]);

  let codeInfos = codes["codeInfos"];
  let pagination = codes["pagination"];

  for (let i = 0; i < codeInfos.length; i++) {
    let hex = Buffer.from(codeInfos[i]["dataHash"]).toString("hex");
    let code_id = codeInfos[i]["codeId"];
    console.log(` code_id = ${code_id} hex = ${hex}`);
    // let str = Utf8ArrayToStr(codeInfos[i]["dataHash"]);
    // let str = String.fromCharCode.apply(null, codeInfos[i]["dataHash"]);
    // console.log(codeInfos[i]["dataHash"]);
    // console.log(hex);
  }

  console.log(pagination);

  // return;

  // -----------x-------------x-------------x-----
  // ----------- CONTRACT DEPLOYMENT -------------
  // -----------x-------------x-------------x-----

  // CONTRACTS WHICH ARE TO BE DEPLOYED ON PERSISTENCE ONE NETWORK FOR DEXTER PROTOCOL
  // let contracts = [
  // { name: "Dexter Vault", path: "../artifacts/dexter_vault.wasm" }, // 14e717077b44530459c66607788069428b7bd15dd248bf1ff72b0c4d17a203d5
  // { name: "Dexter Keeper", path: "../artifacts/dexter_keeper.wasm" }, // 51de835121fcfa4fd772d68889b98294c789bc2a50a80a9132fead99b755e0c3
  // { name: "LP Token", path: "../artifacts/lp_token.wasm" },
  // { name: "XYK Pool", path: "../artifacts/xyk_pool.wasm" },
  // { name: "Weighted Pool", path: "../artifacts/weighted_pool.wasm" },
  // { name: "Stableswap Pool", path: "../artifacts/stableswap_pool.wasm" },
  // { name: "Stable5Swap Pool", path: "../artifacts/stable5pool.wasm" },
  // { name: "Dexter Vesting", path: "../artifacts/dexter_vesting.wasm" },
  // { name: "Dexter Generator", path: "../artifacts/dexter_generator.wasm" },
  // {
  //   name: "Dexter Generator : Proxy",
  //   path: "../artifacts/dexter_generator_proxy.wasm",
  // },
  // { name: "Staking contract", path: "../artifacts/anchor_staking.wasm" },
  // ];

  // LOOP -::- CREATE PROTOCOLS FOR EACH CONTRACT ON-CHAIN
  // for (let i = 0; i < contracts.length; i++) {
  //   let contract_name = contracts[i]["name"];
  //   let contract_path = contracts[i]["path"];

  //   try {
  //     console.log(
  //       `\nSubmitting Proposal to deploy ${contract_name} Contract ...`
  //     );
  // const wasm = fs.readFileSync(contract_path);
  // const wasmStoreProposal = {
  //   typeUrl: "/cosmwasm.wasm.v1.StoreCodeProposal",
  //   value: Uint8Array.from(
  //     cosmwasm.wasm.v1.StoreCodeProposal.encode(
  //       cosmwasm.wasm.v1.StoreCodeProposal.fromPartial({
  //         title: contract_name,
  //         description: `Add wasm code for ${contract_name} contract.`,
  //         runAs: wallet_address,
  //         wasmByteCode: Pako.gzip(wasm, { level: 9 }),
  //         instantiatePermission: {
  //           permission: cosmwasm.wasm.v1.accessTypeFromJSON(1),
  //         },
  //       })
  //     ).finish()
  //   ),
  // };
  //     const res = await Gov_MsgSubmitProposal(client, wasmStoreProposal);
  //     let proposalId = res[0].events[3].attributes[1].value;
  //     // const json = JSON.parse(res.rawLog?);
  //     console.log(res);
  //   } catch (e) {
  //     console.log("Proposal Error has occoured => ", e);
  //   }
  // }

  // -----------x-------------x-------------x---------x---------------
  // ----------- CONTRACT INSTIANTIATION :: DEXTER VAULT -------------
  // -----------x-------------x-------------x---------x---------------

  // CONTRACT IDs
  let vault_code_id = 6;
  let lp_token_code_id = 8;
  let xyk_pool_code_id = 9;
  let stableswap_pool_code_id = 11;
  // let stable5swap_pool_code_id = 1;
  let weighted_pool_code_id = 10;
  let keeper_code_id = 7;

  // INSTIANTIATING CONTRACTS -::- DEXTER VAULT
  let init_msg = {
    pool_configs: [
      {
        code_id: xyk_pool_code_id,
        pool_type: { xyk: {} },
        fee_info: {
          total_fee_bps: "300",
          protocol_fee_percent: "49",
          dev_fee_percent: "15",
          developer_addr: wallet_address,
        },
        is_disabled: false,
        is_generator_disabled: false,
      },
      {
        code_id: stableswap_pool_code_id,
        pool_type: { stable_2_pool: {} },
        fee_info: {
          total_fee_bps: "300",
          protocol_fee_percent: "49",
          dev_fee_percent: "15",
          developer_addr: null,
        },
        is_disabled: false,
        is_generator_disabled: false,
      },
      {
        code_id: weighted_pool_code_id,
        pool_type: { weighted: {} },
        fee_info: {
          total_fee_bps: "300",
          protocol_fee_percent: "49",
          dev_fee_percent: "15",
          developer_addr: null,
        },
        is_disabled: false,
        is_generator_disabled: false,
      },
    ],
    lp_token_code_id: lp_token_code_id.toString(),
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
            codeId: vault_code_id.toString(),
            label: "Dexter Vault",
            msg: Buffer.from(JSON.stringify(init_msg)),
          })
        ).finish()
      ),
    };
    const res = await Gov_MsgSubmitProposal(client, wasmInstantiateProposal);
    let proposalId = res[0].events[3].attributes[1].value;
    // const json = JSON.parse(res.rawLog?);
    console.log(res);
  } catch (e) {
    console.log("Proposal Error has occoured => ", e);
  }

  // -----------x-------------x-------------x---------x---------------
  // ----------- CONTRACT INSTIANTIATION :: DEXTER KEEPER -------------
  // -----------x-------------x-------------x---------x---------------

  // INSTIANTIATING CONTRACTS -::- DEXTER VAULT
  // let keeper_init_msg = {
  //   vault_contract: vault_contract_address,
  // };
  // try {
  // const wasmInstantiateProposal = {
  //   typeUrl: "/cosmwasm.wasm.v1.MsgInstantiateContract",
  //   value: Uint8Array.from(
  //     cosmwasm.wasm.v1.MsgInstantiateContract.encode(
  //       cosmwasm.wasm.v1.MsgInstantiateContract.fromPartial({
  //         sender: wallet_address,
  //         admin: wallet_address,
  //         codeId: keeper_code_id,
  //         label: "Dexter Vault",
  //         msg: Buffer.from(JSON.stringify(keeper_init_msg)),
  //       })
  //     ).finish()
  //   ),
  // };
  // const res = await Gov_MsgSubmitProposal(client, wasmInstantiateProposal);
  //     let proposalId = res[0].events[3].attributes[1].value;
  //     // const json = JSON.parse(res.rawLog?);
  //     console.log(res);
  //   } catch (e) {
  //     console.log("Proposal Error has occoured => ", e);
  //   }
  // }

  //     let proposalId = res[0].events[3].attributes[1].value;
  //     // const json = JSON.parse(res.rawLog?);
  //     // console.log(res);
  //   } catch (e) {
  //     console.log("Proposal Error has occoured => ", e);
  //   }
  // }

  // try {
  //   console.log(
  //     `\nSubmitting Proposal to instantiate Dexter Vault Contract ...`
  //   );
  //   let init_msg = {};
}

Demo().catch(console.log);
