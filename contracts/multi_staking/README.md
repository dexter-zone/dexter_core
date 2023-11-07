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
Creates a new reward schedule. Owner can create a reward sFchedule on the behalf of a user.







