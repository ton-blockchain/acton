import type {languages} from "@codingame/monaco-vscode-editor-api"
import type {LanguageSupport} from "../types"

export const ACTON_TOML_LANGUAGE_ID = "toml"

export const defaultActonTomlSource = `[package]
name = "web-playground"
version = "0.1.0"

[import-mappings]
workspace = "./"
`

export const actonTomlLanguageSupport = {
  id: ACTON_TOML_LANGUAGE_ID,
  label: "Acton.toml",
  fileExtension: "toml",
  defaultSource: defaultActonTomlSource,
  extensionPoint: {
    id: ACTON_TOML_LANGUAGE_ID,
    aliases: ["Acton.toml", "TOML"],
    extensions: [".toml"],
    filenames: ["Acton.toml"],
  },
  monarchLanguage: {
    tokenizer: {
      root: [
        [/#.*$/, "comment"],
        [/\[[^\]]+\]/, "keyword"],
        [/"(?:[^"\\]|\\.)*"/, "string"],
        [/\b\d+(?:\.\d+)*\b/, "number"],
        [/\b(?:true|false)\b/, "keyword"],
        [/[=.,]/, "delimiter"],
        [/[A-Za-z0-9_-]+/, "identifier"],
      ] satisfies languages.IMonarchLanguageRule[],
    },
  },
} as const satisfies LanguageSupport<typeof ACTON_TOML_LANGUAGE_ID>
