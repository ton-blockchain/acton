import {Address, Builder, Cell, Dictionary, loadShardAccount, Slice} from "@ton/core"
import type {
  BackendContractInfo,
  ParsedContractStorage,
  ParsedTransactionBody,
  ParsedValue,
  TransactionInfo,
} from "@acton/shared-ui"
import type {ContractABI} from "gen-typescript-from-tolk-dev/src/abi"
import type {Ty} from "gen-typescript-from-tolk-dev/src/abi-types"
import {DynamicCtx} from "gen-typescript-from-tolk-dev/src/dynamic-ctx"
import {unpackFromSliceDynamic} from "gen-typescript-from-tolk-dev/src/dynamic-serialization"
import {renderTy} from "gen-typescript-from-tolk-dev/src/types-kernel"

interface MessageCandidate {
  readonly bodyTy: Ty
}

interface DeclarationCandidate {
  readonly bodyTy: Ty
  readonly priority: number
}

const getCompilerAbi = (contract: BackendContractInfo | undefined): ContractABI | undefined => {
  const compilerAbi = contract?.compiler_abi
  return compilerAbi ? (compilerAbi as ContractABI) : undefined
}

const getBodyTypeName = (bodyTy: Ty): string => {
  switch (bodyTy.kind) {
    case "StructRef": {
      return bodyTy.structName
    }
    case "AliasRef": {
      return bodyTy.aliasName
    }
    case "EnumRef": {
      return bodyTy.enumName
    }
    default: {
      return renderTy(bodyTy)
    }
  }
}

const getBodyTypeKey = (bodyTy: Ty): string => {
  switch (bodyTy.kind) {
    case "StructRef": {
      return `StructRef:${bodyTy.structName}`
    }
    case "AliasRef": {
      return `AliasRef:${bodyTy.aliasName}`
    }
    case "EnumRef": {
      return `EnumRef:${bodyTy.enumName}`
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
      case "Struct": {
        if (declaration.typeParams && declaration.typeParams.length > 0) {
          continue
        }

        const matchesOpcode =
          opcode !== undefined &&
          declaration.prefix?.prefixLen === 32 &&
          parsePrefixNumber(declaration.prefix.prefixStr) === opcode

        candidates.push({
          bodyTy: {kind: "StructRef", structName: declaration.name},
          priority: matchesOpcode ? 0 : declaration.prefix ? 1 : 2,
        })
        break
      }
      case "Alias": {
        if (declaration.typeParams && declaration.typeParams.length > 0) {
          continue
        }

        candidates.push({
          bodyTy: {kind: "AliasRef", aliasName: declaration.name},
          priority: 3,
        })
        break
      }
      case "Enum": {
        candidates.push({
          bodyTy: {kind: "EnumRef", enumName: declaration.name},
          priority: 4,
        })
        break
      }
    }
  }

  return candidates.sort((left, right) => left.priority - right.priority)
}

const getCandidateMessages = (
  compilerAbi: ContractABI,
  isInternal: boolean,
  opcode: number | undefined,
): readonly MessageCandidate[] => {
  const directCandidates = isInternal ? compilerAbi.incomingMessages : compilerAbi.incomingExternal
  if (!isInternal) {
    return directCandidates
  }

  const deduped = new Map<string, MessageCandidate>()
  for (const candidate of directCandidates) {
    deduped.set(getBodyTypeKey(candidate.bodyTy), candidate)
  }

  for (const candidate of getDeclarationCandidates(compilerAbi, opcode)) {
    const key = getBodyTypeKey(candidate.bodyTy)
    if (!deduped.has(key)) {
      deduped.set(key, {bodyTy: candidate.bodyTy})
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
    return `0x${hex}`
  }

  return `0x${hex.slice(0, HEX_PREVIEW_HEAD_LENGTH)}…${hex.slice(-HEX_PREVIEW_TAIL_LENGTH)}`
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

const tryDecodeWithAbi = (
  tx: TransactionInfo,
  compilerAbi: ContractABI,
): ParsedTransactionBody | undefined => {
  const inMessage = tx.transaction.inMessage
  if (!inMessage) {
    return undefined
  }

  const baseSlice = inMessage.body.asSlice()
  const isInternal = inMessage.info.type === "internal"
  const opcodeSlice = baseSlice.clone()
  if (inMessage.info.type === "internal" && inMessage.info.bounced) {
    if (opcodeSlice.remainingBits < 32) {
      return undefined
    }
    opcodeSlice.loadUint(32)
  }

  const opcode =
    isInternal && opcodeSlice.remainingBits >= 32 ? Number(opcodeSlice.preloadUint(32)) : undefined
  const candidates = getCandidateMessages(compilerAbi, isInternal, opcode)
  if (candidates.length === 0) {
    return undefined
  }

  if (inMessage.info.type === "internal" && inMessage.info.bounced) {
    if (baseSlice.remainingBits < 32) {
      return undefined
    }
    baseSlice.loadUint(32)
  }

  const ctx = new DynamicCtx(compilerAbi)

  for (const candidate of candidates) {
    const parser = baseSlice.clone()
    try {
      const decoded: unknown = unpackFromSliceDynamic(ctx, candidate.bodyTy, parser) as unknown
      if (parser.remainingBits !== 0 || parser.remainingRefs !== 0) {
        continue
      }

      return {
        name: getBodyTypeName(candidate.bodyTy),
        value: toParsedValue(decoded),
      }
    } catch {
      continue
    }
  }

  return undefined
}

const getStorageCandidates = (compilerAbi: ContractABI): readonly Ty[] => {
  const candidates = [compilerAbi.storage.storageTy, compilerAbi.storage.storageAtDeploymentTy]
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

const tryDecodeStorageWithAbi = (
  shardAccountBase64: string,
  compilerAbi: ContractABI,
): ParsedContractStorage | undefined => {
  const baseSlice = getStorageDataSlice(shardAccountBase64)
  if (!baseSlice) {
    return undefined
  }

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

export const applyParsedBodies = (
  transactions: TransactionInfo[],
  backendContracts: Record<string, BackendContractInfo>,
): TransactionInfo[] => {
  const fallbackAbis = Object.values(backendContracts)
    .map(contract => getCompilerAbi(contract))
    .filter((compilerAbi): compilerAbi is ContractABI => compilerAbi !== undefined)

  for (const tx of transactions) {
    tx.parsedBody = undefined
    tx.parsedStorageBefore = undefined
    tx.parsedStorageAfter = undefined

    const targetAbi = tx.contractName
      ? getCompilerAbi(backendContracts[tx.contractName])
      : undefined
    if (targetAbi) {
      tx.parsedBody = tryDecodeWithAbi(tx, targetAbi)
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

      tx.parsedBody = tryDecodeWithAbi(tx, fallbackAbi)
      if (tx.parsedBody) {
        break
      }
    }
  }

  return transactions
}
