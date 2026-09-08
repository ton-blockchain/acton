import type {ParsedValue} from "@acton/ui"
import type {Cell} from "@ton/core"

/**
 * A semantic match produced by one protocol-specific Cell Inspector parser
 *
 * Domain parsers own both strict binary recognition and the presentation model
 * so protocol fields can use product language instead of generated TL-B names
 */
export interface DomainParserMatch {
  readonly data: unknown
  readonly parsedValue: ParsedValue
  readonly label: string
  readonly details?: Readonly<Record<string, string>>
  readonly confidence: {
    readonly score: number
    readonly reasons: readonly string[]
  }
}

/** A parser for one related group of protocol-specific cell layouts */
export interface DomainParser {
  readonly id: string
  readonly parse: (cell: Cell) => DomainParserMatch | undefined
}
