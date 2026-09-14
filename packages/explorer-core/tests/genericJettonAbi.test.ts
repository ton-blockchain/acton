import {beforeAll, expect, test} from "bun:test"
import {Address, beginCell} from "@ton/core"
import {decodeCellWithAbi, decodeStorageDataCell} from "@acton/transaction-ui"

import bundle from "../../../crates/acton-abi-catalog/data/data-abis.json"
import {
  getBundledCompilerAbiCatalog,
  getBundledCompilerAbiForInterface,
} from "../src/api/compilerAbiCatalog"
import {resolveCompilerAbis} from "../src/api/compilerAbiResolver"
import type {TonClient} from "../src/api/client"
import type {ExplorerMetadataRegistry} from "../src/metadata/types"
import type {ExtendedContractABI} from "../src/api/compilerAbi"

beforeAll(async () => {
  const originalFetch = globalThis.fetch
  globalThis.fetch = (async () => Response.json(bundle)) as typeof fetch
  try {
    await getBundledCompilerAbiCatalog()
  } finally {
    globalThis.fetch = originalFetch
  }
})

test("standard Jetton entries describe public interfaces without storage or implementation errors", async () => {
  const entries = await Promise.all([
    getBundledCompilerAbiForInterface("jetton_master"),
    getBundledCompilerAbiForInterface("jetton_wallet"),
  ])
  expect(
    entries.map(entry => ({
      name: entry?.display_name,
      hashes: entry?.code_hashes,
      storage: entry?.compiler_abi.storage,
      errors: entry?.compiler_abi.thrown_errors,
      methods: entry?.compiler_abi.get_methods.map(method => method.name),
      decodedStorage: decodeStorageDataCell(
        beginCell().storeUint(42, 32).endCell().toBoc().toString("base64"),
        entry?.compiler_abi,
      ),
    })),
  ).toMatchSnapshot()
})

test("account interface fallback preserves exact ABIs and never registers generic ABIs by hash", async () => {
  const exact = (await getBundledCompilerAbiCatalog()).find(entry => entry.code_hashes.length > 0)
  if (!exact) throw new Error("Expected an exact catalog ABI")
  const interfaces = [
    ["jetton_master"],
    ["jetton_wallet"],
    ["jetton_master"],
    [],
    ["jetton_master", "jetton_wallet"],
  ]
  const accounts = interfaces.map((values, index) => ({
    address: new Address(0, Buffer.alloc(32, index + 1)).toRawString(),
    code_hash: `code-${index}`,
    interfaces: values,
  }))
  const result = await resolveCompilerAbis({
    client: {getAccountStates: async () => ({accounts})} as unknown as TonClient,
    metadataRegistry: {
      getCompilerAbis: async () => ({"code-2": exact}),
    } as unknown as ExplorerMetadataRegistry,
    addresses: accounts.map(account => account.address),
  })
  expect({
    byAddress: accounts.map(
      account => result?.abiByAddress.get(account.address)?.contract_name ?? null,
    ),
    genericByHash: ["code-0", "code-1"].map(hash => result?.abiByCodeHash.get(hash) ?? null),
  }).toMatchSnapshot()
  expect(result?.abiByAddress.get(accounts[2].address)).toBe(exact.compiler_abi)
})

test("generic wallet decodes standard transfers with null responses and both forward payload encodings", async () => {
  const abi = (await getBundledCompilerAbiForInterface("jetton_wallet")) as ExtendedContractABI
  const owner = new Address(0, Buffer.alloc(32, 1))
  const comment = beginCell().storeUint(0, 32).storeStringTail("TEP-74 example").endCell()
  const results = [false, true].map(referenced => {
    const body = beginCell()
      .storeUint(0x0f_8a_7e_a5, 32)
      .storeUint(42, 64)
      .storeCoins(123_456_789n)
      .storeAddress(owner)
      .storeAddress(null)
      .storeMaybeRef(null)
      .storeCoins(1n)
      .storeBit(referenced)
    if (referenced) body.storeRef(comment)
    else body.storeSlice(comment.beginParse())
    return decodeCellWithAbi(body.endCell(), abi)
  })
  expect(results).toMatchSnapshot()
})
