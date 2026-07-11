//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

const RUST_REGEX_META_CHARACTERS = /[$()*+.?[\\\]^{|}]/g

export function createActonTestFilterPattern(testNames: readonly string[]): string {
  const uniqueNames = [...new Set(testNames.filter(name => name !== ""))]
  if (uniqueNames.length === 0) {
    return ""
  }

  const escapedNames = uniqueNames.map(name => escapeRustRegexLiteral(name))
  if (escapedNames.length === 1) {
    return `^${escapedNames[0]}$`
  }

  return `^(?:${escapedNames.join("|")})$`
}

export function escapeRustRegexLiteral(value: string): string {
  return value.replace(RUST_REGEX_META_CHARACTERS, "\\$&")
}
