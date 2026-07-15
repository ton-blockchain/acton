import type {ThemedToken} from "shiki/types"

import {getHighlightedCodeHighlighter, highlightedCodeThemeName} from "./core"
import type {HighlightedCodeLanguage, HighlightedCodeTheme, HighlightedCodeToken} from "./types"

export type {HighlightedCodeLanguage, HighlightedCodeTheme, HighlightedCodeToken} from "./types"

export async function highlightCodeToTokens(
  value: string,
  language: HighlightedCodeLanguage,
  theme: HighlightedCodeTheme,
): Promise<readonly (readonly HighlightedCodeToken[])[]> {
  const highlighter = await getHighlightedCodeHighlighter()
  const result = highlighter.codeToTokens(value, {
    lang: language,
    theme: highlightedCodeThemeName(theme),
  })

  return result.tokens.map(line => line.map(toHighlightedCodeToken))
}

function toHighlightedCodeToken(token: ThemedToken): HighlightedCodeToken {
  return {
    content: token.content,
    color: token.color,
    fontStyle: token.fontStyle,
    htmlStyle: token.htmlStyle,
  }
}
