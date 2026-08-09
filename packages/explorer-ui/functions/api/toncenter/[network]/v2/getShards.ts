import {
  getNonEmptyArray,
  HISTORICAL_DATA_CACHE_CONTROL,
  INT32_MAX,
  jsonError,
  normalizeInteger,
  proxyToncenterJson,
  type PagesContext,
  validateToncenterRequest,
} from "../../../../../worker/toncenterProxy"

export async function onRequest(context: PagesContext): Promise<Response> {
  const network = validateToncenterRequest(context)
  if (network instanceof Response) {
    return network
  }

  const input = new URL(context.request.url).searchParams
  for (const key of input.keys()) {
    if (key !== "seqno") {
      return jsonError(400, `Unsupported query parameter: ${key}`)
    }
  }
  const seqnos = input.getAll("seqno")
  const seqno = seqnos.length === 1 ? normalizeInteger(seqnos[0] ?? "", 0n, INT32_MAX) : undefined
  if (!seqno) {
    return jsonError(400, "Exactly one int32 seqno query parameter is required")
  }

  const searchParams = new URLSearchParams({seqno})
  return await proxyToncenterJson(context, {
    network,
    version: "v2",
    endpoint: "getShards",
    searchParams,
    cacheControlFor: payload =>
      isNonEmptyShardsResponse(payload) ? HISTORICAL_DATA_CACHE_CONTROL : undefined,
  })
}

function isNonEmptyShardsResponse(value: unknown): boolean {
  const result =
    typeof value === "object" && value !== null
      ? (value as Record<string, unknown>).result
      : undefined
  return getNonEmptyArray(result, "shards") !== undefined
}
