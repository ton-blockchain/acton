import {writeFile} from "node:fs/promises"

import {CONFLICT_RESOLUTIONS} from "./conflict-resolutions.ts"
import {mergeSources} from "./merge.ts"
import {resolveConflicts} from "./resolve.ts"
import {readSources} from "./sources.ts"

const ADDRESSES_JSON_URL = new URL("../src/addresses.json", import.meta.url)

async function main(): Promise<void> {
  const sources = await readSources()
  const merged = mergeSources(sources)
  const resolved = resolveConflicts(merged.conflicts, CONFLICT_RESOLUTIONS)

  if (resolved.unresolved.length > 0) {
    throw new Error(`Found ${resolved.unresolved.length} unresolved conflicts`)
  }

  const addresses = [...merged.addresses, ...resolved.addresses].toSorted((left, right) =>
    left.address.localeCompare(right.address),
  )

  await writeFile(ADDRESSES_JSON_URL, `${JSON.stringify(addresses, null, 2)}\n`, "utf8")

  for (const source of sources) {
    console.log(`${source.id}: read ${source.addresses.length} addresses`)
  }

  console.log(`merged: ${addresses.length} addresses`)
  console.log("wrote: src/addresses.json")
}

await main()
