//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

import {parseTopLevelTomlTableKeys} from "../toml/acton-toml"

export function parseActonContractIds(content: string): string[] {
  return parseTopLevelTomlTableKeys(content, "contracts")
}

export function normalizeRetraceHash(value: string): string | null {
  const trimmed = value.trim()
  const normalized = trimmed.replace(/^0x/iu, "")

  return /^[a-f0-9]{64}$/iu.test(normalized) ? normalized : null
}
