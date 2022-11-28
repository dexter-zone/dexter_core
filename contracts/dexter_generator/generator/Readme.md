# Dexter - Generator Contract

The Generator contract here is taken from the Astroport's generator contract and supports allocating token rewards (DEX tokens) for various LP tokens and distributing them pro-rata to LP stakers. The Generator also supports proxy staking via 3rd party contracts that offer a second reward besides the DE token emissions. Allowed reward proxies are managed via a whitelist.

## Contract State

| Message              | Description                                                                                                                                                              |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `CONFIG`             | Stores Generator contract's core Configuration parameters in a [`Config`] struct                                                                                         |
| `POOL_INFO`          | This is a map that contains information about all generators. The key is the address of a LP token, the value is an object of type [`PoolInfo`].                         |
| `USER_INFO`          | This is a map that contains information about all stakers. The key is a concatenation of user address and LP token address, the value is an object of type [`UserInfo`]. |
| `PROXY_REWARD_ASSET` | The key-value here maps proxy contract addresses to the associated reward assets                                                                                         |
| `OWNERSHIP_PROPOSAL` | The item here stores the proposal to change contract ownership.                                                                                                          |
| `TMP_USER_ACTION`    | The item used during chained Msg calls to keep store of which msg is to be called                                                                                        |

## Supported Execute Msgs

| Message                   | Description                                                                                                                                                    |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `SetTokensPerBlock`       | Admin function. Set a new amount of DEX tokens to distribute per block                                                                                         |
| `UpdateConfig`            | Admin function. Failitates updating some of the configuration param of the Dexter Generator Contract                                                           |
| `SetupPools`              | Admin function. Setup generators with their respective allocation points.                                                                                      |
| `DeactivatePool`          | Admin function. Sets the allocation points to zero for the generator associated with the specified LP token. Recalculates total allocation points.             |
| `SetAllowedRewardProxies` | Admin function. Allowed reward proxy contracts that can interact with the Generator.                                                                           |
| `SendOrphanProxyReward`   | Admin function. Sends orphan proxy rewards (which were left behind after emergency withdrawals) to another address                                             |
| `UpdateAllowedProxies`    | Admin function. Add or remove a proxy contract that can interact with the Generator                                                                            |
| `Cw20HookMsg`             | Update rewards and transfer them to user.                                                                                                                      |
| `Cw20HookMsg::Deposit`    | Deposit performs a token deposit on behalf of the message sender.                                                                                              |
| `Cw20HookMsg::DepositFor` | DepositFor performs a token deposit on behalf of another address that's not the message sender.                                                                |
| `ClaimRewards`            | ClaimRewards updates rewards and transfer accrued rewards to the user.                                                                                         |
| `Unstake`                 | Unstake LP tokens from the Generator. LP tokens need to be unbonded for a period of time before they can be withdrawn.                                         |
| `EmergencyUnstake`        | Unstake LP tokens from the Generator without withdrawing outstanding rewards. LP tokens need to be unbonded for a period of time before they can be withdrawn. |
| `Unlock`                  | Unlock and withdraw LP tokens from the Generator                                                                                                               |
| `ProposeNewOwner`         | Admin function. Creates a request to change contract ownership                                                                                                 |
| `DropOwnershipProposal`   | Admin function. Removes a request to change contract ownership                                                                                                 |
| `ClaimOwnership`          | New Admin function. Claims contract ownership block                                                                                                            |

## Supported Query Msgs

| Message                | Description                                                                                                                                                                                     |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Config`               | Config returns the main contract parameters                                                                                                                                                     |
| `ActivePoolLength`     | Admin function. Set a new amount of DEX tokens to distribute per block                                                                                                                          |
| `PoolLength`           | Returns the length of the array that contains all the active pool generators                                                                                                                    |
| `Deposit`              | Deposit returns the LP token amount deposited in a specific generator                                                                                                                           |
| `PendingToken`         | PendingToken returns the amount of rewards that can be claimed by an account that deposited a specific LP token in a generator                                                                  |
| `RewardInfo`           | RewardInfo returns reward information for a specified LP token                                                                                                                                  |
| `OrphanProxyRewards`   | OrphanProxyRewards returns orphaned reward information for the specified LP token                                                                                                               |
| `PoolInfo`             | PoolInfo returns information about a pool associated with the specified LP token alongside the total pending amount of DEX and proxy rewards claimable by generator stakers (for that LP token) |
| `SimulateFutureReward` | SimulateFutureReward returns the amount of DEX that will be distributed until a future block and for a specific generator                                                                       |
