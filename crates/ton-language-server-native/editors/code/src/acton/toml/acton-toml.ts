//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

import {parse, type TomlTable} from "smol-toml"

export interface TomlAssignmentKey {
  readonly key: string
  readonly start: number
  readonly end: number
}

export function parseTomlContent(content: string): TomlTable | null {
  try {
    return parse(content)
  } catch {
    return null
  }
}

export function parseTopLevelTomlTableKeys(content: string, tableName: string): string[] {
  const toml = parseTomlContent(content)
  if (!toml) return []

  const table = asTomlTable(toml[tableName])
  if (!table) return []

  return Object.entries(table)
    .filter(([, value]) => asTomlTable(value) !== null)
    .map(([key]) => key)
}

export function parseStringTomlTable(content: string, tableName: string): Map<string, string> {
  const toml = parseTomlContent(content)
  if (!toml) return new Map()

  const table = asTomlTable(toml[tableName])
  if (!table) return new Map()

  const result: Map<string, string> = new Map()
  for (const [key, value] of Object.entries(table)) {
    if (typeof value === "string") {
      result.set(key, value)
    }
  }
  return result
}

export function parseTomlTableHeaderPath(line: string): string[] | null {
  const trimmed = line.trim()
  if (!trimmed.startsWith("[")) return null

  const toml = parseTomlContent(`${trimmed}\n`)
  if (!toml) return null

  return singleTablePath(toml)
}

export function parseTomlAssignmentKey(line: string): TomlAssignmentKey | null {
  const equalsIndex = findTomlEquals(line)
  if (equalsIndex === -1) return null

  const toml = parseTomlContent(`${line}\n`)
  if (!toml) return null

  const entries = Object.entries(toml)
  if (entries.length !== 1) return null

  const rawKey = line.slice(0, equalsIndex).trim()
  if (rawKey === "") return null

  const start = line.indexOf(rawKey)
  return {
    key: entries[0][0],
    start,
    end: start + rawKey.length,
  }
}

function findTomlEquals(line: string): number {
  let inSingleQuote = false
  let inDoubleQuote = false
  let escaped = false

  let index = 0
  for (const char of line) {
    if (escaped) {
      escaped = false
      index += char.length
      continue
    }

    if (inDoubleQuote && char === "\\") {
      escaped = true
      index += char.length
      continue
    }

    if (!inDoubleQuote && char === "'") {
      inSingleQuote = !inSingleQuote
      index += char.length
      continue
    }

    if (!inSingleQuote && char === '"') {
      inDoubleQuote = !inDoubleQuote
      index += char.length
      continue
    }

    if (!inSingleQuote && !inDoubleQuote && char === "#") {
      return -1
    }

    if (!inSingleQuote && !inDoubleQuote && char === "=") {
      return index
    }

    index += char.length
  }

  return -1
}

function singleTablePath(table: TomlTable): string[] | null {
  const entries = Object.entries(table)
  if (entries.length !== 1) return null

  const [key, value] = entries[0]
  const next = asTomlTable(value)
  if (!next) return null

  if (Object.keys(next).length === 0) {
    return [key]
  }

  const nested = singleTablePath(next)
  return nested ? [key, ...nested] : null
}

function asTomlTable(value: unknown): TomlTable | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null
  return value as TomlTable
}
