import type {ThemeRegistration} from "shiki/types"

export const jetbrainsDarculaTheme: ThemeRegistration = {
  name: "jetbrains-darcula",
  displayName: "JetBrains Darcula",
  type: "dark",
  semanticHighlighting: true,
  colors: {
    "editor.background": "#1E1F22",
    "editor.foreground": "#BCBEC4",
    "editor.lineHighlightBackground": "#26282E",
    "editor.selectionBackground": "#214283",
  },
  tokenColors: [
    {
      scope: ["comment", "punctuation.definition.comment", "string.comment"],
      settings: {foreground: "#7A7E85"},
    },
    {
      scope: [
        "keyword",
        "keyword.control",
        "keyword.operator.new",
        "storage",
        "storage.type",
        "storage.modifier",
        "constant.language.boolean",
        "constant.language.null",
        "constant.language.undefined",
      ],
      settings: {foreground: "#CF8E6D"},
    },
    {
      scope: [
        "keyword.operator",
        "delimiter",
        "punctuation",
        "punctuation.separator",
        "meta.brace",
      ],
      settings: {foreground: "#BCBEC4"},
    },
    {
      scope: ["string", "string.template", "punctuation.definition.string", "attribute.value"],
      settings: {foreground: "#6AAB73"},
    },
    {
      scope: ["constant.numeric", "number", "constant.character", "constant.other"],
      settings: {foreground: "#2AACB8"},
    },
    {
      scope: ["entity.name.function", "support.function", "variable.function"],
      settings: {foreground: "#56A8F5"},
    },
    {
      scope: [
        "entity.name.type",
        "entity.name.class",
        "support.type",
        "support.class",
        "type.identifier",
      ],
      settings: {foreground: "#A2BA6DFF"},
    },
    {
      scope: ["property", "meta.property-name", "meta.object-literal.key"],
      settings: {foreground: "#C77DBB"},
    },
    {
      scope: ["variable.parameter", "variable", "identifier", "delimiter", "punctuation"],
      settings: {foreground: "#BCBEC4"},
    },
    {
      scope: ["entity.name.tag", "entity.name.namespace"],
      settings: {foreground: "#D5B778"},
    },
    {
      scope: ["entity.name.function.method", "meta.function-call", "support.function.method"],
      settings: {foreground: "#57AAF7"},
    },
    {
      scope: ["constant.language", "constant.other.symbol", "entity.name.constant"],
      settings: {foreground: "#C77DBB", fontStyle: "bold"},
    },
    {
      scope: ["constant.character.escape", "string.regexp", "constant.other.escape"],
      settings: {foreground: "#CF8E6D"},
    },
    {
      scope: ["invalid", "invalid.illegal", "invalid.broken"],
      settings: {foreground: "#FA6675", fontStyle: "underline"},
    },
  ],
}

export const jetbrainsLightTheme: ThemeRegistration = {
  name: "jetbrains-light",
  displayName: "JetBrains Light",
  type: "light",
  semanticHighlighting: true,
  colors: {
    "editor.background": "#FFFFFF",
    "editor.foreground": "#000000",
    "editor.lineHighlightBackground": "#F2F2F2",
    "editor.selectionBackground": "#D4DFFF",
  },
  tokenColors: [
    {
      scope: ["comment", "punctuation.definition.comment", "string.comment"],
      settings: {foreground: "#8C8C8C"},
    },
    {
      scope: [
        "keyword",
        "keyword.control",
        "keyword.operator.new",
        "storage",
        "storage.type",
        "storage.modifier",
        "constant.language.boolean",
        "constant.language.null",
        "constant.language.undefined",
      ],
      settings: {foreground: "#0033B3"},
    },
    {
      scope: ["string", "string.template", "punctuation.definition.string", "attribute.value"],
      settings: {foreground: "#067D17"},
    },
    {
      scope: ["constant.numeric", "number", "constant.character", "constant.other"],
      settings: {foreground: "#1750EB"},
    },
    {
      scope: ["entity.name.function", "support.function", "variable.function"],
      settings: {foreground: "#00627A"},
    },
    {
      scope: [
        "entity.name.type",
        "entity.name.class",
        "support.type",
        "support.class",
        "type.identifier",
      ],
      settings: {foreground: "#000000"},
    },
    {
      scope: ["property", "meta.property-name", "meta.object-literal.key"],
      settings: {foreground: "#871094"},
    },
    {
      scope: ["variable.parameter", "variable", "identifier", "delimiter", "punctuation"],
      settings: {foreground: "#000000"},
    },
    {
      scope: ["entity.name.tag", "entity.name.namespace"],
      settings: {foreground: "#8A653B"},
    },
    {
      scope: ["invalid", "invalid.illegal", "invalid.broken"],
      settings: {foreground: "#CF3F3F", fontStyle: "underline"},
    },
  ],
}
