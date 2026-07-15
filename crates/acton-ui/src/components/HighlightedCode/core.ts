import {createHighlighterCore} from "shiki/core"
import {createJavaScriptRegexEngine} from "shiki/engine/javascript"
import type {LanguageRegistration} from "shiki/types"

import funcGrammarRaw from "../../../../../docs/grammars/grammar-func.json"
import tasmGrammarRaw from "../../../../../docs/grammars/grammar-tasm.json"
import tlbGrammarRaw from "../../../../../docs/grammars/grammar-tlb.json"
import tolkGrammarRaw from "../../../../../docs/grammars/grammar-tolk.json"

import {jetbrainsDarculaTheme, jetbrainsLightTheme} from "./themes"
import type {HighlightedCodeLanguage, HighlightedCodeTheme} from "./types"

const grammarWithName = (grammar: unknown, name: HighlightedCodeLanguage): LanguageRegistration =>
  ({
    ...(grammar as Record<string, unknown>),
    name,
  }) as LanguageRegistration

let highlighterPromise: ReturnType<typeof createHighlighterCore> | undefined

export const getHighlightedCodeHighlighter = () => {
  highlighterPromise ??= createHighlighterCore({
    themes: [jetbrainsLightTheme, jetbrainsDarculaTheme],
    langs: [
      grammarWithName(funcGrammarRaw, "func"),
      grammarWithName(tasmGrammarRaw, "tasm"),
      grammarWithName(tlbGrammarRaw, "tlb"),
      grammarWithName(tolkGrammarRaw, "tolk"),
      import("shiki/langs/json.mjs"),
    ],
    engine: createJavaScriptRegexEngine(),
  })

  return highlighterPromise
}

export const highlightedCodeThemeName = (theme: HighlightedCodeTheme) =>
  theme === "dark" ? "jetbrains-darcula" : "jetbrains-light"
