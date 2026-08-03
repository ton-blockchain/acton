import {writeFile} from "node:fs/promises"

import {Address} from "@ton/core"

import {CONFLICT_RESOLUTIONS} from "./conflict-resolutions.ts"
import {mergeSources} from "./merge.ts"
import {resolveConflicts} from "./resolve.ts"
import {readSources} from "./sources.ts"

const MAINNET_JSON_URL = new URL("../src/mainnet.json", import.meta.url)
const TESTNET_JSON_URL = new URL("../src/testnet.json", import.meta.url)

const isTestnetAddress = (address: string): boolean =>
  Address.isFriendly(address) && Address.parseFriendly(address).isTestOnly

async function main(): Promise<void> {
  const sources = await readSources()

  for (const source of sources) {
    console.log(`${source.id}: read ${source.addresses.length} addresses`)
  }

  const testnetRawAddresses = new Set(
    sources.flatMap(source =>
      source.addresses
        .filter(({address}) => isTestnetAddress(address))
        .map(({address}) => Address.parse(address).toRawString()),
    ),
  )

  const merged = mergeSources(sources)
  const resolved = resolveConflicts(merged.conflicts, CONFLICT_RESOLUTIONS)

  if (resolved.unresolved.length > 0) {
    throw new Error(`Found ${resolved.unresolved.length} unresolved conflicts`)
  }

  const addresses = [...merged.addresses, ...resolved.addresses].toSorted((left, right) =>
    left.address.localeCompare(right.address),
  )
  const mainnetAddresses: typeof addresses = []
  const testnetAddresses: typeof addresses = []

  for (const address of addresses) {
    if (testnetRawAddresses.has(address.address)) {
      testnetAddresses.push(address)
    } else {
      mainnetAddresses.push(address)
    }
  }

  await Promise.all([
    writeFile(MAINNET_JSON_URL, `${JSON.stringify(mainnetAddresses, null, 2)}\n`, "utf8"),
    writeFile(TESTNET_JSON_URL, `${JSON.stringify(testnetAddresses, null, 2)}\n`, "utf8"),
  ])

  console.log(`mainnet: merged ${mainnetAddresses.length} addresses`)
  console.log("wrote: src/mainnet.json")
  console.log(`testnet: merged ${testnetAddresses.length} addresses`)
  console.log("wrote: src/testnet.json")
}

await main()
