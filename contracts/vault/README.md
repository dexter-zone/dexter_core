# DEXTER Protocol -::- Vault Contract

The Vault is the core of Dexter; it is a smart contract that holds and manages all tokens in each Dexter Pool. It is also the portal through which most Dexter operations (swaps/joins/exits) take place.

THe Vault owner (the Dexter DAO) can add new pool types to the Dexter Registery.
The Vault charges swap fee on swaps that take place in Dexter pools and the fee is transferred to the Keeper contract.

## Contract State

| Message              | Description                                                                                                    |
| -------------------- | -------------------------------------------------------------------------------------------------------------- |
| `CONFIG`             | Stores Vault contract's core Configuration parameters in a [`Config`] struct                                   |
| `REGISTERY`          | Stores configuration data associated with each [`PoolType`] supported by the Vault in a [`PoolConfig`] struct  |
| `ACTIVE_POOLS`       | Stores current state of each Pool instance identified by its ID supported by the Vault in a [`PoolInfo`] struc |
| `OWNERSHIP_PROPOSAL` | Ownership Proposal currently active in the Vault in a [`OwnershipProposal`] struc                              |
| `TMP_POOL_INFO`      | Temporarily stores the PoolInfo of the Pool which is currently being created in a [`PoolInfo`] struc           |

---

- **Separating Token Accounting and Pool Logic**

  The Dexter's Vault architecture is inspired by the Balancer's Vault and similarly separates the token accounting and management from the pool logic. This separation simplifies pool contracts, since they no longer need to actively manage their assets; pools only need to calculate amounts for swaps, joins, and exits.
  This architecture brings different pool designs under the same umbrella; the Vault is agnostic to pool math and can accommodate any system that satisfies a few requirements. Anyone who comes up with a novel idea for a trading system can make a custom pool plugged directly into Dexter's existing liquidity instead of needing to build their own Decentralized Exchange.

- **Security**

  It's crucial to note that the Vault is designed to keep pool balances strictly independent. Maintaining this independence protects from malicious or negligently designed tokens or custom pools from draining funds from any other pools.

## Supported Execute Messages

| Message                             | Description                                                                                                                                                                                                                                             |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ExecuteMsg::UpdateConfig`          | Executable only by `config.owner`. Facilitates updating `config.fee_collector`, `config.generator_address`, `config.lp_token_code_id` parameters.                                                                                                       |
| `ExecuteMsg::UpdatePoolConfig`      | Executable only by `pool_config.fee_info.developer_addr` or `config.owner` if its not set. Facilitates enabling / disabling new pool instances creation (`pool_config.is_disabled`) , and updating Fee (` pool_config.fee_info`) for new pool instances |
| `ExecuteMsg::AddToRegistery`        | Adds a new pool with a new [`PoolType`] Key.                                                                                                                                                                                                            |
| `ExecuteMsg::CreatePoolInstance`    | Creates a new pool with the specified parameters in the `asset_infos` variable.                                                                                                                                                                         |
| `ExecuteMsg::JoinPool`              | Entry point for a user to Join a pool supported by the Vault. User can join by providing the pool id and either the number of assets to be provided or the LP tokens to be minted to the user (as defined by the Pool Contract).                        |
| `ExecuteMsg::Swap`                  | Entry point for a swap tx between offer and ask assets. The swap request details are passed in [`SingleSwapRequest`] Type parameter.                                                                                                                    |
| `ExecuteMsg::ProposeNewOwner`       | Creates a new request to change ownership. Only owner can execute it.                                                                                                                                                                                   |
| `ExecuteMsg::DropOwnershipProposal` | ARemoves a request to change ownership. Only owner can execute it                                                                                                                                                                                       |
| `ExecuteMsg::ClaimOwnership`        | New owner claims ownership. Only new proposed owner can execute it                                                                                                                                                                                      |

## Supported Query Messages

| Message                                  | Description                                                                                            |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `QueryMsg::Config()`                     | Returns the stored Vault Configuration settings in custom [`ConfigResponse`] structure                 |
| `QueryMsg::QueryRigistery([`PoolType`])` | Returns the provided [`PoolType`]'s Configuration settings in custom [`PoolConfigResponse`] structure  |
| `QueryMsg::GetPoolById([`Uint128`])`     | Returns the current stored state of pool with the provided ID in custom [`PoolInfoResponse`] structure |
| `QueryMsg::GetPoolByAddress([`String`])` | Returns the current stored state of the Pool in custom [`PoolInfoResponse`] structure                  |

## Instantiate -::- InstantiateMsg

The instantiation message takes in the token code ID (`lp_token_code_id`) for the token type used by Dexter for instantiating LP tokens against supported pool instances. It also takes in address of the `fee_collector` that collects fees for governance, the `owner` address which owns access to the admin priviledges on the Vault contract, the Generator contract address (`generator_address`) and the initial pool types supported by Dexter Vault which are then stored in the REGISTERY.

```json
{
  "lp_token_code_id": 123,
  "fee_collector": "persistence...",
  "owner": "persistence...",
  "generator_address": "persistence...",
  "pool_configs": "Vec<PoolConfig>"
}
```

## Execute -::- ExecuteMsg

### `Receive(Cw20ReceiveMsg)` - Receives LP Tokens when removing Liquidity

### `UpdateConfig` - Updates contract variables & executable only by `config.owner`, namely the code ID of the token implementation used in dexter, the address that receives governance fees and the Generator contract address.

```json
{
  "update_config": {
    "lp_token_code_id": 123,
    "fee_collector": "terra...",
    "generator_address": "terra..."
  }
}
```

### `UpdatePoolConfig` - Executable only by `pool_config.fee_info.developer_addr` or `config.owner` if its not set. Facilitates enabling / disabling new pool instances creation , and updating Fee for new pool instances

```json
{
  "update_pool_config": {
    "pool_type": "PoolType",
    "is_disabled": "Option<bool>",
    "new_fee_info": "Option<FeeInfo>"
  }
}
```

### `AddToRegistery` - Executable only by `config.owner`. Adds a new pool with a new [`PoolType`] Key.

```json
{
  "add_to_registery": {
    "new_pool_config": "PoolConfig"
  }
}
```

### `CreatePoolInstance` - Creates a new pool with the specified parameters in the `asset_infos` variable.

```json
{
  "CreatePoolInstance": {
    "pool_type": "PoolType",
    "asset_infos": "Vec<AssetInfo>",
    "lp_token_name": "Option<String>",
    "lp_token_symbol": "Option<String>",
    "init_params": "Option<Binary>"
  }
}
```

### `JoinPool` - Entry point for a user to Join a pool supported by the Vault. User needs to approve Vault contract for allowance on the CW20 tokens before calling the JoinPool Function

```json
{
  "join_pool": {
    "pool_id": "Uint128",
    "recipient": "Option<String>",
    "assets": " Option<Vec<Asset>>",
    "lp_to_mint": "Option<Uint128>",
    "slippage_tolerance": "Option<Decimal>",
    "auto_stake": "Option<bool>"
  }
}
```

### `Swap` - Entry point for a swap tx between offer and ask assets. User needs to approve Vault contract for allowance on the CW20 tokens before calling the Swap Function

```json
{
  "swap": {
    "swap_request": "SingleSwapRequest",
    "recipient": "Option<String>"
  }
}
```

### `ProposeNewOwner` - Entry point for a swap tx between offer and ask assets.

```json
{
  "propose_new_owner": {
    "owner": "String",
    "expires_in": "u64"
  }
}
```

### `DropOwnershipProposal` - Entry point for a swap tx between offer and ask assets.

```json
{
  "drop_ownership_proposal": {}
}
```

### `ClaimOwnership` - Entry point for a swap tx between offer and ask assets.

```json
{
  "claim_ownership": {}
}
```

## Execute -::- Cw20HookMsg

### `ExitPool` - Withdrawing liquidity from the pool identified by `pool_id`

```json
{
  "exit_pool": {
    "pool_id": "Uint128",
    "recipient": "Option<String>",
    "assets": "Option<Vec<Asset>>",
    "burn_amount": "Option<Uint128>"
  }
}
```

## Query -::- QueryMsg

### `Config` -Config returns controls settings that specified in custom [`ConfigResponse`] structure

```json
{
  "config": {}
}
```

### `QueryRigistery` -Config returns controls settings that specified in custom [`ConfigResponse`] structure

```json
{
  "query_registery": {
    "pool_type": "PoolType"
  }
}
```

### `GetPoolById` -Config returns controls settings that specified in custom [`ConfigResponse`] structure

```json
{
  "get_pool_by_id": {
    "pool_id": "Uint128"
  }
}
```

### `GetPoolByAddress` -Config returns controls settings that specified in custom [`ConfigResponse`] structure

```json
{
  "get_pool_by_address": {
    "pool_addr": "String"
  }
}
```

## Enums & Structs

### `PoolType` enum - This enum describes the key for the different Pool types supported by Dexter

```
enum PoolType {
    Xyk {},
    Stable2Pool {},
    Stable3Pool {},
    Weighted {},
    Custom(String),
}
```

- New Pool Types can be added via using the PoolType::Custom(String) type.

### `SwapType` enum - This enum describes available Swap types.

```
enum SwapType {
    GiveIn {},
    GiveOut {},
    Custom(String),
}
```

- New Pools can support custom swap types based on math compute logic they want to execute

### `FeeInfo` struct - This struct describes the Fee configuration supported by a particular pool type.

```
struct FeeInfo {
    swap_fee_dir: SwapType,
    total_fee_bps: u16,
    protocol_fee_percent: u16,
    dev_fee_percent: u16,
    developer_addr: Option<Addr>,
}
```

### `Config` struct - This struct describes the main control config of Vault.

```
struct Config {
    /// The Contract address that used for controls settings for factory, pools and tokenomics contracts
    owner: Addr,
    /// The Contract ID that is used for instantiating LP tokens for new pools
    lp_token_code_id: u64,
    /// The contract address to which protocol fees are sent
    fee_collector: Option<Addr>,
    /// The contract where users can stake LP tokens for 3rd party rewards. Used for `auto-stake` feature
    generator_address: Option<Addr>,
    /// The next pool ID to be used for creating new pools
    next_pool_id: Uint128,
}
```

### `PoolConfig` struct - This struct stores a pool type's configuration.

```
struct PoolConfig {
    /// ID of contract which is used to create pools of this type
    code_id: u64,
    /// The pools type (provided in a [`PoolType`])
    pool_type: PoolType,
    fee_info: FeeInfo,
    /// Whether a pool type is disabled or not. If it is disabled, new pools cannot be
    /// created, but existing ones can still read the pool configuration
    is_disabled: bool,
    /// Setting this to true means that pools of this type will not be able
    /// to get added to generator
    is_generator_disabled: bool
}
```

### `PoolInfo` struct - This struct stores a pool type's configuration.

```
struct PoolInfo {
    /// ID of contract which is allowed to create pools of this type
    pool_id: Uint128,
    /// Address of the Pool Contract
    pool_addr: Option<Addr>,
    /// Address of the LP Token Contract
    lp_token_addr: Option<Addr>,
    /// Assets and their respective balances
    assets: Vec<Asset>,
    /// The pools type (provided in a [`PoolType`])
    pool_type: PoolType,
    /// The address to which the collected developer fee is transferred
    developer_addr: Option<Addr>,
}
```

### `SingleSwapRequest` struct - This struct stores a pool type's configuration.

```
struct SingleSwapRequest {
    pool_id: Uint128,
    asset_in: AssetInfo,
    asset_out: AssetInfo,
    swap_type: SwapType,
    amount: Uint128,
    max_spread: Option<Decimal>,
    belief_price: Option<Decimal>,
}
```

## Build schema and run unit-tests

```
cargo schema
cargo test
```

## License

TBD
