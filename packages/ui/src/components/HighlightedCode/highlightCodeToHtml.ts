import {getHighlightedCodeHighlighter, highlightedCodeThemeName} from "./core"
import type {HighlightedCodeLanguage, HighlightedCodeTheme} from "./types"

export async function highlightCodeToHtml(
  value: string,
  language: HighlightedCodeLanguage,
  theme: HighlightedCodeTheme,
) {
  const highlighter = await getHighlightedCodeHighlighter()
  return highlighter.codeToHtml(value, {
    lang: language,
    theme: highlightedCodeThemeName(theme),
  })
}
