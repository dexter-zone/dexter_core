# Dexter Protocol :: Stable Pool

Dexter's Stable Pool implements the stableswap invariant for its compute calculations on Liquidity provision / withdrawal and swaps. Most of the stableswap compute code is taken from the Astroport's `pair_stable` contract

## Contract State

| Message      | Description                                                                                     |
| ------------ | ----------------------------------------------------------------------------------------------- |
| `CONFIG`     | Stores pool contract's core Configuration parameters in a [`Config`] struct                     |
| `TWAPINFO`   | Stores Twap prices for the tokens supported by the pool in a [`Twap`] struct                    |
| `MATHCONFIG` | Stores custom configuration parameters related with the stableswap invariant like AMP parameter |

---

- **Separating Token Accounting and Pool Logic**

  The Dexter Pools are responsible only for the math computes which dictate number of tokens to be transferred during swaps / liquidity provisioning events, and do not handle the token transfers themselves. Dexter's Vault queries the Pool contracts to compute how many tokens to transfer and processes those transfers itself.

  This separation simplifies pool contracts, since they no longer need to actively manage their assets; pools only need to calculate amounts for swaps, joins, and exits.
  Anyone who comes up with a novel idea for a trading system can make a custom pool and have it added to Dexter's PoolType Registery via approval from the Dexter DAO instead of needing to build their own Decentralized Exchange.

## Supported Execute Messages

| Message                       | Description                                                                                                                                                                                           |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ExecuteMsg::UpdateConfig`    | Executable only by Dexter Vault's owner. Updates the pool's math configuration with the specified parameters in the `params` variable. Accepts only `StartChangingAmp` and `StopChangingAmp` commands |
| `ExecuteMsg::UpdateLiquidity` | Executable only by Dexter Vault. Updates locally stored asset balances state in `config.assets` for the pool and updates the TWAP.                                                                    |

## Supported Query Messages

| Message                                                                                            | Description                                                                                                                                                                                                             |
| -------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `QueryMsg::Config()`                                                                               | Returns the stored Vault Configuration settings in custom [`ConfigResponse`] struct                                                                                                                                     |
| `QueryMsg::FeeParams()`                                                                            | Returns the provided [`PoolType`]'s Configuration settings in custom [`FeeResponse`] struct                                                                                                                             |
| `QueryMsg::PoolId()`                                                                               | Returns Pool ID which is of type [`Uint128`]                                                                                                                                                                            |
| `QueryMsg::OnJoinPool( assets_in, mint_amount , slippage_tolerance )`                              | Returns [`AfterJoinResponse`] type which contains - `return_assets` info, number of LP shares to be minted, the `response` of type [`ResponseType`] and `fee` of type [`Option<Asset>`] which is the fee to be charged. |
| `QueryMsg::OnExitPool( assets_out, burn_amount )`                                                  | Returns [`AfterExitResponse`] type which contains - `assets_out` info, number of LP shares to be burnt, the `response` of type [`ResponseType`] and `fee` of type [`Option<Asset>`] which is the fee to be charged.     |
| `QueryMsg::OnSwap( swap_type, offer_asset, ask_asset, amount, max_spread, belief_price )`          | Returns [`SwapResponse`] type which contains - `trade_params` info, the `response` of type [`ResponseType`] and `fee` of type [`Option<Asset>`] which is the fee to be charged.                                         |
| `QueryMsg::CumulativePrice( swap_type, offer_asset, ask_asset, amount, max_spread, belief_price )` | Returns information about the cumulative price of the asset in a [`CumulativePriceResponse`] object.                                                                                                                    |
| `QueryMsg::CumulativePrices( )`                                                                    | Returns information about the cumulative prices in a [`CumulativePricesResponse`] object.                                                                                                                               |

## Enums & Structs

### `MathConfig` struct - This struct describes the main math configuration of the stable-pool.

```
struct MathConfig {
    // This is the current amplification used in the pool
    pub init_amp: u64,
    // This is the start time when amplification starts to scale up or down
    pub init_amp_time: u64,
    // This is the target amplification to reach at `next_amp_time`
    pub next_amp: u64,
    // This is the timestamp when the current pool amplification should be `next_amp`
    pub next_amp_time: u64,
}
```

### `StablePoolUpdateParams` enum - This enum stores the options available to start and stop changing a stableswap pool's amplification. Used in the Execute::UpdateConfig function.

```
enum StablePoolUpdateParams {
    StartChangingAmp { next_amp: u64, next_amp_time: u64 },
    StopChangingAmp {},
}
```

### `ResponseType` enum - This enum is used to describe if the math computations (joins/exits/swaps) will be successful or not

```
enum ResponseType {
    Success {},
    Failure (String),
}
```

### `Config` struct - This struct describes the main control config of pool.

```
struct Config {
    /// ID of contract which is allowed to create pools of this type
    pub pool_id: Uint128,
    /// The address of the LP token associated with this pool
    pub lp_token_addr: Option<Addr>,
    /// the vault contract address
    pub vault_addr: Addr,
    /// Assets supported by the pool
    pub assets: Vec<Asset>,
    /// The pools type (provided in a [`PoolType`])
    pub pool_type: PoolType,
    /// The Fee details of the pool
    pub fee_info: FeeStructs,
    /// The block time when pool liquidity was last updated
    pub block_time_last: u64,
}
```

### `Trade` struct - This helper struct is used for swap operations

```
struct Trade {
    /// The number of tokens to be sent by the user to the Vault
    pub amount_in: Uint128,
    /// The number of tokens to be received by the user from the Vault
    pub amount_out: Uint128,
    /// The spread associated with the swap tx
    pub spread: Uint128,
}
```

### `AfterJoinResponse` struct - Helper struct for [`QueryMsg::OnJoinPool`]

```
struct AfterJoinResponse {
    // Is a sorted list consisting of amount of info of tokens which will be provided by the user to the Vault as liquidity
    pub provided_assets: Vec<Asset>,
    // Is the amount of LP tokens to be minted
    pub new_shares: Uint128,
    // Is the response type :: Success or Failure
    pub response: ResponseType,
    // Is the fee to be charged
    pub fee: Option<Asset>,
}
```

### `AfterExitResponse` struct - Helper struct for [`QueryMsg::OnExitPool`]

```
struct AfterExitResponse {
    /// Assets which will be transferred to the recipient against tokens being burnt
    pub assets_out: Vec<Asset>,
    /// Number of LP tokens to burn
    pub burn_shares: Uint128,
    /// Operation will be a `Success` or `Failure`
    pub response: ResponseType,
    /// Fee to be charged
    pub fee: Option<Asset>,
}
```

### `SwapResponse` struct - Helper struct for [`QueryMsg::OnSwap`]

```
struct SwapResponse {
    ///  Is of type [`Trade`] which contains all params related with the trade
    pub trade_params: Trade,
    /// Operation will be a `Success` or `Failure`
    pub response: ResponseType,
    /// Fee to be charged
    pub fee: Option<Asset>,
}
```

## Build schema and run unit-tests

```
cargo schema
cargo test
```

## License

TBD
