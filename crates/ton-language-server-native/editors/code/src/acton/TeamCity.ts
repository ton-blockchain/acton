//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

import {fileURLToPath} from "node:url"

export interface TeamCityTestingStartedMessage {
  readonly name: "testingStarted"
  readonly attributes: Readonly<Record<string, never>>
}

export interface TeamCityTestingFinishedMessage {
  readonly name: "testingFinished"
  readonly attributes: Readonly<Record<string, never>>
}

export interface TeamCityTestSuiteStartedAttributes {
  readonly name?: string
  readonly nodeId?: string
  readonly parentNodeId?: string
  readonly nodeType?: string
  readonly locationHint?: string
}

export interface TeamCityTestSuiteStartedMessage {
  readonly name: "testSuiteStarted"
  readonly attributes: Readonly<TeamCityTestSuiteStartedAttributes>
}

export interface TeamCityTestSuiteFinishedAttributes {
  readonly name?: string
  readonly nodeId?: string
  readonly parentNodeId?: string
}

export interface TeamCityTestSuiteFinishedMessage {
  readonly name: "testSuiteFinished"
  readonly attributes: Readonly<TeamCityTestSuiteFinishedAttributes>
}

export interface TeamCityTestStartedAttributes {
  readonly name?: string
  readonly nodeId?: string
  readonly parentNodeId?: string
  readonly locationHint?: string
}

export interface TeamCityTestStartedMessage {
  readonly name: "testStarted"
  readonly attributes: Readonly<TeamCityTestStartedAttributes>
}

export interface TeamCityTestFinishedAttributes {
  readonly name?: string
  readonly nodeId?: string
  readonly parentNodeId?: string
  readonly duration?: string
}

export interface TeamCityTestFinishedMessage {
  readonly name: "testFinished"
  readonly attributes: Readonly<TeamCityTestFinishedAttributes>
}

export interface TeamCityTestFailedAttributes {
  readonly name?: string
  readonly nodeId?: string
  readonly parentNodeId?: string
  readonly duration?: string
  readonly message?: string
  readonly details?: string
  readonly expected?: string
  readonly actual?: string
}

export interface TeamCityTestFailedMessage {
  readonly name: "testFailed"
  readonly attributes: Readonly<TeamCityTestFailedAttributes>
}

export interface TeamCityTestIgnoredAttributes {
  readonly name?: string
  readonly nodeId?: string
  readonly parentNodeId?: string
  readonly duration?: string
  readonly message?: string
  readonly details?: string
}

export interface TeamCityTestIgnoredMessage {
  readonly name: "testIgnored"
  readonly attributes: Readonly<TeamCityTestIgnoredAttributes>
}

export type TeamCityServiceMessage =
  | TeamCityTestingStartedMessage
  | TeamCityTestingFinishedMessage
  | TeamCityTestSuiteStartedMessage
  | TeamCityTestSuiteFinishedMessage
  | TeamCityTestStartedMessage
  | TeamCityTestFinishedMessage
  | TeamCityTestFailedMessage
  | TeamCityTestIgnoredMessage

export type TeamCityTestStatusMessage =
  | TeamCityTestStartedMessage
  | TeamCityTestFinishedMessage
  | TeamCityTestFailedMessage
  | TeamCityTestIgnoredMessage

export function isTeamCityMessageLine(line: string): boolean {
  return line.startsWith("##teamcity[")
}

export function parseTeamCityMessage(line: string): TeamCityServiceMessage | undefined {
  if (!line.startsWith("##teamcity[") || !line.endsWith("]")) {
    return undefined
  }

  const content = line.slice("##teamcity[".length, -1)
  const nameMatch = /^(\w+)/.exec(content)
  if (!nameMatch) {
    return undefined
  }

  const name = nameMatch[1]
  const attributesRaw = content.slice(name.length).trim()
  const attributes = attributesRaw === "" ? {} : parseTeamCityAttributes(attributesRaw)

  return toTypedTeamCityMessage(name, attributes)
}

export function stripTeamCityMessages(text: string): string {
  return text
    .split(/\r?\n/)
    .filter(line => !isTeamCityMessageLine(line.trimStart()))
    .join("\n")
}

export function extractTeamCityFileHint(
  locationHint: string | undefined,
  suiteName: string | undefined,
): string | undefined {
  if (locationHint) {
    try {
      const url = new URL(locationHint)
      if (url.protocol === "file:") {
        return fileURLToPath(url)
      }
    } catch {
      // ignore parse errors and fallback to suite name
    }
  }

  return suiteName
}

function toTypedTeamCityMessage(
  name: string,
  attributes: Record<string, string>,
): TeamCityServiceMessage | undefined {
  switch (name) {
    case "testingStarted": {
      return {name, attributes: {}}
    }
    case "testingFinished": {
      return {name, attributes: {}}
    }
    case "testSuiteStarted": {
      return {name, attributes}
    }
    case "testSuiteFinished": {
      return {name, attributes}
    }
    case "testStarted": {
      return {name, attributes}
    }
    case "testFinished": {
      return {name, attributes}
    }
    case "testFailed": {
      return {name, attributes}
    }
    case "testIgnored": {
      return {name, attributes}
    }
    default: {
      return undefined
    }
  }
}

function parseTeamCityAttributes(raw: string): Record<string, string> {
  const attributes: Record<string, string> = {}
  let i = 0

  while (i < raw.length) {
    while (i < raw.length && raw[i] === " ") {
      i++
    }
    if (i >= raw.length) {
      break
    }

    const keyStart = i
    while (i < raw.length && /\w/.test(raw[i])) {
      i++
    }
    const key = raw.slice(keyStart, i)

    if (!key || raw[i] !== "=" || raw[i + 1] !== "'") {
      break
    }

    i += 2
    let value = ""

    while (i < raw.length) {
      const ch = raw[i]
      if (ch === "|") {
        if (i + 1 < raw.length) {
          value += raw.slice(i, i + 2)
          i += 2
          continue
        }
        value += ch
        i++
        continue
      }
      if (ch === "'") {
        i++
        break
      }

      value += ch
      i++
    }

    attributes[key] = decodeTeamCityValue(value)
  }

  return attributes
}

function decodeTeamCityValue(value: string): string {
  let decoded = ""

  for (let i = 0; i < value.length; i++) {
    const ch = value[i]
    if (ch !== "|") {
      decoded += ch
      continue
    }

    if (i + 1 >= value.length) {
      decoded += ch
      continue
    }

    const escaped = value[i + 1]
    i++

    switch (escaped) {
      case "n": {
        decoded += "\n"
        break
      }
      case "r": {
        decoded += "\r"
        break
      }
      case "'": {
        decoded += "'"
        break
      }
      case "[": {
        decoded += "["
        break
      }
      case "]": {
        decoded += "]"
        break
      }
      case "|": {
        decoded += "|"
        break
      }
      default: {
        decoded += escaped
        break
      }
    }
  }

  return decoded
}
