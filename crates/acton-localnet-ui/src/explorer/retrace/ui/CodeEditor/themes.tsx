import {editor} from "monaco-editor"

export const LIGHT_THEME: editor.IStandaloneThemeData = {
  base: "vs",
  inherit: true,
  rules: [
    // FunC
    {token: "keyword", foreground: "#0033B3", fontStyle: "bold"},
    {token: "string", foreground: "#067D17"},
    {token: "string.number", foreground: "#1750EB"},
    {token: "string.slice", foreground: "#1750EB"},
    {token: "string.asm", foreground: "#067D17", fontStyle: "italic"},
    {token: "number", foreground: "#1750EB"},
    {token: "number.hex", foreground: "#1750EB"},
    {token: "number.version", foreground: "#1750EB"},
    {token: "comment", foreground: "#8C8C8C", fontStyle: "italic"},
    {token: "annotation", foreground: "#9E880D"},
    {token: "keyword.directive", foreground: "#9C5D27"},
    {token: "identifier.function", foreground: "#795E26"},
    {token: "identifier.field", foreground: "#001080"},
    {token: "identifier.constant", foreground: "#871094", fontStyle: "bold"},
    {token: "identifier.special", foreground: "#795E26", fontStyle: "bold"},
    {token: "identifier.backtick", foreground: "#000000"},
    {token: "keyword.underscore", foreground: "#0033B3"},
    {token: "operator", foreground: "#000000"},

    // TASM
    {token: "instruction", foreground: "#0033B3"},
  ],
  colors: {
    // Inlay hints (implicit RET)
    "editorInlayHint.typeForeground": "#7e6820",
    "editorInlayHint.typeBackground": "#fff3cd",
  },
}

export const DARK_THEME: editor.IStandaloneThemeData = {
  base: "vs-dark",
  inherit: true,
  rules: [
    // FunC
    {token: "string", foreground: "#CE9178"},
    {token: "string.number", foreground: "#B5CEA8"},
    {token: "string.slice", foreground: "#B5CEA8"},
    {token: "string.asm", foreground: "#CE9178", fontStyle: "italic"},
    {token: "number", foreground: "#B5CEA8"},
    {token: "number.hex", foreground: "#B5CEA8"},
    {token: "number.version", foreground: "#B5CEA8"},
    {token: "comment", foreground: "#6A9955", fontStyle: "italic"},
    {token: "annotation", foreground: "#cac46b"},
    {token: "keyword.directive", foreground: "#C586C0"},
    {token: "identifier.function", foreground: "#DCDCAA"},
    {token: "identifier.field", foreground: "#9CDCFE"},
    {token: "identifier.constant", foreground: "#4FC1FF", fontStyle: "bold"},
    {token: "identifier.special", foreground: "#DCDCAA", fontStyle: "bold"},
    {token: "identifier.backtick", foreground: "#D4D4D4"},
    {token: "keyword.underscore", foreground: "#569CD6"},
    {token: "operator", foreground: "#D4D4D4"},

    // TASM
    {token: "instruction", foreground: "#749DED"},
    {token: "alias", foreground: "#9a9a9a"},
  ],
  colors: {
    "editor.background": "#1c1c1e",
    // Inlay hints (implicit RET)
    "editorInlayHint.typeForeground": "#FF6B6B",
    "editorInlayHint.typeBackground": "#3A1F1F",
  },
}
