import type {Message, MessageRelaxed} from "@ton/core"
import {Cell, loadShardAccount, type Slice} from "@ton/core"
import type {ContractABI, SymTable, Ty} from "@ton/tolk-abi-to-typescript"
import {
  DynamicCtx,
  renderTy,
  SymTable as CompilerSymTable,
  unpackFromSliceDynamic,
} from "@ton/tolk-abi-to-typescript"

import type {BackendContractInfo} from "../model/backend"
import type {
  ContractData,
  ParsedContractStorage,
  ParsedTransactionBody,
  ParsedValue,
  TransactionInfo,
} from "../model/transaction"
import {unpackStorageValue} from "./decodeStorageValue"
import {toParsedValue, type ParsedValueTypeContext} from "./toParsedValue"

interface MessageCandidate {
  readonly body_ty_idx: number
}

interface DeclarationCandidate {
  readonly body_ty_idx: number
  readonly priority: number
}

interface ParsableMessage {
  readonly info: Message["info"] | MessageRelaxed["info"]
  readonly body: Cell
}

type MessageAbiDirection = "incoming" | "outgoing"

interface MessageAbiDecodeAttempt {
  readonly abi: ContractABI
  readonly direction: MessageAbiDirection
}

/** Compiler ABI together with the registry metadata that identified it. */
export interface ExtendedContractABI {
  readonly compiler_abi: ContractABI
  readonly display_name?: string
  readonly code_hashes: readonly string[]
  readonly links?: readonly unknown[]
}

export type CellDecodeCategory = "comment" | "message" | "storage"

export type CellMessageDirection = "incoming-internal" | "incoming-external" | "outgoing"

export interface CellDecodeConsumption {
  readonly initialBits: number
  readonly initialRefs: number
  readonly remainingBits: number
  readonly remainingRefs: number
  readonly complete: boolean
}

export type CellDecodeProvenance =
  | {
      readonly source: "text-comment"
      readonly parser: "built-in"
    }
  | {
      readonly source: "compiler-abi"
      readonly displayName?: string
      readonly codeHashes: readonly string[]
    }

export interface DecodedCellWithAbi {
  readonly category: CellDecodeCategory
  readonly direction?: CellMessageDirection
  readonly directionCandidates?: readonly CellMessageDirection[]
  readonly name: string
  readonly value: ParsedValue
  readonly provenance: CellDecodeProvenance
  readonly consumption?: CellDecodeConsumption
}

const BOUNCED_BODY_PREFIX = 0xff_ff_ff_ff
const RICH_BOUNCE_BODY_PREFIX = 0xff_ff_ff_fe

const getBodyTypeName = (symbols: SymTable, bodyTyIdx: number): string => {
  return renderTy(symbols, bodyTyIdx)
}

const hasAcceptableMessageDecodeRemainder = (initialSlice: Slice, parser: Slice): boolean => {
  if (parser.remainingRefs !== 0) {
    return false
  }

  // Some message schemas leave trailing bits outside the ABI payload
  // (for example, attached signatures). Accept them as long as decoding
  // consumed something and did not leave trailing refs behind.
  return (
    parser.remainingBits === 0 ||
    parser.remainingBits < initialSlice.remainingBits ||
    parser.remainingRefs < initialSlice.remainingRefs
  )
}

const getBodyTypeKey = (bodyTyIdx: number): string => {
  return `ty#${bodyTyIdx}`
}

type AbiDeclaration = Readonly<ContractABI["declarations"][number]>

const createSymTable = (abi: ContractABI): SymTable =>
  new CompilerSymTable(
    abi.declarations,
    abi.unique_types,
    abi.struct_instantiations,
    abi.alias_instantiations,
  )

const getDeclarationOpcode = (declaration: AbiDeclaration | undefined): number | undefined => {
  if (declaration?.kind === "struct" && declaration.prefix?.prefix_len === 32) {
    return declaration.prefix.prefix_num
  }
  return undefined
}

const findDeclaration = (abi: ContractABI, bodyTy: Ty): AbiDeclaration | undefined => {
  switch (bodyTy.kind) {
    case "StructRef": {
      return abi.declarations.find(
        declaration => declaration.kind === "struct" && declaration.name === bodyTy.struct_name,
      )
    }
    case "AliasRef": {
      return abi.declarations.find(
        declaration => declaration.kind === "alias" && declaration.name === bodyTy.alias_name,
      )
    }
    case "EnumRef": {
      return abi.declarations.find(
        declaration => declaration.kind === "enum" && declaration.name === bodyTy.enum_name,
      )
    }
    default: {
      return undefined
    }
  }
}

const resolveOpcodeNameFromBodyType = (
  abi: ContractABI,
  symbols: SymTable,
  bodyTyIdx: number,
  opcode: number,
  visitedTyIdx = new Set<number>(),
): string | undefined => {
  if (visitedTyIdx.has(bodyTyIdx)) {
    return undefined
  }
  visitedTyIdx.add(bodyTyIdx)

  let bodyTy: Ty
  try {
    bodyTy = symbols.tyByIdx(bodyTyIdx)
  } catch {
    return undefined
  }

  if (bodyTy.kind === "union") {
    for (const variant of bodyTy.variants) {
      if (variant.prefix_len === 32 && variant.prefix_num === opcode) {
        return getBodyTypeName(symbols, variant.variant_ty_idx)
      }
    }
  }

  const declaration = findDeclaration(abi, bodyTy)
  if (!declaration) {
    return undefined
  }

  if (declaration.kind === "struct" && getDeclarationOpcode(declaration) === opcode) {
    return declaration.name
  }

  if (declaration.kind === "alias") {
    let targetTyIdx = declaration.target_ty_idx
    try {
      targetTyIdx = symbols.aliasTargetOf(bodyTyIdx).ty_idx
    } catch {
      // Non-AliasRef ty_idx can still reach an alias declaration only for malformed ABI.
    }
    return resolveOpcodeNameFromBodyType(abi, symbols, targetTyIdx, opcode, visitedTyIdx)
  }

  if (declaration.kind === "enum") {
    return declaration.members.find(member => Number(BigInt(member.value)) === opcode)?.name
  }

  return undefined
}

export const resolveAbiOpcodeName = (
  abi: ContractABI | undefined,
  opcode: number,
  direction?: "incoming" | "outgoing",
): string | undefined => {
  if (!abi) {
    return undefined
  }
  const symbols = createSymTable(abi)

  const messages =
    direction === "outgoing"
      ? abi.outgoing_messages
      : direction === "incoming"
        ? [...abi.incoming_messages, ...abi.incoming_external]
        : [...abi.incoming_messages, ...abi.incoming_external, ...abi.outgoing_messages]

  for (const message of messages) {
    const name = resolveOpcodeNameFromBodyType(abi, symbols, message.body_ty_idx, opcode)
    if (name) {
      return name
    }
  }

  return abi.declarations.find(declaration => getDeclarationOpcode(declaration) === opcode)?.name
}

const getDeclarationCandidates = (
  abi: ContractABI,
  opcode: number | undefined,
): DeclarationCandidate[] => {
  const candidates: DeclarationCandidate[] = []

  for (const declaration of abi.declarations) {
    switch (declaration.kind) {
      case "struct": {
        if (declaration.type_params && declaration.type_params.length > 0) {
          continue
        }
        if (declaration.prefix && declaration.prefix.prefix_len !== 32) {
          continue
        }

        const matchesOpcode =
          opcode !== undefined &&
          declaration.prefix?.prefix_len === 32 &&
          declaration.prefix.prefix_num === opcode

        candidates.push({
          body_ty_idx: declaration.ty_idx,
          priority: matchesOpcode ? 0 : declaration.prefix ? 1 : 2,
        })
        break
      }
      case "alias": {
        if (declaration.type_params && declaration.type_params.length > 0) {
          continue
        }

        candidates.push({
          body_ty_idx: declaration.ty_idx,
          priority: 3,
        })
        break
      }
      case "enum": {
        candidates.push({
          body_ty_idx: declaration.ty_idx,
          priority: 4,
        })
        break
      }
    }
  }

  return candidates.sort((left, right) => left.priority - right.priority)
}

const getIncomingCandidates = (
  abi: ContractABI,
  isInternal: boolean,
  opcode: number | undefined,
): readonly MessageCandidate[] => {
  const directCandidates = isInternal ? abi.incoming_messages : abi.incoming_external
  if (!isInternal) {
    return directCandidates
  }

  const deduped = new Map<string, MessageCandidate>()
  for (const candidate of directCandidates) {
    deduped.set(getBodyTypeKey(candidate.body_ty_idx), candidate)
  }

  for (const candidate of getDeclarationCandidates(abi, opcode)) {
    const key = getBodyTypeKey(candidate.body_ty_idx)
    if (!deduped.has(key)) {
      deduped.set(key, {body_ty_idx: candidate.body_ty_idx})
    }
  }

  return [...deduped.values()]
}

const getOutgoingCandidates = (
  abi: ContractABI,
  opcode: number | undefined,
): readonly MessageCandidate[] => {
  const deduped = new Map<string, MessageCandidate>()

  for (const candidate of abi.outgoing_messages) {
    deduped.set(getBodyTypeKey(candidate.body_ty_idx), candidate)
  }

  for (const candidate of getDeclarationCandidates(abi, opcode)) {
    const key = getBodyTypeKey(candidate.body_ty_idx)
    if (!deduped.has(key)) {
      deduped.set(key, {body_ty_idx: candidate.body_ty_idx})
    }
  }

  return [...deduped.values()]
}

interface NestedPayloadSliceCandidate {
  readonly slice: Slice
  readonly wrapper?: "inline" | "ref"
}

const MAX_NESTED_PAYLOAD_DEPTH = 4

function withNestedPayloadDepth(
  context: ParsedValueTypeContext,
  tyIdx: number,
  nestedPayloadDepth: number,
): ParsedValueTypeContext {
  return {...context, tyIdx, nestedPayloadDepth}
}

const getNestedPayloadCandidates = (
  abi: ContractABI,
  symbols: SymTable,
  opcode: number,
): readonly MessageCandidate[] => {
  const deduped = new Map<string, MessageCandidate>()

  for (const candidate of [
    ...abi.incoming_messages,
    ...abi.incoming_external,
    ...abi.outgoing_messages,
  ]) {
    if (resolveOpcodeNameFromBodyType(abi, symbols, candidate.body_ty_idx, opcode)) {
      deduped.set(getBodyTypeKey(candidate.body_ty_idx), candidate)
    }
  }

  for (const declaration of abi.declarations) {
    if (getDeclarationOpcode(declaration) === opcode) {
      deduped.set(getBodyTypeKey(declaration.ty_idx), {body_ty_idx: declaration.ty_idx})
    }
  }

  return [...deduped.values()]
}

function getNestedPayloadSliceCandidates(slice: Slice): readonly NestedPayloadSliceCandidate[] {
  const payloadSlices: NestedPayloadSliceCandidate[] = []
  if (slice.remainingBits >= 1) {
    const parser = slice.clone()
    const storedInRef = parser.loadBoolean()
    if (!storedInRef) {
      payloadSlices.push({slice: parser, wrapper: "inline"})
    } else if (parser.remainingRefs >= 1) {
      const refPayload = parser.loadRef().beginParse()
      if (parser.remainingBits === 0 && parser.remainingRefs === 0) {
        payloadSlices.push({slice: refPayload, wrapper: "ref"})
      }
    }
  }

  payloadSlices.push({slice})
  return payloadSlices
}

function nestedPayloadOpcodeValue(slice: Slice): ParsedValue | undefined {
  if (slice.remainingBits < 32) {
    return undefined
  }

  return {
    kind: "scalar",
    value: `0x${slice.clone().preloadUint(32).toString(16).padStart(8, "0")}`,
  }
}

function toUndecodedNestedPayloadValue(
  candidate: NestedPayloadSliceCandidate | undefined,
): ParsedValue | undefined {
  if (!candidate?.wrapper) {
    return undefined
  }

  const opcode = nestedPayloadOpcodeValue(candidate.slice)
  return {
    kind: "object",
    typeName: candidate.wrapper === "inline" ? "PayloadInline" : "PayloadInRef",
    entries: [
      ...(opcode ? [{key: "opcode", value: opcode}] : []),
      {key: "payload", value: toParsedValue(candidate.slice)},
    ],
  }
}

function tryDecodeNestedTextCommentPayload(slice: Slice): ParsedValue | undefined {
  const parser = slice.clone()
  if (parser.remainingBits < 32 || parser.loadUint(32) !== 0) {
    return undefined
  }

  return {
    kind: "object",
    typeName: "Text Comment",
    entries: [{key: "text", value: textCommentTailValue(parser)}],
  }
}

function tryDecodeNestedPayloadSlice(
  slice: Slice,
  context: ParsedValueTypeContext,
): ParsedValue | undefined {
  const nestedPayloadDepth = context.nestedPayloadDepth ?? 0
  if (nestedPayloadDepth >= MAX_NESTED_PAYLOAD_DEPTH) {
    return undefined
  }

  const payloadCandidates = getNestedPayloadSliceCandidates(slice)
  for (const payloadCandidate of payloadCandidates) {
    const decodedPayload = tryDecodeNestedPayloadContent(payloadCandidate.slice, context)
    if (decodedPayload) {
      return decodedPayload
    }
  }

  return toUndecodedNestedPayloadValue(payloadCandidates.find(candidate => candidate.wrapper))
}

function tryDecodeNestedPayloadContent(
  slice: Slice,
  context: ParsedValueTypeContext,
): ParsedValue | undefined {
  const abiCandidates = [context.abi, ...(context.abiCandidates ?? [])].filter(
    (abi): abi is ContractABI => abi !== undefined,
  )
  if (abiCandidates.length === 0 || slice.remainingBits < 32) {
    return undefined
  }

  const nestedPayloadDepth = context.nestedPayloadDepth ?? 0
  const opcode = Number(slice.clone().preloadUint(32))
  const textCommentPayload = opcode === 0 ? tryDecodeNestedTextCommentPayload(slice) : undefined
  if (textCommentPayload) {
    return textCommentPayload
  }

  for (const abi of abiCandidates) {
    const ctx = new DynamicCtx(abi)
    for (const candidate of getNestedPayloadCandidates(abi, ctx.symbols, opcode)) {
      const parser = slice.clone()
      try {
        const decoded: unknown = unpackFromSliceDynamic(
          ctx,
          candidate.body_ty_idx,
          parser,
        ) as unknown
        if (parser.remainingBits !== 0 || parser.remainingRefs !== 0) {
          continue
        }

        return toParsedValue(
          decoded,
          withNestedPayloadDepth(
            {...context, abi, symbols: ctx.symbols},
            candidate.body_ty_idx,
            nestedPayloadDepth + 1,
          ),
        )
      } catch {
        continue
      }
    }
  }

  return undefined
}

const createBodyParser = (message: ParsableMessage): Slice | undefined => {
  const parser = message.body.asSlice()
  if (message.info.type !== "internal" || !message.info.bounced) {
    return parser
  }

  if (parser.remainingBits < 32) {
    // There are not enough bits for a standard bounce prefix, but the body can
    // still be a valid short prefixless message declared by the contract ABI.
    return parser
  }

  const prefix = Number(parser.preloadUint(32))
  if (prefix === RICH_BOUNCE_BODY_PREFIX) {
    parser.loadUint(32)
    if (parser.remainingRefs < 1) {
      return undefined
    }

    // Rich bounces wrap the original message body into `originalBody:^Cell`.
    return parser.loadRef().beginParse()
  }

  if (prefix === BOUNCED_BODY_PREFIX) {
    parser.loadUint(32)
  }

  return parser
}

const isBouncedInternalMessage = (message: ParsableMessage): boolean =>
  message.info.type === "internal" && message.info.bounced

const getOpcodeAfterBouncePrefix = (message: ParsableMessage): number | undefined => {
  const opcodeSlice = createBodyParser(message)
  if (!opcodeSlice || opcodeSlice.remainingBits < 32) {
    return undefined
  }

  return Number(opcodeSlice.preloadUint(32))
}

export const getMessageOpcode = (
  message: ParsableMessage,
  parsedBody?: ParsedTransactionBody,
): number | undefined => {
  if (parsedBody) {
    // Once ABI decoding succeeds, its schema is authoritative: a prefixless
    // body has no opcode even if its first field happens to be at least 32 bits.
    return parsedBody.opcode
  }

  const slice = createBodyParser(message)
  if (!slice || slice.remainingBits < 32) {
    return undefined
  }

  return Number(slice.preloadUint(32))
}

const tryReadTextCommentString = (slice: Slice): string | undefined => {
  const parser = slice.clone()
  try {
    const text = parser.loadStringTail()
    return parser.remainingBits === 0 && parser.remainingRefs === 0 ? text : undefined
  } catch {
    return undefined
  }
}

const textCommentTailValue = (slice: Slice): ParsedValue => {
  const text = tryReadTextCommentString(slice)
  if (text !== undefined) {
    return {kind: "scalar", value: text}
  }

  return toParsedValue(slice)
}

const tryDecodeTextCommentSlice = (baseSlice: Slice): ParsedTransactionBody | undefined => {
  if (baseSlice.remainingBits < 32) {
    return undefined
  }

  const parser = baseSlice.clone()
  if (parser.loadUint(32) !== 0) {
    return undefined
  }

  return {
    name: "Text Comment",
    opcode: 0,
    value: {
      kind: "object",
      typeName: "Text Comment",
      entries: [{key: "text", value: textCommentTailValue(parser)}],
    },
  }
}

const tryDecodeTextCommentBody = (message: ParsableMessage): ParsedTransactionBody | undefined => {
  const baseSlice = createBodyParser(message)
  return baseSlice ? tryDecodeTextCommentSlice(baseSlice) : undefined
}

const resolveCandidateOpcodeName = (
  abi: ContractABI,
  symbols: SymTable,
  candidate: MessageCandidate,
  opcode: number | undefined,
): string | undefined => {
  if (opcode === undefined) {
    return undefined
  }

  return resolveOpcodeNameFromBodyType(abi, symbols, candidate.body_ty_idx, opcode)
}

const createBouncedOpcodeBody = (
  name: string,
  opcode: number,
  body: Slice,
): ParsedTransactionBody => ({
  name,
  opcode,
  value: {
    kind: "object",
    typeName: name,
    entries: [{key: "body", value: toParsedValue(body)}],
  },
})

interface SliceCandidateDecodeOptions {
  readonly opcode?: number
  readonly requireOpcodeMatch?: boolean
  readonly preserveUndecodedBody?: boolean
  readonly onAcceptedRemainder?: (parser: Slice) => void
}

const tryDecodeSliceWithCandidates = (
  baseSlice: Slice,
  abi: ContractABI,
  candidates: readonly MessageCandidate[],
  nestedPayloadAbis: readonly ContractABI[] = [],
  options: SliceCandidateDecodeOptions = {},
): ParsedTransactionBody | undefined => {
  if (candidates.length === 0) {
    return undefined
  }

  const ctx = new DynamicCtx(abi)
  const matchedOpcodeName = options.requireOpcodeMatch
    ? candidates
        .map(candidate => resolveCandidateOpcodeName(abi, ctx.symbols, candidate, options.opcode))
        .find(name => name !== undefined)
    : undefined

  for (const candidate of candidates) {
    const parser = baseSlice.clone()
    const candidateOpcodeName = resolveCandidateOpcodeName(
      abi,
      ctx.symbols,
      candidate,
      options.opcode,
    )
    try {
      const decoded: unknown = unpackFromSliceDynamic(ctx, candidate.body_ty_idx, parser) as unknown
      if (!hasAcceptableMessageDecodeRemainder(baseSlice, parser)) {
        continue
      }

      const parsedBody = {
        name: getBodyTypeName(ctx.symbols, candidate.body_ty_idx),
        ...(candidateOpcodeName !== undefined && options.opcode !== undefined
          ? {opcode: options.opcode}
          : {}),
        value: toParsedValue(decoded, {
          symbols: ctx.symbols,
          tyIdx: candidate.body_ty_idx,
          abi,
          abiCandidates: nestedPayloadAbis,
          decodeRemaining: tryDecodeNestedPayloadSlice,
          nestedPayloadDepth: 0,
        }),
      }

      if (options.requireOpcodeMatch && !candidateOpcodeName) {
        continue
      }

      options.onAcceptedRemainder?.(parser)
      return parsedBody
    } catch {
      continue
    }
  }

  if (options.preserveUndecodedBody && matchedOpcodeName && options.opcode !== undefined) {
    return createBouncedOpcodeBody(matchedOpcodeName, options.opcode, baseSlice)
  }

  return undefined
}

const tryDecodeMessageWithCandidates = (
  message: ParsableMessage,
  abi: ContractABI,
  candidates: readonly MessageCandidate[],
  nestedPayloadAbis: readonly ContractABI[] = [],
): ParsedTransactionBody | undefined => {
  const baseSlice = createBodyParser(message)
  if (!baseSlice) {
    return undefined
  }

  const bouncedInternal = isBouncedInternalMessage(message)
  const opcode = getOpcodeAfterBouncePrefix(message)
  return tryDecodeSliceWithCandidates(baseSlice, abi, candidates, nestedPayloadAbis, {
    opcode,
    // A single direction-specific ABI entry is unambiguous even when the body
    // has no opcode (for example, a signed Wallet V4 external message). With
    // multiple entries, require an opcode match instead of guessing whichever
    // prefixless type happens to decode first.
    requireOpcodeMatch: candidates.length > 1,
    preserveUndecodedBody: bouncedInternal,
  })
}

const tryDecodeDeclaredMessageWithAbi = (
  message: ParsableMessage,
  abi: ContractABI,
  direction: MessageAbiDirection,
  nestedPayloadAbis?: readonly ContractABI[],
): ParsedTransactionBody | undefined => {
  const candidates =
    direction === "outgoing"
      ? abi.outgoing_messages
      : message.info.type === "external-in"
        ? abi.incoming_external
        : abi.incoming_messages

  return tryDecodeMessageWithCandidates(message, abi, candidates, nestedPayloadAbis)
}

const tryDecodeIncomingMessageWithAbi = (
  message: ParsableMessage,
  abi: ContractABI,
  nestedPayloadAbis?: readonly ContractABI[],
): ParsedTransactionBody | undefined => {
  const opcode = getOpcodeAfterBouncePrefix(message)
  const candidates = getIncomingCandidates(abi, message.info.type === "internal", opcode)
  return tryDecodeMessageWithCandidates(message, abi, candidates, nestedPayloadAbis)
}

const tryDecodeOutgoingMessageWithAbi = (
  message: ParsableMessage,
  abi: ContractABI,
  nestedPayloadAbis?: readonly ContractABI[],
): ParsedTransactionBody | undefined => {
  const opcode = getOpcodeAfterBouncePrefix(message)
  const candidates = getOutgoingCandidates(abi, opcode)
  return tryDecodeMessageWithCandidates(message, abi, candidates, nestedPayloadAbis)
}

const getStorageCandidates = (compilerAbi: ContractABI): readonly number[] => {
  const candidates = [
    compilerAbi.storage.storage_ty_idx,
    compilerAbi.storage.storage_at_deployment_ty_idx,
  ]
    .filter(
      (tyIdx): tyIdx is number =>
        tyIdx !== undefined && compilerAbi.unique_types[tyIdx]?.kind !== "nullLiteral",
    )
    .map(tyIdx => [getBodyTypeKey(tyIdx), tyIdx] as const)

  return [...new Map(candidates).values()]
}

const parseShardAccount = (shardAccountBase64: string) => {
  try {
    return loadShardAccount(Cell.fromBase64(shardAccountBase64).beginParse())
  } catch {
    return
  }
}

export const getShardAccountBalance = (shardAccountBase64: string): bigint | undefined => {
  const shard = parseShardAccount(shardAccountBase64)
  if (!shard) return

  return shard.account?.storage.balance.coins ?? 0n
}

const tryDecodeStorageSliceWithAbi = (
  baseSlice: Slice,
  abi: ContractABI,
): ParsedContractStorage | undefined => {
  const candidates = getStorageCandidates(abi)
  if (candidates.length === 0) {
    return undefined
  }

  const ctx = new DynamicCtx(abi)

  for (const candidate of candidates) {
    try {
      const decoded = unpackStorageValue(ctx, candidate, baseSlice)

      return {
        name: getBodyTypeName(ctx.symbols, candidate),
        value: toParsedValue(decoded, {
          symbols: ctx.symbols,
          tyIdx: candidate,
          decodeRemaining: tryDecodeNestedPayloadSlice,
        }),
      }
    } catch {
      // Try the next storage candidate.
    }
  }

  return undefined
}

const tryDecodeStorageWithAbi = (
  shardAccountBase64: string,
  abi: ContractABI,
): ParsedContractStorage | undefined => {
  const state = parseShardAccount(shardAccountBase64)?.account?.storage.state
  if (state?.type !== "active" || !state.state.data) {
    return undefined
  }

  return tryDecodeStorageSliceWithAbi(state.state.data.beginParse(), abi)
}

const tryDecodeStorageCellWithAbi = (
  dataCell: Cell,
  abi: ContractABI,
): ParsedContractStorage | undefined => {
  return tryDecodeStorageSliceWithAbi(dataCell.beginParse(), abi)
}

/**
 * Decodes a standalone cell with a compiler ABI without manufacturing a TON message envelope.
 *
 * Message bodies are tried before storage because ABI message prefixes make them more specific.
 * The direction describes the ABI candidate group that matched; it is not inferred from the cell.
 */
export const decodeCellWithAbi = (
  cell: Cell,
  abi: ExtendedContractABI,
  additionalAbis: readonly ExtendedContractABI[] = [],
): DecodedCellWithAbi | undefined => {
  let baseSlice: Slice
  try {
    baseSlice = cell.beginParse()
  } catch {
    return undefined
  }

  const textComment = tryDecodeTextCommentSlice(baseSlice)
  if (textComment) {
    return {
      category: "comment",
      name: textComment.name,
      value: textComment.value,
      provenance: {source: "text-comment", parser: "built-in"},
    }
  }

  const compilerAbi = abi.compiler_abi
  const nestedPayloadAbis = [
    compilerAbi,
    ...additionalAbis.map(candidate => candidate.compiler_abi),
  ]
  const opcode = baseSlice.remainingBits >= 32 ? Number(baseSlice.preloadUint(32)) : undefined
  const symbols = createSymTable(compilerAbi)
  const declarationCandidates =
    opcode === undefined
      ? []
      : getDeclarationCandidates(compilerAbi, opcode).filter(
          candidate =>
            resolveCandidateOpcodeName(compilerAbi, symbols, candidate, opcode) !== undefined,
        )
  const messageCandidates: readonly [
    CellMessageDirection | undefined,
    readonly MessageCandidate[],
  ][] = [
    ["incoming-internal", compilerAbi.incoming_messages],
    ["incoming-external", compilerAbi.incoming_external],
    ["outgoing", compilerAbi.outgoing_messages],
    [undefined, declarationCandidates],
  ]

  const messageMatches: {
    readonly direction?: CellMessageDirection
    readonly message: ParsedTransactionBody
    readonly consumption?: CellDecodeConsumption
  }[] = []
  for (const [direction, candidates] of messageCandidates) {
    let messageConsumption: CellDecodeConsumption | undefined
    const message = tryDecodeSliceWithCandidates(
      baseSlice,
      compilerAbi,
      candidates,
      nestedPayloadAbis,
      {
        onAcceptedRemainder: parser => {
          messageConsumption = cellDecodeConsumption(baseSlice, parser)
        },
      },
    )
    if (message) {
      messageMatches.push({direction, message, consumption: messageConsumption})
    }
  }

  const selectedMessage = messageMatches[0]
  if (selectedMessage) {
    const directionCandidates = [
      ...new Set(
        messageMatches
          .map(match => match.direction)
          .filter((direction): direction is CellMessageDirection => direction !== undefined),
      ),
    ]
    return {
      category: "message",
      ...(directionCandidates.length === 1 ? {direction: directionCandidates[0]} : {}),
      ...(directionCandidates.length > 1 ? {directionCandidates} : {}),
      name: selectedMessage.message.name,
      value: selectedMessage.message.value,
      consumption: selectedMessage.consumption,
      provenance: {
        source: "compiler-abi",
        displayName: abi.display_name,
        codeHashes: abi.code_hashes,
      },
    }
  }

  const storage = tryDecodeStorageSliceWithAbi(baseSlice, compilerAbi)
  if (!storage) {
    return undefined
  }

  return {
    category: "storage",
    name: storage.name,
    value: storage.value,
    consumption: {
      initialBits: baseSlice.remainingBits,
      initialRefs: baseSlice.remainingRefs,
      remainingBits: 0,
      remainingRefs: 0,
      complete: true,
    },
    provenance: {
      source: "compiler-abi",
      displayName: abi.display_name,
      codeHashes: abi.code_hashes,
    },
  }
}

const cellDecodeConsumption = (initial: Slice, parser: Slice): CellDecodeConsumption => ({
  initialBits: initial.remainingBits,
  initialRefs: initial.remainingRefs,
  remainingBits: parser.remainingBits,
  remainingRefs: parser.remainingRefs,
  complete: parser.remainingBits === 0 && parser.remainingRefs === 0,
})

export const decodeStorageDataCell = (
  dataCellBase64: string | null | undefined,
  abi: ContractABI | undefined,
): ParsedContractStorage | undefined => {
  if (!dataCellBase64 || !abi) {
    return undefined
  }

  try {
    return tryDecodeStorageCellWithAbi(Cell.fromBase64(dataCellBase64), abi)
  } catch {
    return undefined
  }
}

export const decodeStorageShardAccount = (
  shardAccountBase64: string | null | undefined,
  abi: ContractABI | undefined,
): ParsedContractStorage | undefined => {
  if (!shardAccountBase64 || !abi) {
    return undefined
  }

  return tryDecodeStorageWithAbi(shardAccountBase64, abi)
}

export const resolveMessageOpcodeName = (
  message: ParsableMessage,
  contracts: Map<string, ContractData>,
  sourceAddress?: string,
  parsedBody?: ParsedTransactionBody,
): string | undefined => {
  const opcode = getMessageOpcode(message, parsedBody)
  if (opcode === undefined) {
    return undefined
  }
  if (opcode === 0) {
    return "Text Comment"
  }

  const destinationContract =
    message.info.type === "internal" || message.info.type === "external-in"
      ? contracts.get(message.info.dest.toString())
      : undefined
  const messageSourceAddress =
    message.info.type !== "external-in" && message.info.src
      ? message.info.src.toString()
      : sourceAddress
  const sourceContract = messageSourceAddress ? contracts.get(messageSourceAddress) : undefined
  const isBouncedInternal = message.info.type === "internal" && message.info.bounced

  if (isBouncedInternal) {
    return (
      resolveAbiOpcodeName(destinationContract?.abi, opcode, "outgoing") ??
      resolveAbiOpcodeName(sourceContract?.abi, opcode, "incoming") ??
      [...contracts.values()]
        .map(contract => resolveAbiOpcodeName(contract.abi, opcode))
        .find(name => name !== undefined)
    )
  }

  return (
    resolveAbiOpcodeName(destinationContract?.abi, opcode, "incoming") ??
    resolveAbiOpcodeName(sourceContract?.abi, opcode, "outgoing") ??
    [...contracts.values()]
      .map(contract => resolveAbiOpcodeName(contract.abi, opcode))
      .find(name => name !== undefined)
  )
}

export const decodeMessageBody = (
  message: ParsableMessage,
  contracts: Map<string, ContractData>,
  sourceAddress?: string,
  additionalAbis: readonly ContractABI[] = [],
): ParsedTransactionBody | undefined => {
  const messageSourceAddress =
    message.info.type !== "external-in" && message.info.src
      ? message.info.src.toString()
      : sourceAddress

  const sourceContract = messageSourceAddress ? contracts.get(messageSourceAddress) : undefined

  const destinationContract =
    message.info.type === "internal" || message.info.type === "external-in"
      ? contracts.get(message.info.dest.toString())
      : undefined

  const allContracts = [...contracts.values()]

  const nestedPayloadAbis = [
    destinationContract?.abi,
    sourceContract?.abi,
    ...allContracts.map(contract => contract.abi),
    ...additionalAbis,
  ].filter((abi): abi is ContractABI => abi !== undefined)

  const endpointAttempts: MessageAbiDecodeAttempt[] = []
  const fallbackAttempts: MessageAbiDecodeAttempt[] = []

  const appendAttempts = (
    target: MessageAbiDecodeAttempt[],
    direction: MessageAbiDirection,
    candidates: readonly (ContractData | undefined)[],
  ) => {
    for (const contract of candidates) {
      if (contract?.abi) {
        target.push({abi: contract.abi, direction})
      }
    }
  }

  if (message.info.type === "internal") {
    if (message.info.bounced) {
      // First interpret a bounced body as an original outgoing message of either endpoint.
      appendAttempts(endpointAttempts, "outgoing", [destinationContract, sourceContract])
      // If that fails, interpret it as an incoming message, checking the source endpoint first.
      appendAttempts(endpointAttempts, "incoming", [sourceContract, destinationContract])
      // Then add outgoing ABI fallbacks from every known contract for the bounced body.
      appendAttempts(fallbackAttempts, "outgoing", [
        destinationContract,
        sourceContract,
        ...allContracts,
      ])
      // Finally add incoming ABI fallbacks from every known contract for the bounced body.
      appendAttempts(fallbackAttempts, "incoming", [
        sourceContract,
        destinationContract,
        ...allContracts,
      ])
    } else {
      // A regular internal message is first decoded by the receiver's incoming ABI.
      appendAttempts(endpointAttempts, "incoming", [destinationContract])
      // If the receiver does not match, decode it by the sender's outgoing ABI.
      appendAttempts(endpointAttempts, "outgoing", [sourceContract])
      // Add incoming schemas from other known contracts as a lower-priority fallback.
      appendAttempts(fallbackAttempts, "incoming", [destinationContract, ...allContracts])
      // Add outgoing schemas from other known contracts after all incoming fallbacks.
      appendAttempts(fallbackAttempts, "outgoing", [sourceContract, ...allContracts])
    }
  } else if (message.info.type === "external-out") {
    // An external-out message is first decoded by the source contract's outgoing ABI.
    appendAttempts(endpointAttempts, "outgoing", [sourceContract])
    // If the source does not match, try outgoing schemas from every known contract.
    appendAttempts(fallbackAttempts, "outgoing", [sourceContract, ...allContracts])
  } else {
    // An external-in message is first decoded by the destination contract's external ABI.
    appendAttempts(endpointAttempts, "incoming", [destinationContract])
    // If the destination does not match, try external-in schemas from every known contract.
    appendAttempts(fallbackAttempts, "incoming", [destinationContract, ...allContracts])
  }

  // Prefer the message lists explicitly declared by the endpoint ABIs, then
  // the same lists from other known contracts. This lets a single prefixless
  // schema decode before text-comment heuristics and ensures receiver incoming
  // messages win over sender outgoing messages.
  for (const attempts of [endpointAttempts, fallbackAttempts]) {
    for (const attempt of attempts) {
      const parsedBody = tryDecodeDeclaredMessageWithAbi(
        message,
        attempt.abi,
        attempt.direction,
        nestedPayloadAbis,
      )
      if (parsedBody) {
        return parsedBody
      }
    }
  }

  // Only after all explicitly declared messages fail, try declaration-based
  // fallbacks in the same direction order.
  for (const attempt of fallbackAttempts) {
    const parsedBody =
      attempt.direction === "incoming"
        ? tryDecodeIncomingMessageWithAbi(message, attempt.abi, nestedPayloadAbis)
        : tryDecodeOutgoingMessageWithAbi(message, attempt.abi, nestedPayloadAbis)
    if (parsedBody) {
      return parsedBody
    }
  }

  return tryDecodeTextCommentBody(message)
}

export const decodeTransactionMessageBody = (
  tx: TransactionInfo,
  contracts: Map<string, ContractData>,
  allContracts: readonly BackendContractInfo[],
  compilerAbisByCodeHash?: ReadonlyMap<string, ContractData["abi"]>,
): ParsedTransactionBody | undefined => {
  if (tx.parsedBody) {
    return tx.parsedBody
  }

  const inMessage = tx.transaction.inMessage
  if (!inMessage) {
    return undefined
  }

  const additionalAbis = [
    ...allContracts.map(contract => contract.abi),
    ...(compilerAbisByCodeHash ? [...compilerAbisByCodeHash.values()] : []),
  ].filter((abi): abi is ContractABI => abi !== undefined)

  return decodeMessageBody(inMessage, contracts, tx.address?.toString(), additionalAbis)
}

const tryDecodeTransactionBodyWithAbi = (
  tx: TransactionInfo,
  abi: ContractABI,
  nestedPayloadAbis: readonly ContractABI[] = [],
): ParsedTransactionBody | undefined => {
  const inMessage = tx.transaction.inMessage
  if (!inMessage) {
    return undefined
  }

  if (inMessage.info.type === "internal" && inMessage.info.bounced) {
    return (
      tryDecodeDeclaredMessageWithAbi(inMessage, abi, "outgoing", nestedPayloadAbis) ??
      tryDecodeDeclaredMessageWithAbi(inMessage, abi, "incoming", nestedPayloadAbis) ??
      tryDecodeOutgoingMessageWithAbi(inMessage, abi, nestedPayloadAbis) ??
      tryDecodeIncomingMessageWithAbi(inMessage, abi, nestedPayloadAbis) ??
      tryDecodeTextCommentBody(inMessage)
    )
  }

  return (
    tryDecodeDeclaredMessageWithAbi(inMessage, abi, "incoming", nestedPayloadAbis) ??
    tryDecodeIncomingMessageWithAbi(inMessage, abi, nestedPayloadAbis) ??
    tryDecodeTextCommentBody(inMessage)
  )
}

export const decodeStateInitData = (
  dataCell: Cell | undefined,
  contract: ContractData | undefined,
  contractName: string | undefined,
  allContracts: readonly BackendContractInfo[],
): ParsedContractStorage | undefined => {
  if (!dataCell) {
    return undefined
  }

  const targetAbi =
    contract?.abi ??
    (contractName ? allContracts.find(item => item.name === contractName)?.abi : undefined)

  if (targetAbi) {
    const parsedStorage = tryDecodeStorageCellWithAbi(dataCell, targetAbi)
    if (parsedStorage) {
      return parsedStorage
    }
  }

  return undefined
}

export const applyParsedBodies = (
  transactions: TransactionInfo[],
  backendContracts: Record<string, BackendContractInfo>,
): TransactionInfo[] => {
  const fallbackAbis = Object.values(backendContracts)
    .map(contract => contract.abi)
    .filter((abi): abi is ContractABI => abi !== undefined)

  for (const tx of transactions) {
    tx.parsedBody = undefined
    tx.parsedStorageBefore = undefined
    tx.parsedStorageAfter = undefined

    const targetAbi = tx.contractName ? backendContracts[tx.contractName]?.abi : undefined

    if (targetAbi) {
      tx.parsedBody = tryDecodeTransactionBodyWithAbi(tx, targetAbi, fallbackAbis)
      tx.parsedStorageBefore = tryDecodeStorageWithAbi(tx.shardAccountBefore, targetAbi)
      tx.parsedStorageAfter = tryDecodeStorageWithAbi(tx.shardAccountAfter, targetAbi)
      if (tx.parsedBody) {
        continue
      }
    }

    for (const fallbackAbi of fallbackAbis) {
      if (fallbackAbi === targetAbi) {
        continue
      }

      tx.parsedBody = tryDecodeTransactionBodyWithAbi(tx, fallbackAbi, fallbackAbis)
      if (tx.parsedBody) {
        break
      }
    }
  }

  return transactions
}
