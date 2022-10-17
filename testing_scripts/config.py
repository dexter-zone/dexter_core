
LCD_URL = "https://rest.testnet.persistence.one"
CHAIN_ID = "test-core-1"


addresses = {
  "test_tokens_addresses": [
    "persistence1u2zdjcczjrenwmf57fmrpensk4the84azdm05m3unm387rm8asdsh0yf27",
    "persistence1rtdulljz3dntzpu085c7mzre9dg4trgdddu4tqk7uuuvu6xrfu8s8wcs45",
    "persistence13hwj6afyxgue26f966hd5jkcvvjeruv7f9cdtd5d9mrtyrnn73ysyxvc8c",
    "persistence1gd54cnu80s8qdqcyhyvn06m87vlmch2uf4wvz4z08svawvc2rhysgvav55"
  ],
  "xyk_pool_addr": "persistence1ut5qjunqrj6pnmg9vjlm8eufulquzdgqfw4xtg02kez0fdmzn9sqv804rp",
  "xyk_lp_token_addr": "persistence1xk0s8xgktn9x5vwcgtjdxqzadg88fgn33p8u9cnpdxwemvxscvasejtgv7",
  "xyk_2_pool_addr": "persistence1xvcthy3yrjaeg4y29c5zd2ckefgx99h2ge5ppxtwslnvyqwar7aq2lzgpz",
  "xyk_2_lp_token_addr": "persistence15ul08t80lm6kp6fs424e3c9gg6eys7wcvkyl6lud45ulfl0fxrnsjdek2u",
  "stableswap_pool_addr": "persistence1k528kg8h3q56j5yazshv39fafmhjzl4540u7w36g6q2amgyrpwpsvexl2d",
  "stableswap_lp_token_addr": "persistence1jdsm42szlkrsnht95w4xesk5yluud2rge9vr4vuv84sxd9w32uwsvv0lvh",
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

POOLS = {
    "pool1": {
        "id": 15,
        "type" : "xyk",
        "pool_addr": addresses["xyk_pool_addr"],
        "lp_token_addr": addresses["xyk_lp_token_addr"],
        "assets" : [
            {'token': {'contract_addr': 'persistence1u2zdjcczjrenwmf57fmrpensk4the84azdm05m3unm387rm8asdsh0yf27'}},
            {'native_token': {'denom': 'uxprt'}}
        ]
    },
    "pool2": {
        "id": 19,
        "type" : "xyk",
        "pool_addr": addresses["xyk_2_pool_addr"],
        "lp_token_addr": addresses["xyk_2_lp_token_addr"],
        "assets" : [
            {'token': {'contract_addr': 'persistence1u2zdjcczjrenwmf57fmrpensk4the84azdm05m3unm387rm8asdsh0yf27'}},
            {'token': {'contract_addr': 'persistence1rtdulljz3dntzpu085c7mzre9dg4trgdddu4tqk7uuuvu6xrfu8s8wcs45'}}
        ]        
    },
    "pool3": {
        "id": 20,
        "type" : "stableswap",
        "pool_addr": addresses["stableswap_pool_addr"],
        "lp_token_addr": addresses["stableswap_lp_token_addr"],
        "assets" : [
            {'token': {'contract_addr': 'persistence1u2zdjcczjrenwmf57fmrpensk4the84azdm05m3unm387rm8asdsh0yf27'}},
            {'token': {'contract_addr': 'persistence1rtdulljz3dntzpu085c7mzre9dg4trgdddu4tqk7uuuvu6xrfu8s8wcs45'}}
        ]           
    },
    "pool4": {
        "id": 4,
        "type" : "stableswap",
        "pool_addr": addresses["stableswap_2_pool_addr"],
        "lp_token_addr": addresses["stableswap_2_lp_token_addr"],
        "assets" : [
            {'token': {'contract_addr': 'persistence1vguuxez2h5ekltfj9gjd62fs5k4rl2zy5hfrncasykzw08rezpfst7tmng'}},
            {'native_token': {'denom': 'uxprt'}}
        ]           
    },
    "pool5": {
        "id": 5,
        "type" : "stable5swap",
        "pool_addr": addresses["stable5swap_pool_addr"],
        "lp_token_addr": addresses["stable5swap_lp_token_addr"],
        "assets" : [
            {'token': {'contract_addr': 'persistence1rl8su3hadqqq2v86lscpuklsh2mh84cxqvjdew4jt9yd07dzekyq85jyzr'}},
            {'token': {'contract_addr': 'persistence1vguuxez2h5ekltfj9gjd62fs5k4rl2zy5hfrncasykzw08rezpfst7tmng'}}
        ]           
    },
    "pool6": {
        "id": 6,
        "type" : "stable5swap",
        "pool_addr": addresses["stable5swap_2_pool_addr"],
        "lp_token_addr": addresses["stable5swap_2_lp_token_addr"],
        "assets" : [
            {'token': {'contract_addr': 'persistence1rl8su3hadqqq2v86lscpuklsh2mh84cxqvjdew4jt9yd07dzekyq85jyzr'}},
            {'token': {'contract_addr': 'persistence1vguuxez2h5ekltfj9gjd62fs5k4rl2zy5hfrncasykzw08rezpfst7tmng'}},
            {'token': {'contract_addr': 'persistence1vhjnzk9ly03dugffvzfcwgry4dgc8x0sv0nqqtfxj3ajn7rn5ghqtpaner'}},
            {'native_token': {'denom': 'uxprt'}}
        ]           
    },
    "pool7": {
        "id": 7,
        "type" : "weighted",
        "pool_addr": addresses["weighted_pool_addr"],
        "lp_token_addr": addresses["weighted_lp_token_addr"],
        "assets" : [
            {'token': {'contract_addr': 'persistence1rl8su3hadqqq2v86lscpuklsh2mh84cxqvjdew4jt9yd07dzekyq85jyzr'}},
            {'token': {'contract_addr': 'persistence1vguuxez2h5ekltfj9gjd62fs5k4rl2zy5hfrncasykzw08rezpfst7tmng'}},
            {'token': {'contract_addr': 'persistence1vhjnzk9ly03dugffvzfcwgry4dgc8x0sv0nqqtfxj3ajn7rn5ghqtpaner'}},
            {'native_token': {'denom': 'uxprt'}}
        ]           
    },
}
