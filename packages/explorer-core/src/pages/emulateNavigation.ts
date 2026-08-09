import {
  decodeAbiMessageBuilderDraft,
  type AbiMessageDirection,
  type AbiMessageTransport,
  type ContractABI,
} from "@acton/transaction-ui"
import {formatGramAmount} from "@acton/ui"
import {beginCell, storeMessage, type Message} from "@ton/core"
import {
  readEmulateNavigationPayload,
  type EmulateAbiEndpoint,
  type EmulateNavigationPayload,
  type EmulateNavigationState,
} from "./emulateNavigationPayload"

export {
  readEmulateNavigationPayload,
  type EmulateAbiEndpoint,
  type EmulateNavigationPayload,
  type EmulateNavigationState,
} from "./emulateNavigationPayload"

export const EMULATE_HANDOFF_QUERY_PARAM = "handoff"
const EMULATE_HANDOFF_STORAGE_PREFIX = "acton:emulate-handoff:"
const EMULATE_HANDOFF_TTL_MS = 15 * 60 * 1000

export interface EmulateNavigationAbis {
  readonly destination?: ContractABI
  readonly source?: ContractABI
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
      messageValue = formatGramAmount(message.info.value.coins, {showUnit: false})
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
            abiSourceMode: "auto",
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
