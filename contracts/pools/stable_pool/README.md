# Dexter: Stableswap Pool

Dexter implements a generic version of Curve's stableswap invariant for upto 5 assets in the pool and implements compute calculations on liquidity provision / withdrawal and swaps.

Dexter's contract architecture is unique in that it separates the ownership of the assets in the pool in the Vault contract. Pool contracts are only responsible for the math computes which dictate number of tokens to be transferred during swaps / liquidity provisioning events, and do not handle the token transfers themselves. 

Dexter's Vault queries the Pool contracts to compute how many tokens to transfer and processes those transfers itself.

This separation simplifies pool contracts, since they no longer need to actively manage their assets; pools only need to calculate amounts for swaps, joins, and exits. New pool types, can be easily added which only implement the math computes and do not need to worry about the token transfer logic.


## Contract State

| Message      | Description                                                                                     |
| ------------ | ----------------------------------------------------------------------------------------------- |
| `CONFIG`     | Stores pool contract's core Configuration parameters in a [`Config`] struct. This is the base config that is same for all pool types. |
| `TWAPINFO`   | Stores Twap prices for the tokens supported by the pool in a [`Twap`] struct                    |
| `MATHCONFIG` | Stores custom configuration parameters related with the stableswap invariant like AMP parameter |
| `STABLESWAP_CONFIG` | Stores additional configurations like scaling factors (for metastable pools) and the max allowed spread |
| `PRECISIONS` | Stores precision of all assets in the pool. For CW20 tokens, it is fetched using the contract, and for native assets, it must be specified during pool creation |

---


## Supported Execute Messages

### Update Config

Executable only by Dexter Vault's owner. Updates the pool's math configuration with the specified parameters in the `params` variable encoded in base64. 

Accepts the following commands in the `params` variable:

- `StartChangingAmp` : Starts the process of changing the amplification factor of the pool. It takes following params:

    - `next_amp` : The target amplification factor to reach at `next_amp_time`
    - `next_amp_time` : The timestamp when the amplification factor should be `next_amp`. This should at least be equal to MIN_AMP_CHANGE_TIME which is currently hard-coded to 1 day.

- `StopChangingAmp` : Stops the process of changing the amplification factor of the pool. It takes no params. It stops the amp at the current value.

- `UpdateScalingFactor` : Updates the scaling factor of the asset in the pool. It takes following params:

    - `asset_info` : The asset whose scaling factor is to be updated
    - `scaling_factor` : The new scaling factor for the asset

- `UpdateScalingFactorManager` : Updates the scaling factor manager of the pool. It takes following params:

    - `manager` : The new scaling factor manager for the pool

- `UpdateMaxAllowedSpread` : Updates the max allowed spread between the price of the asset and the price of the pool. If the spread is greater than this value, the swap will fail. It takes following params:
    - `max_allowed_spread` : The new max allowed spread for the pool

The relevant struct can be found [here](../contracts/pools/stable_pool/src/state.rs#L122)

#### Request 

Update message
```json
{
    "start_changing_amp": {
        "next_amp": 1000000000000000000,
        "next_amp_time": 1629820800
    }
}
```

We base64 encode the above message and pass it as the `params` variable in the Execute::UpdateConfig message.

Update execute message to contract goes like

```json
{
    "update_config": {
        "params": "<BASE64_ENCODED_MESSAGE>"
    }
}
```

### Update Liquidity

Executable only by Dexter Vault. Updates locally stored asset balances state in `config.assets` for the pool and updates the TWAP.


#### Request

```json
{
    "update_liquidity": {
        "assets": [
            {
                "info": {
                    "native_token": {
                        "denom": "uxprt"
                    }
                },
                "amount": "1000000000000"
            },
            {
                "info": {
                    "native_token": {
                        "denom": "stk/uxprt"
                    }
                },
                "amount": "1000000000000"
            }
        ]
    }
}
```

### Update Fee 

Executable only by the Dexter Vault where it is triggerd by the Vault owner. Updates the fee for this particular pool. The fee is specified in basis points.


#### Request

```json
{
    "update_fee": {
        "total_fee_bps": "1000"
    }
}
```


## Supported Queries

### Config
Returns the stored pool configuration

#### Request

```json
{
    "config": {}
}
```

#### Response

```json
{
    "pool_id": "1",
    "lp_token_addr": "persistence1...",
    "vault_addr": "persistence1...",
    "assets": [
        {
            "info": {
                "native_token": {
                    "denom": "uxprt"
                }
            },
            "amount": "1000000000000"
        },
        {
            "info": {
                "native_token": {
                    "denom": "stk/uxprt"
                }
            },
            "amount": "1000000000000"
        }
    ],
    "pool_type": {
        "stableswap": {}
    },
    "fee_info": {
        "total_fee_bps": "1000"
    },
    "block_time_last": "1629820800",
    "math_params": null,
    "additional_params": null
}
```

### Fee Params

Returns the stored fee parameters for the pool type

#### Request

```json
{
    "fee_params": {}
}
```

#### Response

```json
{
    "total_fee_bps": "1000"
}
```

### Pool ID

Returns the pool ID of the pool

#### Request

```json
{
    "pool_id": {}
}
```

#### Response

```json
{
    "pool_id": "1"
}
```

### On Join Pool

Takes either the amounts of assets to be deposited or the amount of LP tokens to be minted for a join operation and returns the following:
- The amount of assets to be deposited
- Amount of LP tokens to be minted based on the current pool state
- Fee to be charged. It mostly applies in a non balanced pools. The calculations is based on the Curve's stableswap invariant

**Additional note**: This request never fails. If the join operation is not possible, the response will contain the reason for failure in the `response` field.

#### Request

```json
{
    "on_join_pool": {
        "assets_in": [
            {
                "info": {
                    "native_token": {
                        "denom": "uxprt"
                    }
                },
                "amount": "1000000000000"
            },
            {
                "info": {
                    "native_token": {
                        "denom": "stk/uxprt"
                    }
                },
                "amount": "1000000000000"
            }
        ]
    }
}
```

#### Response

```json
{
    "provided_assets": [
        {
            "info": {
                "native_token": {
                    "denom": "uxprt"
                }
            },
            "amount": "1000000000000"
        },
        {
            "info": {
                "native_token": {
                    "denom": "stk/uxprt"
                }
            },
            "amount": "1000000000000"
        }
    ],
    "new_shares": "1000000000000",
    "response": {
        "success": {}
    },
    "fee": [
        {
            "info": {
                "native_token": {
                    "denom": "uxprt"
                }
            },
            "amount": "1000000000"
        }
    ]
}
```

### On Exit Pool

Useful for estimating the amount of assets received after an exit operation. It takes an exit type parameter which can either of the following:
1. `ExactLpBurn` : User wants to burn a specific amount of LP tokens and receive the assets in return. It returns assets in the same ratio as the pool. This type of withdraw is called balanced withdraw.

2. `ExactAssetsOut`: In this type of withdraw, user specifies the particular type of tokens that the user want to take out of the pool.
The pool logic estimates the LP token to be burnt based on the current pool state. Since, the assets are returned exactly as specified by the user and not according to the pool ratio, we call this type of withdraw as imbalanced withdraw.

Currently, we have disabled the imbalanced withdraws for the stableswap pool for pool stability reasons. It can be enabled using the chain governance.

**Additional note**: This request never fails. If the exit operation is not possible, the response will contain the reason for failure in the `response` field.

#### Request

```json
{
    "on_exit_pool": {
        "exit_type": {
            "exact_assets_out": {
                "assets_out": [
                    {
                        "info": {
                            "native_token": {
                                "denom": "uxprt"
                            }
                        },
                        "amount": "1000000000000"
                    },
                    {
                        "info": {
                            "native_token": {
                                "denom": "stk/uxprt"
                            }
                        },
                        "amount": "1000000000000"
                    }
                ]
            }
        }
    }
}
```

#### Response

```json
{
    "assets_out": [
        {
            "info": {
                "native_token": {
                    "denom": "uxprt"
                }
            },
            "amount": "100000000"
        },
        {
            "info": {
                "native_token": {
                    "denom": "stk/uxprt"
                }
            },
            "amount": "100000000"
        }
    ],
    "burn_shares": "10000000",
    "response": {
        "success": {}
    },
    "fee": [
        {
            "info": {
                "native_token": {
                    "denom": "uxprt"
                }
            },
            "amount": "1000000000"
        }
    ]
}
```

### On Swap 

Allows for a swap simulation. Takes following parameters and returns the expected swap result based on the current pool state.

- `offer_asset`: The asset to be sent by the user

- `ask_asset`: The asset to be received by the user

- `swap_type`: It has 2 types, `GiveIn` or `GiveOut` to specify the context of the amount parameter. If `GiveIn`, the amount is the amount of `offer_asset` to be sent by the user. If `GiveOut`, the amount is the amount of `ask_asset` to be received by the user.

- `amount`:  The amount of `offer_asset` or `ask_asset` depending on the `swap_type` parameter

- `max_spread`: The max spread between the price of the asset and the price of the pool. If the spread is greater than this value, the swap will fail. For the purpose of stableswap pool, the exchange rate for the spread calculation is 1 and for metastable pools, it is the current redemption rate of the asset in the LST protocol. For example, if the `max_spread` is set at 0.02, the swap will fail if pool is not able to provide a rate better than 0.98 for the current swap.

- `belief_price`: The price at which the user wants to swap. If the pool is able to provide a better rate than this, the swap will succeed. For example, if the `belief_price` is set at 1.1, the swap will succeed if the pool is able to provide a rate better than 1.1 for the current swap.


The expected swap response has the following fields. It contains the following fields:

- `trade` : Trade related infromaton, it has following fields
    - `amount_in` : The amount of `offer_asset` to be sent by the user
    - `amount_out` : The amount of `ask_asset` to be received by the user
    - `spread` : The spread associated with the swap tx.

- `response` : It has 2 types, `Success` or `Failure` to specify the context of the swap result. If `Success`, the swap will succeed. If `Failure`, the swap will fail.

- `fee` : The fee to be charged. The calculations is based on the Curve's stableswap invariant.

#### Request

```json
{
    "on_swap": {
        "offer_asset": {
            "native_token": {
                "denom": "uxprt"
            }
        },
        "ask_asset": {
            "native_token": {
                "denom": "stk/uxprt"
            }
        },
        "swap_type": {
            "give_in": {}
        },
        "amount": "100000000",
        "max_spread": "0.02",
        "belief_price": "0.8"
    }
}
```

#### Response

```json
{
    "trade": {
        "amount_in": "100000000",
        "amount_out": "79997000",
        "spread": "0.01"
    },
    "response": {
        "success": {}
    },
    "fee": [
        {
            "info": {
                "native_token": {
                    "denom": "uxprt"
                }
            },
            "amount": "300000"
        }
    ]
}
```

### Cumulative Price

Returns the cumulative price of the asset in the pool. This is for the TWAP calculation for any external party to use the TWAP price of the asset in the Dexter pool. 

Culumative price can be calculated across two block times and the TWAP price can be calculated using the following formula:

```rust
TWAP = (cumulative_price_2 - cumulative_price_1) / (block_time_2 - block_time_1)
```

#### Request

```json
{
    "cumulative_price": {
        "offer_asset": {
            "native_token": {
                "denom": "uxprt"
            }
        },
        "ask_asset": {
            "native_token": {
                "denom": "stk/uxprt"
            }
        }
    }
}
```

#### Response

```json
{
    "exchange_info": [{
        "offer_info": {
            "native_token": {
                "denom": "uxprt"
            }
        },
        "ask_info": {
            "native_token": {
                "denom": "stk/uxprt"
            }
        },
        "rate": "0.8",
    }],
    "total_share": "1000000000000",
}
```

**Note**: `total_share` is the total amount of LP tokens in the pool at the current block time.


### Cumulative Prices

Returns the cumulative prices of all the assets in the pool for all possible exchange pairs. This is for calculation for any external party to use the TWAP price of the asset in the Dexter pool across two block times.

#### Request

```json
{
    "cumulative_prices": {}
}
```

#### Response

```json
{
    "exchange_info": [{
        "offer_info": {
            "native_token": {
                "denom": "uxprt"
            }
        },
        "ask_info": {
            "native_token": {
                "denom": "stk/uxprt"
            }
        },
        "rate": "0.8",
    }],
    "total_share": "1000000000000",
}
```
