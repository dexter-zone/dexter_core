#!/bin/bash
set -e

cat > scripts/proposals/vault_proposal.json <<EOF
{
  "propose": {
    "title": "Update Dexter Vault Configuration",
    "description": "Set the primary vault config with pool types, fees, and multistaking integration",
    "msgs": [
      {
        "wasm": {
          "execute": {
            "contract_addr": "bbn18rdj3asllguwr6lnyu2sw8p8nut0shuj3sme27ndvvw4gakjnjqqczzj4x",
            "msg": "$(cat scripts/proposals/msg1.b64)",
            "funds": []
          }
        }
      },
      {
        "wasm": {
          "execute": {
            "contract_addr": "bbn18rdj3asllguwr6lnyu2sw8p8nut0shuj3sme27ndvvw4gakjnjqqczzj4x",
            "msg": "$(cat scripts/proposals/msg2.b64)",
            "funds": []
          }
        }
      },
      {
        "wasm": {
          "execute": {
            "contract_addr": "bbn18rdj3asllguwr6lnyu2sw8p8nut0shuj3sme27ndvvw4gakjnjqqczzj4x",
            "msg": "$(cat scripts/proposals/msg3.b64)",
            "funds": []
          }
        }
      }
    ]
  }
}
EOF

cat > scripts/proposals/whitelist_lp_token_proposal.json <<EOF
{
  "propose": {
    "title": "Whitelist LP Token in MultiStaking",
    "description": "Allow the LP token in the Dexter MultiStaking contract",
    "msgs": [
      {
        "wasm": {
          "execute": {
            "contract_addr": "bbn1jvl8avv45sj92q9x9c84fq2ymddya6dkwv9euf7y365tkzma38zqda76q4",
            "msg": "$(echo '{\"allow_lp_token\":{\"lp_token\":\"bbn16kac4jc6fzv7lct62yc5uxtf8pvyl8jwem6ygwkh88hm089rp9hsa8c0kp\"}}' | base64 | tr -d '\n')",
            "funds": []
          }
        }
      }
    ]
  }
}
EOF

echo "To submit the proposal, run the following command:"
echo "babylond tx wasm execute bbn1wawu0fe6jy2w9ngaf89xs3mwgfsm0fpdtumfls4wx3ltwcp7amqq5tug7s \"\$(cat scripts/proposals/vault_proposal.json)\" --from dexter-multisig --keyring-backend file --keyring-dir . --gas auto --gas-adjustment 1.5 --node \$BABYLON_RPC_URL --chain-id \$BABYLON_CHAIN_ID --fees 600ubbn"

echo "To submit the whitelist LP token proposal, run the following command:"
echo "babylond tx wasm execute bbn1wawu0fe6jy2w9ngaf89xs3mwgfsm0fpdtumfls4wx3ltwcp7amqq5tug7s \"\$(cat scripts/proposals/whitelist_lp_token_proposal.json)\" --from dexter-multisig --keyring-backend file --keyring-dir . --gas auto --gas-adjustment 1.5 --node \$BABYLON_RPC_URL --chain-id \$BABYLON_CHAIN_ID --fees 600ubbn"