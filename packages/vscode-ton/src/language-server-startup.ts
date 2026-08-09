export type LanguageServerStartupIssue = "missing" | "unavailable" | "unsupported"

const UNSUPPORTED_ARGUMENT_PATTERNS = [
  /unexpected argument/i,
  /(?:unknown|unrecognized|invalid) (?:argument|option|flag|command|subcommand)/i,
  /found argument .* was(?:n['’]t| not) expected/i,
]
const UNAVAILABLE_EXECUTABLE_ERROR_CODES = new Set(["EACCES", "ENOEXEC", "EPERM"])

export function classifyLanguageServerProbeFailure(
  errorCode: string | undefined,
  exitCode: number | null,
  output: string,
): LanguageServerStartupIssue | undefined {
  let issue: LanguageServerStartupIssue | undefined
  if (errorCode === "ENOENT") {
    issue = "missing"
  } else if (errorCode && UNAVAILABLE_EXECUTABLE_ERROR_CODES.has(errorCode)) {
    issue = "unavailable"
  } else if (
    exitCode !== 0 &&
    UNSUPPORTED_ARGUMENT_PATTERNS.some(pattern => pattern.test(output))
  ) {
    issue = "unsupported"
  }

  return issue
}
