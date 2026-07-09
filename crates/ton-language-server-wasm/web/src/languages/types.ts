import type {languages} from "@codingame/monaco-vscode-editor-api"

export type LanguageSupport<TId extends string = string> = {
  readonly id: TId
  readonly label: string
  readonly fileExtension: string
  readonly defaultSource: string
  readonly extensionPoint: languages.ILanguageExtensionPoint
  readonly monarchLanguage: languages.IMonarchLanguage
}
