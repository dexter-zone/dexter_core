# Dexter Superfluid LP contract

Dexter superfluid LP contract enables 1-click LP position from staked chain tokens directly to a bonded LP position.

This is part of the stkXPRT rollout. For more details, see [here]()

## Roles

**Owner**: Manages the contract admin parameters. Currently the only admin parameter is the vault contract address.

**User**: Person who locks LST tokens in the contract and then uses them to create a bonded LP position.


## Supported state transition functions

### Owner executable

Following transition functions can only be executed by the `owner` of the contract.

#### 1. _**Update config**_

Updates the contract admin parameters. 

```json
{
  "update_config": {
    "vault_address": "persistence1..."
  }
}
```

#### 2. _**Add allowed lockable token**_

Adds a new token denom to the list of allowed lockable tokens. This is used to whitelist tokens that can be locked in the contract.

```json
{
  "add_allowed_lockable_token": {
    "native_token": {
      "denom": "stk/uxprt"
    }
  }
}
```

### User executable

#### 1. _**Lock LST**_

Locks LST tokens in the contract which can then be used to create a bonded LP position. This is ideally part of a superfluid LP transaction.

Only a set of whitelisted token denoms an be locked in the contract. These can be added by the contract owner.

This calls from the liquidstake module once the LST has been minted for the user, to lock it in the same message.

Post this, another messages can be sent, ideally in the same transaction to use the locked LST to create a bonded LP position using the correspondng transition function.

```json
{
  "lock_lst": {
    "asset": {
        "amount": "1000000000000000000000000",
        "info": {
            "native_token": {
              "denom": "stk/uxprt"
            }
        }
    }
  }
}
```

#### 2. _**Join pool and bond using Locked LST**_

Allows a user who locked his LST to use any combination of the locked LST and extra tokens that he sent to add liquidity to a pool and also bond it. Ideally, this message is part of a single transaction that converts a staked asset to LST and then lock it using the `lock_lst` transition function.


```json
{
  "join_pool_and_bond_using_locked_lst": {
    "pool_id": "1",
    "total_assets": [
      {
        "amount": "900000000",
        "info": {
          "native_token": {
            "denom": "stk/uxprt"
          }
        }
      },
      {
        "amount": "1000000000",
        "info": {
          "token": {
            "contract_addr": "persistence1..."
          }
        }
      }
    ],
    "min_lp_to_receive": "100000000"
  }
}
```

