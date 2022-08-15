# Dexter - Generator Vesting

The Generator Vesting contract progressively unlocks the DEX token that can then be distributed to LP stakers via the Generator contract. This Vesting contract has been taken from the Astroport's vesting contract available here - https://github.com/astroport-fi/astroport-core/tree/feat/concentrated_liquidity/contracts/tokenomics/vesting

### InstantiateMsg

Initializes the contract with the address of the DEX token and the Owner contract

```
{
    owner: "persistence....",
    pub token_addr: "persistence....",
}
```

#### receive

CW20 receive msg.

```
{
    "receive": {
        "sender": "persistence...",
        "amount": "123",
        "msg": "<base64_encoded_json_string>"
    }
}
```

#### RegisterVestingAccounts

Creates vesting schedules for the DEX token. Each vesting token should have the Generator contract address as the VestingContractAddress. Also, each schedule will unlock tokens at a different rate according to its time duration.

Execute this message by calling the DEX token contract address.

```
{
    "send": {
        "contract": <VestingContractAddress>,
        "amount": "999",
        "msg": "base64-encodedStringOfWithdrawMsg"
    }
}
```

In send.msg, you may encode this JSON string into base64 encoding.

```
{
    "RegisterVestingAccounts": {
        "vesting_accounts": [{
            "address": "persistence...",
            "schedules": {
                "start_point": {
                    "time": "1634125119000000000",
                    "amount": "123"
                },
                "end_point": {
                    "time": "1664125119000000000",
                    "amount": "123"
                }
            }
        }]
    }
}
```

#### claim

Transfer vested tokens from all vesting schedules that have the same VestingContractAddress (address that's vesting tokens).

```
{
    "claim": {
        "recipient": "persistence...",
        "amount": "123"
    }
}
```

### QueryMsg

All query messages are described below. A custom struct is defined for each query response.

#### config

Returns the vesting token contract address (the DEX token address).

```
{ "config": {} }
```

#### vesting_account

Returns all vesting schedules with their details for a specific vesting recipient.

```
{ "vesting_account": { "address": "persistence..." } }
```

#### vesting_accounts

Returns a paginated list of vesting schedules in chronological order. Given fields are optional.

```
{
"vesting_accounts": {
    "start_after": "persistence...",
    "limit": 10,
    "order_by": {   "desc": {}  }
    }
}
```

#### available amount

Returns the claimable amount (vested but not yet claimed) of DEX tokens that a vesting target can claim.

```
{ "available_amount": { "address": "persistence..." } }
```
