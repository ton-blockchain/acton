import {Address, Builder, Cell, Dictionary, Slice, loadShardAccount} from "@ton/core"
import type {Message, MessageRelaxed} from "@ton/core"
import type {ContractABI, Ty} from "gen-typescript-from-tolk-dev"
import {DynamicCtx, renderTy, unpackFromSliceDynamic} from "gen-typescript-from-tolk-dev"

import type {BackendContractInfo} from "@/types"
import type {
  ContractData,
  ParsedContractStorage,
  ParsedTransactionBody,
  ParsedValue,
  TransactionInfo,
} from "@/types/transaction"

interface MessageCandidate {
  readonly body_ty: Ty
}

interface DeclarationCandidate {
  readonly body_ty: Ty
  readonly priority: number
}

interface ParsableMessage {
  readonly info: Message["info"] | MessageRelaxed["info"]
  readonly body: Cell
}

const BOUNCED_BODY_PREFIX = 0xff_ff_ff_ff
const RICH_BOUNCE_BODY_PREFIX = 0xff_ff_ff_fe

const getContractCompilerAbi = (contract: ContractData | undefined): ContractABI | undefined => {
  const compilerAbi = contract?.compilerAbi
  return compilerAbi ? (compilerAbi as ContractABI) : undefined
}

const getBackendCompilerAbi = (
  contract: BackendContractInfo | undefined,
): ContractABI | undefined => {
  const compilerAbi = contract?.compiler_abi
  return compilerAbi ? (compilerAbi as ContractABI) : undefined
}

const getBodyTypeName = (bodyTy: Ty): string => {
  switch (bodyTy.kind) {
    case "StructRef": {
      return bodyTy.struct_name
    }
    case "AliasRef": {
      return bodyTy.alias_name
    }
    case "EnumRef": {
      return bodyTy.enum_name
    }
    default: {
      return renderTy(bodyTy)
    }
  }
}

const getBodyTypeKey = (bodyTy: Ty): string => {
  switch (bodyTy.kind) {
    case "StructRef": {
      return `StructRef:${bodyTy.struct_name}`
    }
    case "AliasRef": {
      return `AliasRef:${bodyTy.alias_name}`
    }
    case "EnumRef": {
      return `EnumRef:${bodyTy.enum_name}`
    }
    default: {
      return renderTy(bodyTy)
    }
  }
}

const parsePrefixNumber = (prefixStr: string): number | undefined => {
  try {
    return Number(BigInt(prefixStr))
  } catch {
    return undefined
  }
}

const getDeclarationCandidates = (
  compilerAbi: ContractABI,
  opcode: number | undefined,
): DeclarationCandidate[] => {
  const candidates: DeclarationCandidate[] = []

  for (const declaration of compilerAbi.declarations) {
    switch (declaration.kind) {
      case "struct": {
        if (declaration.type_params && declaration.type_params.length > 0) {
          continue
        }

        const matchesOpcode =
          opcode !== undefined &&
          declaration.prefix?.prefix_len === 32 &&
          parsePrefixNumber(declaration.prefix.prefix_str) === opcode

        candidates.push({
          body_ty: {kind: "StructRef", struct_name: declaration.name},
          priority: matchesOpcode ? 0 : declaration.prefix ? 1 : 2,
        })
        break
      }
      case "alias": {
        if (declaration.type_params && declaration.type_params.length > 0) {
          continue
        }

        candidates.push({
          body_ty: {kind: "AliasRef", alias_name: declaration.name},
          priority: 3,
        })
        break
      }
      case "enum": {
        candidates.push({
          body_ty: {kind: "EnumRef", enum_name: declaration.name},
          priority: 4,
        })
        break
      }
    }
  }

  return candidates.sort((left, right) => left.priority - right.priority)
}

const getIncomingCandidates = (
  compilerAbi: ContractABI,
  isInternal: boolean,
  opcode: number | undefined,
): readonly MessageCandidate[] => {
  const directCandidates = isInternal
    ? compilerAbi.incoming_messages
    : compilerAbi.incoming_external
  if (!isInternal) {
    return directCandidates
  }

  const deduped = new Map<string, MessageCandidate>()
  for (const candidate of directCandidates) {
    deduped.set(getBodyTypeKey(candidate.body_ty), candidate)
  }

  for (const candidate of getDeclarationCandidates(compilerAbi, opcode)) {
    const key = getBodyTypeKey(candidate.body_ty)
    if (!deduped.has(key)) {
      deduped.set(key, {body_ty: candidate.body_ty})
    }
  }

  return [...deduped.values()]
}

const getOutgoingCandidates = (
  compilerAbi: ContractABI,
  opcode: number | undefined,
): readonly MessageCandidate[] => {
  const deduped = new Map<string, MessageCandidate>()

  for (const candidate of compilerAbi.outgoing_messages) {
    deduped.set(getBodyTypeKey(candidate.body_ty), candidate)
  }

  for (const candidate of getDeclarationCandidates(compilerAbi, opcode)) {
    const key = getBodyTypeKey(candidate.body_ty)
    if (!deduped.has(key)) {
      deduped.set(key, {body_ty: candidate.body_ty})
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

const toParsedValue = (value: unknown): ParsedValue => {
  if (value === null) {
    return {kind: "null"}
  }

  if (value === undefined) {
    return {kind: "scalar", value: "undefined"}
  }

  if (typeof value === "boolean") {
    return {kind: "boolean", value}
  }

  if (typeof value === "bigint" || typeof value === "number" || typeof value === "string") {
    return {kind: "scalar", value: value.toString()}
  }

  if (value instanceof Address) {
    return {kind: "address", value: value.toString()}
  }

  if (value instanceof Cell) {
    return {kind: "scalar", value: formatSerializedCellPreview("Cell", value)}
  }

  if (value instanceof Slice) {
    return {kind: "scalar", value: formatSerializedCellPreview("Slice", value.asCell())}
  }

  if (value instanceof Builder) {
    return {kind: "scalar", value: formatSerializedCellPreview("Builder", value.asCell())}
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

const tryDecodeMessageWithCandidates = (
  message: ParsableMessage,
  compilerAbi: ContractABI,
  candidates: readonly MessageCandidate[],
): ParsedTransactionBody | undefined => {
  if (candidates.length === 0) {
    return undefined
  }

  const baseSlice = createBodyParser(message)
  if (!baseSlice) {
    return undefined
  }

  const ctx = new DynamicCtx(compilerAbi)

  for (const candidate of candidates) {
    const parser = baseSlice.clone()
    try {
      const decoded: unknown = unpackFromSliceDynamic(ctx, candidate.body_ty, parser) as unknown
      if (parser.remainingBits !== 0 || parser.remainingRefs !== 0) {
        continue
      }

      return {
        name: getBodyTypeName(candidate.body_ty),
        value: toParsedValue(decoded),
      }
    } catch {
      continue
    }
  }

  return undefined
}

const tryDecodeIncomingMessageWithAbi = (
  message: ParsableMessage,
  compilerAbi: ContractABI,
): ParsedTransactionBody | undefined => {
  const opcode = getOpcodeAfterBouncePrefix(message)
  const candidates = getIncomingCandidates(compilerAbi, message.info.type === "internal", opcode)
  return tryDecodeMessageWithCandidates(message, compilerAbi, candidates)
}

const tryDecodeOutgoingMessageWithAbi = (
  message: ParsableMessage,
  compilerAbi: ContractABI,
): ParsedTransactionBody | undefined => {
  const opcode = getOpcodeAfterBouncePrefix(message)
  const candidates = getOutgoingCandidates(compilerAbi, opcode)
  return tryDecodeMessageWithCandidates(message, compilerAbi, candidates)
}

const getStorageCandidates = (compilerAbi: ContractABI): readonly Ty[] => {
  const candidates = [compilerAbi.storage.storage_ty, compilerAbi.storage.storage_at_deployment_ty]
    .filter((ty): ty is Ty => ty !== undefined && ty.kind !== "nullLiteral")
    .map(ty => [getBodyTypeKey(ty), ty] as const)

  return [...new Map(candidates).values()]
}

const getStorageDataSlice = (shardAccountBase64: string): Slice | undefined => {
  try {
    const shard = loadShardAccount(Cell.fromBase64(shardAccountBase64).beginParse())
    const state = shard.account?.storage.state
    if (state?.type !== "active" || !state.state.data) {
      return undefined
    }

    return state.state.data.beginParse()
  } catch {
    return undefined
  }
}

const tryDecodeStorageSliceWithAbi = (
  baseSlice: Slice,
  compilerAbi: ContractABI,
): ParsedContractStorage | undefined => {
  const candidates = getStorageCandidates(compilerAbi)
  if (candidates.length === 0) {
    return undefined
  }

  const ctx = new DynamicCtx(compilerAbi)

  for (const candidate of candidates) {
    const parser = baseSlice.clone()
    try {
      const decoded: unknown = unpackFromSliceDynamic(ctx, candidate, parser) as unknown
      if (parser.remainingBits !== 0 || parser.remainingRefs !== 0) {
        continue
      }

      return {
        name: getBodyTypeName(candidate),
        value: toParsedValue(decoded),
      }
    } catch {
      continue
    }
  }

  return undefined
}

const tryDecodeStorageWithAbi = (
  shardAccountBase64: string,
  compilerAbi: ContractABI,
): ParsedContractStorage | undefined => {
  const baseSlice = getStorageDataSlice(shardAccountBase64)
  if (!baseSlice) {
    return undefined
  }

  return tryDecodeStorageSliceWithAbi(baseSlice, compilerAbi)
}

const tryDecodeStorageCellWithAbi = (
  dataCell: Cell,
  compilerAbi: ContractABI,
): ParsedContractStorage | undefined => {
  return tryDecodeStorageSliceWithAbi(dataCell.beginParse(), compilerAbi)
}

const findOpcodeNameInMessages = (
  opcode: number,
  messages: readonly {readonly opcode: number | undefined; readonly name: string}[] | undefined,
): string | undefined => {
  return messages?.find(message => message.opcode === opcode)?.name
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

  const destinationContract =
    message.info.type === "internal" ? contracts.get(message.info.dest.toString()) : undefined
  const sourceContract = sourceAddress ? contracts.get(sourceAddress) : undefined
  const isBouncedInternal = message.info.type === "internal" && message.info.bounced

  if (isBouncedInternal) {
    return (
      destinationContract?.outgoingMessageNamesByOpcode?.get(opcode) ??
      sourceContract?.incomingMessageNamesByOpcode?.get(opcode) ??
      findOpcodeNameInMessages(opcode, destinationContract?.abi?.messages) ??
      findOpcodeNameInMessages(opcode, sourceContract?.abi?.messages) ??
      [...contracts.values()]
        .map(contract => findOpcodeNameInMessages(opcode, contract.abi?.messages))
        .find(name => name !== undefined)
    )
  }

  return (
    destinationContract?.incomingMessageNamesByOpcode?.get(opcode) ??
    sourceContract?.outgoingMessageNamesByOpcode?.get(opcode) ??
    findOpcodeNameInMessages(opcode, destinationContract?.abi?.messages) ??
    findOpcodeNameInMessages(opcode, sourceContract?.abi?.messages) ??
    [...contracts.values()]
      .map(contract => findOpcodeNameInMessages(opcode, contract.abi?.messages))
      .find(name => name !== undefined)
  )
}

export const decodeMessageBody = (
  message: ParsableMessage,
  contracts: Map<string, ContractData>,
  sourceAddress?: string,
): ParsedTransactionBody | undefined => {
  const sourceContract = sourceAddress ? contracts.get(sourceAddress) : undefined
  const destinationContract =
    message.info.type === "internal" ? contracts.get(message.info.dest.toString()) : undefined
  const allContracts = [...contracts.values()]

  if (message.info.type === "internal") {
    if (message.info.bounced) {
      for (const contract of [destinationContract, sourceContract, ...allContracts]) {
        const compilerAbi = getContractCompilerAbi(contract)
        if (!compilerAbi) {
          continue
        }

        const parsedBody = tryDecodeOutgoingMessageWithAbi(message, compilerAbi)
        if (parsedBody) {
          return parsedBody
        }
      }

      for (const contract of [sourceContract, destinationContract, ...allContracts]) {
        const compilerAbi = getContractCompilerAbi(contract)
        if (!compilerAbi) {
          continue
        }

        const parsedBody = tryDecodeIncomingMessageWithAbi(message, compilerAbi)
        if (parsedBody) {
          return parsedBody
        }
      }

      return undefined
    }

    for (const contract of [destinationContract, ...allContracts]) {
      const compilerAbi = getContractCompilerAbi(contract)
      if (!compilerAbi) {
        continue
      }

      const parsedBody = tryDecodeIncomingMessageWithAbi(message, compilerAbi)
      if (parsedBody) {
        return parsedBody
      }
    }

    for (const contract of [sourceContract, ...allContracts]) {
      const compilerAbi = getContractCompilerAbi(contract)
      if (!compilerAbi) {
        continue
      }

      const parsedBody = tryDecodeOutgoingMessageWithAbi(message, compilerAbi)
      if (parsedBody) {
        return parsedBody
      }
    }

    return undefined
  }

  if (message.info.type === "external-out") {
    for (const contract of [sourceContract, ...allContracts]) {
      const compilerAbi = getContractCompilerAbi(contract)
      if (!compilerAbi) {
        continue
      }

      const parsedBody = tryDecodeOutgoingMessageWithAbi(message, compilerAbi)
      if (parsedBody) {
        return parsedBody
      }
    }

    return undefined
  }

  for (const contract of allContracts) {
    const compilerAbi = getContractCompilerAbi(contract)
    if (!compilerAbi) {
      continue
    }

    const parsedBody = tryDecodeIncomingMessageWithAbi(message, compilerAbi)
    if (parsedBody) {
      return parsedBody
    }
  }

  return undefined
}

const tryDecodeTransactionBodyWithAbi = (
  tx: TransactionInfo,
  compilerAbi: ContractABI,
): ParsedTransactionBody | undefined => {
  const inMessage = tx.transaction.inMessage
  if (!inMessage) {
    return undefined
  }

  if (inMessage.info.type === "internal" && inMessage.info.bounced) {
    return (
      tryDecodeOutgoingMessageWithAbi(inMessage, compilerAbi) ??
      tryDecodeIncomingMessageWithAbi(inMessage, compilerAbi)
    )
  }

  return tryDecodeIncomingMessageWithAbi(inMessage, compilerAbi)
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
    getContractCompilerAbi(contract) ??
    (contractName
      ? getBackendCompilerAbi(allContracts.find(item => item.name === contractName))
      : undefined)

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
    .map(contract => getBackendCompilerAbi(contract))
    .filter((compilerAbi): compilerAbi is ContractABI => compilerAbi !== undefined)

  for (const tx of transactions) {
    tx.parsedBody = undefined
    tx.parsedStorageBefore = undefined
    tx.parsedStorageAfter = undefined

    const targetAbi = tx.contractName
      ? getBackendCompilerAbi(backendContracts[tx.contractName])
      : undefined

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
