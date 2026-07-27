import {CONFLICT_RESOLUTIONS} from "./conflict-resolutions.ts"
import {mergeSources} from "./merge.ts"
import {resolveConflicts} from "./resolve.ts"
import {readSources} from "./sources.ts"

async function main(): Promise<void> {
  const sources = await readSources()
  const merged = mergeSources(sources)
  const resolved = resolveConflicts(merged.conflicts, CONFLICT_RESOLUTIONS)
  const addresses = [...merged.addresses, ...resolved.addresses]

  if (resolved.unresolved.length > 0) {
    throw new Error(`Found ${resolved.unresolved.length} unresolved conflicts`)
  }

  for (const source of sources) {
    console.log(`${source.id}: read ${source.addresses.length} addresses`)
  }

  console.log(`merged: ${addresses.length} addresses`)
}

await main()
