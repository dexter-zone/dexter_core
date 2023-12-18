# Dexter: Stableswap Pool

Dexter implements a generic version of Curve's stableswap invariant for upto 5 assets in the pool and implements compute calculations on liquidity provision / withdrawal and swaps.

Dexter's contract architecture is unique in that it separates the ownership of the assets in the pool in the Vault contract. Pool contracts are only responsible for the math computes which dictate number of tokens to be transferred during swaps / liquidity provisioning events, and do not handle the token transfers themselves. 

Dexter's Vault queries the Pool contracts to compute how many tokens to transfer and processes those transfers itself.

This separation simplifies pool contracts, since they no longer need to actively manage their assets; pools only need to calculate amounts for swaps, joins, and exits. New pool types, can be easily added which only implement the math computes and do not need to worry about the token transfer logic.


## Contract State


