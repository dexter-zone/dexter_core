import imp
from cosmos_sdk.client.lcd import LCDClient
from cosmos_sdk.key.mnemonic import MnemonicKey
from cosmos_sdk.core.wasm import  MsgExecuteContract 
from cosmos_sdk.core.fee import Fee
from cosmos_sdk.core import Coins, Coin
import base64
import json



class dexter_helpers_mixin():

    ###############################################
    ################ VAULT QUERIES ################
    ###############################################

    def query_vault_config(self,  contract_addr):
        try:
            sim_response = self.client.wasm.contract_query(contract_addr , {"config":{}})
            return sim_response         
        except:
            return None

    def query_vault_query_registery(self,  contract_addr, pool_type):
        try:
            sim_response = self.client.wasm.contract_query(contract_addr , {"query_rigistery" : { pool_type: pool_type }})
            return sim_response         
        except:
            return None

    def query_vault_GetPoolById(self,  contract_addr, pool_id):
        try:
            sim_response = self.client.wasm.contract_query(contract_addr ,  { "get_pool_by_id" : { pool_id: pool_id }})
            return sim_response         
        except:
            return None

    def query_vault_GetPoolByAddress(self,  contract_addr, pool_addr):
        try:
            sim_response = self.client.wasm.contract_query(contract_addr ,  { "get_pool_by_address" : { pool_addr: pool_addr }})
            return sim_response         
        except:
            return None


    ###############################################
    ################ POOL QUERIES ################
    ###############################################

    def query_pool_config(self,  contract_addr):
        try:
            sim_response = self.client.wasm.contract_query(contract_addr , {"config":{}})
            return sim_response         
        except:
            return None

    def query_pool_fee_params(self,  contract_addr):
        try:
            sim_response = self.client.wasm.contract_query(contract_addr , {"fee_params":{}})
            return sim_response         
        except:
            return None


    def query_pool_pool_id(self,  contract_addr):
        try:
            sim_response = self.client.wasm.contract_query(contract_addr , {"pool_id":{}})
            return sim_response         
        except:
            return None

    def query_pool_on_join_pool(self,  contract_addr, assets_in=None, mint_amount=None, slippage_tolerance=None):
        try:
            sim_response = self.client.wasm.contract_query(contract_addr , {"on_join_pool":{ 
                assets_in: assets_in,
                mint_amount: mint_amount,
                slippage_tolerance: slippage_tolerance
            }})
            return sim_response         
        except:
            return None

    def query_pool_on_exit_pool(self,  contract_addr, assets_out=None, burn_amount=None):
        try:
            sim_response = self.client.wasm.contract_query(contract_addr , {"on_exit_pool":{ 
                assets_out: assets_out,
                burn_amount: burn_amount
            }})
            return sim_response         
        except:
            return None

    def query_pool_on_swap(self,  contract_addr,swap_type,offer_asset,ask_asset,amount, max_spread=None, belief_price=None):
        try:
            sim_response = self.client.wasm.contract_query(contract_addr , {"on_swap":{ 
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


    def query_cumulative_price(self,  contract_addr, offer_asset, ask_asset):
        try:
            sim_response = self.client.wasm.contract_query(contract_addr , {"cumulative_price":{ 
            offer_asset: offer_asset,
            ask_asset: ask_asset,
            }})
            return sim_response         
        except:
            return None


    def query_cumulative_prices(self,  contract_addr):
        try:
            sim_response = self.client.wasm.contract_query(contract_addr , {"cumulative_prices":{ 
            }})
            return sim_response         
        except:
            return None


    ###############################################
    ########### VAULT TRANSACTIONS ################
    ###############################################

    def execute_vault_UpdateConfig(self,  wallet, vault_addr,lp_token_code_id=None,fee_collector=None,generator_address=None ):
        msg = { "update_config": {'lp_token_code_id': lp_token_code_id,  "fee_collector": fee_collector, "generator_address":generator_address  }}
        convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
        tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
        res = self.client.tx.broadcast(tx)

    def execute_vault_UpdatePoolConfig(self,  wallet, vault_addr,pool_type,is_disabled=None,new_fee_info=None ):
        msg = { "update_pool_config": {'pool_type': pool_type,  "is_disabled": is_disabled, "new_fee_info":new_fee_info  }}
        convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
        tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
        res = self.client.tx.broadcast(tx)

    def execute_vault_AddToRegistery(self,  wallet, vault_addr,new_pool_config):
        msg = { "add_to_registery": {'new_pool_config': new_pool_config }}
        convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
        tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
        res = self.client.tx.broadcast(tx)

    def execute_vault_CreatePoolInstance(self,  wallet, vault_addr, pool_type, asset_infos, lp_token_name=None, lp_token_symbol=None, init_params=None ):
        msg = { "create_pool_instance": {        pool_type: pool_type,
            asset_infos: asset_infos,
            lp_token_name: lp_token_name,
            lp_token_symbol: lp_token_symbol,
            init_params: init_params }}
        convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
        tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
        res = self.client.tx.broadcast(tx)

    def execute_vault_JoinPool(self,  wallet, vault_addr, pool_id, recipient=None, 
                                assets=None, lp_to_mint=None, init_params=None ):
        msg = { "join_pool": {         
            pool_id: pool_id,
            recipient: recipient,
            assets:assets,
            lp_to_mint: lp_to_mint,
            init_params: init_params }}
        convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
        tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
        res = self.client.tx.broadcast(tx)

    def execute_vault_Swap(self,  wallet, vault_addr, swap_request, recipient=None ):
        msg = { "swap": {         
            swap_request: swap_request,
            recipient: recipient }}
        convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
        tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
        res = self.client.tx.broadcast(tx)

    def execute_vault_ProposeNewOwner(self,  wallet, vault_addr, owner, expires_in=None ):
        msg = { "propose_new_owner": {         
            owner: owner,
            expires_in: expires_in }}
        convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
        tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
        res = self.client.tx.broadcast(tx)

    def execute_vault_DropOwnershipProposal(self,  wallet, vault_addr):
        msg = { "drop_ownership_proposal": {   }}
        convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
        tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
        res = self.client.tx.broadcast(tx)

    def execute_vault_ClaimOwnership(self,  wallet, vault_addr):
        msg = { "claim_ownership": {}}
        convertMsgPrep = MsgExecuteContract(wallet.key.acc_address, vault_addr, msg)
        tx = wallet.create_and_sign_tx(msgs=[convertMsgPrep], fee=Fee(5000000, Coins(uxprt=6250000)),)
        res = self.client.tx.broadcast(tx)


    def execute_vault_exit_pool(self,  wallet, vault_addr, lp_token_addr, amount, pool_id,  recipient=None, assets=None, burn_amount=None  ):
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
        res = self.client.tx.broadcast(tx)


    def dict_to_b64(data: dict) -> str:
        return base64.b64encode(bytes(json.dumps(data), "ascii")).decode()