
LCD_URL = "http://rest.testnet.persistence.one"
CHAIN_ID = "test-core-1"


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

POOLS = {
    "xyk_1": {
        "pool_addr": addresses["xyk_pool_addr"],
        "lp_token_addr": addresses["xyk_lp_token_addr"],
    },
    "xyk_2": {
        "pool_addr": addresses["xyk_2_pool_addr"],
        "lp_token_addr": addresses["xyk_2_lp_token_addr"],
    },
    "stableswap_1": {
        "pool_addr": addresses["stableswap_pool_addr"],
        "lp_token_addr": addresses["stableswap_lp_token_addr"],
    },
    "stableswap_2": {
        "pool_addr": addresses["stableswap_2_pool_addr"],
        "lp_token_addr": addresses["stableswap_2_lp_token_addr"],
    },
    "stable5swap_1": {
        "pool_addr": addresses["stable5swap_pool_addr"],
        "lp_token_addr": addresses["stable5swap_lp_token_addr"],
    },
    "stable5swap_2": {
        "pool_addr": addresses["stable5swap_2_pool_addr"],
        "lp_token_addr": addresses["stable5swap_2_lp_token_addr"],
    },
    "weighted_1": {
        "pool_addr": addresses["weighted_pool_addr"],
        "lp_token_addr": addresses["weighted_lp_token_addr"],
    },
}
