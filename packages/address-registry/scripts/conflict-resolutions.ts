import type {ConflictResolution} from "./resolve.ts"

export const CONFLICT_RESOLUTIONS = [
  {
    address: "0:97ad93444915089e812238ff10abe9066d0b03ea3dba2a8630fb9c9f88aa455c",
    source: "ton-assets",
    name: "Wallet in Telegram",
  },
  {
    address: "0:1b44ec553616b0e4551037038834bdf0bdaa1f0440e10f3331d9841585612a68",
    source: "ton-assets",
    name: "Wallet in Telegram",
  },
  {
    address: "0:cf06c94efdae26faaa1d85092fc4daee78eba09e69c36dac051c1d8ffde35f40",
    source: "ton-assets",
    name: "Wallet in Telegram",
  },
  {
    address: "0:91887059ba56a03790b0696d7a402ac0691dff62fcd398e813f97d3b2736fa58",
    source: "ton-assets",
    name: "Wallet in Telegram",
  },
  {
    address: "0:6db0f022632b6269f0d4dbf8ea60a8448bbc2bcfc6812e31af8cc6405c268af7",
    source: "ton-assets",
    name: "Wallet in Telegram",
  },
  {
    address: "0:dd6ff02c59634745529b99a8d5beeea9f6c38a9188e6a7e96a424e3820c8ac0a",
    source: "ton-assets",
    name: "Wallet in Telegram",
  },
  {
    address: "0:023895aef955024920a291c6f3715e291df1b3dd254eafa8b09e21a2d58d5897",
    source: "ton-assets",
    name: "Wallet in Telegram",
  },
  {
    address: "0:f5066318091a8cfb530ec732104ce38f7783d92a6679ffcc0ce0960b0e94c7d0",
    source: "ton-assets",
    name: "Wallet in Telegram",
  },
  {
    address: "0:0c5684355f5410a923fb7dd59314905dc7ff157b28c9130671c7a4f17007e898",
    source: "ton-assets",
    name: "Wallet in Telegram",
  },
  {
    address: "0:23fe558cdf44249381aef36472e64a5072a24ba97fb61b2f5791da6013aec92d",
    source: "ton-assets",
    name: "Wallet in Telegram",
  },
  {
    address: "0:b7be71ac1b40e436db36721d9c20e82d44e78fc1e86a8ad15e00b1cc16831dcf",
    source: "ton-assets",
    name: "Wallet in Telegram",
  },
  {
    address: "0:55b7223f04dd1f2e1b3f28e4ad628fe6cab60004f280fb0a58cd27fc590b2eac",
    source: "ton-assets",
    name: "Wallet in Telegram",
  },
  {
    address: "0:c3f1da8ecda8f8cd42bace224ea3f1b6971eaa7f54c492d4d190527b4f573f7c",
    source: "ton-assets",
    name: "Bybit 1",
  },
  {
    address: "0:4a1f3ebcc66bdcefa90245bdf541a50ada1c688a487534b1587abffd93038834",
    source: "ton-assets",
    name: "Bybit 2",
  },
  {
    address: "0:cd525c15904d7b4027eae08b318624260f371ce8c9f4fcca0cdb7b583854107c",
    source: "ton-assets",
    name: "Bybit 3",
  },
  {
    address: "0:ae62a192da96e266161a702cc9f6de91c0c3441f17ceba51bc442dfb2031a30a",
    source: "ton-assets",
    name: "Bybit 4",
  },
  {
    address: "0:e3ce05c4da977132c24a3b102da8b5fbf2ea9173337b49de346e4a4b057f7579",
    source: "ton-assets",
    name: "Bybit 5",
  },
  {
    address: "0:e33ef85d174eb457c072d1ee10215b69ab9b64192f7421302ab1c8e26b7ad4bd",
    source: "ton-assets",
    name: "Bybit 6",
  },
  {
    address: "0:7f97f36dda3f7dedc9e12f8c09c4f89e7fbc17527ab3f67de4c3b155297a56e4",
    source: "ton-assets",
    name: "Bybit 7",
  },
  {
    address: "0:7d133d4e425c8e00de015513a44e66e6d163b21e71720aec7579965e5de28c55",
    source: "ton-assets",
    name: "Bybit 8",
  },
  {
    address: "0:f2dc1860686cd658987e2bed8216532d8c004460dadea907cc7d4ad7236c496b",
    source: "ton-assets",
    name: "Bybit 9",
  },
  {
    address: "0:04f3c1c795ac8905e31734362b149875ab9522ae5383ab7abb2e10da5da8ba82",
    source: "address-book",
    name: "Portals Market Deposit",
  },
  {
    address: "0:b39ec4a1b09985a39e11d46c06ce66c7239cdee20cb185d7fd18a5020b1ee402",
    source: "address-book",
    name: "Portals Market",
  },
  {
    address: "0:80d4123841167ca989ac912443cc99a4b9c1a87584536427ff6fd85c92395ae9",
    source: "address-book",
    name: "KuCoin 2",
  },
  {
    address: "0:b37e57033db21d10b950e6143b658c10c3bf425bd193025960aef7f22dbcf4fc",
    source: "address-book",
    name: "KuCoin 3",
  },
  {
    address: "0:07ce60b7e5f255a88c3707f3fbc79e2cf924ed6b4b7d09c6324f0ba2338a48fa",
    source: "address-book",
    name: "KuCoin 4",
  },
  {
    address: "0:5f00decb7da51881764dc3959cec60609045f6ca1b89e646bde49d492705d77f",
    source: "ton-assets",
    name: "OKX 1",
  },
  {
    address: "0:f9bdc0de461c1a2e002fada9550bbb095c8f9f73668d70c517a9cc7e050f8e5a",
    source: "ton-assets",
    name: "OKX 9",
  },
  {
    address: "0:66a1e72196d64d6faf48fa4b2ea861f67b0484d2d14a59caf701d8c286ce44e5",
    source: "ton-assets",
    name: "OKX 11",
  },
  {
    address: "-1:386e6a793b70ed6235e67560183040734cfb2ef9c3cc720e758b767d6df748ef",
    source: "address-book",
    name: "Rangers Pool 1",
  },
  {
    address: "-1:f5ab0b1ce4ffe9c62776307f209da46f44901ad64991cbce9eab4242e9759768",
    source: "address-book",
    name: "Rangers Pool 2",
  },
  {
    address: "-1:1189458eea400d0c5dc5b1a22eda8dd009baba5465b2a99c5145733c07d9916c",
    source: "address-book",
    name: "HB Pool 1",
  },
  {
    address: "-1:bc8bbd5c4bba73f5fbb3402c5134e231df51336933e99ac365b866f3242e9c54",
    source: "address-book",
    name: "HB Pool 2",
  },
  {
    address: "-1:20429a7ba4a0fd1b306b72bde3bbd7da3e31161cf7ab859e2d1694a20e80d420",
    source: "address-book",
    name: "Very First Pool 1",
  },
  {
    address: "-1:dda1d87623c063a6fc4922b286e68c28e7f0b5459973919b570b71c62ee35913",
    source: "address-book",
    name: "Very First Pool 2",
  },
  {
    address: "0:779dcc815138d9500e449c5291e7f12738c23d575b5310000f6a253bd607384e",
    source: "ton-assets",
    name: "STON.fi DEX",
  },
  {
    address: "0:70a4118401bf8d823531a66011020565f02e05ff91ac5f1677769b00d6acd07a",
    source: "ton-assets",
    name: "STON.fi DEX",
  },
  {
    address: "0:92e1411ae546892f33b2c8a89ea90390d8ff4cfbb917a643b91e73f706fdb9d1",
    source: "ton-assets",
    name: "STON.fi DEX",
  },
  {
    address: "0:7251e83282040cfd2387cf677b2864f7f021720fa040cda543a9194119442cea",
    source: "ton-assets",
    name: "Tonkeeper battery refunds",
  },
  {
    address: "0:9b8ab637507230b99de26a55ea6d9cd4fef0cffcaafe2d1f15e835d5f5d38a43",
    source: "ton-assets",
    name: "TONAPI gas proxy (old)",
  },
  {
    address: "0:1c06b78eb4c0c014b51308221f6263643746fe7be60b4831a8409051cba0306f",
    source: "ton-assets",
    name: "TONAPI gas proxy (old)",
  },
  {
    address: "0:0bc884e676ba3dcaabe75cea71c38d6691ed0d6a89cfd95d2772c32f7be01262",
    source: "ton-assets",
    name: "TONAPI gas proxy (old)",
  },
  {
    address: "0:f33f5a1e309236c21fd412b9d522e24a6a6ef3745c01f7ec7d731bc0f844c334",
    source: "ton-assets",
    name: "TONAPI gas proxy (old)",
  },
  {
    address: "0:e3b375a5f71ea17bec125d3a88f6483575ee909b16010eefed9b64fe9b0d64e5",
    source: "ton-assets",
    name: "TONAPI gas proxy (old)",
  },
  {
    address: "0:73727a419e7d7f1ae1c455e58ee432f26e3a75b31078f99cedcac403f47619be",
    source: "ton-assets",
    name: "TONAPI gas proxy (old)",
  },
  {
    address: "0:d5a60826d1d4f157085d2bc751d037c61f1fe2d55322cd5bc0297456c513dd69",
    source: "ton-assets",
    name: "TONAPI gas proxy (old)",
  },
  {
    address: "0:a1809e9a6f64adde7f0f485742968433a621a4f3b5e1c5920a7077d7b63c3411",
    source: "ton-assets",
    name: "TONAPI gas proxy (old)",
  },
  {
    address: "0:c6f5916443f6f707139b108edce317ea52a8c4c5e5afaf9a3c6e93d64685d95d",
    source: "ton-assets",
    name: "TONAPI gas proxy (old)",
  },
  {
    address: "-1:3333333333333333333333333333333333333333333333333333333333333333",
    source: "ton-assets",
    name: "Elector Contract",
  },
  {
    address: "0:dddcd3cdad60af4c0d69389f567ad51d0c263fa4968d655ab424c69aadaf9322",
    source: "ton-assets",
    name: "Wallet Bot 1",
  },
  {
    address: "0:436a76c2794a88e3fbfec6b9c0374fc8db046f10868b835420d9937973a665d4",
    source: "ton-assets",
    name: "Wallet Bot 2",
  },
  {
    address: "0:6e814e6450e8c578067bb656298fb4d01397dd15018d7f34dc369eaba4c2111c",
    source: "ton-assets",
    name: "Wallet Bot 3",
  },
  {
    address: "0:83dfd552e63729b472fcbcc8c45ebcc6691702558b68ec7527e1ba403a0f31a8",
    source: "ton-assets",
    name: "TON Foundation (OLD)",
  },
  {
    address: "-1:5555555555555555555555555555555555555555555555555555555555555555",
    source: "ton-assets",
    name: "Config Contract",
  },
  {
    address: "0:0000000000000000000000000000000000000000000000000000000000000000",
    source: "ton-assets",
    name: "Zero Address",
  },
  {
    address: "0:b113a994b5024a16719f69139328eb759596c38a25f59028b146fecdc3621dfe",
    source: "acton",
    name: "Tether USD (USDT)",
  },
  {
    address: "-1:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    source: "ton-assets",
    name: "Blackhole Address",
  },
  {
    address: "0:310b71b340182396f5ba08903081a1ef6ab4df571a3ca7b05effa44c4a3b0f92",
    source: "address-book",
    name: "Megaton Finance DEX",
  },
  {
    address: "0:bffadd270a738531da7b13ba8fc403826c2586173f9ede9c316fab53bc59ac86",
    source: "address-book",
    name: "TONCO DEX Router",
  },
  {
    address: "0:ec85cfc7fd1009362e421b5a0141f510d6540107ab73e7829fda1ef9e97a75e3",
    source: "address-book",
    name: "Crypto Bot",
  },
  {
    address: "0:3d2cb74e041df056045d3ba4bef650f938c553db86d1dab1f80bc9c238fe3ffe",
    source: "address-book",
    name: "Rapira Exchange 1",
  },
  {
    address: "0:708545cef8fc0118022596a056044a61833e923613bf70fd1e25f033fefd8a3c",
    source: "address-book",
    name: "Rapira Exchange 2",
  },
  {
    address: "0:ca1d9edeef40b3a9dbd9082f3767859547c3ce0bf641d09d58e33a3cf06fb309",
    source: "ton-assets",
    name: "Binance Hot Wallet",
  },
  {
    address: "0:ad8afecfacc248996885d8be6824280f9f3a7e54c2f6080b7971b9f556c280c4",
    source: "ton-assets",
    name: "Crypto Bot",
  },
  {
    address: "0:ed1691307050047117b998b561d8de82d31fbf84910ced6eb5fc92e7485ef8a7",
    source: "ton-assets",
    name: "TON Believers Fund",
  },
  {
    address: "0:817438e5b0e6eb113ab74d73fc95f7ec19247dbe6ee27a42c3d0164d614c3ca1",
    source: "ton-assets",
    name: "Ourbit",
  },
  {
    address: "0:9a9cb80adfbd1662f5108766d73355ac2c03304fda1d25a479670e34efcd72b3",
    source: "ton-assets",
    name: "Marketapp Marketplace",
  },
  {
    address: "0:53191a57801de242aef8a4735c9cacc553f4b88f85869ee6ce89860b53924538",
    source: "ton-assets",
    name: "Marketapp Deployer",
  },
  {
    address: "0:e58d0685ac8e90a05c34bef9fa18375bb7ea090b7834197061a2cfaf4b6aa0e4",
    source: "ton-assets",
    name: "Marketapp Gift Sender",
  },
  {
    address: "0:a1376e8bf9f266ec7c6b11ce6e5cd02a9bda363b9ea888a2e239d2383572bc9a",
    source: "ton-assets",
    name: "AvanChange 2",
  },
  {
    address: "0:852443f8599fe6a5da34fe43049ac4e0beb3071bb2bfb56635ea9421287c283a",
    source: "address-book",
    name: "Fragment · Telegram Stars",
  },
  {
    address: "0:5e69bec3dfc448c32a5e81b37b619810cf00db6fc41f30cc18f28b89737a8f97",
    source: "address-book",
    name: "Fragment · Telegram Ads",
  },
  {
    address: "0:43512860d54980cf24d59868a30e679927fb1373c10964db7500edcdf690abc4",
    source: "address-book",
    name: "Telegram Rewards",
  },
  {
    address: "0:e6f3d8824f46b1efbab9afc684793428c55fed69b46a15a49be69a29bc49e530",
    source: "address-book",
    name: "Telegram Rewards",
  },
  {
    address: "0:2ecf5e47d591eb67fa6c56b02b6bb1de6a530855e16ad3082eaa59859e8d5fdc",
    source: "ton-assets",
    name: "Telegram Team",
  },
  {
    address: "-1:4d5c0210b35daddaa219fac459dba0fdefb1fae4e97a0d0797739fe050d694ca",
    source: "ton-assets",
    name: "BSC Bridge",
  },
  {
    address: "0:07235bc6bb0edde161be0ad7c3ad6a843cdb6652e2a32851057899996aaa9f59",
    source: "ton-assets",
    name: "BSC Bridge Collector",
  },
  {
    address: "-1:0ebd7ff9ca70e06e9e22a8922f5ae75211a9d6a34a8094e8e1587b606bdbb662",
    source: "ton-assets",
    name: "BSC Bridge Governance",
  },
  {
    address: "-1:dd24c4a1f2b88f8b7053513b5cc6c5a31bc44b2a72dcb4d8c0338af0f0d37ec5",
    source: "ton-assets",
    name: "ETH Bridge",
  },
  {
    address: "0:aecef20e5cb8c7dadbf7e60638b73848bd70aa6560ee2bd91484519728b22683",
    source: "ton-assets",
    name: "ETH Bridge Collector",
  },
  {
    address: "-1:3b9bbfd0ad5338b9700f0833380ee17d463e51c1ae671ee6f08901bde899b202",
    source: "ton-assets",
    name: "ETH Bridge Governance",
  },
  {
    address: "0:419712bc541c4489d6f1c38a105722d293b8847fd54f2717ad69142911b8be5c",
    source: "ton-assets",
    name: "Bitfinex 2",
  },
  {
    address: "0:94a7cf34ef6a4b5063500975d70c524cdc2e6ff465d38d5ee8bb7dcc8f7af45e",
    source: "ton-assets",
    name: "TON Coin Pool Withdraw 1",
  },
  {
    address: "0:c1dc654b598ab84cda4f12372efed8907cedf0901865a58649087373e01b1c24",
    source: "ton-assets",
    name: "xRocket Bot (Old)",
  },
  {
    address: "0:f069c060822b0443e87e1d6eb752223d9ad200fa54404f5ba95cf8a88b284290",
    source: "ton-assets",
    name: "Merchant WebMoney",
  },
  {
    address: "0:bdf3fa8098d129b54b4f73b5bac5d1e1fd91eb054169c3916dfc8ccd536d1000",
    source: "ton-assets",
    name: "Tonstakers master",
  },
  {
    address: "0:3349ddd903d5ff139df8f3f471855a86858de47cde9f992d108be5c58216e926",
    source: "ton-assets",
    name: "Omniston Escrow Minter",
  },
  {
    address: "0:ed53bc999e5a4af69a3f9c3de5376f7d90c487e1528f331e716dbe85903d5112",
    source: "ton-assets",
    name: "Notcoin",
  },
  {
    address: "-1:34517c7bdf5187c55af4f8b61fdc321588c7ab768dee24b006df29106458d7cf",
    source: "ton-assets",
    name: "Log tests Contract",
  },
  {
    address: "0:85af78e8d035e920117cda654615cdf371d464480b629e110d3c5310d85ab362",
    source: "address-book",
    name: "Huobi Withdrawal",
  },
] as const satisfies readonly ConflictResolution[]
