# dexter

dex based on cosmwasm
![Untitled](https://s3-us-west-2.amazonaws.com/secure.notion-static.com/ca31d66a-3745-4529-9a7e-61c6cabdd623/Untitled.png)

## **Dexter :: Architecture Overview**

Dexter is the first DEX which is implemented as a generalized state transition executor where the transition’s math computes are queried from the respective Pool contracts, enabling a decentralized, non-custodial aggregated liquidity and exchange rate discovery among different tokens on Persistence.

![Dexter :: Architecture Overview](./docs/overview.png)

At launch, Dexter will be supporting the following pool types,

- XYK Pool
- Stableswap Pool
- Stable3 Pool
- Weighted Pool
