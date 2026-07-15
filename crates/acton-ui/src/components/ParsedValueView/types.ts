export interface ParsedTransactionBody {
  readonly name: string
  readonly value: ParsedValue
}

export interface ParsedValueObjectEntry {
  readonly key: string
  readonly value: ParsedValue
}

export interface ParsedValueMapEntry {
  readonly key: ParsedValue
  readonly value: ParsedValue
}

/**
 * Minimal presentation model consumed by ParsedValueView.
 *
 * Domain parsers may return richer objects: the component deliberately depends
 * only on this recursive value shape and does not import ABI parser types.
 */
export type ParsedValue =
  | {readonly kind: "null"}
  | {readonly kind: "void"}
  | {readonly kind: "address"; readonly value: string}
  | {readonly kind: "boolean"; readonly value: boolean}
  | {
      readonly kind: "scalar"
      readonly value: string
      readonly rawValue?: string
      readonly typeName?: string
    }
  | {readonly kind: "array"; readonly items: readonly ParsedValue[]}
  | {
      readonly kind: "object"
      readonly typeName?: string
      readonly entries: readonly ParsedValueObjectEntry[]
    }
  | {
      readonly kind: "map"
      readonly typeName?: string
      readonly entries: readonly ParsedValueMapEntry[]
    }

export type ParsedValueLeaf = Exclude<ParsedValue, {readonly kind: "array" | "object" | "map"}>
