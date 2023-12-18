# Dexter Admin Parameters

Dexter admin parameters are spread throughout the codebase. This document is an attempt to consolidate them in one place. All these parameters can be governed by the `owner` of the respective Dexter contracts (vault and the multi-staking contract)

With the rollout of Governance Admin based control of the `owner` role, all these parameters can be changed by triggering right kind of of call from the Governance Admin contract by it's `ExecuteMsgs` entrypoint using a Governance proposal.

## Admin roles and their responsibilities

Currently, there are these admin roles in the Dexter contracts:

### Vault Admin 
- The vault admin role is the main controller of the protocol. 
- It can add pool managers, update the vault config, or update the config of  any of the existing pools or pool types. 

All the things that the vault admin can do are explained in depth in the respective Contracts section.

### Pool Manager
- Pool creation
- Can invoke Emergency Pause - based on Pool type or Pool ID

### Multistaking Admin
- Can create new reward schedules.
- Can add or remove new whitelisted tokens for rewards.

### Keeper Admin
- Keeper admin can withdraw the funds set aside as part of the protocol treasury(30% of the swap fees)  that are generated from the Swaps.


The roles are mapped to following accounts currently:

| Contract | Role | Account |
| --- | --- | --- |
| Vault | Vault Owner | Dexter Governance Admin contract |
| Vault | Pool Manager | Dexter team multisig contract |
| Keeper | Keeper Owner | Dexter treasury multisig contract |
| Multistaking | Multistaking Owner | Dexter Governance Admin contract |

## Contracts

Each contract exposes some functions which could be executed by one of the admins mentioned above.

### Vault

**Vault Owner**: Vault owner has most control over the protocol. It can add pool managers, update the vault config, or update the config of any of the existing pools or pool types. 

All of the functions that the vault owner can execute be found [here](<LINK TO VAULT DOC>)

The parameters that can be configured are:

| Parameter | Description |
| --- | --- |
| Fee Collector Address | Address of the contract which collects fees. Currently, the Dexter keeper contract |
| LP Token Code ID | Code ID of the LP token contract. This is used to create new LP tokens for new pools. Ideally not updated post initial deployment |
| Pool Creation Fee | Fee in the specified denom for creating a new pool. This is used to prevent spamming of the pool creation feature. |
| Auto-stake implementation | Address of the contract which implements the auto-stake feature. Currently, the Dexter multistaking contract |
| Whitelisted managers | List of addresses which currently have the manager role |
| Default fee for a pool type | Default fee for a pool type. This is used when a new pool of that type is created. |
| Pool type pause configuration | Pause configuration for each pool type. This is used to pause/unpause specific operations for a pool type. |
| Pool pause configuration | Pause configuration for each pool. This is used to pause/unpause specific operations for a pool. |

<br>

**Pool Manager**: Pool manager can create new pools, or pause/unpause existing pools.

All of the functions that the pool manager can execute be found [here](<LINK TO POOL MANAGER DOC>)

### Stable Pool

**Vault Owner**: Vault owner can update the config of the stable pool type. The parameters that can be updated are:

| Parameter | Description |
| --- | --- |
| AMP | The stable swap amplification factor |
| Scaling factor manager | The address of the scaling factor manager. Scaling factor manager is responsible for updating the scaling factor of one (or more) assets in the pool |
| Max allowed spread | Max allowed spread between the price of the asset and the price of the pool. If the spread is greater than this value, the swap will fail |

Entry point related information can be found [here](<LINK TO STABLE POOL DOC>)

### Weighted Pool

Weighted pool doesn't currently have any configurable parameters.

### Multistaking

**Multistaking Owner**: Multistaking owner can create new reward schedules, or add/remove whitelisted tokens for rewards. 

All of the functions that the multistaking owner can execute be found [here](../contracts/multi_staking#owner-executable)

To summarize, the parameters that can be configured are:

| Parameter | Description |
| --- | --- |
| keeper_addr | Fee collector address |
| unlock_period | Lockup period after unbonding for which the tokens are in an unlocking state |
| instant_unbond_fee_bp | Fee in basis points for Instant LP unbonding feature |
| instant_unbond_min_fee_bp | Min fee for tier based fee structure for instant-unlock feature for unlocking liquidity during the unlocking period |
| fee_tier_interval | Interval for fee tier change. The unlock period is divided into tiers for partial unbond fee during unlock period |



### Keeper 

**Keeper Owner**: Keeper owner can withdraw the funds set aside as part of the protocol treasury(30% of the swap fees)  that are generated from the Swaps.

All of the functions that the keeper owner can execute be found [here](<LINK TO KEEPER DOC>)



