import type {languages} from "@codingame/monaco-vscode-editor-api"
import type {LanguageSupport} from "../types"

export const FIFT_LANGUAGE_ID = "fift"

export const fiftLanguageSupport = {
  id: FIFT_LANGUAGE_ID,
  label: "Fift",
  fileExtension: "fif",
  defaultSource: `PROGRAM{
DECLPROC entry
entry PROC:<{
  IF:<{
    1 PUSHINT
  }>ELSE<{
    2 PUSHINT
  }>
  WHILE:<{
    3 PUSHINT
  }>DO<{
    4 PUSHINT
  }>
  <{
    5 PUSHINT
  }>
}>
END>c`,
  extensionPoint: {
    id: FIFT_LANGUAGE_ID,
    aliases: ["Fift", "fift"],
    extensions: [".fif", ".fift"],
  },
  monarchLanguage: {
    tokenizer: {
      root: [
        [/\/\/.*$/, "comment"],
        [/(?:PROGRAM\{|END>c|PROC(?:INLINE|REF)?:<\{|METHOD:<\{)/, "keyword"],
        [/(?:IFJMP:<\{|IF:<\{|WHILE:<\{|REPEAT:<\{|UNTIL:<\{|ELSE<\{|}>DO<\{|}>)/, "keyword"],
        [/"(?:[^"\\]|\\.)*"/, "string"],
        [/(?:x|b|B)\{[0-9a-fA-F_]*\}/, "number.hex"],
        [/-?\d+/, "number"],
        [/\b(?:DECLPROC|DECLMETHOD|DECLGLOBVAR|CALLDICT|INLINECALLDICT|PUSHINT)\b/, "keyword"],
        [/[{}()[\]<>:]/, "delimiter"],
        [/[A-Za-z_][A-Za-z0-9_]*/, "identifier"],
      ] satisfies languages.IMonarchLanguageRule[],
    },
  },
} as const satisfies LanguageSupport<typeof FIFT_LANGUAGE_ID>
