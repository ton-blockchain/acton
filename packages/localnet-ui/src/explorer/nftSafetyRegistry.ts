import type {NftItem} from "./api/types"

import {NFT_IMAGE_SOURCE_KEYS} from "./components/imageFallbacks"

export const NSFW_NFT_REGISTRY = {
  imageUrls: [
    "https://cache.tonapi.io/imgproxy/wDYWflKVCrSqqoVsEEXrJWTgPzfxJZUbygFjXcqdOcc/rs:fill:1500:1500:1/g:no/aHR0cHM6Ly80NjI5LmNsb3VkbWV0cmljcy5jeW91L2ltYWdlcy8zP3Q9MTc4MzYyNzgyMzIzNw.webp",
  ],
  imageHostSuffixes: ["cloudmetrics.cyou"],
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
  collectionNames: [
    "Bunker Of Death 1781768564560",
    "t.ме/ոft_Ꮟսⲅո",
    "t.мe/nft_bսⲅո",
    "t.me/ոƒτ_вսгn",
    "t.ме/ոft_ƅսⲅո",
    "t.ме/nft_ƅurn",
  ],
} as const

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

const nsfwImageUrls = new Set(NSFW_NFT_REGISTRY.imageUrls.map(normalizeImageUrl))
const nsfwImageHostSuffixes = new Set(NSFW_NFT_REGISTRY.imageHostSuffixes)
const nsfwContentHashes = new Set(NSFW_NFT_REGISTRY.contentHashes.map(normalizeContentHash))
const nsfwCollectionNames = new Set(NSFW_NFT_REGISTRY.collectionNames.map(normalizeCollectionName))

const isRegisteredNsfwImageUrl = (value: string): boolean => {
  if (nsfwImageUrls.has(normalizeImageUrl(value))) {
    return true
  }

  try {
    const hostname = new URL(value).hostname.toLowerCase()
    return [...nsfwImageHostSuffixes].some(
      suffix => hostname === suffix || hostname.endsWith(`.${suffix}`),
    )
  } catch {
    return false
  }
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
export const isRegisteredNsfwNft = ({
  imageUrl,
  contentHash,
  collectionName,
}: NftSafetyCandidate): boolean =>
  (imageUrl !== undefined &&
    (isRegisteredNsfwImageUrl(imageUrl) ||
      isRegisteredNsfwImageUrl(imageUrlFromToncenterProxyUrl(imageUrl) ?? "") ||
      nsfwContentHashes.has(contentHashFromToncenterProxyUrl(imageUrl) ?? ""))) ||
  (contentHash !== undefined && nsfwContentHashes.has(normalizeContentHash(contentHash))) ||
  (collectionName !== undefined && nsfwCollectionNames.has(normalizeCollectionName(collectionName)))

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
