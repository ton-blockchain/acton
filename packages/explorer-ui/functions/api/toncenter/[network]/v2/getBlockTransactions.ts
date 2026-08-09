import {
  getNonEmptyArray,
  HISTORICAL_DATA_CACHE_CONTROL,
  INT32_MAX,
  INT32_MIN,
  INT64_MAX,
  INT64_MIN,
  jsonError,
  normalizeInteger,
  proxyToncenterJson,
  type PagesContext,
  validateToncenterRequest,
} from "../../../../../worker/toncenterProxy"

const PARAMETER_NAMES = [
  "workchain",
  "shard",
  "seqno",
  "root_hash",
  "file_hash",
  "count",
  "after_lt",
  "after_hash",
] as const
const PARAMETER_SET = new Set<string>(PARAMETER_NAMES)
const REQUIRED_PARAMETER_SET = new Set<string>(["workchain", "shard", "seqno"])

export async function onRequest(context: PagesContext): Promise<Response> {
  const network = validateToncenterRequest(context)
  if (network instanceof Response) {
    return network
  }

  const prepared = normalizeSearchParams(new URL(context.request.url).searchParams)
  if ("error" in prepared) {
    return jsonError(400, prepared.error)
  }

  return await proxyToncenterJson(context, {
    network,
    version: "v2",
    endpoint: "getBlockTransactions",
    searchParams: prepared.searchParams,
    cacheControlFor: payload =>
      isNonEmptyBlockTransactionsResponse(payload) ? HISTORICAL_DATA_CACHE_CONTROL : undefined,
  })
}

function normalizeSearchParams(
  input: URLSearchParams,
): {searchParams: URLSearchParams} | {error: string} {
  for (const key of input.keys()) {
    if (!PARAMETER_SET.has(key)) {
      return {error: `Unsupported query parameter: ${key}`}
    }
  }

  const searchParams = new URLSearchParams()
  for (const name of PARAMETER_NAMES) {
    const values = input.getAll(name)
    if (values.length === 0 && REQUIRED_PARAMETER_SET.has(name)) {
      return {error: `Exactly one ${name} query parameter is required`}
    }
    if (values.length > 1) {
      return {error: `Only one ${name} query parameter is allowed`}
    }

    const value = values[0]
    if (value === undefined) {
      continue
    }
    const normalized = normalizeParameter(name, value)
    if (!normalized) {
      return {error: `Invalid ${name} query parameter`}
    }
    searchParams.set(name, normalized)
  }

  if (searchParams.has("after_lt") !== searchParams.has("after_hash")) {
    return {error: "after_lt and after_hash must be provided together"}
  }
  if (searchParams.has("root_hash") !== searchParams.has("file_hash")) {
    return {error: "root_hash and file_hash must be provided together"}
  }
  return {searchParams}
}

function normalizeParameter(name: string, value: string): string | undefined {
  if (name === "workchain") {
    return normalizeInteger(value, INT32_MIN, INT32_MAX)
  }
  if (name === "shard") {
    return normalizeInteger(value, INT64_MIN, INT64_MAX)
  }
  if (name === "seqno") {
    return normalizeInteger(value, 1n, INT32_MAX)
  }
  if (name === "count") {
    return normalizeInteger(value, 1n, 1_000n)
  }
  if (name === "after_lt") {
    return normalizeInteger(value, 0n, INT64_MAX)
  }
  if (name === "after_hash") {
    return /^[0-9a-f]{64}$/i.test(value) ? value : undefined
  }
  return value.length > 0 && value.length <= 128 && /^[A-Za-z0-9+/_=-]+$/.test(value)
    ? value
    : undefined
}

function isNonEmptyBlockTransactionsResponse(value: unknown): boolean {
  const result =
    typeof value === "object" && value !== null
      ? (value as Record<string, unknown>).result
      : undefined
  return getNonEmptyArray(result, "transactions") !== undefined
}
