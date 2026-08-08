// biome-ignore lint/suspicious/noControlCharactersInRegex: ANSI escapes start with ESC.
const ANSI_ESCAPE_PATTERN = /\u001B\[[0-?]*[ -/]*[@-~]/g

export function stripAnsiCodes(text: string): string {
  return text.replace(ANSI_ESCAPE_PATTERN, "")
}
