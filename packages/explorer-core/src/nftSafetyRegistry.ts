import {sha256_sync} from "@ton/crypto"

import type {NftItem} from "./api/types"

import {NFT_IMAGE_SOURCE_KEYS} from "./components/imageFallbacks"

export const NSFW_NFT_REGISTRY = {
  imageUrlHashes: ["53f666edac772f162656b483f07c8e6811b0ce6a42f20e243ba513280f622e29"],
  imageHostSuffixHashes: ["dd1b7a6727862b9faa5e38655ab1d8c11f943b518b73a70da1367912dbe8a5e3"],
  contentHashes: [
    "ead9e3c5f260785e8852c0dca7ad1fd7e6690b03009a158a849243e59685aa7d",
    "1c7120b6ca981bd77fff11376b0cfaf5ceefdcd8206c4ea030446f2c7e0a4f23",
    "ced2746e922cd0a2a1852a82c2d84ec1c3f9fc683fd2f6de9cf28bed7dfbabf4",
    "b782c09a750da9598ef84db6cb76f0f142071e9674625ca70a534d6addae794b",
    "534b1ac5d274999ae861c6b515166f57d002f2c86c3fd619a25581abda15b88a",
    "9697402e217a999d065c81db61e159a8f7e9d5dc108433e64eb74c3726397581",
    "a1d6fab43259491b282fa553216c1416dc03ddb23adb3fdb9293f8b0e1238a55",
    "c227582c2110266d04ba33ca6d5a117221d058fb055f25c6176acfff69994902",
  ],
  collectionNameHashes: [
    "280fff43bf5c7aea8aef4c4757953cfb4cb637e6bdc4c066203c1473b9d28b1a",
    "2140db963b117a2e103815200b18769a7f265f18f113228709230741f5880a41",
    "278b57f87e6a286c4aff92d7098a13eb102d709d46abc33214b632482ffbddab",
    "8b861ee6eb6c056279bce13318957248c1ecc7ecebff9e63bb7e367ef65d7eaf",
    "8088a3d22d88109a95a3f905075a9632586b9a3b5518eb096ef01a6936f9190d",
    "3ea3e6af075ccf60eb6f217859ff70ddf1903ea3c7ed24c57f7b64ecedee6f4f",
    "d4bb2725a28f0e9093eaa8b1f181084ac5c2e03af98c694a04cc1a9cd99d8880",
  ],
} as const

export interface NftSafetyRegistry {
  readonly imageUrlHashes: readonly string[]
  readonly imageHostSuffixHashes: readonly string[]
  readonly contentHashes: readonly string[]
  readonly collectionNameHashes: readonly string[]
}

const hashRegistryValue = (value: string): string => sha256_sync(value).toString("hex")

const normalizeImageUrl = (value: string): string => {
  try {
    const url = new URL(value)
    url.search = ""
    url.hash = ""
    url.pathname = url.pathname.replace(/\/+$/, "") || "/"
    return url.toString()
  } catch {
    return value.trim()
  }
}

const normalizeContentHash = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/^sha256:/, "")

const normalizeCollectionName = (value: string): string =>
  value.trim().toLowerCase().replace(/\s+/g, " ")

const sourceFromToncenterProxyUrl = (value: string): string | undefined => {
  try {
    const url = new URL(value)
    if (url.hostname !== "toncenter.com" && !url.hostname.endsWith(".toncenter.com")) {
      return undefined
    }

    const encodedSource = url.pathname.split("/").at(-1)
    if (!encodedSource) {
      return undefined
    }

    const base64 = encodedSource.replace(/-/g, "+").replace(/_/g, "/")
    return atob(base64.padEnd(Math.ceil(base64.length / 4) * 4, "="))
  } catch {
    return undefined
  }
}

const contentHashFromToncenterProxyUrl = (value: string): string | undefined => {
  const source = sourceFromToncenterProxyUrl(value)
  return source !== undefined && /^local:\/\/\/sha256\/[0-9a-f]{64}$/i.test(source)
    ? source.slice("local:///sha256/".length).toLowerCase()
    : undefined
}

const imageUrlFromToncenterProxyUrl = (value: string): string | undefined => {
  const source = sourceFromToncenterProxyUrl(value)
  return source !== undefined && /^https?:\/\//i.test(source) ? source : undefined
}

interface NftSafetyCandidate {
  readonly imageUrl?: string
  readonly contentHash?: string
  readonly collectionName?: string
}

/**
 * URL and collection checks prevent known content from being rendered before it is downloaded.
 * Content hashes catch the same image when it is served from a URL that is not in the registry.
 */
export const createNftSafetyMatcher = (registry: NftSafetyRegistry) => {
  const nsfwImageUrlHashes = new Set(registry.imageUrlHashes)
  const nsfwImageHostSuffixHashes = new Set(registry.imageHostSuffixHashes)
  const nsfwContentHashes = new Set(registry.contentHashes.map(normalizeContentHash))
  const nsfwCollectionNameHashes = new Set(registry.collectionNameHashes)

  const isRegisteredNsfwImageUrl = (value: string): boolean => {
    if (nsfwImageUrlHashes.has(hashRegistryValue(normalizeImageUrl(value)))) {
      return true
    }

    try {
      const hostnameParts = new URL(value).hostname.toLowerCase().split(".")
      return hostnameParts.some((_, index) =>
        nsfwImageHostSuffixHashes.has(hashRegistryValue(hostnameParts.slice(index).join("."))),
      )
    } catch {
      return false
    }
  }

  return ({imageUrl, contentHash, collectionName}: NftSafetyCandidate): boolean =>
    (imageUrl !== undefined &&
      (isRegisteredNsfwImageUrl(imageUrl) ||
        isRegisteredNsfwImageUrl(imageUrlFromToncenterProxyUrl(imageUrl) ?? "") ||
        nsfwContentHashes.has(contentHashFromToncenterProxyUrl(imageUrl) ?? ""))) ||
    (contentHash !== undefined && nsfwContentHashes.has(normalizeContentHash(contentHash))) ||
    (collectionName !== undefined &&
      nsfwCollectionNameHashes.has(hashRegistryValue(normalizeCollectionName(collectionName))))
}

const matchesNsfwRegistry = createNftSafetyMatcher(NSFW_NFT_REGISTRY)

export const isRegisteredNsfwNft = (candidate: NftSafetyCandidate): boolean =>
  matchesNsfwRegistry(candidate)

const contentString = (
  content: Record<string, unknown> | undefined,
  key: string,
): string | undefined => {
  const value = content?.[key]
  return typeof value === "string" && value.length > 0 ? value : undefined
}

export const isNftItemNsfw = (item: NftItem): boolean => {
  if (item.is_nsfw === true) {
    return true
  }

  const collectionName =
    contentString(item.content, "collection_name") ||
    contentString(item.collection?.collection_content, "name")
  if (collectionName && isRegisteredNsfwNft({collectionName})) {
    return true
  }

  return NFT_IMAGE_SOURCE_KEYS.some(key => {
    const imageUrl = contentString(item.content, key)
    return imageUrl !== undefined && isRegisteredNsfwNft({imageUrl})
  })
}
