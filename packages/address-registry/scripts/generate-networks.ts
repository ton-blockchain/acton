import {readFile, writeFile} from "node:fs/promises"
import {env} from "node:process"

import {readNetworkAccountStates} from "./network/account-states-api.ts"
import {findMatchingStatusAddresses} from "./network/account-statuses.ts"

const MAINNET_BASE_JSON_URL = new URL("../src/mainnet-base.json", import.meta.url)
const TESTNET_BASE_JSON_URL = new URL("../src/testnet-base.json", import.meta.url)
const MAINNET_JSON_URL = new URL("../src/mainnet.json", import.meta.url)
const TESTNET_JSON_URL = new URL("../src/testnet.json", import.meta.url)

interface AddressEntry {
  readonly address: string
  readonly name: string
}

const readAddressEntries = async (url: URL): Promise<readonly AddressEntry[]> =>
  JSON.parse(await readFile(url, "utf8")) as readonly AddressEntry[]

async function main(): Promise<void> {
  // biome-ignore lint/style/noProcessEnv: this optional local credential is not application config
  const apiKey = env.TONCENTER_API_KEY
  const [mainnetBaseAddresses, testnetBaseAddresses] = await Promise.all([
    readAddressEntries(MAINNET_BASE_JSON_URL),
    readAddressEntries(TESTNET_BASE_JSON_URL),
  ])
  const states = await readNetworkAccountStates(
    mainnetBaseAddresses.map(({address}) => address),
    apiKey,
  )
  const matchingStatusAddresses = findMatchingStatusAddresses(states.mainnet, states.testnet)

  const entries = new Map(
    mainnetBaseAddresses
      .filter(({address}) => matchingStatusAddresses.has(address))
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
