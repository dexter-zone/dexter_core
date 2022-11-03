## **Dexter :: Architecture Overview**

Dexter is the first DEX which is implemented as a generalized state transition executor where the transition’s math computes are queried from the respective Pool contracts, enabling a decentralized, non-custodial aggregated liquidity and exchange rate discovery among different tokens on Persistence.

![Dexter :: Architecture Overview](./docs/overview.png)

### Scope

At launch, it will support the Stable-5-Pool type (introduced by curve) and the Weighted pool type (introduced by balancer), each of which designed for the following objectives,

- **Stable-5-Pool -::-** Can be leveraged to develop specialized pools for liquid staking assets which can provide best trade execution.
- **Weighted Pool -::-** Can be leveraged to develop re-balancing driven pool strategies which can maximize LP returns.

Dexter generator (modified version of astroport generator) can be used to incentivize pool’s LP tokens with dual rewards, i.e the DEX’s token (not live at launch) and a proxy reward incentive that is given by any 3rd party staking contract.

- Custom generator proxy adapter contracts need to be written for these 3rd party staking contracts to add them to the generator.

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
