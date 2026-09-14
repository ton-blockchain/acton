import {writeFile} from "node:fs/promises"
import {env} from "node:process"

import {Address} from "@ton/core"

import type {TonAssetsJetton} from "./sources/ton-assets.ts"
import {TON_ASSETS_JETTONS_URL, parseTonAssetsJettons} from "./sources/ton-assets.ts"
import {readText} from "./sources/shared.ts"

const JETTONS_JSON_URL = new URL("../src/jettons.json", import.meta.url)
const TONCENTER_JETTON_MASTERS_URL = "https://toncenter.com/api/v3/jetton/masters"
const TONCENTER_ADDRESS_BATCH_SIZE = 50
const TONCENTER_REQUEST_DELAY_MS = 100
const TONCENTER_REQUEST_TIMEOUT_MS = 15_000

const wait = (durationMs: number): Promise<void> =>
  new Promise(resolve => globalThis.setTimeout(resolve, durationMs))

const readToncenterText = async (url: string): Promise<string> => {
  // biome-ignore lint/style/noProcessEnv: credentials are optional local generator inputs
  const apiKey = env.TONCENTER_API_KEY
  const response = await fetch(url, {
    headers: apiKey ? {"X-API-Key": apiKey} : undefined,
    signal: AbortSignal.timeout(TONCENTER_REQUEST_TIMEOUT_MS),
  })
  if (!response.ok) {
    throw new Error(`Failed to read Toncenter metadata: HTTP ${response.status}`)
  }

  return response.text()
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const metadataImage = (value: unknown): string | undefined => {
  if (!(isRecord(value) && Array.isArray(value.token_info))) {
    return
  }

  const tokenInfo = value.token_info.find(
    entry => isRecord(entry) && entry.type === "jetton_masters" && entry.valid === true,
  )
  if (!isRecord(tokenInfo)) {
    return
  }

  const {extra} = tokenInfo
  if (isRecord(extra) && typeof extra._image_small === "string" && extra._image_small) {
    return extra._image_small
  }

  return typeof tokenInfo.image === "string" && tokenInfo.image ? tokenInfo.image : undefined
}

const parseToncenterImages = (text: string): ReadonlyMap<string, string> => {
  const response: unknown = JSON.parse(text)
  if (!(isRecord(response) && isRecord(response.metadata))) {
    return new Map()
  }

  const images = new Map<string, string>()
  for (const [sourceAddress, metadata] of Object.entries(response.metadata)) {
    const image = metadataImage(metadata)
    if (!image) {
      continue
    }

    try {
      images.set(Address.parse(sourceAddress).toRawString(), image)
    } catch {
      // Ignore metadata keys that are not TON addresses.
    }
  }

  return images
}

const resolveImages = async (
  jettons: readonly TonAssetsJetton[],
): Promise<readonly TonAssetsJetton[]> => {
  const addresses = jettons.map(jetton => jetton.address)
  const batches = Array.from(
    {length: Math.ceil(addresses.length / TONCENTER_ADDRESS_BATCH_SIZE)},
    (_, index) =>
      addresses.slice(
        index * TONCENTER_ADDRESS_BATCH_SIZE,
        (index + 1) * TONCENTER_ADDRESS_BATCH_SIZE,
      ),
  )
  const images = new Map<string, string>()
  const startedAt = performance.now()
  let failures = 0

  for (const [index, batchAddresses] of batches.entries()) {
    const url = new URL(TONCENTER_JETTON_MASTERS_URL)
    for (const address of batchAddresses) {
      url.searchParams.append("address", address)
    }
    url.searchParams.set("limit", batchAddresses.length.toString())

    try {
      // Requests stay sequential to respect Toncenter rate limits during scheduled syncs.
      // biome-ignore lint/performance/noAwaitInLoops: concurrency would defeat that pacing
      const batchImages = parseToncenterImages(await readToncenterText(url.toString()))
      for (const [address, image] of batchImages) {
        images.set(address, image)
      }
    } catch {
      failures += 1
    }

    if ((index + 1) % 5 === 0 || index + 1 === batches.length) {
      console.log(
        JSON.stringify({
          operation: "resolve_jetton_images",
          target: `${index + 1}/${batches.length} batches`,
          durationMs: Math.round(performance.now() - startedAt),
          outcome: "progress",
        }),
      )
    }

    if (index + 1 < batches.length) {
      await wait(TONCENTER_REQUEST_DELAY_MS)
    }
  }

  console.log(
    JSON.stringify({
      operation: "resolve_jetton_images",
      target: `${images.size}/${addresses.length} images, ${failures} failed batches`,
      durationMs: Math.round(performance.now() - startedAt),
      outcome: failures === 0 ? "complete" : "partial",
    }),
  )

  return jettons.map(jetton => {
    const image = images.get(jetton.address) ?? jetton.image
    return image ? {...jetton, image} : jetton
  })
}

async function main(): Promise<void> {
  const jettons = (
    await resolveImages(
      parseTonAssetsJettons(await readText(TON_ASSETS_JETTONS_URL), TON_ASSETS_JETTONS_URL),
    )
  ).toSorted(
    (left, right) =>
      left.name.localeCompare(right.name) ||
      left.symbol.localeCompare(right.symbol) ||
      left.address.localeCompare(right.address),
  )

  const uniqueAddresses = new Set(jettons.map(({address}) => address))
  if (uniqueAddresses.size !== jettons.length) {
    throw new Error("ton-assets jetton catalog contains duplicate master addresses")
  }

  await writeFile(JETTONS_JSON_URL, `${JSON.stringify(jettons, null, 2)}\n`, "utf8")

  console.log(`jettons: generated ${jettons.length} entries`)
  console.log("wrote: src/jettons.json")
}

await main()
