import {expect, mock, test} from "bun:test"
import {beginCell} from "@ton/core"

import {TonClient} from "../src/api/client"

test("address information falls back to the node when the index has no account", async () => {
  const originalFetch = globalThis.fetch
  const requests: string[] = []
  globalThis.fetch = mock(async input => {
    const url = new URL(input.toString())
    requests.push(url.toString())
    if (url.pathname.endsWith("/addressInformation")) {
      return Response.json({
        balance: "0",
        code: null,
        data: null,
        frozen_hash: null,
        last_transaction_hash: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        last_transaction_lt: "0",
        status: "uninit",
      })
    }
    return Response.json({
      ok: true,
      result: {
        balance: "99885000",
        code: "code-boc",
        data: "data-boc",
        frozen_hash: null,
        last_transaction_hash: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        last_transaction_lt: "0",
        state: "active",
      },
    })
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    expect({
      result: await client.getAddressInformation("EQImportedAccount"),
      requests,
    }).toMatchInlineSnapshot(`
      {
        "requests": [
          "https://toncenter.example/api/v3/addressInformation?address=EQImportedAccount&include_boc=true",
          "https://toncenter.example/api/v2/getAddressInformation?address=EQImportedAccount",
        ],
        "result": {
          "balance": "99885000",
          "code": "code-boc",
          "data": "data-boc",
          "frozen_hash": null,
          "last_transaction_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
          "last_transaction_lt": "0",
          "status": "active",
        },
      }
    `)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("address information does not query the node when indexed state is present", async () => {
  const originalFetch = globalThis.fetch
  const requests: string[] = []
  globalThis.fetch = mock(async input => {
    requests.push(input.toString())
    return Response.json({
      balance: "42",
      code: "code-boc",
      data: "data-boc",
      frozen_hash: null,
      last_transaction_hash: "transaction-hash",
      last_transaction_lt: "10",
      status: "active",
    })
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    expect({
      result: await client.getAddressInformation("EQIndexedAccount"),
      requests,
    }).toMatchInlineSnapshot(`
      {
        "requests": [
          "https://toncenter.example/api/v3/addressInformation?address=EQIndexedAccount&include_boc=true",
        ],
        "result": {
          "balance": "42",
          "code": "code-boc",
          "data": "data-boc",
          "frozen_hash": null,
          "last_transaction_hash": "transaction-hash",
          "last_transaction_lt": "10",
          "status": "active",
        },
      }
    `)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("raw blocks are loaded from the selected TonAPI LiteServer", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  const blockCell = beginCell().storeUint(0x11_ef_55_aa, 32).endCell()
  globalThis.fetch = mock(async input => {
    requests.push(new URL(input.toString()))
    return Response.json({data: blockCell.toBoc().toString("hex")})
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })
    const extendedBlockId =
      "(-1,8000000000000000,81088003,aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)"

    const result = await client.getRawBlockBoc(extendedBlockId, "testnet")

    expect(result.hash().equals(blockCell.hash())).toBe(true)
    expect(requests).toHaveLength(1)
    expect(requests[0]?.origin).toBe("https://testnet.tonapi.io")
    expect(decodeURIComponent(requests[0]?.pathname.split("/").at(-1) ?? "")).toBe(extendedBlockId)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("wallet DNS lookup returns every domain for the requested address", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  const address = "UQDYzZmfsrGzhObKJUw4gzdeIxEai3jAFbiGKGwxvxHinf4K"
  const domains = [
    "monk.t.me",
    "wolf.t.me",
    "saint.t.me",
    "viking.t.me",
    "durovloh.ton",
    "puppeteer.ton",
    "upbanking.t.me",
    "ton-rooster.ton",
    "yourtonismy.ton",
    "dubaigoodbye.ton",
    "durovscammer.ton",
    "xn--037ha7bb.ton",
    "tg-tonloveton.ton",
    "wetrustinton.t.me",
  ] as const
  globalThis.fetch = mock(async input => {
    requests.push(new URL(input.toString()))
    return Response.json({
      records: domains.map(domain => ({domain})),
    })
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await expect(client.getWalletDnsNames(address)).resolves.toEqual(domains)
    expect(requests).toHaveLength(1)
    expect(requests[0]?.pathname).toBe("/api/v3/dns/records")
    expect(requests[0]?.searchParams.get("wallet")).toBe(address)
    expect(requests[0]?.searchParams.get("limit")).toBe("1000")
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("multisig requests preserve every address and opt into nested data", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  globalThis.fetch = mock(async input => {
    const url = new URL(input.toString())
    requests.push(url)
    return Response.json(
      url.pathname.endsWith("/wallets")
        ? {multisigs: [], address_book: {}}
        : {orders: [], address_book: {}},
    )
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await client.getMultisigWallets(["EQWalletOne", "EQWalletTwo"], true)
    await client.getMultisigOrders(["EQOrderOne", "EQOrderTwo"], true)

    expect(requests.map(request => request.toString())).toEqual([
      "https://toncenter.example/api/v3/multisig/wallets?address=EQWalletOne&address=EQWalletTwo&include_orders=true",
      "https://toncenter.example/api/v3/multisig/orders?address=EQOrderOne&address=EQOrderTwo&parse_actions=true",
    ])
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("recent Jetton requests use transfer ordering and batch master lookup", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  globalThis.fetch = mock(async input => {
    const url = new URL(input.toString())
    requests.push(url)
    return Response.json(
      url.pathname.endsWith("/transfers")
        ? {
            jetton_transfers: [
              {
                jetton_master: "0:master-one",
                transaction_aborted: false,
                transaction_lt: "42",
                transaction_now: 1_753_800_000,
              },
            ],
          }
        : {jetton_masters: [], metadata: {}},
    )
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await expect(client.getJettonTransfers(500, 1000)).resolves.toMatchInlineSnapshot(`
      [
        {
          "jetton_master": "0:master-one",
          "transaction_aborted": false,
          "transaction_lt": "42",
          "transaction_now": 1753800000,
        },
      ]
    `)
    await client.getJettonMasters(["0:master-one", "0:master-two"])

    expect(requests.map(request => request.toString())).toMatchInlineSnapshot(`
      [
        "https://toncenter.example/api/v3/jetton/transfers?limit=500&offset=1000&sort=desc",
        "https://toncenter.example/api/v3/jetton/masters?address=0%3Amaster-one&address=0%3Amaster-two&limit=2",
      ]
    `)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("domain DNS lookup uses the indexed V3 wallet record when available", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  const walletAddress = "EQCIndexedWallet"
  globalThis.fetch = mock(async input => {
    requests.push(new URL(input.toString()))
    return Response.json({
      records: [{domain: "foundation.ton", dns_wallet: walletAddress}],
    })
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await expect(client.resolveDnsWalletAddress("foundation.ton")).resolves.toBe(walletAddress)
    expect(requests.map(request => request.toString())).toEqual([
      "https://toncenter.example/api/v3/dns/records?domain=foundation.ton",
    ])
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("domain DNS lookup falls back to Toncenter V2 when V3 has no records", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  const dnsRoot = "e56754f83426f69b09267bd876ac97c44821345b7e266bd956a7bfbfb98df35c"
  const walletAddress = "EQB8PZ-Cp6UzydbLvjukx1OQL3LmqeYV-tJ3qVMw_mNYgqow"
  globalThis.fetch = mock(async input => {
    const url = new URL(input.toString())
    requests.push(url)
    if (url.pathname.endsWith("/dns/records")) {
      return Response.json({records: []})
    }
    return Response.json({
      ok: true,
      result: {
        "@type": "dns.resolved",
        entries: [
          {
            "@type": "dns.entry",
            name: "",
            category: "wallet-category",
            entry: {
              "@type": "dns.entryDataSmcAddress",
              smc_address: {
                "@type": "accountAddress",
                account_address: walletAddress,
              },
            },
          },
          {
            "@type": "dns.entry",
            name: "",
            category: "site-category",
            entry: {
              "@type": "dns.entryDataAdnlAddress",
              adnl_address: {
                "@type": "adnlAddress",
                adnl_address: "site-address",
              },
            },
          },
        ],
      },
    })
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await expect(client.resolveDnsWalletAddress("dev.ton-site.ton")).resolves.toBe(walletAddress)
    expect(requests.map(request => request.toString())).toEqual([
      "https://toncenter.example/api/v3/dns/records?domain=dev.ton-site.ton",
      `https://toncenter.example/api/v2/dnsResolve?address=-1%3A${dnsRoot}&name=dev.ton-site.ton&category=wallet&ttl=10`,
    ])
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("getShardAccountCell reads the unwrapped V2 response", async () => {
  const originalFetch = globalThis.fetch
  const requests: string[] = []
  globalThis.fetch = mock(async input => {
    requests.push(input.toString())
    return Response.json({
      ok: true,
      result: {
        "@type": "tvm.cell",
        bytes: "te6cckEBAQEAAgAAAA==",
      },
    })
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await expect(client.getShardAccountCell("EQAddress", 123)).resolves.toBe("te6cckEBAQEAAgAAAA==")
    expect(requests).toEqual([
      "https://toncenter.example/api/v2/getShardAccountCell?address=EQAddress&seqno=123",
    ])
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("localnet message submission uses the endpoint for each message type", async () => {
  const originalFetch = globalThis.fetch
  const requests: Array<{readonly url: URL; readonly init?: RequestInit}> = []
  globalThis.fetch = mock(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(input.toString())
    requests.push({url, init})
    return Response.json({
      ok: true,
      result: {
        hash: url.pathname.endsWith("/sendBocReturnHash") ? "external-hash" : "internal-hash",
      },
    })
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await expect(client.sendInternalMessage("internal-boc")).resolves.toBe("internal-hash")
    await expect(client.sendExternalMessage("external-boc")).resolves.toBe("external-hash")

    expect(requests.map(request => request.url.pathname)).toEqual([
      "/api/acton_sendInternalMessage",
      "/api/v2/sendBocReturnHash",
    ])
    expect(requests.map(request => JSON.parse(String(request.init?.body)))).toEqual([
      {boc: "internal-boc"},
      {boc: "external-boc"},
    ])
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("masterchain shard blocks are resolved from the V2 shard snapshot", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  const shardBlock = {
    workchain: 0,
    shard: "8000000000000000",
    seqno: 84_021_699,
    root_hash: "root-hash",
    file_hash: "file-hash",
    created_by: "created-by",
    rand_seed: "rand-seed",
    start_lt: "89777846000000",
    end_lt: "89777846000001",
    gen_utime: "1783903686",
    tx_count: 0,
  }
  globalThis.fetch = mock(async input => {
    const url = new URL(input.toString())
    requests.push(url)
    if (url.pathname.endsWith("/getShards")) {
      return Response.json({
        ok: true,
        result: {
          "@type": "blocks.shards",
          shards: [
            {
              "@type": "ton.blockIdExt",
              workchain: 0,
              shard: "-9223372036854775808",
              seqno: shardBlock.seqno,
              root_hash: shardBlock.root_hash,
              file_hash: shardBlock.file_hash,
            },
          ],
        },
      })
    }
    return Response.json({blocks: [shardBlock]})
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      toncenterProxyV2BaseUrl: "https://actonscan.example/api/toncenter/testnet/v2",
      toncenterProxyV3BaseUrl: "https://actonscan.example/api/toncenter/testnet/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await expect(client.getMasterchainBlockShards(79_299_165)).resolves.toEqual({
      blocks: [shardBlock],
    })
    expect(requests).toHaveLength(2)
    expect(requests[0]?.origin).toBe("https://actonscan.example")
    expect(requests[0]?.pathname).toBe("/api/toncenter/testnet/v2/getShards")
    expect(requests[0]?.searchParams.get("seqno")).toBe("79299165")
    expect(requests[1]?.origin).toBe("https://actonscan.example")
    expect(requests[1]?.pathname).toBe("/api/toncenter/testnet/v3/blocks")
    expect(requests[1]?.searchParams.get("workchain")).toBe("0")
    expect(requests[1]?.searchParams.get("shard")).toBe("8000000000000000")
    expect(requests[1]?.searchParams.get("seqno")).toBe("84021699")
    expect(requests[1]?.searchParams.get("root_hash")).toBe(shardBlock.root_hash)
    expect(requests[1]?.searchParams.get("file_hash")).toBe(shardBlock.file_hash)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("account history requests forward the requested sort order", async () => {
  const originalFetch = globalThis.fetch
  const requests: string[] = []
  globalThis.fetch = mock(async input => {
    const url = new URL(input.toString())
    requests.push(url.toString())
    return Response.json(
      url.pathname.endsWith("/actions")
        ? {actions: [], address_book: {}, metadata: {}}
        : {transactions: [], address_book: {}},
    )
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await client.getAccountTransactions("EQAddress", 25, 10, "asc")
    await client.getAccountActions("EQAddress", 15, 5, "asc")
    await client.getAccountActions("EQAddress", 21, 10, "desc", {
      traceId: "trace-id",
      startLt: "101",
      endLt: "200",
    })

    expect(requests).toEqual([
      "https://toncenter.example/api/v3/transactions?account=EQAddress&limit=25&offset=10&sort=asc",
      "https://toncenter.example/api/v3/actions?account=EQAddress&limit=15&offset=5&sort=asc",
      "https://toncenter.example/api/v3/actions?account=EQAddress&trace_id=trace-id&start_lt=101&end_lt=200&limit=21&offset=10&sort=desc",
    ])
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("account action metadata resolves type-only jetton masters", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  globalThis.fetch = mock(async input => {
    const url = new URL(input.toString())
    requests.push(url)
    if (url.pathname.endsWith("/actions")) {
      return Response.json({
        actions: [
          {
            type: "jetton_mint",
            details: {asset: "0:master", amount: "1000000000"},
          },
        ],
        address_book: {},
        metadata: {
          "0:master": {
            token_info: [{type: "jetton_masters"}],
          },
        },
      })
    }

    return Response.json({
      jetton_masters: [
        {
          address: "0:master",
          admin_address: null,
          code_hash: "code",
          data_hash: "data",
          jetton_content: {name: "Acton Token", symbol: "ACT", decimals: "9"},
          jetton_wallet_code_hash: "wallet-code",
          last_transaction_lt: "1",
          mintable: true,
          total_supply: "1000000000",
        },
      ],
      metadata: {},
    })
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })
    const response = await client.getAccountActions("EQAddress")

    expect({
      requests: requests.map(request => request.toString()),
      tokenInfo: response.metadata["0:master"]?.token_info,
    }).toMatchInlineSnapshot(`
      {
        "requests": [
          "https://toncenter.example/api/v3/actions?account=EQAddress&limit=20&sort=desc",
          "https://toncenter.example/api/v3/jetton/masters?address=0%3Amaster&limit=1",
        ],
        "tokenInfo": [
          {
            "decimals": "9",
            "mintable": true,
            "name": "Acton Token",
            "symbol": "ACT",
            "total_supply": "1000000000",
            "type": "jetton_masters",
          },
        ],
      }
    `)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("account action metadata skips master lookup when the symbol is present", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  globalThis.fetch = mock(async input => {
    const url = new URL(input.toString())
    requests.push(url)
    return Response.json({
      actions: [
        {
          type: "jetton_mint",
          details: {asset: "0:master", amount: "1000000000"},
        },
      ],
      address_book: {},
      metadata: {
        "0:master": {
          token_info: [{type: "jetton_masters", name: "Acton Token", symbol: "ACT"}],
        },
      },
    })
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })
    const response = await client.getAccountActions("EQAddress")

    expect({
      requests: requests.map(request => request.toString()),
      tokenInfo: response.metadata["0:master"]?.token_info,
    }).toMatchInlineSnapshot(`
      {
        "requests": [
          "https://toncenter.example/api/v3/actions?account=EQAddress&limit=20&sort=desc",
        ],
        "tokenInfo": [
          {
            "name": "Acton Token",
            "symbol": "ACT",
            "type": "jetton_masters",
          },
        ],
      }
    `)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("account history streaming subscribes to and dispatches transactions and actions", async () => {
  const originalFetch = globalThis.fetch
  let subscriptionBody = ""
  const streamEvents = [
    {
      type: "transactions",
      finality: "confirmed",
      transactions: [{hash: "transaction-hash"}],
    },
    {
      type: "actions",
      finality: "confirmed",
      actions: [{action_id: "action-id"}],
      address_book: {"0:account": {user_friendly: "EQAccount"}},
      metadata: {"0:account": {token_info: []}},
    },
  ]
  globalThis.fetch = mock(async (_input: RequestInfo | URL, init?: RequestInit) => {
    subscriptionBody = String(init?.body ?? "")
    return new Response(streamEvents.map(event => `data: ${JSON.stringify(event)}\n\n`).join(""), {
      headers: {"Content-Type": "text/event-stream"},
    })
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })
    const receivedEvents: unknown[] = []

    const receivedActions = new Promise<void>((resolve, reject) => {
      client.subscribeAccountHistory("EQAddress", {
        onTransactions: event => receivedEvents.push(event),
        onActions: event => {
          receivedEvents.push(event)
          resolve()
        },
        onError: reject,
      })
    })
    await receivedActions

    expect({
      receivedEvents,
      subscription: JSON.parse(subscriptionBody) as unknown,
    }).toMatchSnapshot()
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("jetton wallet requests forward pagination options", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  globalThis.fetch = mock(async input => {
    requests.push(new URL(input.toString()))
    return Response.json({jetton_wallets: []})
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await client.getJettonWallets(["EQOwner"], undefined, {limit: 100, offset: 200})
    await client.getJettonWallets(undefined, ["EQJetton"], {
      limit: 100,
      offset: 300,
      sort: "desc",
    })

    expect(requests.map(request => request.toString())).toEqual([
      "https://toncenter.example/api/v3/jetton/wallets?owner_address=EQOwner&limit=100&offset=200",
      "https://toncenter.example/api/v3/jetton/wallets?jetton_address=EQJetton&limit=100&offset=300&sort=desc",
    ])
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("jetton wallet metadata without content stays unresolved for master lookup", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  globalThis.fetch = mock(async input => {
    requests.push(new URL(input.toString()))
    return Response.json({
      jetton_wallets: [
        {
          address: "0:wallet",
          balance: "100",
          owner: "0:owner",
          jetton: "0:master",
          last_transaction_lt: "1",
          code_hash: "code",
          data_hash: "data",
        },
      ],
      metadata: {
        "0:master": {
          is_indexed: false,
          token_info: [{type: "jetton_masters"}],
        },
      },
    })
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })
    const wallets = await client.getJettonWallets(["EQOwner"])

    expect({
      requests: requests.map(request => request.toString()),
      wallets: wallets.map(wallet => ({
        balance: wallet.balance,
        jetton: wallet.jetton,
        master: wallet.master ?? null,
      })),
    }).toMatchInlineSnapshot(`
      {
        "requests": [
          "https://toncenter.example/api/v3/jetton/wallets?owner_address=EQOwner",
        ],
        "wallets": [
          {
            "balance": "100",
            "jetton": "0:master",
            "master": null,
          },
        ],
      }
    `)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("NFT item requests forward owner pagination options", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  globalThis.fetch = mock(async input => {
    requests.push(new URL(input.toString()))
    return Response.json({nft_items: []})
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await client.getNftItems({
      owner_address: ["EQOwner"],
      limit: 100,
      offset: 200,
      sortByLastTransactionLt: true,
    })

    expect(requests[0]?.toString()).toBe(
      "https://toncenter.example/api/v3/nft/items?owner_address=EQOwner&limit=100&offset=200&sort_by_last_transaction_lt=true",
    )
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("transaction lookup requests one full transaction by hash", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  globalThis.fetch = mock(async input => {
    requests.push(new URL(input.toString()))
    return Response.json({transactions: [{hash: "transaction-hash"}], address_book: {}})
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    const result = await client.getTransactionByHash("requested-hash")

    expect({
      request: requests[0]?.toString(),
      result,
    }).toMatchInlineSnapshot(`
      {
        "request": "https://toncenter.example/api/v3/transactions?hash=requested-hash&limit=1",
        "result": {
          "address_book": {},
          "transactions": [
            {
              "hash": "transaction-hash",
            },
          ],
        },
      }
    `)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("trace and block lookups can use their same-origin proxies", async () => {
  const originalFetch = globalThis.fetch
  const requests: Array<{readonly url: string; readonly apiKey: string | null}> = []
  globalThis.fetch = mock(async (input: RequestInfo | URL, init?: RequestInit) => {
    requests.push({
      url: input.toString(),
      apiKey: new Headers(init?.headers).get("X-API-Key"),
    })
    return Response.json({traces: [], address_book: {}, metadata: {}})
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://testnet.toncenter.example/api/v2",
      v3BaseUrl: "https://testnet.toncenter.example/api/v3",
      toncenterProxyV3BaseUrl: "https://actonscan.example/api/toncenter/testnet/v3",
      addressNameBaseUrl: "https://testnet.toncenter.example/api",
      toncenterApiKey: "browser-api-key",
    })

    await client.getTraces("a".repeat(64), {includeActions: true})
    await client.getBlocks({workchain: -1, shard: "8000000000000000", seqno: 42, limit: 1})
    await client.getBlockTransactions({
      workchain: -1,
      shard: "8000000000000000",
      seqno: 42,
      limit: 100,
      offset: 100,
    })

    expect(requests).toMatchInlineSnapshot(`
      [
        {
          "apiKey": null,
          "url": "https://actonscan.example/api/toncenter/testnet/v3/traces?tx_hash=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa&include_actions=true",
        },
        {
          "apiKey": null,
          "url": "https://actonscan.example/api/toncenter/testnet/v3/blocks?workchain=-1&shard=8000000000000000&seqno=42&limit=1",
        },
        {
          "apiKey": null,
          "url": "https://actonscan.example/api/toncenter/testnet/v3/transactions?workchain=-1&shard=8000000000000000&seqno=42&limit=100&offset=100",
        },
      ]
    `)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("block transaction fallback uses the V2 proxy and signed shard cursor", async () => {
  const originalFetch = globalThis.fetch
  let requestUrl = ""
  globalThis.fetch = mock((input: RequestInfo | URL) => {
    requestUrl = input.toString()
    return Promise.resolve(
      Response.json({
        ok: true,
        result: {
          "@type": "blocks.transactions",
          id: {},
          req_count: 100,
          incomplete: false,
          transactions: [],
        },
      }),
    )
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      toncenterProxyV2BaseUrl: "https://actonscan.example/api/toncenter/mainnet/v2",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await client.getBlockTransactionsV2({
      workchain: -1,
      shard: "8000000000000000",
      seqno: 42,
      rootHash: "root/hash=",
      fileHash: "file/hash=",
      count: 100,
      afterLt: "123",
      afterHash: "f".repeat(64),
    })

    expect(requestUrl).toBe(
      "https://actonscan.example/api/toncenter/mainnet/v2/getBlockTransactions?workchain=-1&shard=-9223372036854775808&seqno=42&root_hash=root%2Fhash%3D&file_hash=file%2Fhash%3D&count=100&after_lt=123&after_hash=ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    )
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("custom networks keep trace and block lookups on their configured APIs", async () => {
  const originalFetch = globalThis.fetch
  const requests: Array<{readonly url: string; readonly apiKey: string | null}> = []
  globalThis.fetch = mock(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(input.toString())
    requests.push({
      url: url.toString(),
      apiKey: new Headers(init?.headers).get("X-API-Key"),
    })
    return url.pathname.endsWith("/getShards")
      ? Response.json({ok: true, result: {shards: []}})
      : Response.json({blocks: [], traces: [], transactions: []})
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://custom-toncenter.example/api/v2",
      v3BaseUrl: "https://custom-toncenter.example/api/v3",
      addressNameBaseUrl: "https://custom-toncenter.example/api",
      toncenterApiCompatible: true,
      toncenterApiKey: "custom-browser-key",
    })

    await client.getTraces("a".repeat(64))
    await client.getBlocks({workchain: -1, shard: "8000000000000000", seqno: 42})
    await client.getBlockTransactions({
      workchain: -1,
      shard: "8000000000000000",
      seqno: 42,
    })
    await client.getMasterchainBlockShards(42)

    expect(requests).toMatchInlineSnapshot(`
      [
        {
          "apiKey": "custom-browser-key",
          "url": "https://custom-toncenter.example/api/v3/traces?tx_hash=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        },
        {
          "apiKey": "custom-browser-key",
          "url": "https://custom-toncenter.example/api/v3/blocks?workchain=-1&shard=8000000000000000&seqno=42",
        },
        {
          "apiKey": "custom-browser-key",
          "url": "https://custom-toncenter.example/api/v3/transactions?workchain=-1&shard=8000000000000000&seqno=42&limit=100",
        },
        {
          "apiKey": "custom-browser-key",
          "url": "https://custom-toncenter.example/api/v2/getShards?seqno=42",
        },
      ]
    `)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("message transaction lookup forwards the causal direction", async () => {
  const originalFetch = globalThis.fetch
  const requests: URL[] = []
  globalThis.fetch = mock(async input => {
    requests.push(new URL(input.toString()))
    return Response.json({transactions: [], address_book: {}})
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    await client.getTransactionsByMessageHash("message-hash", "out")

    expect(requests[0]?.toString()).toMatchInlineSnapshot(
      `"https://toncenter.example/api/v3/transactionsByMessage?msg_hash=message-hash&direction=out&limit=1"`,
    )
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("NFT pages preserve their raw size while excluding flagged or registered NSFW items", async () => {
  const originalFetch = globalThis.fetch
  globalThis.fetch = mock(async () =>
    Response.json({
      nft_items: [
        {
          address: "0:nft",
          code_hash: "code-hash",
          content: {},
          data_hash: "data-hash",
          index: "3",
          init: true,
          last_transaction_lt: "42",
          on_sale: false,
        },
        {
          address: "0:nsfw",
          code_hash: "nsfw-code-hash",
          content: {},
          data_hash: "nsfw-data-hash",
          index: "4",
          init: true,
          last_transaction_lt: "43",
          on_sale: false,
        },
        {
          address: "0:registered-nsfw",
          code_hash: "registered-code-hash",
          content: {
            _image_small:
              "https://proxy.toncenter.com/F0W0fr2CnSPVMdgFNe9x87X1TkFGKz7rUBtHpWmNXwc/pr:small/bG9jYWw6Ly8vc2hhMjU2L2VhZDllM2M1ZjI2MDc4NWU4ODUyYzBkY2E3YWQxZmQ3ZTY2OTBiMDMwMDlhMTU4YTg0OTI0M2U1OTY4NWFhN2Q",
          },
          data_hash: "registered-data-hash",
          index: "5",
          init: true,
          last_transaction_lt: "44",
          on_sale: false,
        },
      ],
      metadata: {
        "0:nft": {
          token_info: [
            {
              type: "nft_items",
              name: "Flagged NFT",
              is_nsfw: false,
              is_scam: true,
            },
          ],
        },
        "0:nsfw": {
          token_info: [
            {
              type: "nft_items",
              name: "Hidden NFT",
              is_nsfw: true,
              is_scam: false,
            },
          ],
        },
      },
    }),
  ) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "https://toncenter.example/api/v2",
      v3BaseUrl: "https://toncenter.example/api/v3",
      addressNameBaseUrl: "https://toncenter.example/api",
    })

    expect(await client.getNftItemsPage({address: ["0:nft"]})).toMatchInlineSnapshot(`
      {
        "items": [
          {
            "address": "0:nft",
            "code_hash": "code-hash",
            "content": {
              "name": "Flagged NFT",
            },
            "data_hash": "data-hash",
            "index": "3",
            "init": true,
            "is_nsfw": false,
            "is_scam": true,
            "last_transaction_lt": "42",
            "on_sale": false,
          },
        ],
        "rawItemCount": 3,
      }
    `)
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("localnet state and checkpoint methods transfer JSON through the control API", async () => {
  const originalFetch = globalThis.fetch
  const requests: Array<{readonly url: URL; readonly init?: RequestInit}> = []
  globalThis.fetch = mock(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(input.toString())
    requests.push({url, init})

    if (url.pathname.endsWith("/acton_dumpState")) {
      return new Response('{"version":1,"kind":"state"}', {
        headers: {"Content-Type": "application/json"},
      })
    }
    if (url.pathname.endsWith("/acton_exportCheckpoint")) {
      return new Response('{"version":1,"kind":"checkpoint"}', {
        headers: {"Content-Type": "application/json"},
      })
    }
    if (url.pathname.endsWith("/acton_listCheckpoints")) {
      return Response.json({
        ok: true,
        result: [{name: "before-deploy", block_seqno: 7}],
      })
    }
    if (url.pathname.endsWith("/acton_clearCheckpoints")) {
      return Response.json({ok: true, result: {deleted: 1}})
    }
    if (url.pathname.endsWith("/acton_loadState")) {
      return Response.json({ok: true, result: null})
    }
    if (url.pathname.endsWith("/acton_importCheckpoint")) {
      return Response.json({
        ok: true,
        result: {name: url.searchParams.get("name"), block_seqno: 7},
      })
    }

    const body = JSON.parse(String(init?.body)) as {name?: string}
    return Response.json({
      ok: true,
      result: {
        name: body.name ?? url.searchParams.get("name"),
        block_seqno: 7,
      },
    })
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "http://localhost:8081/api/v2",
      v3BaseUrl: "http://localhost:8081/api/v3",
      addressNameBaseUrl: "http://localhost:8081",
      localnetApiToken: "test-token",
    })
    const state = new Blob(['{"version":1}'], {type: "application/json"})

    expect(await (await client.downloadState()).text()).toContain('"kind":"state"')
    await expect(client.loadState(state)).resolves.toBeUndefined()
    await expect(client.createCheckpoint("before-deploy")).resolves.toEqual({
      name: "before-deploy",
      block_seqno: 7,
    })
    await expect(client.listCheckpoints()).resolves.toEqual([
      {name: "before-deploy", block_seqno: 7},
    ])
    await expect(client.restoreCheckpoint("before-deploy")).resolves.toEqual({
      name: "before-deploy",
      block_seqno: 7,
    })
    expect(await (await client.downloadCheckpoint("before-deploy")).text()).toContain(
      '"kind":"checkpoint"',
    )
    await expect(client.importCheckpoint("imported", state)).resolves.toEqual({
      name: "imported",
      block_seqno: 7,
    })
    await expect(client.deleteCheckpoint("before-deploy")).resolves.toEqual({
      name: "before-deploy",
      block_seqno: 7,
    })
    await expect(client.clearCheckpoints()).resolves.toBe(1)

    expect(requests.map(request => request.url.pathname)).toEqual([
      "/acton_dumpState",
      "/acton_loadState",
      "/acton_createCheckpoint",
      "/acton_listCheckpoints",
      "/acton_restoreCheckpoint",
      "/acton_exportCheckpoint",
      "/acton_importCheckpoint",
      "/acton_deleteCheckpoint",
      "/acton_clearCheckpoints",
    ])
    expect(requests[6]?.url.searchParams.get("name")).toBe("imported")
    expect(requests[6]?.url.searchParams.get("force")).toBe("false")
    for (const request of requests) {
      expect(new Headers(request.init?.headers).get("Authorization")).toBe("Bearer test-token")
    }
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("account funding sends exact nanograms beyond the safe integer range", async () => {
  const originalFetch = globalThis.fetch
  let requestBody: string | undefined
  globalThis.fetch = mock(async (_input, init) => {
    requestBody = String(init?.body)
    return Response.json({success: true, hash: "message-hash"})
  }) as typeof fetch

  try {
    const client = new TonClient({
      v2BaseUrl: "http://localhost:8081/api/v2",
      v3BaseUrl: "http://localhost:8081/api/v3",
      addressNameBaseUrl: "http://localhost:8081",
    })

    await expect(
      client.fundAccount("EQExactRecipient", 123_456_789_012_345_678_901_234_567_890n),
    ).resolves.toBe("message-hash")
    expect(requestBody).toBe(
      '{"address":"EQExactRecipient","amount":123456789012345678901234567890}',
    )
  } finally {
    globalThis.fetch = originalFetch
  }
})
