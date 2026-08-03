import type {ContractABI} from "@ton/tolk-abi-to-typescript"

export type EmulateAbiEndpoint = "destination" | "source"

interface EmulateNavigationCommonPayload {
  readonly targetAddress: string
  readonly sourceAddress: string
  readonly messageValue: string
  readonly messageTransport: "internal" | "external"
  readonly bounce: boolean
  readonly mcSeqnoInput: string
  readonly rawMessage: string
}

export type EmulateNavigationPayload = EmulateNavigationCommonPayload &
  (
    | {
        readonly inputMode: "builder"
        readonly builder: {
          readonly abi?: ContractABI
          readonly abiSourceMode: "auto" | "manual"
          readonly abiEndpoint: EmulateAbiEndpoint
          readonly messageName: string
          readonly argsJson: string
        }
      }
    | {
        readonly inputMode: "raw"
      }
  )

export interface EmulateNavigationState {
  readonly emulatePayload: EmulateNavigationPayload
}

export function readEmulateNavigationPayload(state: unknown): EmulateNavigationPayload | undefined {
  if (!isRecord(state) || !isRecord(state.emulatePayload)) {
    return undefined
  }

  const payload = state.emulatePayload
  if (
    (payload.inputMode !== "builder" && payload.inputMode !== "raw") ||
    typeof payload.targetAddress !== "string" ||
    typeof payload.sourceAddress !== "string" ||
    typeof payload.messageValue !== "string" ||
    (payload.messageTransport !== "internal" && payload.messageTransport !== "external") ||
    typeof payload.bounce !== "boolean" ||
    typeof payload.mcSeqnoInput !== "string" ||
    typeof payload.rawMessage !== "string"
  ) {
    return undefined
  }

  const common = {
    targetAddress: payload.targetAddress,
    sourceAddress: payload.sourceAddress,
    messageValue: payload.messageValue,
    messageTransport: payload.messageTransport,
    bounce: payload.bounce,
    mcSeqnoInput: payload.mcSeqnoInput,
    rawMessage: payload.rawMessage,
  } satisfies EmulateNavigationCommonPayload

  if (payload.inputMode === "raw") {
    return {...common, inputMode: "raw"}
  }

  const builder = payload.builder
  if (
    !isRecord(builder) ||
    (builder.abi !== undefined && !isContractAbi(builder.abi)) ||
    (builder.abiSourceMode !== "auto" && builder.abiSourceMode !== "manual") ||
    (builder.abiSourceMode === "manual" && builder.abi === undefined) ||
    (builder.abiEndpoint !== "destination" && builder.abiEndpoint !== "source") ||
    typeof builder.messageName !== "string" ||
    typeof builder.argsJson !== "string" ||
    !isJson(builder.argsJson)
  ) {
    return undefined
  }

  return {
    ...common,
    inputMode: "builder",
    builder: {
      abi: builder.abi,
      abiSourceMode: builder.abiSourceMode,
      abiEndpoint: builder.abiEndpoint,
      messageName: builder.messageName,
      argsJson: builder.argsJson,
    },
  }
}

function isContractAbi(value: unknown): value is ContractABI {
  return (
    isRecord(value) &&
    typeof value.contract_name === "string" &&
    value.contract_name.length > 0 &&
    typeof value.compiler_name === "string" &&
    typeof value.compiler_version === "string" &&
    isRecord(value.storage) &&
    Array.isArray(value.unique_types) &&
    Array.isArray(value.struct_instantiations) &&
    Array.isArray(value.alias_instantiations) &&
    Array.isArray(value.declarations) &&
    Array.isArray(value.incoming_messages) &&
    Array.isArray(value.incoming_external) &&
    Array.isArray(value.outgoing_messages) &&
    Array.isArray(value.emitted_events) &&
    Array.isArray(value.get_methods) &&
    Array.isArray(value.thrown_errors)
  )
}

function isJson(value: string): boolean {
  try {
    JSON.parse(value)
    return true
  } catch {
    return false
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value)
}
