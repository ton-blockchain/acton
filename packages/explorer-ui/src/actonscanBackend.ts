import type {
  LoadNetworkTps,
  NetworkTpsSnapshot,
  NetworkTpsWindow,
} from "@acton/explorer-core/api/networkStats"

const TPS_PATH = "api/v1/stats/tps"
const DEFAULT_ACTONSCAN_BACKEND_URL = "https://api.actonscan.com/"
const REQUEST_TIMEOUT_MS = 4000

export const loadNetworkTps: LoadNetworkTps = async signal => {
  const requestSignal = AbortSignal.any([signal, AbortSignal.timeout(REQUEST_TIMEOUT_MS)])
  const response = await fetch(tpsUrl(), {
    headers: {Accept: "application/json"},
    signal: requestSignal,
  })
  if (!response.ok) {
    throw new Error(`Actonscan backend returned HTTP ${response.status}`)
  }
  return parseSnapshot(await response.json())
}

function tpsUrl(): string {
  const configured =
    import.meta.env.VITE_ACTONSCAN_BACKEND_URL?.trim() || DEFAULT_ACTONSCAN_BACKEND_URL
  return `${configured.replace(/\/$/, "")}/${TPS_PATH}`
}

function parseSnapshot(value: unknown): NetworkTpsSnapshot {
  if (!isRecord(value) || (value.status !== "syncing" && value.status !== "ready")) {
    throw new Error("Actonscan backend returned an invalid TPS snapshot")
  }
  if (!Array.isArray(value.windows)) {
    throw new Error("Actonscan backend returned invalid TPS windows")
  }

  return {
    status: value.status,
    latest_masterchain_seqno: optionalNumber(value.latest_masterchain_seqno),
    latest_block_time: optionalNumber(value.latest_block_time),
    windows: value.windows.map(parseWindow),
  }
}

function parseWindow(value: unknown): NetworkTpsWindow {
  if (
    !isRecord(value) ||
    typeof value.window_seconds !== "number" ||
    typeof value.coverage_seconds !== "number" ||
    typeof value.transactions !== "number" ||
    typeof value.tps !== "number" ||
    typeof value.complete !== "boolean"
  ) {
    throw new Error("Actonscan backend returned an invalid TPS window")
  }
  return {
    window_seconds: value.window_seconds,
    coverage_seconds: value.coverage_seconds,
    transactions: value.transactions,
    tps: value.tps,
    complete: value.complete,
  }
}

function optionalNumber(value: unknown): number | undefined {
  if (value === null || value === undefined) return undefined
  if (typeof value !== "number") {
    throw new Error("Actonscan backend returned an invalid numeric value")
  }
  return value
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
