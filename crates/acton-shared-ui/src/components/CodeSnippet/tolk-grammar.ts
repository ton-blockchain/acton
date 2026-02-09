export const tolkGrammar = {
  $schema: "https://raw.githubusercontent.com/martinring/tmlanguage/master/tmlanguage.json",
  name: "tolk",
  scopeName: "source.tolk",
  foldingStartMarker: String.raw`\{\s*$`,
  foldingStopMarker: String.raw`^\s*\}`,
  fileTypes: ["tolk"],
  patterns: [
    {
      name: "comment.line.double-slash",
      match: "//(.*)",
    },
    {
      name: "comment.block",
      begin: String.raw`/\*`,
      end: String.raw`\*/`,
    },
    {
      name: "string.quoted.triple.tolk",
      begin: '"""',
      end: '"""',
      patterns: [
        {
          name: "constant.character.escape.tolk",
          match: String.raw`\\.`,
        },
      ],
    },
    {
      name: "string.quoted.double.tolk",
      begin: '"',
      end: '"',
      patterns: [
        {
          name: "constant.character.escape.tolk",
          match: String.raw`\\([nrt0\\'"u]|u[0-9a-fA-F]{4})`,
        },
      ],
    },
    {
      name: "constant.numeric",
      match: String.raw`\b(-?([\d]+|0x[\da-fA-F]+|0b[01]+))\b`,
    },
    {
      name: "keyword.control",
      match: String.raw`\b(do|if|try|else|while|break|throw|catch|return|assert|repeat|continue|asm|builtin|match|lazy)\b`,
    },
    {
      name: "keyword.operator",
      match: String.raw`\+|-|\*|/|%|\?|:|,|;|\(|\)|\[|\]|{|}|=|<|>|!|&|\||\^|==|!=|<=|>=|<<|>>|&&|\|\||~/|\^/|\+=|-=|\*=|/=|%=|&=|\|=|\^=|->|<=>|~>>|\^>>|<<=|>>=|=>|\?\?`,
    },
    {
      name: "keyword.other",
      match: String.raw`\b(import|export|true|false|null|redef|mutate|tolk|as|is|!is|private|readonly|string|contract)\b`,
    },
    {
      name: "keyword.other",
      match: String.raw`\bself\b`,
    },
    {
      name: "entity.name.type.parameter",
      match: String.raw`\b[TU]\b`,
    },
    {
      match: String.raw`\b(val|var)\s+(?:type|enum|int|map|cell|Cell|void|dict|bool|any_address|slice|string|tuple|builder|continuation|never|coins|address|int\d+|uint\d+|bits\d+|bytes\d+)\b`,
      captures: {
        "1": {name: "storage.modifier"},
      },
    },
    {
      name: "constant.other",
      match: String.raw`\b[A-Z][A-Z0-9_]{2,}\b`,
    },
    {
      name: "storage.type",
      match: String.raw`\b(type|enum|int|map|cell|Cell|void|dict|bool|any_address|slice|string|tuple|builder|continuation|never|coins|int\d+|uint\d+|bits\d+|bytes\d+)\b`,
    },
    {
      name: "storage.type",
      match: String.raw`(?<!\.)\baddress\b(?!\s*:)`,
    },
    {
      name: "storage.modifier",
      match: String.raw`\b(global|const|var|val|fun|get|struct|contract)\b`,
    },
    {
      name: "entity.name.type",
      match: String.raw`@\w+`,
    },
    {
      name: "entity.name.function",
      match: "(`[^`]+`|[a-zA-Z$_][a-zA-Z0-9$_]*)(?=\\s*(?:<[^>]+>)?\\s*\\()",
    },
    {
      name: "entity.name.type",
      match: String.raw`\b[A-Z][a-zA-Z0-9]*\b`,
    },
    {
      name: "variable.name",
      match: "`[^`]+`|[a-zA-Z$_][a-zA-Z0-9$_]*",
    },
  ],
  repository: {},
}
