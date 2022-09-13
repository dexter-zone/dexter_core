# from model import DexterModel
import asyncio
from cosmos_sdk.client.lcd import LCDClient
from cosmos_sdk.key.mnemonic import MnemonicKey
from cosmos_sdk.core.wasm import  MsgExecuteContract 
from cosmos_sdk.core.fee import Fee
from cosmos_sdk.core import Coins, Coin


from config import CHAIN_ID, LCD_URL
from mixin_dexter_helper import dexter_helpers_mixin


class DexterModel(dexter_helpers_mixin):
    def __init__(self):

        mnemonic = "toss hammer lazy dish they ritual suggest favorite sword alcohol enact enforce mechanic spoon gather knock giggle indicate indicate nose actor brand basket confirm"


        self.client = LCDClient(chain_id=CHAIN_ID, url=LCD_URL)
        self.wallet = self.client.wallet(MnemonicKey(mnemonic,"persistence"))

        block_num = self.client.tendermint.block_info()['block']['header']['height']
        self.wallet_addr = self.wallet.key.acc_address
        print(f"Wallet address = {self.wallet_addr} || Block number = {block_num}")

        VAULT_ADDR = "persistence1jyhyqjxf3pc7vzwyqhwe53up5pj0e53zw3xu2589uqgkvqngswnqgrmstf"



        res = self.query_pool_config("persistence1lxansfc8vkujy997e3xksd3ugsppv6a9jt32pjtgaxr0zkcnkznqu22a4s")
        # id_res = self.query_pool_id("persistence1lxansfc8vkujy997e3xksd3ugsppv6a9jt32pjtgaxr0zkcnkznqu22a4s")
        # print(id_res)

        token_balance = self.query_balance("persistence1vguuxez2h5ekltfj9gjd62fs5k4rl2zy5hfrncasykzw08rezpfst7tmng", self.wallet_addr)
        print(token_balance)

        increase_allowance_tx = self.execute_increase_allowance("persistence1vguuxez2h5ekltfj9gjd62fs5k4rl2zy5hfrncasykzw08rezpfst7tmng", VAULT_ADDR, 1000000000)
        print(increase_allowance_tx)


        # pool_assets = res['assets']
        # print("Pool Assets")
        # for asset in pool_assets:
        #     print(asset["info"])
            # if asset["info"].get("native_token"):
            #     print("Native Token")

        # self.execute_vault_JoinPool(1, None, )
        






def execute_simulation():
        dexter_simulation = DexterModel()
        i = 0

        # dexter_simulation.update_agents_state()

        # while i < 10000: 
        #     await dexter_simulation.step()
        #     i = i + 1




if __name__ == "__main__":
    loop = asyncio.get_event_loop()
    execute_simulation()
    loop.close()

    # while(1):
    #     try:
    #         loop.run_until_complete(execute_simulation())
    #     except Exception as e:
    #         print(e)  
    #         if e == KeyboardInterrupt:
    #             break
    #         # pass

    # asyncio.sleep(59*59)
