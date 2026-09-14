import type {SyntheticEvent} from "react"

const TOKEN_PLACEHOLDER_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 237 237" fill="none"><style>path{fill:#4DB8FF}@media (prefers-color-scheme: dark){path{fill:#ecebeb}}</style><path d="M118.204 0.000292436C183.486 0.000292436 236.408 52.9224 236.408 118.205C236.408 183.487 183.486 236.408 118.204 236.408C52.9216 236.408 0.000184007 183.487 0 118.205C0 52.9225 52.9215 0.000452012 118.204 0.000292436ZM74.1011 62.1965C57.6799 62.1965 47.268 79.912 55.5308 94.2347L109.964 188.582C113.619 194.922 122.781 194.922 126.436 188.582L180.88 94.2347C189.132 79.9343 178.72 62.1966 162.31 62.1965H74.1011ZM162.288 78.8412C166.031 78.8412 168.234 82.8121 166.45 85.9075L137.856 137.091L137.851 137.099L126.506 159.046V78.8412H162.288ZM109.872 78.8517V159.024L98.5376 137.088L98.5334 137.08L69.9294 85.9215L69.8468 85.7725C68.2134 82.6997 70.405 78.8517 74.0899 78.8517H109.872Z"/></svg>'

export const TOKEN_PLACEHOLDER_IMAGE = `data:image/svg+xml,${encodeURIComponent(TOKEN_PLACEHOLDER_SVG)}`

const NFT_PLACEHOLDER_SVG = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="none">
  <path fill="#92939d" fill-opacity=".08" d="M0 0h256v256H0z"/>
  <g stroke="#92939d" stroke-width="4" stroke-linecap="round" stroke-linejoin="round">
    <path opacity=".3" d="M88 164h-4a12 12 0 0 1-12-12V84a12 12 0 0 1 12-12h68a12 12 0 0 1 12 12v4"/>
    <rect x="92" y="92" width="92" height="92" rx="12"/>
    <circle cx="118" cy="118" r="6"/>
    <path d="m104 164 22-24 14 14 12-14 20 24"/>
  </g>
</svg>`

/** Shared local artwork for missing or unavailable NFT images, independent of token branding. */
export const NFT_PLACEHOLDER_IMAGE = `data:image/svg+xml,${encodeURIComponent(NFT_PLACEHOLDER_SVG)}`

export const TOKEN_IMAGE_SOURCE_KEYS = [
  "_image_small",
  "_image_medium",
  "_image_big",
  "image",
] as const

export const NFT_IMAGE_SOURCE_KEYS = [
  "_image_small",
  "preview",
  "_image_medium",
  "_image_big",
  "image_url",
  "image",
] as const

export const NFT_CARD_IMAGE_SOURCE_KEYS = [
  "_image_medium",
  "_image_big",
  "image_url",
  "image",
  "preview",
  "_image_small",
] as const

export const NFT_COLLECTION_CARD_IMAGE_SOURCE_KEYS = [
  "collection_image_medium",
  "collection_image_big",
  "collection_image",
  "collection_image_small",
  ...NFT_CARD_IMAGE_SOURCE_KEYS,
] as const

export function deduplicateImageSources(sources: readonly string[]): string[] {
  return sources.filter(
    (source, index) =>
      source !== TOKEN_PLACEHOLDER_IMAGE &&
      source !== NFT_PLACEHOLDER_IMAGE &&
      sources.indexOf(source) === index,
  )
}

export function getImageSources(
  content: Record<string, unknown> | undefined,
  keys: readonly string[] = TOKEN_IMAGE_SOURCE_KEYS,
): string[] {
  const sources: string[] = []
  for (const key of keys) {
    const value = content?.[key]
    if (typeof value === "string" && value.length > 0 && !sources.includes(value)) {
      sources.push(value)
    }
  }
  return sources
}

/**
 * Uses explicit TON DNS metadata for domain artwork, keeping the original images as fallbacks.
 * NFT names alone are not DNS identifiers; callers still own content safety filtering.
 */
export function getNftImageSources(
  content: Record<string, unknown> | undefined,
  keys: readonly string[] = NFT_IMAGE_SOURCE_KEYS,
): string[] {
  const sources = getImageSources(content, keys)
  const extra = content?.extra
  const domain =
    content?.domain ??
    (extra && typeof extra === "object" && "domain" in extra ? extra.domain : undefined)
  const label = typeof domain === "string" ? /^([a-z0-9-]+)\.ton$/i.exec(domain)?.[1] : undefined

  return label
    ? deduplicateImageSources([
        `https://dns-image.mytonwallet.org/img?d=${encodeURIComponent(label.toLowerCase())}`,
        ...sources,
      ])
    : sources
}

export function getPrimaryImageSource(
  content: Record<string, unknown> | undefined,
  keys?: readonly string[],
): string {
  return getImageSources(content, keys)[0] ?? TOKEN_PLACEHOLDER_IMAGE
}

export function replaceBrokenImageWithFallback(
  event: SyntheticEvent<HTMLImageElement>,
  sources: readonly string[],
  placeholder = TOKEN_PLACEHOLDER_IMAGE,
) {
  const image = event.currentTarget
  const currentSource = image.getAttribute("src")
  if (currentSource === placeholder) {
    return
  }

  const candidates = [...deduplicateImageSources(sources), placeholder]
  const currentIndex = currentSource ? candidates.indexOf(currentSource) : -1
  const nextSource = candidates
    .slice(currentIndex >= 0 ? currentIndex + 1 : 0)
    .find(source => source !== currentSource)

  if (nextSource) {
    image.src = nextSource
  }
}
