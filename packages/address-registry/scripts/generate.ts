import {readSources} from "./sources.ts"

async function main(): Promise<void> {
  const sources = await readSources()

  for (const source of sources) {
    console.log(`${source.id}: read ${source.addresses.length} addresses`)
  }
}

await main()
