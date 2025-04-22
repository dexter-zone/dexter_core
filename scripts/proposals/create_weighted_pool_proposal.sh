#!/bin/bash
set -e

# Addresses and denoms
VAULT_CONTRACT="bbn18rdj3asllguwr6lnyu2sw8p8nut0shuj3sme27ndvvw4gakjnjqqczzj4x"
MULTISIG="dexter-multisig"
IBC_DENOM="ibc/632D09C1324B38144BD3B8879EC56303496798209341138900F338CCDBADD970"

# Prepare the CreatePoolInstance message for a $BABY/$XPRT pool, assets sorted alphabetically (ubbn, then IBC)
cat > scripts/proposals/msg_weighted_pool.json <<EOF
{
  "create_pool_instance": {
    "pool_type": { "weighted": {} },
    "asset_infos": [
      { "native_token": { "denom": "$IBC_DENOM" } },
      { "native_token": { "denom": "ubbn" } }
    ],
    "native_asset_precisions": [
      { "denom": "$IBC_DENOM", "precision": 6 },
      { "denom": "ubbn", "precision": 6 }
    ],
    "init_params": "$(echo '{"weights":[{"info":{"native_token":{"denom":"'$IBC_DENOM'"}}, "amount":"1"},{"info":{"native_token":{"denom":"ubbn"}}, "amount":"1"}],"exit_fee":"0"}' | base64 | tr -d '\n')",
    "fee_info": null
  }
}
EOF

# Encode the message in base64 (as required by proposal format)
cat scripts/proposals/msg_weighted_pool.json | base64 | tr -d '\n' > scripts/proposals/msg_weighted_pool.b64

# Write the proposal JSON (no comments)
cat > scripts/proposals/weighted_pool_proposal.json <<EOF
{
  "propose": {
    "title": "Create 50/50 Weighted Pool: \$BABY/\$XPRT",
    "description": "Create a weighted pool between \$BABY (ubbn) and \$XPRT ($IBC_DENOM) with 50/50 weights",
    "msgs": [
      {
        "wasm": {
          "execute": {
            "contract_addr": "$VAULT_CONTRACT",
            "msg": "$(cat scripts/proposals/msg_weighted_pool.b64)",
            "funds": []
          }
        }
      }
    ]
  }
}
EOF

echo "To submit the proposal, run the following command:"
echo "babylond tx wasm execute $VAULT_CONTRACT \"\$(cat scripts/proposals/weighted_pool_proposal.json)\" --from $MULTISIG --keyring-backend file --keyring-dir . --gas auto --gas-adjustment 1.5 --node \$BABYLON_RPC_URL --chain-id \$BABYLON_CHAIN_ID --fees 600ubbn"
