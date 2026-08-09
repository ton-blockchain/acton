import type {ParsedValueLeaf} from "../ParsedValueView/types"

export type ParsedValueDiffStatus = "unchanged" | "changed" | "added" | "removed"

export type ParsedValueDiffContainerKind = "object" | "array" | "map"

export interface ParsedValueDiffEntry {
  readonly key: string
  readonly value: ParsedValueDiff
}

/**
 * Minimal recursive diff model consumed by ParsedValueDiffView.
 *
 * Domain code owns comparison and normalization. The component receives only
 * presentation-ready leaf values, container labels, and change statuses.
 */
export type ParsedValueDiff =
  | {
      readonly kind: "leaf"
      readonly status: ParsedValueDiffStatus
      readonly before: ParsedValueLeaf | undefined
      readonly after: ParsedValueLeaf | undefined
    }
  | {
      readonly kind: "object"
      readonly status: ParsedValueDiffStatus
      readonly objectKind: ParsedValueDiffContainerKind
      readonly typeName?: string
      readonly entries: readonly ParsedValueDiffEntry[]
    }
