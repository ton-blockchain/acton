import {Address, beginCell, external, storeMessage, type Message} from "@ton/core"
import {parseGramAmount} from "@acton/ui"
import {
  DynamicCtx,
  packToBuilderDynamic,
  renderTy,
  unpackFromSliceDynamic,
  type SymTable,
  type ContractABI,
  type Ty,
  type UnionVariant,
} from "@ton/tolk-abi-to-typescript"

import {
  abiValueToFormValue,
  createAbiSymbols,
  normalizeAbiDynamicArg,
  sampleAbiValueForTy,
  stringifyAbiJson,
} from "./abiValue"

export type AbiMessageTransport = "external" | "internal"
export type AbiMessageDirection = "incoming" | "outgoing"

export interface AbiMessageBuilderOption {
  readonly id: string
  readonly transport: AbiMessageTransport
  readonly label: string
  readonly typeLabel: string
  readonly bodyTyIdx: number
  readonly valueTyIdx: number
  readonly sampleJson: string
  readonly union?: {
    readonly label: string
    readonly hasValueField: boolean
  }
}

export interface BuildAbiMessageBocOptions {
  readonly abi: ContractABI
  readonly option: AbiMessageBuilderOption
  readonly destination: string
  readonly source?: string
  readonly value?: string
  readonly bounce?: boolean
  readonly argsJson: string
}

export interface BuildEmptyMessageBocOptions {
  readonly transport: AbiMessageTransport
  readonly destination: string
  readonly source?: string
  readonly value?: string
  readonly bounce?: boolean
}

export interface DecodedAbiMessageBuilderDraft {
  readonly option: AbiMessageBuilderOption
  readonly argsJson: string
}

export const abiMessageBuilderOptionMatchesName = (
  option: AbiMessageBuilderOption,
  messageName: string,
): boolean => option.label === messageName || option.typeLabel === messageName

export function listAbiMessageBuilderOptions(
  abi: ContractABI,
  transport: AbiMessageTransport,
  direction: AbiMessageDirection = "incoming",
): readonly AbiMessageBuilderOption[] {
  const symbols = createAbiMessageSymbols(abi)
  const messages =
    direction === "outgoing"
      ? (abi.outgoing_messages ?? [])
      : transport === "external"
        ? (abi.incoming_external ?? [])
        : (abi.incoming_messages ?? [])

  return messages.flatMap((message, messageIndex) =>
    expandMessageBodyOptions(symbols, transport, direction, messageIndex, message.body_ty_idx),
  )
}

export function createAbiMessageSymbols(abi: ContractABI): SymTable {
  return createAbiSymbols(abi)
}

export function decodeAbiMessageBuilderDraft(
  abi: ContractABI,
  transport: AbiMessageTransport,
  body: Message["body"],
  direction: AbiMessageDirection = "incoming",
  messageName?: string,
): DecodedAbiMessageBuilderDraft | undefined {
  const ctx = new DynamicCtx(abi)

  for (const option of listAbiMessageBuilderOptions(abi, transport, direction)) {
    if (messageName && !abiMessageBuilderOptionMatchesName(option, messageName)) {
      continue
    }

    try {
      const slice = body.beginParse()
      const decoded = unpackFromSliceDynamic(ctx, option.bodyTyIdx, slice)
      if (slice.remainingBits !== 0 || slice.remainingRefs !== 0) {
        continue
      }

      const value = extractBuilderInput(option, decoded)
      if (value === undefined) {
        continue
      }

      return {
        option,
        argsJson: stringifyAbiJson(abiValueToFormValue(value)),
      }
    } catch {
      // A body may legitimately match another ABI message.
    }
  }

  return undefined
}

export function buildAbiMessageBoc({
  abi,
  option,
  destination,
  source,
  value,
  bounce = true,
  argsJson,
}: BuildAbiMessageBocOptions): string {
  const ctx = new DynamicCtx(abi)
  const destinationAddress = Address.parse(destination.trim())
  const input = parseBuilderArgsJson(argsJson)
  const normalizedInput = normalizeAbiDynamicArg(ctx, option.valueTyIdx, input)
  const bodyValue = option.union ? buildUnionInput(option, normalizedInput) : normalizedInput
  const bodyBuilder = beginCell()
  packToBuilderDynamic(ctx, option.bodyTyIdx, bodyValue, bodyBuilder)
  return buildMessageBoc({
    transport: option.transport,
    destinationAddress,
    source,
    value,
    bounce,
    body: bodyBuilder.endCell(),
  })
}

export function buildEmptyMessageBoc({
  transport,
  destination,
  source,
  value,
  bounce = true,
}: BuildEmptyMessageBocOptions): string {
  return buildMessageBoc({
    transport,
    destinationAddress: Address.parse(destination.trim()),
    source,
    value,
    bounce,
    body: beginCell().endCell(),
  })
}

function buildMessageBoc({
  transport,
  destinationAddress,
  source,
  value,
  bounce,
  body,
}: {
  readonly transport: AbiMessageTransport
  readonly destinationAddress: Address
  readonly source?: string
  readonly value?: string
  readonly bounce: boolean
  readonly body: Message["body"]
}): string {
  const message =
    transport === "external"
      ? external({to: destinationAddress, body})
      : buildInternalMessage({
          source: Address.parse(requireField(source, "Source address")),
          destination: destinationAddress,
          value: parseMessageValue(value),
          bounce,
          body,
        })

  return beginCell().store(storeMessage(message)).endCell().toBoc().toString("hex")
}

export function formatAbiMessageOptionSummary(option: AbiMessageBuilderOption): string {
  return option.union ? `${option.label} / ${option.typeLabel}` : option.typeLabel
}

function expandMessageBodyOptions(
  symbols: SymTable,
  transport: AbiMessageTransport,
  direction: AbiMessageDirection,
  messageIndex: number,
  bodyTyIdx: number,
): readonly AbiMessageBuilderOption[] {
  const idPrefix = direction === "incoming" ? transport : `${direction}:${transport}`
  const bodyTy = tryTyByIdx(symbols, bodyTyIdx)
  if (bodyTy?.kind === "union") {
    return createUnionLabels(symbols, bodyTy.variants).map((variant, variantIndex) => {
      const sample = sampleAbiValueForTy(symbols, variant.variant_ty_idx)
      const typeLabel = safeRenderTy(symbols, variant.variant_ty_idx)
      const label = variant.labelStr || typeLabel

      return {
        id: `${idPrefix}:${messageIndex}:${variantIndex}`,
        transport,
        label,
        typeLabel,
        bodyTyIdx,
        valueTyIdx: variant.variant_ty_idx,
        sampleJson: stringifyAbiJson(sample),
        union: {
          label: variant.labelStr,
          hasValueField: variant.hasValueField,
        },
      }
    })
  }

  const label = messageLabel(symbols, bodyTyIdx)
  return [
    {
      id: `${idPrefix}:${messageIndex}`,
      transport,
      label,
      typeLabel: safeRenderTy(symbols, bodyTyIdx),
      bodyTyIdx,
      valueTyIdx: bodyTyIdx,
      sampleJson: stringifyAbiJson(sampleAbiValueForTy(symbols, bodyTyIdx)),
    },
  ]
}

function buildUnionInput(option: AbiMessageBuilderOption, normalizedInput: unknown): unknown {
  const union = option.union
  if (!union) {
    return normalizedInput
  }
  if (union.hasValueField) {
    return {$: union.label, value: normalizedInput}
  }
  if (isRecord(normalizedInput)) {
    return {$: union.label, ...normalizedInput}
  }
  return {$: union.label}
}

function extractBuilderInput(option: AbiMessageBuilderOption, decoded: unknown): unknown {
  const union = option.union
  if (!union) {
    return decoded
  }
  if (!isRecord(decoded) || decoded.$ !== union.label) {
    return undefined
  }
  if (union.hasValueField) {
    return decoded.value
  }

  const {$: _label, ...value} = decoded
  return value
}

function buildInternalMessage({
  source,
  destination,
  value,
  bounce,
  body,
}: {
  readonly source: Address
  readonly destination: Address
  readonly value: bigint
  readonly bounce: boolean
  readonly body: Message["body"]
}): Message {
  return {
    info: {
      type: "internal",
      ihrDisabled: true,
      bounce,
      bounced: false,
      src: source,
      dest: destination,
      value: {coins: value},
      ihrFee: 0n,
      forwardFee: 0n,
      createdLt: 0n,
      createdAt: 0,
    },
    body,
  }
}

function parseBuilderArgsJson(value: string): unknown {
  const trimmed = value.trim()
  return trimmed ? JSON.parse(trimmed) : {}
}

function parseMessageValue(value: string | undefined): bigint {
  const trimmed = value?.trim()
  if (!trimmed) return 0n

  const amount = parseGramAmount(trimmed)
  if (amount === undefined) {
    throw new Error("Message value must be a valid GRAM amount")
  }
  return amount
}

function requireField(value: string | undefined, label: string): string {
  const trimmed = value?.trim()
  if (!trimmed) {
    throw new Error(`${label} is required.`)
  }
  return trimmed
}

function messageLabel(symbols: SymTable, tyIdx: number): string {
  const ty = tryTyByIdx(symbols, tyIdx)
  if (ty?.kind === "StructRef") {
    return ty.struct_name
  }
  if (ty?.kind === "AliasRef") {
    return ty.alias_name
  }
  return safeRenderTy(symbols, tyIdx)
}

function safeRenderTy(symbols: SymTable, tyIdx: number): string {
  try {
    return renderTy(symbols, tyIdx)
  } catch {
    return `ty#${tyIdx}`
  }
}

function createUnionLabels(
  symbols: SymTable,
  variants: readonly UnionVariant[],
): readonly (UnionVariant & {readonly labelStr: string; readonly hasValueField: boolean})[] {
  const labels = variants.map(variant => createTypeLabel(symbols, variant.variant_ty_idx))
  const duplicatedLabels = new Set(labels.filter((label, index) => labels.indexOf(label) !== index))

  return variants.map((variant, index) => {
    const label = labels[index]
    const labelTy = tryTyByIdx(symbols, variant.variant_ty_idx)
    const fullLabel = duplicatedLabels.has(label)
      ? safeRenderTy(symbols, variant.variant_ty_idx)
      : label
    return {
      ...variant,
      labelStr: labelTy?.kind === "nullLiteral" ? "" : fullLabel,
      hasValueField: duplicatedLabels.has(label)
        ? true
        : !isStructWithOwnLabel(symbols, variant.variant_ty_idx),
    }
  })
}

function createTypeLabel(symbols: SymTable, tyIdx: number): string {
  const ty = tryTyByIdx(symbols, tyIdx)
  if (!ty) {
    return `ty#${tyIdx}`
  }

  switch (ty.kind) {
    case "StructRef": {
      return ty.struct_name
    }
    case "AliasRef": {
      return createTypeLabel(symbols, symbols.aliasTargetOf(tyIdx).ty_idx)
    }
    case "cellOf": {
      return "Cell"
    }
    case "nullLiteral": {
      return "null"
    }
    case "void": {
      return "void"
    }
    default: {
      return safeRenderTy(symbols, tyIdx)
    }
  }
}

function isStructWithOwnLabel(symbols: SymTable, tyIdx: number): boolean {
  const ty = tryTyByIdx(symbols, tyIdx)
  if (ty?.kind === "StructRef") {
    return true
  }
  if (ty?.kind === "AliasRef") {
    return isStructWithOwnLabel(symbols, symbols.aliasTargetOf(tyIdx).ty_idx)
  }
  return false
}

function tryTyByIdx(symbols: SymTable, tyIdx: number): Ty | undefined {
  try {
    return symbols.tyByIdx(tyIdx)
  } catch {
    return undefined
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value)
}
