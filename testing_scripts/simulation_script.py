from cosmos_sdk.client.lcd import LCDClient
from cosmos_sdk.key.mnemonic import MnemonicKey
from cosmos_sdk.core.wasm import  MsgExecuteContract 
from cosmos_sdk.core.fee import Fee
from cosmos_sdk.core import Coins, Coin
import base64
import json

# client = LCDClient(chain_id="core-1", url="http://rest.core.persistence.one")
client = LCDClient(chain_id="test-core-1", url="http://rest.testnet.persistence.one")


num = client.tendermint.block_info()['block']['header']['height']
mnemonic = "toss hammer lazy dish they ritual suggest favorite sword alcohol enact enforce mechanic spoon gather knock giggle indicate indicate nose actor brand basket confirm"
wallet = client.wallet(MnemonicKey(mnemonic,"persistence"))
addr = wallet.key.acc_address
print(f"Wallet address = {addr}")

addresses = {
  "test_tokens_addresses": [
    "persistence1vguuxez2h5ekltfj9gjd62fs5k4rl2zy5hfrncasykzw08rezpfst7tmng",
    "persistence1rl8su3hadqqq2v86lscpuklsh2mh84cxqvjdew4jt9yd07dzekyq85jyzr",
    "persistence1vhjnzk9ly03dugffvzfcwgry4dgc8x0sv0nqqtfxj3ajn7rn5ghqtpaner",
    "persistence1u2zdjcczjrenwmf57fmrpensk4the84azdm05m3unm387rm8asdsh0yf27",
    "persistence1rtdulljz3dntzpu085c7mzre9dg4trgdddu4tqk7uuuvu6xrfu8s8wcs45",
    "persistence13hwj6afyxgue26f966hd5jkcvvjeruv7f9cdtd5d9mrtyrnn73ysyxvc8c",
    "persistence1gd54cnu80s8qdqcyhyvn06m87vlmch2uf4wvz4z08svawvc2rhysgvav55"
  ],
  "xyk_pool_addr": "persistence1lxansfc8vkujy997e3xksd3ugsppv6a9jt32pjtgaxr0zkcnkznqu22a4s",
  "xyk_lp_token_addr": "persistence186k0cp83c3wyvapgh8fxf66ededemzrfujvjfsx0xw3vr0u9g8sqmtm0ly",
  "xyk_2_pool_addr": "persistence1xx35wwa2nhfvfm50lj3ukv077mjxuy9pefxxnctxe9kczk6tz3hq8j7lt0",
  "xyk_2_lp_token_addr": "persistence1s3pk90ccfl6ueehnj8s9pdgyjjlspmr3m5rv46arjh5v4g08dd0qjhajs5",
  "stableswap_pool_addr": "persistence1kkwp7pd4ts6gukm3e820kyftz4vv5jqtmal8pwqezrnq2ddycqas9nk2dh",
  "stableswap_lp_token_addr": "persistence1h4qltxx7tcdye2kkwj8ksedad0xr3frdusrdga97wf3mjcpx6qwqa6ayuz",
  "stableswap_2_pool_addr": "persistence1acrmqqyqq9gwcy2upegzncahqwnzjzy89pssyt0s3ghwsrrqy94srfsw6r",
  "stableswap_2_lp_token_addr": "persistence1kj45m8j2pqrqlw67tqde8lduzla7me38fps8tzzjl2emgp90f0gqjjf5sk",
  "stable5swap_pool_addr": "persistence1a7pjjyvng22a8msatp4zj6ut9tmsd9qvp26gaj7tnrjrqtx7yafqm7ezny",
  "stable5swap_lp_token_addr": "persistence17jllkv6clrkrwsuyxpya505rnhzwenkr4njw3um5eyqjuqm4twzqlt82eh",
  "stable5swap_2_pool_addr": "persistence1aexzn458dzh0lnuqdtzjtacq6tacnluz9ky643xdvw67en2yh97sjq6txg",
  "stable5swap_2_lp_token_addr": "persistence18yqlanxjqxx5lr8r43hsvjf0wyrlec3r8rpxgm2svrh52mzmlh4scappxa",
  "weighted_pool_addr": "persistence1j5h5zftg5su7ytz74f7rryl4f6x3p78lh907fw39eqhax75r94jsgj4n54",
  "weighted_lp_token_addr": "persistence1ejycngcuqyw2h8afhlzkq0cmjegpt96x583jh99anjzeut2rm4sqf0x4wk",
  "vault_contract_address": "persistence1jyhyqjxf3pc7vzwyqhwe53up5pj0e53zw3xu2589uqgkvqngswnqgrmstf",
}



###############################################
################ VAULT QUERIES ################
###############################################

def query_vault_config(client, contract_addr):
    try:
        sim_response = client.wasm.contract_query(contract_addr , {"config":{}})
        return sim_response         
    except:
        return None

res = query_vault_config(client, addresses["vault_contract_address"])
print(res)

def query_vault_query_registery(client, contract_addr, pool_type):
    try:
        sim_response = client.wasm.contract_query(contract_addr , {"query_rigistery" : { pool_type: pool_type }})
        return sim_response         
    except:
        return None

def query_vault_GetPoolById(client, contract_addr, pool_id):
    try:
        sim_response = client.wasm.contract_query(contract_addr ,  { "get_pool_by_id" : { pool_id: pool_id }})
        return sim_response         
    except:
        return None

def query_vault_GetPoolByAddress(client, contract_addr, pool_addr):
    try:
        sim_response = client.wasm.contract_query(contract_addr ,  { "get_pool_by_address" : { pool_addr: pool_addr }})
        return sim_response         
    except:
        return None


###############################################
################ POOL QUERIES ################
###############################################

def query_pool_config(client, contract_addr):
    try:
        sim_response = client.wasm.contract_query(contract_addr , {"config":{}})
        return sim_response         
    except:
        return None

def query_pool_fee_params(client, contract_addr):
    try:
        sim_response = client.wasm.contract_query(contract_addr , {"fee_params":{}})
        return sim_response         
    except:
        return None


def query_pool_pool_id(client, contract_addr):
    try:
        sim_response = client.wasm.contract_query(contract_addr , {"pool_id":{}})
        return sim_response         
    except:
        return None

def query_pool_on_join_pool(client, contract_addr, assets_in=None, mint_amount=None, slippage_tolerance=None):
    try:
        sim_response = client.wasm.contract_query(contract_addr , {"on_join_pool":{ 
            assets_in: assets_in,
            mint_amount: mint_amount,
            slippage_tolerance: slippage_tolerance
         }})
        return sim_response         
    except:
        return None

def query_pool_on_exit_pool(client, contract_addr, assets_out=None, burn_amount=None):
    try:
        sim_response = client.wasm.contract_query(contract_addr , {"on_exit_pool":{ 
            assets_out: assets_out,
            burn_amount: burn_amount
         }})
        return sim_response         
    except:
        return None

def query_pool_on_swap(client, contract_addr,swap_type,offer_asset,ask_asset,amount, max_spread=None, belief_price=None):
    try:
        sim_response = client.wasm.contract_query(contract_addr , {"on_swap":{ 
        swap_type: swap_type,
        offer_asset: offer_asset,
        ask_asset: ask_asset,
        amount: amount,
        max_spread: max_spread,
        belief_price: belief_price,
         }})
        return sim_response         
    except:
        return None


def query_cumulative_price(client, contract_addr, offer_asset, ask_asset):
    try:
        sim_response = client.wasm.contract_query(contract_addr , {"cumulative_price":{ 
        offer_asset: offer_asset,
        ask_asset: ask_asset,
         }})
        return sim_response         
    except:
        return None


def query_cumulative_prices(client, contract_addr):
    try:
        sim_response = client.wasm.contract_query(contract_addr , {"cumulative_prices":{ 
         }})
        return sim_response         
    except:
        return None


###############################################
########### VAULT TRANSACTIONS ################
###############################################

def execute_vault_UpdateConfig(client, wallet, vault_addr,lp_token_code_id=None,fee_collector=None,generator_address=None ):
    msg = { "update_config": {'lp_token_code_id': lp_token_code_id,  "fee_collector": fee_collector, "generator_address":generator_address  }}
    convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
    tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
    res = client.tx.broadcast(tx)

def execute_vault_UpdatePoolConfig(client, wallet, vault_addr,pool_type,is_disabled=None,new_fee_info=None ):
    msg = { "update_pool_config": {'pool_type': pool_type,  "is_disabled": is_disabled, "new_fee_info":new_fee_info  }}
    convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
    tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
    res = client.tx.broadcast(tx)

def execute_vault_AddToRegistery(client, wallet, vault_addr,new_pool_config):
    msg = { "add_to_registery": {'new_pool_config': new_pool_config }}
    convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
    tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
    res = client.tx.broadcast(tx)

def execute_vault_CreatePoolInstance(client, wallet, vault_addr, pool_type, asset_infos, lp_token_name=None, lp_token_symbol=None, init_params=None ):
    msg = { "create_pool_instance": {        pool_type: pool_type,
        asset_infos: asset_infos,
        lp_token_name: lp_token_name,
        lp_token_symbol: lp_token_symbol,
        init_params: init_params }}
    convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
    tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
    res = client.tx.broadcast(tx)

def execute_vault_JoinPool(client, wallet, vault_addr, pool_id, recipient=None, 
                            assets=None, lp_to_mint=None, init_params=None ):
    msg = { "join_pool": {         
        pool_id: pool_id,
        recipient: recipient,
        assets:assets,
        lp_to_mint: lp_to_mint,
        init_params: init_params }}
    convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
    tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
    res = client.tx.broadcast(tx)

def execute_vault_Swap(client, wallet, vault_addr, swap_request, recipient=None ):
    msg = { "swap": {         
        swap_request: swap_request,
        recipient: recipient }}
    convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
    tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
    res = client.tx.broadcast(tx)

def execute_vault_ProposeNewOwner(client, wallet, vault_addr, owner, expires_in=None ):
    msg = { "propose_new_owner": {         
        owner: owner,
        expires_in: expires_in }}
    convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
    tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
    res = client.tx.broadcast(tx)

def execute_vault_DropOwnershipProposal(client, wallet, vault_addr):
    msg = { "drop_ownership_proposal": {   }}
    convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
    tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
    res = client.tx.broadcast(tx)

def execute_vault_ClaimOwnership(client, wallet, vault_addr):
    msg = { "claim_ownership": {}}
    convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
    tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
    res = client.tx.broadcast(tx)


def execute_vault_exit_pool(client, wallet, vault_addr, lp_token_addr, amount, pool_id,  recipient=None, assets=None, burn_amount=None  ):
    exit_msg = { "exit_pool": {
                "pool_id": pool_id,
                "recipient": recipient,
                "assets": assets,
                "burn_amount": burn_amount,
            }}
    cw20_send = { "send": {
        "contract_addr": vault_addr,
        "amount" : amount,
        "msg": dict_to_b64(exit_msg)
    } }
    convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, lp_token_addr, cw20_send)
    tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
    res = client.tx.broadcast(tx)


def dict_to_b64(data: dict) -> str:
    return base64.b64encode(bytes(json.dumps(data), "ascii")).decode()