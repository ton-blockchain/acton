import {languages} from "monaco-editor"

export const tasmLanguageDefinition: languages.IMonarchLanguage = {
  keywords: ["ref", "embed", "exotic", "library"],
  operators: ["=>"],
  tokenizer: {
    root: [
      [/\b(ref|embed|exotic|library)\b/, "keyword"],
      [/\b[A-Z_][A-Z0-9_]*[0-9]?[0-9]?\b/, "instruction"],
      [/s-?[0-9][0-9]?/, "number.hex"],
      [/c[0-9][0-9]?/, "number.hex"],
      [/x\{[0-9a-fA-F_]*}/, "number.hex"],
      [/b\{[01]*}/, "number.binary"],
      [/boc\{[0-9a-fA-F]*}/, "number.hex"],
      [/"([^"\\]|\\.)*"/, "string"],
      [/-?\d+/, "number"],
      [/\{/, {token: "delimiter.curly", next: "@push"}],
      [/}/, {token: "delimiter.curly", next: "@pop"}],
      [/\[/, {token: "delimiter.square", next: "@push"}],
      [/]/, {token: "delimiter.square", next: "@pop"}],
      [/\(\)/, "delimiter.parenthesis"],
      [/\/\/.*?$/, "comment"],
      [/\b(alias|of)\b/, "alias"],
      [/\s+/, "white"],
    ],
  },
}
