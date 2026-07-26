import type {RawMessageEmulationOptions} from "../retrace/txTrace/lib/emulateRawMessage"
import {
  readEmulateNavigationPayload,
  type EmulateNavigationPayload,
} from "./emulateNavigationPayload"

export const EMULATION_SHARE_QUERY_PARAM = "share"
export const SHARED_EMULATION_VERSION = 1

const MAX_SHARED_ACCOUNT_OVERRIDES = 64
const MAX_UINT32 = 0xff_ff_ff_ff

export interface SharedEmulation {
  readonly version: typeof SHARED_EMULATION_VERSION
  readonly input: EmulateNavigationPayload
  readonly options: {
    readonly accountStateOverrides?: RawMessageEmulationOptions["accountStateOverrides"]
    readonly ignoreChksig: boolean
    readonly now?: number
  }
}

interface EmulationShareResponse {
  readonly id: string
  readonly expiresAt: number
}

export async function createEmulationShare(
  apiPath: string,
  emulation: SharedEmulation,
): Promise<EmulationShareResponse> {
  const response = await fetch(apiPath, {
    method: "POST",
    headers: {"content-type": "application/json"},
    body: JSON.stringify(emulation),
  })
  const payload = await readJsonResponse(response, "Failed to create emulation share")
  if (
    !isRecord(payload) ||
    typeof payload.id !== "string" ||
    !payload.id ||
    !isFiniteTimestamp(payload.expiresAt)
  ) {
    throw new Error("Emulation share API returned an invalid response")
  }
  return {id: payload.id, expiresAt: payload.expiresAt}
}

export async function loadEmulationShare(apiPath: string, id: string): Promise<SharedEmulation> {
  const response = await fetch(`${apiPath}/${encodeURIComponent(id)}`)
  const payload = await readJsonResponse(response, "Failed to load shared emulation")
  const emulation = isRecord(payload) ? parseSharedEmulation(payload.emulation) : undefined
  if (!emulation) {
    throw new Error("Emulation share API returned an invalid response")
  }
  return emulation
}

export function parseSharedEmulation(value: unknown): SharedEmulation | undefined {
  if (
    !isRecord(value) ||
    value.version !== SHARED_EMULATION_VERSION ||
    !isRecord(value.options) ||
    typeof value.options.ignoreChksig !== "boolean" ||
    (value.options.now !== undefined && !isUint32(value.options.now))
  ) {
    return undefined
  }

  const input = readEmulateNavigationPayload({emulatePayload: value.input})
  const mcSeqno = Number(input?.mcSeqnoInput)
  if (!input?.rawMessage.trim() || !isUint32(mcSeqno) || input.mcSeqnoInput !== String(mcSeqno)) {
    return undefined
  }

  const accountStateOverrides = parseAccountStateOverrides(value.options.accountStateOverrides)
  if (value.options.accountStateOverrides !== undefined && !accountStateOverrides) {
    return undefined
  }

  return {
    version: SHARED_EMULATION_VERSION,
    input,
    options: {
      accountStateOverrides,
      ignoreChksig: value.options.ignoreChksig,
      now: value.options.now,
    },
  }
}

async function readJsonResponse(response: Response, fallback: string): Promise<unknown> {
  let payload: unknown
  try {
    payload = await response.json()
  } catch {
    if (!response.ok) {
      throw new Error(fallback)
    }
    return undefined
  }

  if (!response.ok) {
    const error = isRecord(payload) && typeof payload.error === "string" ? payload.error : fallback
    throw new Error(error)
  }
  return payload
}

function parseAccountStateOverrides(
  value: unknown,
): RawMessageEmulationOptions["accountStateOverrides"] | undefined {
  if (value === undefined) {
    return undefined
  }
  if (!isRecord(value)) {
    return undefined
  }

  const entries = Object.entries(value)
  if (entries.length > MAX_SHARED_ACCOUNT_OVERRIDES) {
    return undefined
  }

  const result = Object.create(null) as NonNullable<
    RawMessageEmulationOptions["accountStateOverrides"]
  >
  for (const [address, override] of entries) {
    if (!address || !isRecord(override)) {
      return undefined
    }

    const balance = optionalString(override.balance)
    const lastTransactionLt = optionalString(override.lastTransactionLt)
    const lastTransactionHash = optionalString(override.lastTransactionHash)
    const state = parseAccountState(override.state)
    if (
      balance === false ||
      lastTransactionLt === false ||
      lastTransactionHash === false ||
      state === false
    ) {
      return undefined
    }

    result[address] = {
      balance: balance || undefined,
      lastTransactionLt: lastTransactionLt || undefined,
      lastTransactionHash: lastTransactionHash || undefined,
      state: state || undefined,
    }
  }
  return result
}

function parseAccountState(
  value: unknown,
):
  | NonNullable<NonNullable<RawMessageEmulationOptions["accountStateOverrides"]>[string]["state"]>
  | undefined
  | false {
  if (value === undefined) {
    return undefined
  }
  if (!isRecord(value) || typeof value.type !== "string") {
    return false
  }

  if (value.type === "uninit") {
    return {type: "uninit"}
  }
  if (value.type === "frozen") {
    const stateHash = optionalString(value.stateHash)
    return stateHash === false ? false : {type: "frozen", stateHash: stateHash || undefined}
  }
  if (value.type === "active") {
    const codeBoc = optionalString(value.codeBoc)
    const dataBoc = optionalString(value.dataBoc)
    return codeBoc === false || dataBoc === false
      ? false
      : {type: "active", codeBoc: codeBoc || undefined, dataBoc: dataBoc || undefined}
  }
  return false
}

function optionalString(value: unknown): string | undefined | false {
  return value === undefined || typeof value === "string" ? value : false
}

function isUint32(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0 && Number(value) <= MAX_UINT32
}

function isFiniteTimestamp(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value > 0
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value)
}
