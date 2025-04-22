#!/bin/bash
set -e

mkdir -p scripts/proposals

# msg1.json: update_config
cat > scripts/proposals/msg1.json <<EOF
{
  "update_config": {
    "lp_token_code_id": 50,
    "fee_collector": "bbn1w50vqkv30lhmkyq4786kgfxezhwstq20rvxjynye9l03vsy89fuq74grp0",
    "pool_creation_fee": "disabled",
    "auto_stake_impl": {
      "multistaking": {
        "contract_addr": "bbn1jvl8avv45sj92q9x9c84fq2ymddya6dkwv9euf7y365tkzma38zqda76q4"
      }
    },
    "paused": {
      "swap": false,
      "deposit": false,
      "imbalanced_withdraw": false
    }
  }
}
EOF

# msg2.json: add_to_registry for weighted pool
cat > scripts/proposals/msg2.json <<EOF
{
  "add_to_registry": {
    "new_pool_type_config": {
      "code_id": 51,
      "pool_type": { "weighted": {} },
      "default_fee_info": { "total_fee_bps": 50, "protocol_fee_percent": 30 },
      "allow_instantiation": "only_whitelisted_addresses",
      "paused": { "swap": false, "deposit": false, "imbalanced_withdraw": false }
    }
  }
}
EOF

# msg3.json: add_to_registry for stableswap pool
cat > scripts/proposals/msg3.json <<EOF
{
  "add_to_registry": {
    "new_pool_type_config": {
      "code_id": 52,
      "pool_type": { "stable_swap": {} },
      "default_fee_info": { "total_fee_bps": 50, "protocol_fee_percent": 30 },
      "allow_instantiation": "only_whitelisted_addresses",
      "paused": { "swap": false, "deposit": false, "imbalanced_withdraw": false }
    }
  }
}
EOF

echo "msg1.json, msg2.json, msg3.json created in scripts/proposals/"