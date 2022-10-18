NODE_URL := https://rpc.devnet.core.dexter.zone:443
POOL_ID?=1
VAULT_CONTRACT_ADDRESS := ${shell cat artifacts/persistencecore.json | jq -r ".vault_contract_address"}
XYK_POOL_CW_ASSET_ADDRESS := ${shell cat artifacts/persistencecore.json | jq -r ".xyk_pool_asset_infos[1].token.contract_addr"}
XYK_POOL_LP_TOKEN_ADDRESS := ${shell cat artifacts/persistencecore.json | jq -r ".xyk_pool_lp_token_address"}
# VAULT_CONTRACT_ADDRESS := persistence14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9sjvz4fk

JOIN_POOL_REQUEST := ${shell jq -c . < artifacts/contract-tx/join-pool-args.json}
SWAP_REQUEST := ${shell jq -c . < artifacts/contract-tx/swap-args.json}
SWAP_REVERSE_REQUEST := ${shell jq -c . < artifacts/contract-tx/swap-reverse-args.json}
EXIT_POOL_RECEIVE_MSG := ${shell base64 artifacts/contract-tx/exit-pool-receive-args.json}

# EXIT_POOL_REQUEST := ${shell jq -c . < artifacts/contract-tx/exit-pool-request.json}
EXIT_POOL_REQUEST := ${shell cat artifacts/contract-tx/exit-pool-request.json | sed -e "s/RECEIVE_BINARY_DATA/${EXIT_POOL_RECEIVE_MSG}/g" | jq -c}

# build-all-contracts:
	

query-pool:
	persistenceCore q wasm contract-state smart $(VAULT_CONTRACT_ADDRESS) \
	'{"get_pool_by_id": {"pool_id":"${POOL_ID}"}}' \
	--node $(NODE_URL)

query-events:
	persistenceCore q txs \
	--events wasm._contract_address=$(VAULT_CONTRACT_ADDRESS)  \
	--node https://rpc.devnet.core.dexter.zone:443 \
	--output json > output.json

query-lp-token-events:
	persistenceCore q txs \
	--events wasm._contract_address=$(XYK_POOL_LP_TOKEN_ADDRESS)  \
	--node https://rpc.devnet.core.dexter.zone:443 \
	--output json > lp_events_output.json

query-registry:
	persistenceCore q wasm contract-state smart $(VAULT_CONTRACT_ADDRESS) \
	'{"query_rigistery": {"pool_type":{"xyk":{}}}}' \
	--node $(NODE_URL)

add-pool-liquidity:
	echo "Allowing Vault address to Spend CW20 Pool Asset"
	persistenceCore tx wasm execute $(XYK_POOL_CW_ASSET_ADDRESS) \
		'{"increase_allowance":{"amount":"100000","spender":"${VAULT_CONTRACT_ADDRESS}"}}' \
		--node https://rpc.devnet.core.dexter.zone:443 \
		--from test \
		--keyring-backend test \
		--chain-id persistencecore \
		--gas 2000000 \
		-b block

	echo "Sending pool join request .."
	persistenceCore tx wasm execute $(VAULT_CONTRACT_ADDRESS) \
		'$(JOIN_POOL_REQUEST)'  \
		--node https://rpc.devnet.core.dexter.zone:443 \
		--from test \
		--keyring-backend test \
		--chain-id persistencecore \
		-b block \
		--gas 2000000 \
		--amount 100000uxprt

exit-pool:
	echo $(EXIT_POOL_REQUEST)
	persistenceCore tx wasm execute $(XYK_POOL_LP_TOKEN_ADDRESS) \
		'$(EXIT_POOL_REQUEST)' \
		--node https://rpc.devnet.core.dexter.zone:443 \
		--from test \
		--keyring-backend test \
		--chain-id persistencecore \
		-b block \
		--gas 2000000

swap-asset:
	persistenceCore tx wasm execute persistence14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9sjvz4fk \
		'$(SWAP_REQUEST)'  \
		--node https://rpc.devnet.core.dexter.zone:443 \
		--from test \
		--keyring-backend test \
		--chain-id persistencecore \
		-b block \
		--gas 2000000 \
		--amount 1000uxprt

swap-asset-reverse:
	persistenceCore tx wasm execute $(XYK_POOL_CW_ASSET_ADDRESS) \
		'{"increase_allowance":{"amount":"10000","spender":"${VAULT_CONTRACT_ADDRESS}"}}' \
		--node https://rpc.devnet.core.dexter.zone:443 \
		--from test \
		--keyring-backend test \
		--chain-id persistencecore \
		--gas 2000000 \
		-b block
	
	persistenceCore tx wasm execute persistence14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9sjvz4fk \
		'$(SWAP_REVERSE_REQUEST)'  \
		--node https://rpc.devnet.core.dexter.zone:443 \
		--from test \
		--keyring-backend test \
		--chain-id persistencecore \
		-b block \
		--gas 2000000