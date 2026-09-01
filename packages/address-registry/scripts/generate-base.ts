import {writeFile} from "node:fs/promises"

import {Address} from "@ton/core"

import {CONFLICT_RESOLUTIONS} from "./conflict-resolutions.ts"
import {mergeSources} from "./merge.ts"
import type {AddressConflict} from "./merge.ts"
import {resolveConflicts} from "./resolve.ts"
import {readSources} from "./sources.ts"

const MAINNET_BASE_JSON_URL = new URL("../src/mainnet-base.json", import.meta.url)
const TESTNET_BASE_JSON_URL = new URL("../src/testnet-base.json", import.meta.url)
const UNRESOLVED_CONFLICTS_JSON_URL = new URL("../src/unresolved-conflicts.json", import.meta.url)

const isTestnetAddress = (address: string): boolean =>
  Address.isFriendly(address) && Address.parseFriendly(address).isTestOnly

const printConflicts = (conflicts: readonly AddressConflict[]): void => {
  console.warn("\nUnresolved conflicts:")

  for (const [index, conflict] of conflicts.entries()) {
    if (index > 0) {
      console.warn()
    }

    console.warn(conflict.address)

    for (const candidate of conflict.candidates) {
      console.warn(`\t- ${candidate.source}: ${candidate.name}`)
    }
  }

  console.warn()
}

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
  const unresolvedConflicts = resolved.unresolved.toSorted((left, right) =>
    left.address.localeCompare(right.address),
  )

  if (unresolvedConflicts.length > 0) {
    printConflicts(unresolvedConflicts)
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
    writeFile(MAINNET_BASE_JSON_URL, `${JSON.stringify(mainnetAddresses, null, 2)}\n`, "utf8"),
    writeFile(TESTNET_BASE_JSON_URL, `${JSON.stringify(testnetAddresses, null, 2)}\n`, "utf8"),
    writeFile(
      UNRESOLVED_CONFLICTS_JSON_URL,
      `${JSON.stringify(unresolvedConflicts, null, 2)}\n`,
      "utf8",
    ),
  ])

  console.log(`mainnet: merged ${mainnetAddresses.length} addresses`)
  console.log("wrote: src/mainnet-base.json")
  console.log(`testnet base: merged ${testnetAddresses.length} addresses`)
  console.log("wrote: src/testnet-base.json")
  console.log(`unresolved conflicts: ${unresolvedConflicts.length}`)
  console.log("wrote: src/unresolved-conflicts.json")
}

await main()
