import {CellType, type Cell} from "@ton/core"

import type {ParserWarning} from "./model"

export interface RawCellBits {
  readonly length: number
  readonly value: string
  readonly truncated: boolean
}

export interface RawCellLevelMask {
  readonly value: number
  readonly level: number
  readonly hashIndex: number
  readonly hashCount: number
}

export interface RawCellNode {
  readonly kind: "cell" | "cell-ref" | "truncated-cell"
  readonly path: string
  readonly hash: string
  readonly targetPath?: string
  readonly type: string
  readonly exotic: boolean
  readonly bits?: RawCellBits
  readonly refs: {
    readonly count: number
    readonly items: readonly RawCellNode[]
  }
  readonly hashes: Readonly<Record<string, string>>
  readonly depths: Readonly<Record<string, number | null>>
  readonly level: number | null
  readonly levelMask: RawCellLevelMask
  readonly bocHex?: string
  readonly reason?: string
}

export interface RawCellForest {
  readonly kind: "cell-forest"
  readonly rootCount: number
  readonly roots: readonly RawCellNode[]
  readonly uniqueNodes: number
  readonly duplicateRefs: number
  readonly truncatedNodes: number
  readonly warnings: readonly ParserWarning[]
}

export interface RawCellTreeLimits {
  readonly maxDepth: number
  readonly maxNodes: number
  readonly maxBitsPreviewChars: number
  readonly includeBoc: boolean
  readonly maxBocBytes: number
}

export const DEFAULT_RAW_CELL_TREE_LIMITS: RawCellTreeLimits = {
  maxDepth: 32,
  maxNodes: 2000,
  maxBitsPreviewChars: 512,
  includeBoc: false,
  maxBocBytes: 256 * 1024,
}

interface WalkState {
  readonly pathsByHash: Map<string, string>
  readonly pathsByCell: WeakMap<Cell, string>
  readonly warnings: ParserWarning[]
  uniqueNodes: number
  duplicateRefs: number
  truncatedNodes: number
}

const HARD_LIMITS = {
  maxDepth: 128,
  maxNodes: 20_000,
  maxBitsPreviewChars: 4096,
  maxBocBytes: 4 * 1024 * 1024,
} as const

export function describeCellForest(
  roots: readonly Cell[],
  limits: Partial<RawCellTreeLimits> = {},
): RawCellForest {
  const effectiveLimits = normalizeLimits(limits)
  const state: WalkState = {
    pathsByHash: new Map(),
    pathsByCell: new WeakMap(),
    warnings: [],
    uniqueNodes: 0,
    duplicateRefs: 0,
    truncatedNodes: 0,
  }
  const describedRoots = roots.map((root, index) =>
    walkCell(root, `$[${index}]`, 0, effectiveLimits, state),
  )

  return {
    kind: "cell-forest",
    rootCount: roots.length,
    roots: describedRoots,
    uniqueNodes: state.uniqueNodes,
    duplicateRefs: state.duplicateRefs,
    truncatedNodes: state.truncatedNodes,
    warnings: state.warnings,
  }
}

export function describeCellTree(cell: Cell, limits: Partial<RawCellTreeLimits> = {}): RawCellNode {
  const forest = describeCellForest([cell], limits)
  const root = forest.roots[0]
  if (root === undefined) {
    throw new Error("Failed to describe the cell root")
  }
  return root
}

export function cellTypeName(type: CellType): string {
  switch (type) {
    case CellType.Ordinary:
      return "ordinary"
    case CellType.PrunedBranch:
      return "pruned-branch"
    case CellType.Library:
      return "library-reference"
    case CellType.MerkleProof:
      return "merkle-proof"
    case CellType.MerkleUpdate:
      return "merkle-update"
    default:
      return `unknown-${String(type)}`
  }
}

function walkCell(
  cell: Cell,
  path: string,
  depth: number,
  limits: RawCellTreeLimits,
  state: WalkState,
): RawCellNode {
  const hash = safeHash(cell)
  const targetPath =
    state.pathsByCell.get(cell) ??
    (hash === "unavailable" ? undefined : state.pathsByHash.get(hash))
  if (targetPath !== undefined) {
    state.duplicateRefs += 1
    return shallowNode(cell, path, hash, "cell-ref", "duplicate cell", targetPath)
  }

  if (depth > limits.maxDepth) {
    state.truncatedNodes += 1
    pushWarning(state.warnings, {
      code: "tree-depth-limit",
      message: `Raw cells were truncated at depth ${limits.maxDepth}`,
      path,
    })
    return shallowNode(
      cell,
      path,
      hash,
      "truncated-cell",
      `maximum depth ${limits.maxDepth} reached`,
    )
  }

  if (state.uniqueNodes >= limits.maxNodes) {
    state.truncatedNodes += 1
    pushWarning(state.warnings, {
      code: "tree-node-limit",
      message: `Only the first ${limits.maxNodes.toLocaleString()} unique cells are shown`,
      path,
    })
    return shallowNode(
      cell,
      path,
      hash,
      "truncated-cell",
      `maximum node count ${limits.maxNodes} reached`,
    )
  }

  state.pathsByHash.set(hash, path)
  state.pathsByCell.set(cell, path)
  state.uniqueNodes += 1

  const refs = cell.refs.map((ref, index) =>
    walkCell(ref, `${path}.refs[${index}]`, depth + 1, limits, state),
  )
  const serialized = limits.includeBoc ? serializeCell(cell, limits, state, path) : undefined

  return {
    ...baseNode(cell, path, hash),
    kind: "cell",
    bits: previewBits(cell, limits.maxBitsPreviewChars),
    refs: {count: cell.refs.length, items: refs},
    ...(serialized === undefined ? {} : {bocHex: serialized}),
  }
}

function shallowNode(
  cell: Cell,
  path: string,
  hash: string,
  kind: "cell-ref" | "truncated-cell",
  reason: string,
  targetPath?: string,
): RawCellNode {
  return {
    ...baseNode(cell, path, hash),
    kind,
    refs: {count: cell.refs.length, items: []},
    reason,
    ...(targetPath === undefined ? {} : {targetPath}),
  }
}

function baseNode(cell: Cell, path: string, hash: string) {
  return {
    path,
    hash,
    type: cellTypeName(cell.type),
    exotic: cell.isExotic,
    hashes: collectHashes(cell),
    depths: collectDepths(cell),
    level: safeNumber(() => cell.level()),
    levelMask: {
      value: cell.mask.value,
      level: cell.mask.level,
      hashIndex: cell.mask.hashIndex,
      hashCount: cell.mask.hashCount,
    },
  }
}

function previewBits(cell: Cell, maxChars: number): RawCellBits {
  const value = cell.bits.toString()
  const truncated = value.length > maxChars
  return {
    length: cell.bits.length,
    value: truncated ? `${value.slice(0, maxChars)}…` : value,
    truncated,
  }
}

function collectHashes(cell: Cell): Record<string, string> {
  const hashes: Record<string, string> = {}
  for (let level = 0; level <= 3; level += 1) {
    hashes[`level${level}`] = safeHash(cell, level)
  }
  return hashes
}

function collectDepths(cell: Cell): Record<string, number | null> {
  const depths: Record<string, number | null> = {}
  for (let level = 0; level <= 3; level += 1) {
    depths[`level${level}`] = safeNumber(() => cell.depth(level))
  }
  return depths
}

function safeHash(cell: Cell, level?: number): string {
  try {
    return Buffer.from(cell.hash(level)).toString("hex")
  } catch {
    return "unavailable"
  }
}

function safeNumber(read: () => number): number | null {
  try {
    return read()
  } catch {
    return null
  }
}

function serializeCell(
  cell: Cell,
  limits: RawCellTreeLimits,
  state: WalkState,
  path: string,
): string | undefined {
  try {
    const boc = cell.toBoc({idx: false, crc32: false})
    if (boc.length <= limits.maxBocBytes) {
      return boc.toString("hex")
    }

    pushWarning(state.warnings, {
      code: "tree-boc-limit",
      message: `This cell is ${boc.length.toLocaleString()} bytes; inline BoC output is limited to ${limits.maxBocBytes.toLocaleString()} bytes`,
      path,
    })
  } catch {
    // Hashes and structural data remain useful when a special cell cannot be serialized alone.
  }
  return undefined
}

function normalizeLimits(limits: Partial<RawCellTreeLimits>): RawCellTreeLimits {
  return {
    maxDepth: boundedInteger(
      limits.maxDepth,
      DEFAULT_RAW_CELL_TREE_LIMITS.maxDepth,
      0,
      HARD_LIMITS.maxDepth,
    ),
    maxNodes: boundedInteger(
      limits.maxNodes,
      DEFAULT_RAW_CELL_TREE_LIMITS.maxNodes,
      1,
      HARD_LIMITS.maxNodes,
    ),
    maxBitsPreviewChars: boundedInteger(
      limits.maxBitsPreviewChars,
      DEFAULT_RAW_CELL_TREE_LIMITS.maxBitsPreviewChars,
      0,
      HARD_LIMITS.maxBitsPreviewChars,
    ),
    includeBoc: limits.includeBoc ?? DEFAULT_RAW_CELL_TREE_LIMITS.includeBoc,
    maxBocBytes: boundedInteger(
      limits.maxBocBytes,
      DEFAULT_RAW_CELL_TREE_LIMITS.maxBocBytes,
      0,
      HARD_LIMITS.maxBocBytes,
    ),
  }
}

function boundedInteger(
  value: number | undefined,
  fallback: number,
  minimum: number,
  maximum: number,
): number {
  if (value === undefined || !Number.isFinite(value)) {
    return fallback
  }
  return Math.min(maximum, Math.max(minimum, Math.trunc(value)))
}

function pushWarning(warnings: ParserWarning[], warning: ParserWarning): void {
  if (!warnings.some(current => current.code === warning.code)) {
    warnings.push(warning)
  }
}
