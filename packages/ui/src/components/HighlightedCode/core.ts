import {createHighlighterCore} from "shiki/core"
import {createJavaScriptRegexEngine} from "shiki/engine/javascript"
import type {LanguageRegistration} from "shiki/types"

import funcGrammarRaw from "./grammars/Func.tmLanguage.json"
import tactGrammarRaw from "./grammars/Tact.tmLanguage.json"
import tasmGrammarRaw from "./grammars/Tasm.tmLanguage.json"
import tlbGrammarRaw from "./grammars/Tlb.tmLanguage.json"
import tolkGrammarRaw from "./grammars/Tolk.tmLanguage.json"

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
      // Source: tact-lang/tact-sublime@066e45b2de8bc6182ef5cffce8d5f7f99d602d25.
      // The upstream MIT license is stored next to the grammar.
      grammarWithName(tactGrammarRaw, "tact"),
      grammarWithName(tasmGrammarRaw, "tasm"),
      grammarWithName(tlbGrammarRaw, "tlb"),
      grammarWithName(tolkGrammarRaw, "tolk"),
      import("shiki/langs/javascript.mjs"),
      import("shiki/langs/json.mjs"),
      import("shiki/langs/shellscript.mjs"),
      import("shiki/langs/toml.mjs"),
    ],
    engine: createJavaScriptRegexEngine(),
  })

  return highlighterPromise
}

export const highlightedCodeThemeName = (theme: HighlightedCodeTheme) =>
  theme === "dark" ? "jetbrains-darcula" : "jetbrains-light"
