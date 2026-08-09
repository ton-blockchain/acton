import {FIFT_LANGUAGE_ID as fiftLanguageId, fiftLanguageSupport} from "./fift"
import {TASM_LANGUAGE_ID as tasmLanguageId, tasmLanguageSupport} from "./tasm"
import {TLB_LANGUAGE_ID as tlbLanguageId, tlbLanguageSupport} from "./tlb"
import {TOLK_LANGUAGE_ID as tolkLanguageId, tolkLanguageSupport} from "./tolk"
import type {LanguageSupport} from "./types"

export {FIFT_LANGUAGE_ID} from "./fift"
export {TASM_LANGUAGE_ID, TASM_SPEC_URL} from "./tasm"
export {TLB_LANGUAGE_ID} from "./tlb"
export {TOLK_LANGUAGE_ID} from "./tolk"

export const languageSupports = [
  tolkLanguageSupport,
  tasmLanguageSupport,
  tlbLanguageSupport,
  fiftLanguageSupport,
] as const

export type SupportedLanguage = (typeof languageSupports)[number]["id"]

export const defaultLanguageId: SupportedLanguage = tolkLanguageId

export const languageSupportById = {
  [tolkLanguageId]: tolkLanguageSupport,
  [tasmLanguageId]: tasmLanguageSupport,
  [tlbLanguageId]: tlbLanguageSupport,
  [fiftLanguageId]: fiftLanguageSupport,
} satisfies Record<SupportedLanguage, LanguageSupport>

export function isSupportedLanguage(value: unknown): value is SupportedLanguage {
  return (
    value === tolkLanguageId ||
    value === tasmLanguageId ||
    value === tlbLanguageId ||
    value === fiftLanguageId
  )
}

export function normalizeLanguage(value: unknown): SupportedLanguage {
  return isSupportedLanguage(value) ? value : defaultLanguageId
}
