export type HighlightedCodeLanguage = "func" | "json" | "tasm" | "tlb" | "tolk"
export type HighlightedCodeTheme = "dark" | "light"

export interface HighlightedCodeToken {
  readonly content: string
  readonly color?: string
  readonly fontStyle?: number
  readonly htmlStyle?: Readonly<Record<string, string>>
}
