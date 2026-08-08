import {hashToHex} from "../components/utils"

export function normalizeCodeHash(codeHash: string | null | undefined): string | undefined {
  const trimmed = codeHash?.trim()
  if (!trimmed) {
    return undefined
  }

  return hashToHex(trimmed.replace(/^0x/i, ""))
}
