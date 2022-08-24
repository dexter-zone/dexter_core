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

3. Install Node libraries required:

```bash
cd scripts
npm install
```

3. Deploy scripts:

```
Persistence testnet: https://rpc.testnet.persistence.one:443, test-core-1
Persistence mainnet: https://rpc.persistence.one:443, core-1
```

This is currently a WIP.

```bash
node --experimental-json-modules --loader ts-node/esm testnet_deploy.ts
```

4. Persistence network MAINNET details -
   export CHAIN_ID="core-1"
   export RPC_CLIENT_URL="https://rpc.persistence.one:443"

5. Persistence network TESTNET details -
   export CHAIN_ID="test-core-1"
   export RPC_CLIENT_URL=" https://rpc.testnet.persistence.one:443"

### Compile

Make sure the current working directory is set to the root directory of this repository, then

```bash
cargo build
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/workspace-optimizer:0.12.6
```

### PERSISTENCE TESTNET INSTANCE

| Name                       | Code Id | Instantiated Address                                                   |
| -------------------------- | ------- | ---------------------------------------------------------------------- |
| `Dexter Vault`             | 6       | persistence1fyr2mptjswz4w6xmgnpgm93x0q4s4wdl6srv3rtz3utc4f6fmxeqm56xzf |
| `Dexter Keeper`            | 7       | -                                                                      |
| `LP Token :: Test token 1` | 8       | persistence1vguuxez2h5ekltfj9gjd62fs5k4rl2zy5hfrncasykzw08rezpfst7tmng |
| `LP Token :: Test token 2` | 8       | persistence1rl8su3hadqqq2v86lscpuklsh2mh84cxqvjdew4jt9yd07dzekyq85jyzr |
| `LP Token :: Test token 3` | 8       | persistence1vhjnzk9ly03dugffvzfcwgry4dgc8x0sv0nqqtfxj3ajn7rn5ghqtpaner |
| `XYK Pool`                 | 9       | -                                                                      |
| `Weighted Pool`            | 10      | -                                                                      |
| `Stableswap Pool`          | 11      | -                                                                      |
| `Dexter Vesting`           | 12      | -                                                                      |
| `Dexter Generator`         | 13      | -                                                                      |
| `Dexter Generator : Proxy` | 14      | -                                                                      |
| `Example Staking contract` | 15      | -                                                                      |
| `Stable-5-Swap pool`       | 16      | -                                                                      |

code_id = 9 hex = 0da5bfdcf360057b9cfd0a37c3a149310b03c6afe0c219bc931169948defcf4e--> XYK Pool
code_id = 10 hex = 19cbeb7d8a7eb3678982c92e7792276b9a4ecc45cba3ba0a412014508e11a0ee--> Weighted Pool
code_id = 11 hex = be5602ebf73c54f781a4d9d001fff127231c0d8444d22e2b5bb3b0d2f8e9a79b--> Stableswap Pool
code_id = 12 hex = 9fed0b82283c3881c242cc51d80c1d9b73fb8fd038da726d6f850de2736a253f--> Dexter Vesting
code_id = 13 hex = d83433369379a5cec32b7ea4de7574222964ddec9f70d64c7775acbb1e008747--> Dexter Generator
code_id = 14 hex = 9764f035f5daa0215c7fcbcf4774403732c432077cddb34566156d17ff9dd8e2--> Dexter Generator : Proxy
code_id = 15 hex = ee159a69576fed23ca7521ecbc7919206be1d619c57da37d846e061b2580614d--> Example Staking contract
