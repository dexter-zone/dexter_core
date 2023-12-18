# Dexter - Vault contract

Dexter vault contract is the core contract of the Dexter protocol. It handles all protocol liquidity and faciliates pool related operations like:
- Join Pool
- Exit Pool
- Swap tokens using a specific pool

## Roles

**Owner**: owner is the admin of the contract. Owner term is used to distinguish it from the Cosmwasm contract-admin. Current owner role has following privileges:
- Manage the Vault and Pool admin parameters
- Create a pool
- Pause the pool swap and join operations in case of an adverse event

With the rollout of Chain governance based contol on Dexter, Dexter Governance admin assumes this role therefore all the above actions can be triggered by correct proposals on the Persistence chain.

**Manager**: Manager is a subordinate role to the contract owner to manage day-to-day functions. It overall holds less control over the protocol and it primarily exists to aid any actions that due to involvement of Governance, the owner might be slow to act on. It currently has following privileges:
- Create a pool (if allowed by the owner)
- Pause the pool swap and join operations

This role is currently owned by the [Dexter team multisig]()

**User**: User is an individual that interacts with the Dexter Protocol to join a pool, exit from a pool or to perform a swap.


## Supported state transition functions

### Owner and manager executable

#### 1. _**Update Pause Info**_

Can be used to pause a pool's swap, join and imbalanced-withdraw operations. Normal withdraw operations are not affected by always enabled and cannot be paused by any means.
Pause can happen on a Pool Type or a Pool ID level. If a pool type is paused, all pools of that type are paused. If a pool ID is paused, only that pool is paused.

**Request Example:**

```json
{
  "update_pause_info": {
    "update_type": {
        "pool_id": "1",
    },
    "pause_info": {
        "swap": false,
        "join": true,
        "imbalanced_withdraw": true
    }
  }
}
```

#### 2. _**Create Pool Instance**_

Used to create a pool instance. Pools of only the registered pool types can be created. Currently, Dexter supports following pool types:
- Weighted
- Stable

Based on the pool type, this function can be configured by the owner to be either be executable by:
1. Only them
2. Them and the whitelisted manager address(es)
3. Anyone

In current Dexter configuration, we give this privilege to the manager role as well which is currently the [Dexter team multisig]().

**Request Example:**

```json
{
  "create_pool_instance": {
    "pool_type": {
        "weight": {}
    },
    "asset_infos": [
        {
            "native_token": {
                "denom": "stk/uxprt"
            }
        },
        {
            "native_token": {
                "denom": "uxprt"
            }
        }
    ],
    "native_asset_precisions": [
        {
            "denom": "stk/uxprt",
            "precision": 6
        },
        {
            "denom": "uxprt",
            "precision": 6
        }
    ],
    // Optional: Uses pool type's default config if not provided here
    "fee_info": null,
    // base64 encoded params to be passed to the pool contract on instantiation
    "init_params": ""
  }
}
```


### Only owner executable

#### 1. _**Update Config**_

Used to update the Vault and Pool admin parameters. Currently, it can be used to update the following parameters:
- LP Token Code ID. Ideally not used after the initial deployment.
- Pool creation fee
- Fee collector address
- Auto stake implementation address i.e. the address of the contract that implements the auto stake interface. Currently, this is the [Dexter Multi-staking contract]()
- Pause Info on protocol level i.e. pause all pools


Example request
```json
{
  "update_config": {
    "pool_creation_fee": "disabled",
    "paused": {
        "swap": false,
        "join": true,
        "imbalanced_withdraw": true
    }
  }
}
```

#### 2. _**Add Address to Whitelist**_

Add an address to the manager whitelist.

Example request
```json
{
  "add_address_to_whitelist": {
    "address": "persistence1..."
  }
}
```

#### 3. _**Remove Address from Whitelist**_

Remove an address from the manager whitelist.

Example request
```json
{
  "remove_address_from_whitelist": {
    "address": "persistence1..."
  }
}
```

#### 4. _**Update Pool Type config**_

Update the pool type config. Currently, it can be used to update the following parameters:
-**Instantiation config**: Allow the specific pool type to instantiated by anyone or only the owner or the owner and the whitelisted manager address(es) or None.
-**Fee Info**: Update the fee info for the specific pool type. This fee info is used when a pool of this type is created. It can be updated later on the pool instance level as well.
-**Paused**: Updated pause config for the pools of this type.

Example request
```json
{
  "update_pool_type_config": {
    "pool_type": {
        "weighted": {}
    },
    "allow_instantiation": "OnlyWhitelistedAddresses",
    "new_fee_info": null,
    "paused": null
  }
}
```

#### 5. _**Update Pool Instance Config**_

Update the pool instance config. Currently, it can be used to update the following parameters:
-**Fee Info**: Update the fee info for the specific pool instance. This fee info is used when a pool of this type is created. It can be updated later on the pool instance level as well.
-**Paused**: Updated pause config for the specific pool instance.

Example request
```json
{
  "update_pool_instance_config": {
    "pool_id": "1",
    "fee_info": {
        "total_fee_bps": 50,
        "protocol_fee_percent": 30
    },
    "paused": null
  }
}
```
#### 6. _**Propose new owner**_

Propose a new owner for the contract. This is a two step process. First, the current owner proposes a new owner and then the new owner accepts the proposal. This is done to prevent a situation where the current owner proposes a new owner and then transfers the ownership to someone else without that address (person / entity) having the means to act as a new owner.

Example request
```json
{
  "propose_new_owner": {
    "new_owner": "persistence1...",
    "expires_in": "1000"
  }
}
```


#### 7. _**Drop ownership proposal**_

Drop the ownership proposal if needed ideally to correct the course of action.

Example request
```json
{
  "drop_ownership_proposal": {}
}
```

### User executable

#### 1. _**Join Pool**_

Join a pool. This can be done by sending the native tokens to the vault contract, or by allowing the CW20 tokens to be spent by the vault contract if they are a part of a pool.

A user can specify following parameters:
-**Pool ID**: ID of the pool to join
-**Recipient**: Address to receive the LP tokens. Can be left empty to receive the LP tokens on the sender address.
-**Assets**: Assets to join the pool with. This can be a mix of CW20 tokens and native tokens. It can also not be all the assets of the pool i.e. an imbalanced add. The specifics on how many LP tokens are minted is dependent on the pool type and specific join mechanism.
-**Slippage Tolerance (min_lp_to_receive)**: Slippage tolerance for the join operation. This is used to calculate the maximum amount of LP tokens that can be minted for the given assets. If the slippage tolerance is not met, the join operation fails. Can be left empty for no slippage tolerance.
-**Auto stake**: If the LP tokens should be auto staked or not. Requires the auto stake implementation to be set on the vault contract.

Example request
```json
{
  "join_pool": {
    "pool_id": "1",
    "assets": [
        {
            "amount": "1000000",
            "info": {
                "native_token": {
                    "denom": "uxprt"
                }
            }
        },
        {
            "amount": "1000000",
            "info": {
                "native_token": {
                    "denom": "stk/uxprt"
                }
            }
        }
    ],
    "min_lp_to_receive": null,
    "auto_stake": true,
    "recipient": null
  }
}
```

#### 2. _**Swap**_

Swap assets using a pool. This can be done by sending the native tokens to the vault contract, or by allowing the CW20 tokens to be spent by the vault contract if they are a part of a pool.

Following parameters can be specified:
-**Swap Request**: Swap request to be executed. Refer to the [swap request](#swap-request) section for more details.
-**Recipient**: Address to receive the swapped assets. Can be left empty to receive the swapped assets on the sender address.
-**Slippage Tolerance (min_receive)**: Slippage tolerance for the `GiveIn` swap operation. This is to control minimum expected amount of assets to be received. If the slippage tolerance is not met, the swap operation fails. Can be left empty for no slippage tolerance.
-**Slippage Tolerance (max spend)**: Slippage tolerance for the `GiveOut` swap operation. This is to control maximum expected amount of assets to be spent. If the slippage tolerance is not met, the swap operation fails. Can be left empty for no slippage tolerance. Rest of the assets are returned to the sender address.

Example request
```json
{
  "swap": {
    "swap_request": {
        "pool_id": "1",
        "asset_in": {
            "native_token": {
                "denom": "uxprt"
            }
        },
        "asset_out": {
            "native_token": {
                "denom": "stk/uxprt"
            }
        },
        "swap_type": {
            "GiveIn": {}
        },
        "amount": "1000000",
        "max_spread": null,
        "belief_price": null
    },
    "min_receive": null,
    "max_spend": null,
    "recipient": null
  }
}
```

#### 3. _**Exit Pool**_

Exiting a pool happens via a CW20 Send hook msg to transfer ownership of the LP tokens to the vault contract. The vault contract then burns the required LP tokens and sends the assets to the recipient address. If there are any extra LP tokens left, based on the exit type, they are sent back to the sender address.

Parameters for this operation are:
-**Pool ID**: ID of the pool to exit
-**Recipient**: Address to receive the swapped assets. Can be left empty to receive the swapped assets on the sender address.
-**Exit Type**: Type of exit to perform. Can be one of the following:
    -**Exact LP Burn**: Burn the exact amount of LP tokens and send the assets to the recipient address. A user can specify the minimum amount of assets to receive as well to control the slippage tolerance. The assets received are in the pool ratio.
    -**Exact Assets Out**: Burn the exact amount of assets specified and deduct the corresponding representative LP tokens from the user. Can be used for an imbalanced withdraw. Slippage tolerance is controlled by the user by specifying the maximum amount of LP tokens to burn. Rest of the LP tokens are sent back to the sender address.

Example request
```json
{
  "exit_pool": {
    "pool_id": "1",
    "exit_type": {
        "ExactLPBurn": {
            "lp_to_burn": "1000000",
            "min_asset_amounts": [
                {
                    "amount": "1000000",
                    "info": {
                        "native_token": {
                            "denom": "uxprt"
                        }
                    }
                },
                {
                    "amount": "1000000",
                    "info": {
                        "native_token": {
                            "denom": "stk/uxprt"
                        }
                    }
                }
            ]
        }
    },
    "recipient": null
  }
}
```

## Supported Queries

### 1. _**Config**_

Get the current config of the vault contract. This includes the following parameters:
-**LP Token Code ID**: Code ID of the LP token contract. This is used to create new LP tokens for new pools. Ideally not updated post initial deployment
-**Fee Collector Address**: Address of the contract which collects fees. Currently, the Dexter keeper contract
-**Pool Creation Fee**: Fee in the specified denom for creating a new pool. This is used to prevent spamming of the pool creation feature.
-**Auto-stake implementation**: Address of the contract which implements the auto-stake feature. Currently, the Dexter multistaking contract
-**Whitelisted managers**: List of addresses which currently have the manager role
-**Global Pause Info**: Pause configuration for the protocol. This is used to pause/unpause specific operations for the entire protocol. It overrides the pause configuration on the pool type and pool level i.e. if some operation is paused on a protocol level, it cannot be unpaused on a pool type or pool level.

### 2. _**Query Registry**_

Get pool type config present in the registry for a pool type that's registered on the vault contract. 

#### Request

```json
{
  "query_registry": {
    "pool_type": {
        "weighted": {}
    }
  }
}
```

#### Response

```json
{
  "code_id": "3",
  "pool_type": {
    "weighted": {}
  },
  "default_fee_info": {
    "total_fee_bps": 30,
    "protocol_fee_percent": 30
  },
  "allow_instantiation": "OnlyWhitelistedAddresses",
  "paused": {
    "swap": false,
    "join": false,
    "imbalanced_withdraw": true
  }
}
```

### 3. _**Get Pool by ID**_

Get the pool config for a pool ID.

#### Request

```json
{
  "get_pool_by_id": {
    "pool_id": "1"
  }
}
```

#### Response

```json
{
  "pool_id": "1",
  "pool_type": {
    "weighted": {}
  },
  "pool_addr": "persistence1...",
  "lp_token_addr": "persistence1...",
  "fee_info": {
    "total_fee_bps": 30,
    "protocol_fee_percent": 30
  },
  "assets": [
    {
      "amount": "1000000",
      "info": {
        "native_token": {
          "denom": "uxprt"
        }
      }
    },
    {
      "amount": "1000000",
      "info": {
        "native_token": {
          "denom": "stk/uxprt"
        }
      }
    }
  ],
  "paused": {
    "swap": false,
    "join": false,
    "imbalanced_withdraw": true
  }
}
  
```

### 4. _**Get Pool by Address**_

Get the pool config for a pool address.

#### Request

```json
{
  "get_pool_by_address": {
    "pool_addr": "persistence1..."
  }
}
```

#### Response

same as above

### 5. _**Get Pool by LP Token Address**_

Get the pool config for a LP token address.

#### Request

```json
{
  "get_pool_by_lp_token_address": {
    "lp_token_addr": "persistence1..."
  }
}
```

#### Response

same as above