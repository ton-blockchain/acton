import {useEffect} from "react"
import {useMonaco} from "@monaco-editor/react"
import type * as monacoTypes from "monaco-editor"
import {useTheme} from "@acton/ui"

import {DARK_THEME, LIGHT_THEME} from "../themes"
import {funcLanguageDefinition} from "../languages/func-language-definition"
import {tasmLanguageDefinition} from "../languages/tasm-language-definition"
import {tolkLanguageDefinition} from "../languages/tolk-language-definition"
import {FUNC_LANGUAGE_ID, TASM_LANGUAGE_ID, TOLK_LANGUAGE_ID} from "../languages"

export type SupportedLanguage = "tasm" | "func" | "tolk"

interface UseMonacoSetupOptions {
  readonly language: SupportedLanguage
}

interface UseMonacoSetupReturn {
  readonly monaco: typeof monacoTypes | null
  readonly isMac: boolean
  readonly theme: MonacoTheme
}

export type MonacoTheme = "light-theme" | "dark-theme"

export const initializeMonaco = (monaco: typeof monacoTypes, language: SupportedLanguage) => {
  if (language === "tasm") {
    if (!monaco.languages.getLanguages().some(candidate => candidate.id === TASM_LANGUAGE_ID)) {
      monaco.languages.register({id: TASM_LANGUAGE_ID})
    }
    monaco.languages.setMonarchTokensProvider(TASM_LANGUAGE_ID, tasmLanguageDefinition)
  }

  if (language === "func") {
    if (!monaco.languages.getLanguages().some(candidate => candidate.id === FUNC_LANGUAGE_ID)) {
      monaco.languages.register({id: FUNC_LANGUAGE_ID})
    }
    monaco.languages.setMonarchTokensProvider(FUNC_LANGUAGE_ID, funcLanguageDefinition)
  }

  if (language === "tolk") {
    if (!monaco.languages.getLanguages().some(candidate => candidate.id === TOLK_LANGUAGE_ID)) {
      monaco.languages.register({id: TOLK_LANGUAGE_ID})
    }
    monaco.languages.setMonarchTokensProvider(TOLK_LANGUAGE_ID, tolkLanguageDefinition)
  }

  monaco.editor.defineTheme("light-theme", LIGHT_THEME)
  monaco.editor.defineTheme("dark-theme", DARK_THEME)
}

export const useMonacoSetup = ({language}: UseMonacoSetupOptions): UseMonacoSetupReturn => {
  const monaco = useMonaco()
  const {theme: appTheme} = useTheme()
  const theme: MonacoTheme = appTheme === "dark" ? "dark-theme" : "light-theme"
  const isMac =
    typeof globalThis.navigator !== "undefined" &&
    globalThis.navigator.platform.toUpperCase().includes("MAC")

  useEffect(() => {
    if (!monaco) return

    try {
      initializeMonaco(monaco, language)
      monaco.editor.setTheme(theme)
    } catch (error) {
      console.error("Failed to initialize Monaco:", error)
    }
  }, [language, monaco, theme])

  return {
    monaco,
    isMac,
    theme,
  }
}
