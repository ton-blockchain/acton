import type {languages} from "@codingame/monaco-vscode-editor-api"
import type {LanguageSupport} from "../types"

export const TLB_LANGUAGE_ID = "tlb"

export const tlbLanguageSupport = {
  id: TLB_LANGUAGE_ID,
  label: "TL-B",
  fileExtension: "tlb",
  defaultSource: [
    "foo$0 a:# = CommonMsgInfo;",
    "bar$1 b:# = CommonMsgInfo;",
    "baz$2 x:CommonMsgInfo = Wrap;",
  ].join("\n"),
  extensionPoint: {
    id: TLB_LANGUAGE_ID,
    aliases: ["TL-B", "tlb"],
    extensions: [".tlb"],
  },
  monarchLanguage: {
    tokenizer: {
      root: [
        [/;.*$/, "comment"],
        [/[{}()[\]:=;]/, "delimiter"],
        [/\$[0-9a-fA-F_]+/, "number.hex"],
        [/#|##|\^/, "keyword"],
        [/[A-Z][A-Za-z0-9_]*/, "type.identifier"],
        [/[a-z_][A-Za-z0-9_]*/, "identifier"],
      ] satisfies languages.IMonarchLanguageRule[],
    },
  },
} as const satisfies LanguageSupport<typeof TLB_LANGUAGE_ID>
