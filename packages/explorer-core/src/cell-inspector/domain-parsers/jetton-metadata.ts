import type {ParsedValue, ParsedValueObjectEntry} from "@acton/ui"
import {type Cell, Dictionary, type Slice} from "@ton/core"
import {sha256_sync} from "@ton/crypto"

import type {DomainParser, DomainParserMatch} from "./types"

interface JettonMetadataField {
  readonly key: string
  readonly label: string
}

const JETTON_METADATA_FIELDS: readonly JettonMetadataField[] = [
  {key: "name", label: "Name"},
  {key: "symbol", label: "Symbol"},
  {key: "decimals", label: "Decimals"},
  {key: "description", label: "Description"},
  {key: "image", label: "Image"},
  {key: "image_data", label: "Image data"},
  {key: "uri", label: "URI"},
  {key: "content_url", label: "Content URL"},
] as const

const FIELD_BY_HASH = new Map(
  JETTON_METADATA_FIELDS.map(field => [sha256_sync(field.key).toString("hex"), field]),
)
const URI_PATTERN = /^(?:https?|ipfs|tonstorage|ar):\/\//i
const MAX_SNAKE_CELLS = 64

const scalar = (value: string, typeName?: string): ParsedValue => ({
  kind: "scalar",
  value,
  ...(typeName ? {typeName} : {}),
})

const entry = (key: string, value: ParsedValue): ParsedValueObjectEntry => ({key, value})

function readSnakeText(initialSlice: Slice): string | undefined {
  const chunks: Buffer[] = []
  let slice: Slice | undefined = initialSlice
  let cellCount = 0

  while (slice) {
    cellCount += 1
    if (
      cellCount > MAX_SNAKE_CELLS ||
      slice.remainingBits % 8 !== 0 ||
      slice.remainingRefs > 1
    ) {
      return undefined
    }

    chunks.push(slice.loadBuffer(slice.remainingBits / 8))
    slice = slice.remainingRefs === 1 ? slice.loadRef().beginParse() : undefined
  }

  try {
    return new TextDecoder("utf-8", {fatal: true}).decode(Buffer.concat(chunks))
  } catch {
    return undefined
  }
}

function readPrefixedSnakeText(cell: Cell): string | undefined {
  const slice = cell.beginParse()
  if (slice.remainingBits < 8 || slice.loadUint(8) !== 0) {
    return undefined
  }

  return readSnakeText(slice)
}

function parseOnChainJettonMetadata(cell: Cell): DomainParserMatch | undefined {
  const slice = cell.beginParse()
  if (slice.remainingBits < 8 || slice.loadUint(8) !== 0) {
    return undefined
  }

  let dictionary: Dictionary<bigint, Cell>
  try {
    dictionary = Dictionary.load(
      Dictionary.Keys.BigUint(256),
      Dictionary.Values.Cell(),
      slice,
    )
  } catch {
    return undefined
  }

  if (dictionary.size === 0 || slice.remainingBits !== 0 || slice.remainingRefs !== 0) {
    return undefined
  }

  const fields = new Map<string, string>()
  let knownFieldCount = 0
  for (const [key, valueCell] of dictionary) {
    const value = readPrefixedSnakeText(valueCell)
    if (value === undefined) return undefined

    const hash = key.toString(16).padStart(64, "0")
    const field = FIELD_BY_HASH.get(hash)
    if (field) knownFieldCount += 1
    fields.set(field?.key ?? hash, value)
  }

  // A HashmapE of strings is too generic by itself. At least one standard key
  // must match before the value can be called Jetton metadata automatically.
  if (knownFieldCount === 0) {
    return undefined
  }

  const entries: ParsedValueObjectEntry[] = [
    entry("Storage", scalar("On-chain")),
    ...JETTON_METADATA_FIELDS.flatMap(field => {
      const value = fields.get(field.key)
      return value === undefined ? [] : [entry(field.label, scalar(value, "string"))]
    }),
    ...[...fields.entries()]
      .filter(([key]) => !JETTON_METADATA_FIELDS.some(field => field.key === key))
      .map(([hash, value]) => entry(`Field ${hash}`, scalar(value, "string"))),
  ]

  return {
    data: {storage: "on-chain", fields: Object.fromEntries(fields)},
    parsedValue: {
      kind: "object",
      typeName: "Jetton metadata",
      entries,
    },
    label: "Jetton metadata · On-chain",
    details: {
      fields: String(dictionary.size),
      storage: "On-chain",
    },
    confidence: {
      score: 0.99,
      reasons: [
        "The TEP-64 on-chain prefix matched",
        "The metadata dictionary consumed the complete cell",
        `${knownFieldCount} standard metadata field${knownFieldCount === 1 ? "" : "s"} matched`,
      ],
    },
  }
}

function parseOffChainJettonMetadata(cell: Cell): DomainParserMatch | undefined {
  const slice = cell.beginParse()
  if (slice.remainingBits < 8 || slice.loadUint(8) !== 1) {
    return undefined
  }

  const uri = readSnakeText(slice)
  if (!uri || !URI_PATTERN.test(uri)) {
    return undefined
  }

  return metadataUriMatch(uri, "TEP-64 off-chain content", true)
}

function parseMetadataUri(cell: Cell): DomainParserMatch | undefined {
  const wrapped = cell.bits.length === 0 && cell.refs.length === 1
  const valueCell = wrapped ? cell.refs[0] : cell
  const uri = readSnakeText(valueCell.beginParse())

  if (!uri || !URI_PATTERN.test(uri)) {
    return undefined
  }

  return metadataUriMatch(uri, wrapped ? "Referenced snake string" : "Snake string", false)
}

function metadataUriMatch(
  uri: string,
  layout: string,
  isTep64: boolean,
): DomainParserMatch {
  return {
    data: {storage: isTep64 ? "off-chain" : "uri", uri},
    parsedValue: {
      kind: "object",
      typeName: isTep64 ? "Jetton metadata" : "Metadata URI",
      entries: [
        ...(isTep64 ? [entry("Storage", scalar("Off-chain"))] : []),
        entry("URI", scalar(uri, "string")),
      ],
    },
    label: isTep64 ? "Jetton metadata · Off-chain" : "Metadata URI",
    details: {layout},
    confidence: {
      score: isTep64 ? 0.99 : 0.9,
      reasons: [
        ...(isTep64 ? ["The TEP-64 off-chain prefix matched"] : []),
        "The complete snake value is a valid UTF-8 metadata URI",
      ],
    },
  }
}

function parseJettonMetadata(cell: Cell): DomainParserMatch | undefined {
  return (
    parseOnChainJettonMetadata(cell) ??
    parseOffChainJettonMetadata(cell) ??
    parseMetadataUri(cell)
  )
}

/** TEP-64 Jetton metadata and URI-only metadata cells */
export const jettonMetadataDomainParser: DomainParser = {
  id: "jetton-metadata",
  parse: parseJettonMetadata,
}
