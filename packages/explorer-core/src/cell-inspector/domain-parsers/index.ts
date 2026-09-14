import type {Cell} from "@ton/core"

import {dnsDomainParser} from "./dns"
import {jettonMetadataDomainParser} from "./jetton-metadata"
import type {DomainParser, DomainParserMatch} from "./types"

const DOMAIN_PARSERS: readonly DomainParser[] = [dnsDomainParser, jettonMetadataDomainParser]

/**
 * Tries protocol-specific parsers in registry order
 *
 * Each parser must validate its complete layout before returning a match because
 * this registry runs without contract context
 */
export function parseDomainCell(cell: Cell): DomainParserMatch | undefined {
  for (const parser of DOMAIN_PARSERS) {
    const match = parser.parse(cell)
    if (match) return match
  }

  return undefined
}

export type {DomainParser, DomainParserMatch} from "./types"
