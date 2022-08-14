# Dexter - Keeper Contract

Dexter Keeper contract keeps account for all the protocol fees collected by the Dexter Vault. Fee charged during swaps by the Dexter Vault is transferred to the keeper contract

## Supported Execute Messages

| Message                    | Description                                                                                         |
| -------------------------- | --------------------------------------------------------------------------------------------------- |
| `ExecuteMsg::UpdateConfig` | Executable only by Dexter Vault's owner. Facilitates setting DEX token contract or staking contract |

## Supported Query Messages

| Message                                        | Description                                                                          |
| ---------------------------------------------- | ------------------------------------------------------------------------------------ |
| `QueryMsg::Config()`                           | Returns information about the Keeper configs that contains in the [`ConfigResponse`] |
| `QueryMsg::Balances( assets: Vec<AssetInfo> )` | Returns the balance for each asset in the specified input parameters                 |

## Enums & Structs

### `ConfigResponse` struct - This struct is returned by QueryMsg::Config

```
struct ConfigResponse {
    /// The DEX token contract address
    pub dex_token_contract: Option<Addr>,
    /// The vault contract address
    pub vault_contract: Addr,
    /// The DEX token staking contract address
    pub staking_contract: Option<Addr>,
}
```

### `BalancesResponse` struct - This struct is returned by QueryMsg::Balances and contains asset balances as held by the Keeper contract

```
struct BalancesResponse {
    pub balances: Vec<Asset>,
}
```
