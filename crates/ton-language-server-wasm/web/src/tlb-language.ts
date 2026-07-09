import type {languages} from "@codingame/monaco-vscode-editor-api"

export const TLB_LANGUAGE_ID = "tlb"

export const tlbLanguageExtension: languages.ILanguageExtensionPoint = {
  id: TLB_LANGUAGE_ID,
  aliases: ["TL-B", "tlb"],
  extensions: [".tlb"],
}

export const tlbMonarchLanguage: languages.IMonarchLanguage = {
  tokenizer: {
    root: [
      [/;.*$/, "comment"],
      [/[{}()[\]:=;]/, "delimiter"],
      [/\$[0-9a-fA-F_]+/, "number.hex"],
      [/#|##|\^/, "keyword"],
      [/[A-Z][A-Za-z0-9_]*/, "type.identifier"],
      [/[a-z_][A-Za-z0-9_]*/, "identifier"],
    ],
  },
}
