import {
  getNonEmptyArray,
  jsonError,
  proxyToncenterJson,
  type PagesContext,
  validateToncenterRequest,
} from "../../../../../worker/toncenterProxy"

const TRANSACTION_HASH_PATTERN = /^[0-9a-f]{64}$/i
const COMPLETE_TRACE_CACHE_CONTROL = "public, max-age=300, s-maxage=604800"

export async function onRequest(context: PagesContext): Promise<Response> {
  const network = validateToncenterRequest(context)
  if (network instanceof Response) {
    return network
  }

  const requestUrl = new URL(context.request.url)
  const validationError = validateSearchParams(requestUrl.searchParams)
  if (validationError) {
    return jsonError(400, validationError)
  }

  const transactionHash = requestUrl.searchParams.get("tx_hash")?.toLowerCase()
  if (!transactionHash || !TRANSACTION_HASH_PATTERN.test(transactionHash)) {
    return jsonError(400, "tx_hash must be a 64-character hexadecimal transaction hash")
  }

  const includeActions = requestUrl.searchParams.get("include_actions") === "true"
  const searchParams = new URLSearchParams({tx_hash: transactionHash})
  if (includeActions) {
    searchParams.set("include_actions", "true")
  }
  return await proxyToncenterJson(context, {
    network,
    version: "v3",
    endpoint: "traces",
    searchParams,
    cacheControlFor: payload =>
      isCompleteTraceResponse(payload) ? COMPLETE_TRACE_CACHE_CONTROL : undefined,
  })
}

function validateSearchParams(searchParams: URLSearchParams): string | undefined {
  for (const key of searchParams.keys()) {
    if (key !== "tx_hash" && key !== "include_actions") {
      return `Unsupported query parameter: ${key}`
    }
  }
  if (searchParams.getAll("tx_hash").length !== 1) {
    return "Exactly one tx_hash query parameter is required"
  }
  const includeActions = searchParams.getAll("include_actions")
  if (includeActions.length > 1 || (includeActions.length === 1 && includeActions[0] !== "true")) {
    return "include_actions must be true when provided"
  }
  return undefined
}

function isCompleteTraceResponse(value: unknown): boolean {
  const traces = getNonEmptyArray(value, "traces")
  if (!traces) {
    return false
  }
  return traces.every(trace => {
    if (!isRecord(trace)) {
      return false
    }
    const candidate = trace as {
      is_incomplete?: unknown
      trace_info?: {pending_messages?: unknown}
    }
    return candidate.is_incomplete === false && candidate.trace_info?.pending_messages === 0
  })
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null
}
