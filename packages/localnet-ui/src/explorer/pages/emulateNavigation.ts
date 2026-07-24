import {
  decodeAbiMessageBuilderDraft,
  type AbiMessageDirection,
  type AbiMessageTransport,
  type ContractABI,
} from "@acton/transaction-ui"
import {beginCell, fromNano, storeMessage, type Message} from "@ton/core"

export const EMULATE_HANDOFF_QUERY_PARAM = "handoff"
const EMULATE_HANDOFF_STORAGE_PREFIX = "acton:emulate-handoff:"
const EMULATE_HANDOFF_TTL_MS = 15 * 60 * 1000

export type EmulateAbiEndpoint = "destination" | "source"

export interface EmulateNavigationAbis {
  readonly destination?: ContractABI
  readonly source?: ContractABI
}

interface EmulateNavigationCommonPayload {
  readonly targetAddress: string
  readonly sourceAddress: string
  readonly messageValue: string
  readonly messageTransport: AbiMessageTransport
  readonly bounce: boolean
  readonly mcSeqnoInput: string
  readonly rawMessage: string
}

export type EmulateNavigationPayload = EmulateNavigationCommonPayload &
  (
    | {
        readonly inputMode: "builder"
        readonly builder: {
          readonly abi: ContractABI
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

export function createEmulateNavigationState(
  message: Message,
  abis: EmulateNavigationAbis,
  mcSeqno: number | string | undefined,
  messageName?: string,
): EmulateNavigationState {
  const rawMessage = beginCell().store(storeMessage(message)).endCell().toBoc().toString("hex")
  let targetAddress = ""
  let sourceAddress = ""
  let messageValue = "0"
  let messageTransport: AbiMessageTransport = "internal"
  let bounce = true

  switch (message.info.type) {
    case "internal":
      targetAddress = message.info.dest.toString()
      sourceAddress = message.info.src.toString()
      messageValue = fromNano(message.info.value.coins)
      bounce = message.info.bounce
      break
    case "external-in":
      targetAddress = message.info.dest.toString()
      messageTransport = "external"
      break
    case "external-out":
      targetAddress = message.info.dest?.toString() ?? ""
      break
    default:
      break
  }

  const common = {
    targetAddress,
    sourceAddress,
    messageValue,
    messageTransport,
    bounce,
    mcSeqnoInput: mcSeqno === undefined ? "" : String(mcSeqno),
    rawMessage,
  }

  const canUseBuilder =
    message.info.type === "external-in" ||
    (message.info.type === "internal" && !message.info.bounced)
  const abiCandidates: readonly {
    readonly abi: ContractABI | undefined
    readonly endpoint: EmulateAbiEndpoint
    readonly direction: AbiMessageDirection
  }[] =
    message.info.type === "internal"
      ? [
          {abi: abis.destination, endpoint: "destination", direction: "incoming"},
          {abi: abis.source, endpoint: "source", direction: "outgoing"},
        ]
      : [{abi: abis.destination, endpoint: "destination", direction: "incoming"}]

  if (canUseBuilder && messageName) {
    for (const {abi, endpoint, direction} of abiCandidates) {
      if (!abi) {
        continue
      }
      const builderPayload = decodeAbiMessageBuilderDraft(
        abi,
        messageTransport,
        message.body,
        direction,
        messageName,
      )
      if (!builderPayload) {
        continue
      }

      return {
        emulatePayload: {
          inputMode: "builder",
          ...common,
          builder: {
            abi,
            abiEndpoint: endpoint,
            messageName: builderPayload.option.label,
            argsJson: builderPayload.argsJson,
          },
        },
      }
    }
  }

  return {
    emulatePayload: {
      inputMode: "raw",
      ...common,
    },
  }
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

  if (
    payload.inputMode === "builder" &&
    (!isRecord(payload.builder) ||
      !isRecord(payload.builder.abi) ||
      (payload.builder.abiEndpoint !== "destination" && payload.builder.abiEndpoint !== "source") ||
      typeof payload.builder.messageName !== "string" ||
      typeof payload.builder.argsJson !== "string")
  ) {
    return undefined
  }

  return payload as unknown as EmulateNavigationPayload
}

export function saveEmulateNavigationPayload(
  payload: EmulateNavigationPayload,
): string | undefined {
  try {
    const id = globalThis.crypto.randomUUID()
    globalThis.localStorage?.setItem(
      `${EMULATE_HANDOFF_STORAGE_PREFIX}${id}`,
      JSON.stringify({createdAt: Date.now(), emulatePayload: payload}),
    )
    return id
  } catch {
    return undefined
  }
}

export function readStoredEmulateNavigationPayload(
  searchParams: URLSearchParams,
): EmulateNavigationPayload | undefined {
  const id = searchParams.get(EMULATE_HANDOFF_QUERY_PARAM)
  if (!id) {
    return undefined
  }

  try {
    const storageKey = `${EMULATE_HANDOFF_STORAGE_PREFIX}${id}`
    const raw = globalThis.localStorage?.getItem(storageKey)
    if (!raw) {
      return undefined
    }

    const stored = JSON.parse(raw) as unknown
    if (
      !isRecord(stored) ||
      typeof stored.createdAt !== "number" ||
      Date.now() - stored.createdAt > EMULATE_HANDOFF_TTL_MS
    ) {
      globalThis.localStorage?.removeItem(storageKey)
      return undefined
    }

    return readEmulateNavigationPayload(stored)
  } catch {
    return undefined
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value)
}
