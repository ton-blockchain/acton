import {readFile, writeFile} from "node:fs/promises"
import {env} from "node:process"

import {Address} from "@ton/core"

const TESTNET_ACCOUNT_STATES_URL = "https://testnet.toncenter.com/api/v3/accountStates"
const MAINNET_BASE_JSON_URL = new URL("../src/mainnet-base.json", import.meta.url)
const TESTNET_BASE_JSON_URL = new URL("../src/testnet-base.json", import.meta.url)
const MAINNET_JSON_URL = new URL("../src/mainnet.json", import.meta.url)
const TESTNET_JSON_URL = new URL("../src/testnet.json", import.meta.url)
const BATCH_SIZE = 100
const UNAUTHENTICATED_REQUEST_DELAY_MS = 1100

interface AccountState {
  readonly address: string
  readonly status: string
}

interface AccountStatesResponse {
  readonly accounts: readonly AccountState[]
}

interface AddressEntry {
  readonly address: string
  readonly name: string
}

const wait = (milliseconds: number): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, milliseconds))

const readAddressEntries = async (url: URL): Promise<readonly AddressEntry[]> =>
  JSON.parse(await readFile(url, "utf8")) as readonly AddressEntry[]

const readAccountStates = async (
  addresses: readonly string[],
  apiKey: string | undefined,
): Promise<readonly AccountState[]> => {
  const parameters = new URLSearchParams()
  parameters.set("include_boc", "false")
  for (const address of addresses) {
    parameters.append("address", address)
  }

  const response = await fetch(`${TESTNET_ACCOUNT_STATES_URL}?${parameters}`, {
    headers: apiKey ? {"X-API-Key": apiKey} : undefined,
  })
  if (!response.ok) {
    throw new Error(`Failed to read testnet account states: HTTP ${response.status}`)
  }

  const result = (await response.json()) as AccountStatesResponse
  return result.accounts
}

async function main(): Promise<void> {
  // biome-ignore lint/style/noProcessEnv: this optional local credential is not application config
  const apiKey = env.TONCENTER_API_KEY
  const [mainnetBaseAddresses, testnetBaseAddresses] = await Promise.all([
    readAddressEntries(MAINNET_BASE_JSON_URL),
    readAddressEntries(TESTNET_BASE_JSON_URL),
  ])
  const activeAddresses = new Set<string>()

  for (let offset = 0; offset < mainnetBaseAddresses.length; offset += BATCH_SIZE) {
    const batch = mainnetBaseAddresses.slice(offset, offset + BATCH_SIZE)
    // Requests are sequential to respect the public API rate limit without a key.
    // biome-ignore lint/performance/noAwaitInLoops: the endpoint is intentionally rate-limited
    const states = await readAccountStates(
      batch.map(({address}) => address),
      apiKey,
    )

    for (const state of states) {
      if (state.status === "active") {
        activeAddresses.add(Address.parse(state.address).toRawString())
      }
    }

    if (!apiKey && offset + BATCH_SIZE < mainnetBaseAddresses.length) {
      await wait(UNAUTHENTICATED_REQUEST_DELAY_MS)
    }
  }

  const entries = new Map(
    mainnetBaseAddresses
      .filter(({address}) => activeAddresses.has(address))
      .map(entry => [entry.address, entry] as const),
  )
  for (const entry of testnetBaseAddresses) {
    entries.set(entry.address, entry)
  }

  const testnetAddresses = [...entries.values()].toSorted((left, right) =>
    left.address.localeCompare(right.address),
  )
  await Promise.all([
    writeFile(MAINNET_JSON_URL, `${JSON.stringify(mainnetBaseAddresses, null, 2)}\n`, "utf8"),
    writeFile(TESTNET_JSON_URL, `${JSON.stringify(testnetAddresses, null, 2)}\n`, "utf8"),
  ])

  console.log(`mainnet: generated ${mainnetBaseAddresses.length} addresses`)
  console.log("wrote: src/mainnet.json")
  console.log(`testnet: generated ${testnetAddresses.length} addresses`)
  console.log("wrote: src/testnet.json")
}

await main()
