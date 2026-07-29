import {
  getNonEmptyArray,
  HISTORICAL_DATA_CACHE_CONTROL,
  INT32_MAX,
  INT32_MIN,
  jsonError,
  normalizeInteger,
  proxyToncenterJson,
  type PagesContext,
  validateToncenterRequest,
} from "../../../../../worker/toncenterProxy"

const TRANSACTION_PARAMETER_NAMES = ["workchain", "shard", "seqno", "limit", "offset"] as const
const TRANSACTION_PARAMETER_SET = new Set<string>(TRANSACTION_PARAMETER_NAMES)
const REQUIRED_TRANSACTION_PARAMETER_SET = new Set<string>(["workchain", "shard", "seqno"])

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
    version: "v3",
    endpoint: "transactions",
    searchParams: prepared.searchParams,
    cacheControlFor: payload =>
      isNonEmptyTransactionsResponse(payload) ? HISTORICAL_DATA_CACHE_CONTROL : undefined,
  })
}

function normalizeSearchParams(
  input: URLSearchParams,
): {searchParams: URLSearchParams} | {error: string} {
  for (const key of input.keys()) {
    if (!TRANSACTION_PARAMETER_SET.has(key)) {
      return {error: `Unsupported query parameter: ${key}`}
    }
  }

  const searchParams = new URLSearchParams()
  for (const name of TRANSACTION_PARAMETER_NAMES) {
    const values = input.getAll(name)
    if (values.length === 0 && REQUIRED_TRANSACTION_PARAMETER_SET.has(name)) {
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
  return {searchParams}
}

function normalizeParameter(name: string, value: string): string | undefined {
  if (value.length === 0 || value.length > 128) {
    return undefined
  }
  if (name === "shard") {
    return value
  }
  if (name === "workchain") {
    return normalizeInteger(value, INT32_MIN, INT32_MAX)
  }
  if (name === "limit") {
    return normalizeInteger(value, 1n, 1_000n)
  }
  return normalizeInteger(value, 0n, INT32_MAX)
}

function isNonEmptyTransactionsResponse(value: unknown): boolean {
  return getNonEmptyArray(value, "transactions") !== undefined
}
