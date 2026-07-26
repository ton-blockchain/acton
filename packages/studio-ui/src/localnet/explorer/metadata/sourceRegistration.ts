import type {VerificationSourceResponse} from "../api/types"
import {normalizeCodeHash} from "./codeHash"
import type {SourceRegistration} from "./types"

export function sourceRegistrationFromResponse(
  source: VerificationSourceResponse,
  fallbackCodeHash?: string,
): SourceRegistration | undefined {
  const codeHash = normalizeCodeHash(source.code_hash) ?? normalizeCodeHash(fallbackCodeHash)
  if (!codeHash) {
    return undefined
  }
  return {codeHash, source}
}
