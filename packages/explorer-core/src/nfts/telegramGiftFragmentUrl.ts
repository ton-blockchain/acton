const FRAGMENT_GIFT_METADATA_URI =
  /^https:\/\/nft\.fragment\.com\/gift\/([a-z0-9]+-\d+)\.json(?:[?#].*)?$/i

export function getTelegramGiftFragmentUrl(
  content: Record<string, unknown> | undefined,
): string | undefined {
  const uri = content?.uri
  if (typeof uri !== "string") return undefined

  const slug = FRAGMENT_GIFT_METADATA_URI.exec(uri.trim())?.[1]
  return slug ? `https://fragment.com/gift/${slug.toLowerCase()}` : undefined
}
