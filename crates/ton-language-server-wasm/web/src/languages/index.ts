import {TASM_LANGUAGE_ID as tasmLanguageId, tasmLanguageSupport} from "./tasm"
import {TLB_LANGUAGE_ID as tlbLanguageId, tlbLanguageSupport} from "./tlb"
import type {LanguageSupport} from "./types"

export {TASM_LANGUAGE_ID, TASM_SPEC_URL} from "./tasm"
export {TLB_LANGUAGE_ID} from "./tlb"

export const languageSupports = [tasmLanguageSupport, tlbLanguageSupport] as const

export type SupportedLanguage = (typeof languageSupports)[number]["id"]

export const defaultLanguageId: SupportedLanguage = tasmLanguageId

export const languageSupportById = {
  [tasmLanguageId]: tasmLanguageSupport,
  [tlbLanguageId]: tlbLanguageSupport,
} satisfies Record<SupportedLanguage, LanguageSupport>

export function isSupportedLanguage(value: unknown): value is SupportedLanguage {
  return value === tasmLanguageId || value === tlbLanguageId
}

export function normalizeLanguage(value: unknown): SupportedLanguage {
  return isSupportedLanguage(value) ? value : defaultLanguageId
}
