import {createHighlighterCore} from "shiki/core"
import {createJavaScriptRegexEngine} from "shiki/engine/javascript"
import type {ThemedToken} from "shiki/types"

import {jetbrainsDarculaTheme, jetbrainsLightTheme} from "./jetbrains-themes"
import {tolkGrammar} from "./tolk-grammar"

export interface HighlightedToken {
  readonly content: string
  readonly color?: string
  readonly fontStyle?: number
  readonly htmlStyle?: Readonly<Record<string, string>>
}

type HighlightedLine = readonly HighlightedToken[]

let tolkHighlighterPromise: ReturnType<typeof createHighlighterCore> | undefined

const getTolkHighlighter = () => {
  tolkHighlighterPromise ??= createHighlighterCore({
    themes: [jetbrainsLightTheme, jetbrainsDarculaTheme],
    langs: [tolkGrammar],
    engine: createJavaScriptRegexEngine(),
  })

  return tolkHighlighterPromise
}

const getThemeName = (isDark: boolean) => {
  return isDark ? "jetbrains-darcula" : "jetbrains-light"
}

const toHighlightedToken = (token: ThemedToken): HighlightedToken => {
  return {
    content: token.content,
    color: token.color,
    fontStyle: token.fontStyle,
    htmlStyle: token.htmlStyle,
  }
}

export const highlightTolkToHtml = async (code: string, isDark: boolean) => {
  const highlighter = await getTolkHighlighter()
  // noinspection TypeScriptValidateTypes
  return highlighter.codeToHtml(code, {
    lang: "tolk",
    theme: getThemeName(isDark),
  })
}

export const highlightTolkToTokens = async (
  code: string,
  isDark: boolean,
): Promise<readonly HighlightedLine[]> => {
  const highlighter = await getTolkHighlighter()
  // noinspection TypeScriptValidateTypes
  const result = highlighter.codeToTokens(code, {
    lang: "tolk",
    theme: getThemeName(isDark),
  })

  return result.tokens.map(line => line.map(token => toHighlightedToken(token)))
}
