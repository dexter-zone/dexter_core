## **Dexter :: Architecture Overview**

Dexter is the first DEX which is implemented as a generalized state transition executor where the transition’s math computes are queried from the respective Pool contracts, enabling a decentralized, non-custodial aggregated liquidity and exchange rate discovery among different tokens on Persistence.

![Dexter :: Architecture Overview](./docs/overview.png)

At launch, Dexter will be supporting the following pool types,

- XYK Pool
- Stableswap Pool
- Stable5Pool
- Weighted Pool

## Development

### Dependencies

- Rust v1.44.1+
- `wasm32-unknown-unknown` target
- Docker
- [LocalTerra](https://github.com/terra-project/LocalTerra)
- Node.js v16

### Envrionment Setup

1. Install `rustup` via https://rustup.rs/

2. Add `wasm32-unknown-unknown` target

```sh
rustup default stable
rustup target add wasm32-unknown-unknown
```

3. Compile contracts and generate wasm builds. Make sure the current working directory is set to the root directory of this repository, then

```bash
cargo build
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/workspace-optimizer:0.12.6
```

4. Install Node libraries required:

```bash
cd scripts
npm install
```

5. Deploy scripts:

Persistence is a permissioned network. You will have to wait for the proposals to pass and re-run scripts so its partly a manual process. We have shared addresses for our dexter instance deployment on testnet which you can interact with to test pool functions.

```
Persistence testnet: https://rpc.testnet.persistence.one:443, test-core-1
Persistence mainnet: https://rpc.persistence.one:443, core-1
```

```bash
node --experimental-json-modules --loader ts-node/esm testnet_deploy.ts
```

### Persistence Network Endpoints

| Network Name | Chain Id    | RPC Endpoint                              | LCD Endpoint                          |
| ------------ | ----------- | ----------------------------------------- | ------------------------------------- |
| `Mainnet`    | core-1      | "https://rpc.persistence.one:443"         | "http://rest.persistence.one"         |
| `Testnet`    | test-core-1 | "https://rpc.testnet.persistence.one:443" | "http://rest.testnet.persistence.one" |

<br>
### Interacting with Persistence Network (via JS / TS) :

- You can use `cosmossdkjs` to interact with dexter protocol on Persistence network.

```
npm install cosmossdkjs
```

- Create `CosmosChainClient` instance to interact with chain.

```
import { CosmosChainClient, cosmwasm } from "cosmossdkjs";


   const CHAIN_ID = "test-core-1";
   const fee_denom = "uxprt";
   const rpcEndpoint = "https://rpc.testnet.persistence.one:443";

  const client = await CosmosChainClient.init(MNEMONIC , {
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

  // Get wallet address
  const [Account] = await client.wallet.getAccounts();
  const wallet_address = Account.address;
  console.log(`WALLET ADDRESS =  ${wallet_address}`);

  // Get chain height
  const height = await client.wasm.getHeight();
  console.log(`Blockchain height = ${height}`);

  // Get xprt balance
  const balance_res = await client.wasm.getBalance( wallet_address, fee_denom);
  let wallet_balance = Number(balance_res["amount"])/10**6;
  console.log(`Wallet's XPRT balance = ${wallet_balance}`);

   // Execute contract - Create pool
  let create_pool_exec_msg = {
     create_pool_instance: {
       pool_type: { xyk: {} },
       asset_infos: [
         { native_token: { denom: fee_denom } },
         { token: { contract_addr: network.test_tokens_addresses[0] } },
       ],
     },
   };   const res = await client.wasm.execute(
      wallet_address,
      contract_address,
      msg,
      { amount: coins(2_000_000, "uxprt"), gas: "200000" },
      memo,
      funds
    );
    let txhash = res["transactionHash"];
    console.log(`Tx executed -- ${txhash}`);
```

- You can find more examples [here](https://github.com/dexter-zone/dexter_core/blob/main/scripts/helpers/helpers.ts)

<br>
<br>

### Interacting with Persistence Network (via Python) :

- You can use `cosmos_SDK` to interact with dexter protocol on Persistence network. cosmos_SDK is a python package forked from terra_SDK and is being repurposed to be able to support multiple cosmos blockchains. You can refer to terra_SDK's documentation which should work in for cosmos_SDK too.

```
pip install -U cosmos_SDK
```

- Create `LCDClient` instance to interact with chain.

```
from cosmos_sdk.client.lcd import LCDClient
from cosmos_sdk.key.mnemonic import MnemonicKey

mnemonic = <MNEMONIC_PHRASE>

# Persistence client
persistence_client = LCDClient(chain_id="core-1", url="http://rest.core.persistence.one")
mk = MnemonicKey(mnemonic,"persistence")
persistence_wallet = persistence_client.wallet(mk)

# Query a contract
config_response = client.wasm.contract_query(contract_addr , {"config":{}})
print(config_response)
```

- `cosmos_SDK` is a W.I.P and execute functions are not working atm.

### Persistence :: TESTNET INSTANCE

Refer to artifacts/test_core-1.json to get the list of proposal ids and addresses with the dexter deployment on Persistence testnet.

| Name                       | Code Id | Instantiated Address                                                   |
| -------------------------- | ------- | ---------------------------------------------------------------------- |
| `Dexter Vault`             | 6       | persistence1jyhyqjxf3pc7vzwyqhwe53up5pj0e53zw3xu2589uqgkvqngswnqgrmstf |
| `Dexter Keeper`            | 7       | -                                                                      |
| `LP Token :: Test token 1` | 8       | persistence1vguuxez2h5ekltfj9gjd62fs5k4rl2zy5hfrncasykzw08rezpfst7tmng |
| `LP Token :: Test token 2` | 8       | persistence1rl8su3hadqqq2v86lscpuklsh2mh84cxqvjdew4jt9yd07dzekyq85jyzr |
| `LP Token :: Test token 3` | 8       | persistence1vhjnzk9ly03dugffvzfcwgry4dgc8x0sv0nqqtfxj3ajn7rn5ghqtpaner |
| `XYK Pool`                 | 9       | persistence1lxansfc8vkujy997e3xksd3ugsppv6a9jt32pjtgaxr0zkcnkznqu22a4s |
| `Weighted Pool`            | 10      | persistence1j5h5zftg5su7ytz74f7rryl4f6x3p78lh907fw39eqhax75r94jsgj4n54 |
| `Stableswap Pool`          | 11      | persistence1kkwp7pd4ts6gukm3e820kyftz4vv5jqtmal8pwqezrnq2ddycqas9nk2dh |
| `Dexter Vesting`           | 12      | -                                                                      |
| `Dexter Generator`         | 13      | -                                                                      |
| `Dexter Generator : Proxy` | 14      | -                                                                      |
| `Example Staking contract` | 15      | -                                                                      |
| `Stable-5-Swap pool`       | 16      | persistence1a7pjjyvng22a8msatp4zj6ut9tmsd9qvp26gaj7tnrjrqtx7yafqm7ezny |
