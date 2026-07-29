export type HighlightedCodeLanguage =
  | "func"
  | "javascript"
  | "json"
  | "shellscript"
  | "tact"
  | "tasm"
  | "tlb"
  | "tolk"
  | "toml"
export type HighlightedCodeTheme = "dark" | "light"

export interface HighlightedCodeToken {
  readonly content: string
  readonly color?: string
  readonly fontStyle?: number
  readonly htmlStyle?: Readonly<Record<string, string>>
}
