import {mergeSources} from "./merge.ts"
import {readSources} from "./sources.ts"

async function main(): Promise<void> {
  const sources = await readSources()
  const result = mergeSources(sources)

  for (const source of sources) {
    console.log(`${source.id}: read ${source.addresses.length} addresses`)
  }

  console.log(`merged: ${result.addresses.length} addresses`)
  console.log(`conflicts: ${result.conflicts.length}`)
}

await main()
