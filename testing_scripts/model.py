from mesa import Agent, Model
from cosmos_sdk.client.lcd import LCDClient
from cosmos_sdk.key.mnemonic import MnemonicKey
from cosmos_sdk.core.wasm import  MsgExecuteContract 
from cosmos_sdk.core.fee import Fee
from cosmos_sdk.core import Coins, Coin
import base64
import json
import pandas as pd

class DexterModel(Model):
    def __init__(self, N):
        self.num_agents = N
        CHAIN_ID="test-core-1"
        LCD_URL="http://rest.testnet.persistence.one"

        # self.schedule = RandomActivation(self)
        self.terra = LCDClient(chain_id=CHAIN_ID, url=LCD_URL)

