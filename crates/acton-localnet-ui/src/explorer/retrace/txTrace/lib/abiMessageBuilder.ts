import {
  Address,
  beginCell,
  external,
  storeMessage,
  toNano,
  type Message,
} from "@ton/core"
import {
  DynamicCtx,
  packToBuilderDynamic,
  renderTy,
  SymTable,
  type ContractABI,
  type Ty,
  type UnionVariant,
} from "@ton/tolk-abi-to-typescript"

import {
  normalizeAbiDynamicArg,
  sampleAbiValueForTy,
  stringifyAbiJson,
} from "../../../api/abiDynamic"

export type AbiMessageTransport = "external" | "internal"

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

export function listAbiMessageBuilderOptions(
  abi: ContractABI,
  transport: AbiMessageTransport,
): readonly AbiMessageBuilderOption[] {
  const symbols = createAbiMessageSymbols(abi)
  const messages =
    transport === "external" ? abi.incoming_external ?? [] : abi.incoming_messages ?? []

  return messages.flatMap((message, messageIndex) =>
    expandMessageBodyOptions(symbols, transport, messageIndex, message.body_ty_idx),
  )
}

export function createAbiMessageSymbols(abi: ContractABI): SymTable {
  return new SymTable(
    abi.declarations,
    abi.unique_types,
    abi.struct_instantiations,
    abi.alias_instantiations,
  )
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
  const body = bodyBuilder.endCell()
  const message =
    option.transport === "external"
      ? external({to: destinationAddress, body})
      : buildInternalMessage({
          source: Address.parse(requireField(source, "Source address")),
          destination: destinationAddress,
          value: parseMessageValue(value),
          bounce,
          body,
        })

  return beginCell().store(storeMessage(message)).endCell().toBoc().toString("base64")
}

export function formatAbiMessageOptionSummary(option: AbiMessageBuilderOption): string {
  return option.union ? `${option.label} / ${option.typeLabel}` : option.typeLabel
}

function expandMessageBodyOptions(
  symbols: SymTable,
  transport: AbiMessageTransport,
  messageIndex: number,
  bodyTyIdx: number,
): readonly AbiMessageBuilderOption[] {
  const bodyTy = tryTyByIdx(symbols, bodyTyIdx)
  if (bodyTy?.kind === "union") {
    return createUnionLabels(symbols, bodyTy.variants).map((variant, variantIndex) => {
      const sample = sampleAbiValueForTy(symbols, variant.variant_ty_idx)
      const typeLabel = safeRenderTy(symbols, variant.variant_ty_idx)
      const label = variant.labelStr || typeLabel

      return {
        id: `${transport}:${messageIndex}:${variantIndex}`,
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
      id: `${transport}:${messageIndex}`,
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
  return trimmed ? toNano(trimmed) : 0n
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
      hasValueField: duplicatedLabels.has(label) ? true : !isStructWithOwnLabel(symbols, variant.variant_ty_idx),
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
