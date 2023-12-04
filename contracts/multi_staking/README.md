# Dexter - Multi-staking contract

Dexter Multi-staking contract enables LP incentivization mechanism on Dexter. It supports following features:

1. Creation of multiple reward schedules which can be overlapping as well. 
2. Reward schedule assets can be both CW20 and Native tokens.
3. 'Bonding' functionality for the users by which they deposit their LP tokens, to be eligible for receiving rewards.
4. Reward schedule amount is linearly distributed to the users based on their bonding ratio i.e. their bonded amount : total bonded amount for that LP token.

## Roles

**Owner**:  Manages the contract admin parameters. With v2.2 of the Multi-staking contract (v1.1 release of the Dexter Protocol), this role is governed by the Governance Admin contract. Prior to this release, this was managed by a Cosmwasm multi-sig.

**Reward Schedule Creator**: This type of user creates reward schedules. This user is also elligible to redeem some undistributed rewards if left in the contract. See here.

**User**: User who bonds / unbonds LP tokens in the contract.

## Supported state transition functions

### Owner executable

Following transition functions can only be executed by the `owner` of the contract. 

#### 1. _**Create Reward Schedule**_
Allows the contract owner to create a new reward schedule. Owner can create this schedule on behalf of another user (particularly useful with XPRT governance).

Before introduction in V3.0 of the contract, the contract used to follow a proposal / accept based flow for creation of reward schedules. That has been removed in favour of chain governance based approach where Governance Admin contract through it's admin privileges can create a new reward schedule if approved by the chain governance.

##### Example
```json
{
    "create_reward_schedule": {
        "lp_token": "persistence",
        "title": "Create reward schedule for DYDX/USDC Pool",
        "actual_creator": null,
        "start_block_time": 10000,
        "end_block_time": 20000
    }
}

```

See more examples for usages in tests [here](TODO)

### 2. _**Update Config**_
Allows the contract owner to update config params. Can update any of the following parameters selectively by only including that key <br>
Primary params that support update are:

a. **keeper_addr**: fee collector address <br>
b. **unlock_period**: lockup period after unbonding for which the tokens are in an unlocking state. <br>
c. **instant_unbond_fee_bp**: fee in basis points for Instant LP unbonding feature. <br>
d. **instant_unbond_min_fee_bp**: min fee for tier based fee structure for instant-unlock feature for unlocking liquidity during the unlocking period. <br>
e. **fee_tier_interval**: interval for fee tier change. the unlock period is divided into tiers for partial unbond fee during unlock period.

```json
{
    "keeper_addr": "<KEEPER_CONTRACT_ADDRESS>",
}

```

### 3. _**Allow LP Token**_

Allows the contract owner to add a new LP token to the contract. This is needed before reward schedule creation for that LP token.

```json
{
    "allow_lp_token": {
        "lp_token": "<LP_TOKEN_ADDRESS>"
    }
}

```

### 4. _**Remove LP Token**_

Remove LP Token from allowed list. Existing reward schedules for this LP token will remain unaffected.

```json
{
    "remove_lp_token": {
        "lp_token": "<LP_TOKEN_ADDRESS>"
    }
}
```

### 5. _**Propose new owner**_

Allows the contract owner to propose a new owner for the contract. The new owner needs to accept the proposal before becoming the new owner.

```json
{
    "propose_new_owner": {
        "new_owner": "<NEW_OWNER_ADDRESS>"
    }
}
```

### 6. _**Drop ownership (transfer) proposal**_

Allows the contract owner to drop the ownership proposal if they initiated one earlier. Applicable only till the new owner has not accepted the proposal.

```json
{
    "drop_ownership_proposal": {}
}
```

### User executable

Following transition functions can be executed by any user who wants to bond / unbond LP tokens in the contract.

### 1. _**Bond**_

Allows the user to bond LP tokens in the contract. The user can bond multiple times for the same LP token. The bonded amount is added to the existing bonded amount for that user. User needs to allow spending of LP tokens to the contract before bonding.


```json
{
    "bond": {
        "lp_token": "<LP_TOKEN_ADDRESS>",
        "amount": "1000000000000000000000000"
    }
}
```

### 2. _**Unbond**_

Allows the user to unbond LP tokens in the contract. Unbonded tokens enter a lockup period after which they can be withdrawn.
During the lockup period, the user can use Instant Unlock feature to withdraw their LP tokens instantly by paying a fee. The fee is calculated based on the time remaining in the lockup period and the fee tier structure. The fee tier structure is defined by the contract owner and can be updated by the owner. 

```json
{
    "unbond": {
        "lp_token": "<LP_TOKEN_ADDRESS>",
        "amount": "1000000000000000000000000"
    }
}
```

### 3. _**Unlock**_
Allows the user to receive LP tokens after the lockup period. This functions calculates all the currently unlocked (which could be in multiple token locks) and sends them to the user. 

```json
{
    "unlock": {
        "lp_token": "<LP_TOKEN_ADDRESS>",
    }
}
```

### 4. _**Instant Unbond**_

Allows the user to instantly unbond LP tokens in the contract without entering a lockup period. The user needs to pay a fee for this feature. 

```json
{
    "instant_unbond": {
        "lp_token": "<LP_TOKEN_ADDRESS>",
        "amount": "1000000000000000000000000"
    }
}
```

### 5. _**Instant Unloock**_


Allows the user to instantly unlock LP tokens in the contract if they are in the lockup period. The user needs to pay a fee for this feature which is calculated based on the time remaining in the lockup period and the fee tier structure.

```json
{
    "instant_unlock": {
        "lp_token": "<LP_TOKEN_ADDRESS>",
        "token_locks": [
            {
                "unlock_time": 10000,
                "amount": "1000000000"
            }
        ]
    }
}
```

### 5. _**Withdraw (Rewards)**_

Allows a user to withdraw the unclaimed reward from last claim time that has been accrued for them from any reward schedules.

```json
{
    "withdraw": {
        "lp_token": "<LP_TOKEN_ADDRESS>",
    }
}
```

### Reward creator executable

Following transition functions can be executed by the reward schedule creator. This is a special role that is assigned to the user on behalf on whom the owner creates the reward schedule. This is particularly useful for XPRT governance where the governance admin contract would create reward schedules on behalf of the actual creator.

### 1. _**Claim Unallocated Rewards**_

Allows the reward schedule creator to claim any unallocated rewards from the reward schedule. This is useful when the reward schedule has an interim period of 
no bonded tokens and thus no rewards are being distributed. The creator can claim these rewards back.

```json
{
    "claim_unallocated_rewards": {
        "lp_token": "<LP_TOKEN_ADDRESS>",
    }
}
```


### New proposed owner executable

Following transition functions can be executed by the new proposed owner of the contract. This is a special role that is assigned to the user who has been proposed as the new owner of the contract. 

### 1. _**Claim Ownership**_

Allows the new proposed owner to claim ownership of the contract. This is needed after the current owner has proposed a new owner.

```json
{
    "claim_ownership": {}
}
```



### Supported Queries

#### 1. _**Config**_

Returns the current config of the contract.

##### Query:

```json
{
    "config": {}
}
```

##### Response:

```json
{
    // use persistence address
    "owner": "persistence1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
    "keeper": "persistence1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
    "allowed_lp_tokens": [
        "persistence1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v"
    ],
    "unlock_period": 86400,
    "minimum_reward_schedule_proposal_start_delay": 86400,
    "instant_unbond_fee_bp": 100,
    "fee_tier_interval": 86400,
    "instant_unbond_min_fee_bp": 100
}
```


### 2. _**Reward Schedules**_

Returns the list of reward schedules created in the contract for a particular LP token and reward asset.

##### Request

```json
{
    "lp_token": 
}
```

#### Response

