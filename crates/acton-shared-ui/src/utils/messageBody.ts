import type {Message, MessageRelaxed} from "@ton/core"
import {Address, Builder, Cell, Dictionary, loadShardAccount, Slice} from "@ton/core"
import type {ContractABI, SymTable, Ty} from "@ton/tolk-abi-to-typescript"
import {
  DynamicCtx,
  renderTy,
  SymTable as CompilerSymTable,
  unpackFromSliceDynamic,
} from "@ton/tolk-abi-to-typescript"

import type {BackendContractInfo} from "@/types"
import type {
  ContractData,
  ParsedContractStorage,
  ParsedTransactionBody,
  ParsedValue,
  TransactionInfo,
} from "@/types/transaction"

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

const isCellWrapperObject = (value: Record<string, unknown>): value is {ref: unknown} => {
  const keys = Object.keys(value)
  return (
    (keys.length === 1 && keys[0] === "ref") ||
    (value.$ === "Cell" && keys.length === 2 && keys.includes("$") && keys.includes("ref"))
  )
}

const HEX_PREVIEW_HEAD_LENGTH = 24
const HEX_PREVIEW_TAIL_LENGTH = 8

const formatHexPreview = (hex: string): string => {
  if (hex.length <= HEX_PREVIEW_HEAD_LENGTH + HEX_PREVIEW_TAIL_LENGTH) {
    return hex
  }

  return `${hex.slice(0, HEX_PREVIEW_HEAD_LENGTH)}…${hex.slice(-HEX_PREVIEW_TAIL_LENGTH)}`
}

const formatSerializedCellPreview = (
  typeName: "Cell" | "Slice" | "Builder",
  cell: Cell,
): string => {
  const hex = cell.toBoc({idx: false, crc32: false}).toString("hex")
  return `${typeName}(${formatHexPreview(hex)})`
}

const toSerializedCellScalar = (
  typeName: "Cell" | "Slice" | "Builder",
  cell: Cell,
): ParsedValue => ({
  kind: "scalar",
  value: formatSerializedCellPreview(typeName, cell),
  rawValue: cell.toBoc({idx: false, crc32: false}).toString("hex"),
})

interface ParsedValueTypeContext {
  readonly symbols: SymTable
  readonly tyIdx: number
}

function renderTypeName(context: ParsedValueTypeContext | undefined): string | undefined {
  if (!context) {
    return undefined
  }

  try {
    return renderTy(context.symbols, context.tyIdx)
  } catch {
    return undefined
  }
}

function tryGetTy(symbols: SymTable, tyIdx: number): Ty | undefined {
  try {
    return symbols.tyByIdx(tyIdx)
  } catch {
    return undefined
  }
}

function toParsedValueWithType(
  value: unknown,
  context: ParsedValueTypeContext,
): ParsedValue | undefined {
  const ty = tryGetTy(context.symbols, context.tyIdx)
  if (!ty) {
    return undefined
  }

  switch (ty.kind) {
    case "nullable": {
      return value === null
        ? {kind: "null"}
        : toParsedValue(value, {symbols: context.symbols, tyIdx: ty.inner_ty_idx})
    }
    case "cellOf": {
      if (typeof value !== "object" || value === null || !("ref" in value)) {
        return undefined
      }

      return toParsedValue((value as {readonly ref: unknown}).ref, {
        symbols: context.symbols,
        tyIdx: ty.inner_ty_idx,
      })
    }
    case "arrayOf":
    case "lispListOf": {
      if (!Array.isArray(value)) {
        return undefined
      }

      return {
        kind: "array",
        items: value.map(item =>
          toParsedValue(item, {symbols: context.symbols, tyIdx: ty.inner_ty_idx}),
        ),
      }
    }
    case "tensor":
    case "shapedTuple": {
      if (!Array.isArray(value)) {
        return undefined
      }

      return {
        kind: "array",
        items: value.map((item, index) =>
          toParsedValue(item, {
            symbols: context.symbols,
            tyIdx: ty.items_ty_idx[index] ?? context.tyIdx,
          }),
        ),
      }
    }
    case "mapKV": {
      if (!(value instanceof Dictionary)) {
        return undefined
      }

      return {
        kind: "map",
        typeName: renderTy(context.symbols, context.tyIdx),
        entries: [...value].map(([key, itemValue]) => ({
          key: toParsedValue(key, {symbols: context.symbols, tyIdx: ty.key_ty_idx}),
          value: toParsedValue(itemValue, {symbols: context.symbols, tyIdx: ty.value_ty_idx}),
        })),
      }
    }
    case "StructRef": {
      const structRef = context.symbols.getStruct(ty.struct_name)
      if (structRef.custom_pack_unpack?.unpack_from_slice) {
        return undefined
      }

      if (typeof value !== "object" || value === null) {
        return undefined
      }

      const objectValue = value as Record<string, unknown>
      return {
        kind: "object",
        typeName: renderTy(context.symbols, context.tyIdx),
        entries: context.symbols.structFieldsOf(context.tyIdx, false).map(field => ({
          key: field.name,
          value: toParsedValue(objectValue[field.name], {
            symbols: context.symbols,
            tyIdx: field.ty_idx,
          }),
        })),
      }
    }
    case "AliasRef": {
      const aliasRef = context.symbols.getAlias(ty.alias_name)
      if (aliasRef.custom_pack_unpack?.unpack_from_slice) {
        return undefined
      }

      const target = context.symbols.aliasTargetOf(context.tyIdx)
      return toParsedValue(value, {symbols: context.symbols, tyIdx: target.ty_idx})
    }
    default: {
      return undefined
    }
  }
}

const toParsedValue = (value: unknown, typeContext?: ParsedValueTypeContext): ParsedValue => {
  let typedValue: ParsedValue | undefined
  if (typeContext) {
    try {
      typedValue = toParsedValueWithType(value, typeContext)
    } catch {
      typedValue = undefined
    }
  }

  if (typedValue) {
    return typedValue
  }

  if (value === null) {
    return {kind: "null"}
  }

  if (value === undefined) {
    return {kind: "void"}
  }

  if (typeof value === "boolean") {
    return {kind: "boolean", value}
  }

  if (typeof value === "bigint" || typeof value === "number" || typeof value === "string") {
    return {kind: "scalar", value: value.toString(), typeName: renderTypeName(typeContext)}
  }

  if (value instanceof Address) {
    return {kind: "address", value: value.toString()}
  }

  if (value instanceof Cell) {
    return toSerializedCellScalar("Cell", value)
  }

  if (value instanceof Slice) {
    return toSerializedCellScalar("Slice", value.asCell())
  }

  if (value instanceof Builder) {
    return toSerializedCellScalar("Builder", value.asCell())
  }

  if (value instanceof Dictionary) {
    return {
      kind: "map",
      entries: [...value].map(([key, itemValue]) => ({
        key: toParsedValue(key),
        value: toParsedValue(itemValue),
      })),
    }
  }

  if (Array.isArray(value)) {
    return {
      kind: "array",
      items: value.map(item => toParsedValue(item)),
    }
  }

  if (typeof value === "object") {
    const objectValue = value as Record<string, unknown>
    if (isCellWrapperObject(objectValue)) {
      return toParsedValue(objectValue.ref)
    }

    const typeName = typeof objectValue.$ === "string" ? objectValue.$ : undefined
    if (
      typeName === "void" &&
      Object.prototype.hasOwnProperty.call(objectValue, "value") &&
      objectValue.value === undefined
    ) {
      return {kind: "void"}
    }

    return {
      kind: "object",
      typeName,
      entries: Object.entries(objectValue)
        .filter(([key]) => key !== "$")
        .map(([key, itemValue]) => ({
          key,
          value: toParsedValue(itemValue),
        })),
    }
  }

  return {kind: "scalar", value: Object.prototype.toString.call(value)}
}

const createBodyParser = (message: ParsableMessage): Slice | undefined => {
  const parser = message.body.asSlice()
  if (message.info.type !== "internal" || !message.info.bounced) {
    return parser
  }

  if (parser.remainingBits < 32) {
    return undefined
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

export const getMessageOpcode = (message: ParsableMessage): number | undefined => {
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

  return toSerializedCellScalar("Slice", slice.asCell())
}

const tryDecodeTextCommentBody = (message: ParsableMessage): ParsedTransactionBody | undefined => {
  const baseSlice = createBodyParser(message)
  if (!baseSlice || baseSlice.remainingBits < 32) {
    return undefined
  }

  const parser = baseSlice.clone()
  if (parser.loadUint(32) !== 0) {
    return undefined
  }

  return {
    name: "Text Comment",
    value: {
      kind: "object",
      typeName: "Text Comment",
      entries: [{key: "text", value: textCommentTailValue(parser)}],
    },
  }
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

const createBouncedOpcodeBody = (name: string, body: Slice): ParsedTransactionBody => ({
  name,
  value: {
    kind: "object",
    typeName: name,
    entries: [{key: "body", value: toSerializedCellScalar("Slice", body.asCell())}],
  },
})

const tryDecodeMessageWithCandidates = (
  message: ParsableMessage,
  abi: ContractABI,
  candidates: readonly MessageCandidate[],
): ParsedTransactionBody | undefined => {
  if (candidates.length === 0) {
    return undefined
  }

  const baseSlice = createBodyParser(message)
  if (!baseSlice) {
    return undefined
  }

  const ctx = new DynamicCtx(abi)
  const bouncedInternal = isBouncedInternalMessage(message)
  const opcode = bouncedInternal ? getOpcodeAfterBouncePrefix(message) : undefined
  const skipGenericBouncedDecode = bouncedInternal && opcode !== undefined
  const bouncedOpcodeName = bouncedInternal
    ? candidates
        .map(candidate => resolveCandidateOpcodeName(abi, ctx.symbols, candidate, opcode))
        .find(name => name !== undefined)
    : undefined

  for (const candidate of candidates) {
    const parser = baseSlice.clone()
    const candidateOpcodeName = skipGenericBouncedDecode
      ? resolveCandidateOpcodeName(abi, ctx.symbols, candidate, opcode)
      : undefined
    try {
      const decoded: unknown = unpackFromSliceDynamic(ctx, candidate.body_ty_idx, parser) as unknown
      if (!hasAcceptableMessageDecodeRemainder(baseSlice, parser)) {
        continue
      }

      const parsedBody = {
        name: getBodyTypeName(ctx.symbols, candidate.body_ty_idx),
        value: toParsedValue(decoded, {symbols: ctx.symbols, tyIdx: candidate.body_ty_idx}),
      }

      if (skipGenericBouncedDecode && !candidateOpcodeName) {
        continue
      }

      return parsedBody
    } catch {
      continue
    }
  }

  if (bouncedOpcodeName) {
    return createBouncedOpcodeBody(bouncedOpcodeName, baseSlice)
  }

  return undefined
}

const tryDecodeIncomingMessageWithAbi = (
  message: ParsableMessage,
  abi: ContractABI,
): ParsedTransactionBody | undefined => {
  const opcode = getOpcodeAfterBouncePrefix(message)
  const candidates = getIncomingCandidates(abi, message.info.type === "internal", opcode)
  return tryDecodeMessageWithCandidates(message, abi, candidates)
}

const tryDecodeOutgoingMessageWithAbi = (
  message: ParsableMessage,
  abi: ContractABI,
): ParsedTransactionBody | undefined => {
  const opcode = getOpcodeAfterBouncePrefix(message)
  const candidates = getOutgoingCandidates(abi, opcode)
  return tryDecodeMessageWithCandidates(message, abi, candidates)
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

const getStorageDataSlice = (shardAccountBase64: string): Slice | undefined => {
  const shard = parseShardAccount(shardAccountBase64)
  const state = shard?.account?.storage.state
  if (state?.type !== "active" || !state.state.data) {
    return undefined
  }

  return state.state.data.beginParse()
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
    const parser = baseSlice.clone()
    try {
      const decoded: unknown = unpackFromSliceDynamic(ctx, candidate, parser) as unknown
      if (parser.remainingBits !== 0 || parser.remainingRefs !== 0) {
        continue
      }

      return {
        name: getBodyTypeName(ctx.symbols, candidate),
        value: toParsedValue(decoded, {symbols: ctx.symbols, tyIdx: candidate}),
      }
    } catch {
      continue
    }
  }

  return undefined
}

const tryDecodeStorageWithAbi = (
  shardAccountBase64: string,
  abi: ContractABI,
): ParsedContractStorage | undefined => {
  const baseSlice = getStorageDataSlice(shardAccountBase64)
  if (!baseSlice) {
    return undefined
  }

  return tryDecodeStorageSliceWithAbi(baseSlice, abi)
}

const tryDecodeStorageCellWithAbi = (
  dataCell: Cell,
  abi: ContractABI,
): ParsedContractStorage | undefined => {
  return tryDecodeStorageSliceWithAbi(dataCell.beginParse(), abi)
}

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
): string | undefined => {
  const opcode = getMessageOpcode(message)
  if (opcode === undefined) {
    return undefined
  }
  if (opcode === 0) {
    return "Text Comment"
  }

  const destinationContract =
    message.info.type === "internal" ? contracts.get(message.info.dest.toString()) : undefined
  const sourceContract = sourceAddress ? contracts.get(sourceAddress) : undefined
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
): ParsedTransactionBody | undefined => {
  const textCommentBody = tryDecodeTextCommentBody(message)
  if (textCommentBody) {
    return textCommentBody
  }

  const sourceContract = sourceAddress ? contracts.get(sourceAddress) : undefined
  const destinationContract =
    message.info.type === "internal" ? contracts.get(message.info.dest.toString()) : undefined
  const allContracts = [...contracts.values()]

  if (message.info.type === "internal") {
    if (message.info.bounced) {
      for (const contract of [destinationContract, sourceContract, ...allContracts]) {
        const abi = contract?.abi
        if (!abi) {
          continue
        }

        const parsedBody = tryDecodeOutgoingMessageWithAbi(message, abi)
        if (parsedBody) {
          return parsedBody
        }
      }

      for (const contract of [sourceContract, destinationContract, ...allContracts]) {
        const abi = contract?.abi
        if (!abi) {
          continue
        }

        const parsedBody = tryDecodeIncomingMessageWithAbi(message, abi)
        if (parsedBody) {
          return parsedBody
        }
      }

      return undefined
    }

    for (const contract of [destinationContract, ...allContracts]) {
      const abi = contract?.abi
      if (!abi) {
        continue
      }

      const parsedBody = tryDecodeIncomingMessageWithAbi(message, abi)
      if (parsedBody) {
        return parsedBody
      }
    }

    for (const contract of [sourceContract, ...allContracts]) {
      const abi = contract?.abi
      if (!abi) {
        continue
      }

      const parsedBody = tryDecodeOutgoingMessageWithAbi(message, abi)
      if (parsedBody) {
        return parsedBody
      }
    }

    return undefined
  }

  if (message.info.type === "external-out") {
    for (const contract of [sourceContract, ...allContracts]) {
      const abi = contract?.abi
      if (!abi) {
        continue
      }

      const parsedBody = tryDecodeOutgoingMessageWithAbi(message, abi)
      if (parsedBody) {
        return parsedBody
      }
    }

    return undefined
  }

  for (const contract of allContracts) {
    const abi = contract.abi
    if (!abi) {
      continue
    }

    const parsedBody = tryDecodeIncomingMessageWithAbi(message, abi)
    if (parsedBody) {
      return parsedBody
    }
  }

  return undefined
}

const tryDecodeTransactionBodyWithAbi = (
  tx: TransactionInfo,
  abi: ContractABI,
): ParsedTransactionBody | undefined => {
  const inMessage = tx.transaction.inMessage
  if (!inMessage) {
    return undefined
  }

  const textCommentBody = tryDecodeTextCommentBody(inMessage)
  if (textCommentBody) {
    return textCommentBody
  }

  if (inMessage.info.type === "internal" && inMessage.info.bounced) {
    return (
      tryDecodeOutgoingMessageWithAbi(inMessage, abi) ??
      tryDecodeIncomingMessageWithAbi(inMessage, abi)
    )
  }

  return tryDecodeIncomingMessageWithAbi(inMessage, abi)
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
      tx.parsedBody = tryDecodeTransactionBodyWithAbi(tx, targetAbi)
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

      tx.parsedBody = tryDecodeTransactionBodyWithAbi(tx, fallbackAbi)
      if (tx.parsedBody) {
        break
      }
    }
  }

  return transactions
}
