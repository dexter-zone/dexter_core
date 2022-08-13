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
  "pool_configs": [
    {
      "code_id": 123,
      "pool_type": {
        "xyk": {}
      },
      "fee_info": {
        "total_fee_bps": 0.5,
        "protocol_fee_percent": 1,
        "dev_fee_percent": 1,
        "developer_addr": "terra..."
      },
      "is_disabled": false,
      "is_generator_disabled": false
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

# Lockdrop

The lockdrop contract allows users to lock any of the supported Terraswap LP tokens locked for a selected duration against which they will receive ASTRO tokens pro-rata to their weighted share of the LP tokens to the total deposited LP tokens for that particular pool in the contract.

- Upon lockup expiration, users will receive Astroport LP tokens on an equivalent weight basis as per their initial Terraswap LP token deposits.

Note - Users can open muliple lockup positions with different lockup duration for each LP Token pool

## Contract Design

### Handle Messages

| Message                             | Description                                                                                                                                                                                                                                                                                                                |
| ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ExecuteMsg::UpdateConfig`          | Executable only by `config.owner`. Facilitates updating `config.fee_collector`, `config.generator_address`, `config.lp_token_code_id` parameters.                                                                                                                                                                          |
| `ExecuteMsg::UpdatePoolConfig`      | Executable only by `pool_config.fee_info.developer_addr` or `config.owner` if its not set. Facilitates enabling / disabling new pool instances creation (`pool_config.is_disabled`) , and updating Fee (` pool_config.fee_info`) for new pool instances                                                                    |
| `ExecuteMsg::CreatePool`            | Admin function to update any configuraton parameter for a terraswap pool whose LP tokens are currently accepted for the lockdrop                                                                                                                                                                                           |
| `ExecuteMsg::JoinPool`              | Facilitates opening a new user position or adding to an existing position                                                                                                                                                                                                                                                  |
| `ExecuteMsg::Swap`                  | Admin function to increase the ASTRO incentives that are to be distributed                                                                                                                                                                                                                                                 |
| `ExecuteMsg::ProposeNewOwner`       | Facilitates LP token withdrawals from lockup positions by users. 100% amount can be withdrawn during deposit window, which is then limited to 50% during 1st half of deposit window which then decreases linearly during 2nd half of deposit window. Only 1 withdrawal can be made by a user during the withdrawal windows |
| `ExecuteMsg::DropOwnershipProposal` | Admin function. Facilitates migration of liquidity (locked terraswap LP tokens) from Terraswap to Astroport (Astroport LP tokens)                                                                                                                                                                                          |
| `ExecuteMsg::ClaimOwnership`        | Admin function. Facilitates staking of Astroport LP tokens for a particular LP pool with the generator contract                                                                                                                                                                                                            |

### Query Messages

| Message                                  | Description                                                                                            |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `QueryMsg::Config()`                     | Returns the stored Vault Configuration settings in custom [`ConfigResponse`] structure                 |
| `QueryMsg::PoolConfig([`PoolType`])`     | Returns the provided [`PoolType`]'s Configuration settings in custom [`PoolConfigResponse`] structure  |
| `QueryMsg::GetPoolById([`Uint128`])`     | Returns the current stored state of pool with the provided ID in custom [`PoolInfoResponse`] structure |
| `QueryMsg::GetPoolByAddress([`String`])` | Returns the current stored state of the Pool in custom [`PoolInfoResponse`] structure                  |

## Build schema and run unit-tests

```
cargo schema
cargo test
```

## License

TBD
