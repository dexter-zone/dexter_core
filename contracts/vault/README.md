# Dexter Vault

The vault contract can create new Dexter pair contracts (and associated LP token contracts), provide and withdraw liquidity functions and it is used as a directory for all pairs. The default pair types are constant product and stableswap but governance may decide to add custom pools that can have any implementation.


---

## InstantiateMsg

The instantiation message takes in the token code ID for the token type supported on Dexter. It also takes in the `fee_collector` that collects fees for governance, the contract `owner`, the Generator contract address and the initial pool types available to create.

```json
{
  "lp_token_code_id": 123,
  "fee_collector": "terra...",
  "owner": "terra...",
  "generator_address": "terra...",
  "pool_configs": [{
      "code_id": 123,
      "pool_type": {
        "xyk": {}
      },
      "fee_info": {
        "total_fee_bps": 0.5,
        "protocol_fee_percent": 1,
        "dev_fee_percent": 1,
        "developer_addr": "terra...",
        },
      "is_disabled": false,
      "is_generator_disabled": false,
    }
  ]
}
```
## ExecuteMsg

### `update_config`

Updates contract variables, namely the code ID of the token implementation used in dexter, the address that receives governance fees and the Generator contract address.

```json
{
  "update_config": {
    "lp_token_code_id": 123,
    "fee_collector": "terra...",
    "generator_address": "terra..."
  }
}
```

