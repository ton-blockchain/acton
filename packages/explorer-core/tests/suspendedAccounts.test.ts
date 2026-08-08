import {Address, beginCell, Dictionary} from "@ton/core"
import {describe, expect, test} from "bun:test"

import {
  isAddressSuspended,
  parseSuspendedAccountsConfig,
  readSuspendedAccountsConfigCache,
  SUSPENDED_ACCOUNTS_CACHE_TTL_MS,
  writeSuspendedAccountsConfigCache,
} from "../src/api/suspendedAccounts"
import {storeSuspendedAddressList, type Unit} from "../src/cell-inspector/block.tlb.generated"

const ACCOUNT_ID_BITS = 256n
const WORKCHAIN_MODULUS = 1n << 32n

describe("suspended accounts config", () => {
  test("treats an absent ConfigParam 44 as no suspended accounts", () => {
    expect(parseSuspendedAccountsConfig("")).toEqual({rawAddresses: [], suspendedUntil: 0})
    expect(parseSuspendedAccountsConfig(" \n\t")).toEqual({rawAddresses: [], suspendedUntil: 0})
  })

  test("decodes basechain and masterchain addresses from ConfigParam 44", () => {
    const addresses = Dictionary.empty<bigint, Unit>()
    addresses.set(suspendedAddressKey(Address.parseRaw(`0:${"0".repeat(64)}`)), {kind: "Unit"})
    addresses.set(suspendedAddressKey(Address.parseRaw(`-1:${"f".repeat(64)}`)), {kind: "Unit"})

    const cell = beginCell()
      .store(
        storeSuspendedAddressList({
          kind: "SuspendedAddressList",
          addresses,
          suspended_until: 1_803_189_600,
        }),
      )
      .endCell()
    const config = parseSuspendedAccountsConfig(cell.toBoc().toString("base64"))

    expect({
      config,
      expiredAddressSuspended: isAddressSuspended(
        config,
        "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c",
        config.suspendedUntil,
      ),
      invalidAddressSuspended: isAddressSuspended(config, "invalid", 1_800_000_000),
      zeroAddressSuspended: isAddressSuspended(
        config,
        "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c",
        1_800_000_000,
      ),
      ordinaryAddressSuspended: isAddressSuspended(
        config,
        "EQDKbjIcfM6ezt8KjKJJLshZJJSqX7XOA4ff-W72r5gqPrHF",
        1_800_000_000,
      ),
    }).toMatchSnapshot()
  })

  test("caches each network separately and expires stale entries", () => {
    const values = new Map<string, string>()
    const storage = {
      getItem: (key: string) => values.get(key) ?? null,
      removeItem: (key: string) => {
        values.delete(key)
      },
      setItem: (key: string, value: string) => {
        values.set(key, value)
      },
    }
    const config = {
      rawAddresses: [`0:${"0".repeat(64)}`],
      suspendedUntil: 1_803_189_600,
    }
    const cachedAt = 1_800_000_000_000

    writeSuspendedAccountsConfigCache("https://mainnet.example/api/v2", config, storage, cachedAt)

    expect(
      readSuspendedAccountsConfigCache(
        "https://mainnet.example/api/v2",
        storage,
        cachedAt + SUSPENDED_ACCOUNTS_CACHE_TTL_MS - 1,
      ),
    ).toEqual(config)
    expect(
      readSuspendedAccountsConfigCache("https://testnet.example/api/v2", storage, cachedAt),
    ).toBeUndefined()
    expect(
      readSuspendedAccountsConfigCache(
        "https://mainnet.example/api/v2",
        storage,
        cachedAt + SUSPENDED_ACCOUNTS_CACHE_TTL_MS,
      ),
    ).toBeUndefined()
    expect(values.size).toBe(0)
  })
})

function suspendedAddressKey(address: Address): bigint {
  const workchain =
    address.workChain < 0
      ? WORKCHAIN_MODULUS + BigInt(address.workChain)
      : BigInt(address.workChain)
  return (workchain << ACCOUNT_ID_BITS) | BigInt(`0x${address.hash.toString("hex")}`)
}
