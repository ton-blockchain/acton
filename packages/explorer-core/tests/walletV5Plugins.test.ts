import {describe, expect, mock, test} from "bun:test"
import {Address, beginCell} from "@ton/core"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"

import type {ExtendedContractABI} from "../src/api/compilerAbi"
import type {AccountStatesResponse, V3AccountState} from "../src/api/types"
import {getContractTypeLabels} from "../src/components/contractTypeLabels"
import {loadWalletV5PluginTypes} from "../src/components/walletV5Plugins"

const firstAddress = new Address(0, Buffer.alloc(32, 1))
const secondAddress = new Address(-1, Buffer.alloc(32, 2))
const codeHash = "a".repeat(64)
const codeHashBase64 = Buffer.from(codeHash, "hex").toString("base64")
const exactAbi: ExtendedContractABI = {
  compiler_abi: {contract_name: "SubscriptionPlugin"} as ContractABI,
  code_hashes: [codeHash],
  links: [],
}

function account(address: string, changes: Partial<V3AccountState> = {}): V3AccountState {
  return {
    address,
    account_state_hash: "",
    balance: "0",
    contract_methods: [],
    extra_currencies: {},
    interfaces: null,
    last_transaction_hash: "",
    last_transaction_lt: "0",
    status: "active",
    ...changes,
  }
}

function response(accounts: readonly V3AccountState[]): AccountStatesResponse {
  return {accounts, address_book: {}, metadata: {}}
}

describe("Wallet V5 plugin types", () => {
  test("batches unique addresses and resolves exact code ABIs across address and hash encodings", async () => {
    const getAccountStates = mock(async () =>
      response([
        account(firstAddress.toRawString().toUpperCase(), {code_hash: codeHashBase64}),
        account(secondAddress.toString(), {
          code_hash: codeHash,
          interfaces: ["jetton_wallet"],
        }),
      ]),
    )
    const getCompilerAbis = mock(async () => ({[codeHash]: exactAbi}))
    const result = await loadWalletV5PluginTypes({
      client: {getAccountStates},
      metadataRegistry: {getCompilerAbis},
      addresses: [firstAddress.toString(), firstAddress.toRawString(), secondAddress.toString()],
    })

    expect(getAccountStates).toHaveBeenCalledTimes(1)
    expect(getAccountStates).toHaveBeenCalledWith(
      [firstAddress.toRawString(), secondAddress.toRawString()],
      true,
    )
    expect(getCompilerAbis).toHaveBeenCalledWith([codeHash], {throwOnError: true})
    expect(result?.get(firstAddress.toRawString())).toEqual({
      status: "success",
      labels: ["SubscriptionPlugin"],
    })
    expect(result?.get(secondAddress.toRawString())).toEqual({
      status: "success",
      labels: ["SubscriptionPlugin", "Jetton wallet interface"],
    })
  })

  test("identifies library-reference plugins by the underlying code and falls back from malformed BoCs", async () => {
    const libraryCode = beginCell()
      .storeUint(2, 8)
      .storeBuffer(Buffer.from(codeHash, "hex"))
      .endCell({exotic: true})
    const getCompilerAbis = mock(async () => ({[codeHash]: exactAbi}))
    const result = await loadWalletV5PluginTypes({
      client: {
        getAccountStates: async () =>
          response([
            account(firstAddress.toRawString(), {
              code_hash: libraryCode.hash().toString("base64"),
              code_boc: libraryCode.toBoc().toString("base64"),
            }),
            account(secondAddress.toRawString(), {
              code_hash: codeHashBase64,
              code_boc: "invalid-boc",
            }),
          ]),
      },
      metadataRegistry: {getCompilerAbis},
      addresses: [firstAddress.toRawString(), secondAddress.toRawString()],
    })

    expect(getCompilerAbis).toHaveBeenCalledWith([codeHash], {throwOnError: true})
    expect([...(result?.values() ?? [])]).toEqual([
      {status: "success", labels: ["SubscriptionPlugin"]},
      {status: "success", labels: ["SubscriptionPlugin"]},
    ])
  })

  test("uses account interface labels and marks successfully unidentified contracts Unknown", async () => {
    const result = await loadWalletV5PluginTypes({
      client: {
        getAccountStates: async () =>
          response([
            account(firstAddress.toRawString(), {
              code_hash: codeHash,
              interfaces: ["nft_item", "nft_item_simple", " NFT_COLLECTION "],
            }),
            account(secondAddress.toRawString(), {code_hash: codeHash}),
          ]),
      },
      metadataRegistry: {getCompilerAbis: async () => ({[codeHash]: null})},
      addresses: [firstAddress.toRawString(), secondAddress.toRawString()],
    })

    expect(result?.get(firstAddress.toRawString())).toEqual({
      status: "success",
      labels: ["NFT item interface", "NFT collection interface"],
    })
    expect(result?.get(secondAddress.toRawString())).toEqual({
      status: "success",
      labels: ["Unknown"],
    })
    expect(
      getContractTypeLabels({contract_name: "JettonWallet"} as ContractABI, ["jetton_wallet"]),
    ).toEqual(["JettonWallet"])
  })

  test("keeps account request failures and omitted accounts distinct from unknown contract types", async () => {
    const getCompilerAbis = mock(async () => ({}))
    for (const getAccountStates of [
      async () => {
        throw new Error("Account API unavailable")
      },
      async () => response([]),
    ]) {
      const result = await loadWalletV5PluginTypes({
        client: {getAccountStates},
        metadataRegistry: {getCompilerAbis},
        addresses: [firstAddress.toString()],
      })
      expect(result?.get(firstAddress.toRawString())).toEqual({status: "error"})
    }
    expect(getCompilerAbis).not.toHaveBeenCalled()
  })

  test("keeps ABI failures pending even when an indexed interface is available", async () => {
    const thirdAddress = new Address(0, Buffer.alloc(32, 3)).toRawString()
    const result = await loadWalletV5PluginTypes({
      client: {
        getAccountStates: async () =>
          response([
            account(firstAddress.toRawString(), {code_hash: codeHash}),
            account(secondAddress.toRawString(), {
              code_hash: codeHash,
              interfaces: ["multisig_v2"],
            }),
            account(thirdAddress, {status: "uninit"}),
          ]),
      },
      metadataRegistry: {
        getCompilerAbis: async () => {
          throw new Error("Metadata API unavailable")
        },
      },
      addresses: [firstAddress.toRawString(), secondAddress.toRawString(), thirdAddress],
    })

    expect(result?.get(firstAddress.toRawString())).toEqual({status: "error"})
    expect(result?.get(secondAddress.toRawString())).toEqual({
      status: "error",
    })
    expect(result?.get(thirdAddress)).toEqual({status: "success", labels: ["Unknown"]})
  })

  test("limits account batches and preserves successful results when one batch fails", async () => {
    const addresses = Array.from({length: 101}, (_, index) =>
      new Address(0, Buffer.alloc(32, index)).toRawString(),
    )
    const getAccountStates = mock(async (batch: string[]) => {
      if (batch.length === 1) throw new Error("One account batch unavailable")
      return response(batch.map(address => account(address)))
    })
    const getCompilerAbis = mock(async () => ({}))
    const result = await loadWalletV5PluginTypes({
      client: {getAccountStates},
      metadataRegistry: {getCompilerAbis},
      addresses,
    })

    expect(getAccountStates.mock.calls.map(([batch]) => batch.length)).toEqual([100, 1])
    expect(result?.size).toBe(101)
    expect(result?.get(addresses[0])).toEqual({status: "success", labels: ["Unknown"]})
    expect(result?.get(addresses[100])).toEqual({status: "error"})
    expect(getCompilerAbis).not.toHaveBeenCalled()
  })

  test("resolves known plugin types independently of another plugin's ABI failure", async () => {
    const failingHash = "b".repeat(64)
    const result = await loadWalletV5PluginTypes({
      client: {
        getAccountStates: async () =>
          response([
            account(firstAddress.toRawString(), {code_hash: codeHash}),
            account(secondAddress.toRawString(), {code_hash: failingHash}),
          ]),
      },
      metadataRegistry: {
        getCompilerAbis: async ([hash]) => {
          if (hash === failingHash) throw new Error("Metadata API unavailable")
          return {[codeHash]: exactAbi}
        },
      },
      addresses: [firstAddress.toRawString(), secondAddress.toRawString()],
    })

    expect(result?.get(firstAddress.toRawString())).toEqual({
      status: "success",
      labels: ["SubscriptionPlugin"],
    })
    expect(result?.get(secondAddress.toRawString())).toEqual({status: "error"})
  })

  test("discards stale account and metadata responses after a wallet or network change", async () => {
    for (const staleAfter of ["accounts", "metadata"] as const) {
      let active = true
      const getCompilerAbis = mock(async () => {
        active = false
        return {[codeHash]: exactAbi}
      })
      const result = await loadWalletV5PluginTypes({
        client: {
          getAccountStates: async () => {
            if (staleAfter === "accounts") active = false
            return response([account(firstAddress.toRawString(), {code_hash: codeHash})])
          },
        },
        metadataRegistry: {getCompilerAbis},
        addresses: [firstAddress.toRawString()],
        shouldContinue: () => active,
      })

      expect(result).toBeUndefined()
      expect(getCompilerAbis).toHaveBeenCalledTimes(staleAfter === "accounts" ? 0 : 1)
    }
  })

  test("does not request metadata when there are no plugins or the request is already stale", async () => {
    const getAccountStates = mock(async () => response([]))
    const getCompilerAbis = mock(async () => ({}))
    const options = {client: {getAccountStates}, metadataRegistry: {getCompilerAbis}}
    expect(await loadWalletV5PluginTypes({...options, addresses: []})).toEqual(new Map())
    expect(
      await loadWalletV5PluginTypes({
        ...options,
        addresses: [firstAddress.toString()],
        shouldContinue: () => false,
      }),
    ).toBeUndefined()
    expect(getAccountStates).not.toHaveBeenCalled()
    expect(getCompilerAbis).not.toHaveBeenCalled()
  })
})
