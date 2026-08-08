import {
  getNonEmptyArray,
  HISTORICAL_DATA_CACHE_CONTROL,
  INT32_MAX,
  INT32_MIN,
  INT64_MAX,
  jsonError,
  normalizeInteger,
  proxyToncenterJson,
  type PagesContext,
  validateToncenterRequest,
} from "../../../../../worker/toncenterProxy"

const LATEST_BLOCKS_CACHE_CONTROL = "public, max-age=0, s-maxage=2, must-revalidate"
const BLOCK_PARAMETER_NAMES = [
  "workchain",
  "shard",
  "seqno",
  "root_hash",
  "file_hash",
  "mc_seqno",
  "start_utime",
  "end_utime",
  "start_lt",
  "end_lt",
  "limit",
  "offset",
  "sort",
] as const
const BLOCK_PARAMETER_SET = new Set<string>(BLOCK_PARAMETER_NAMES)

export async function onRequest(context: PagesContext): Promise<Response> {
  const network = validateToncenterRequest(context)
  if (network instanceof Response) {
    return network
  }

  const prepared = normalizeBlocksSearchParams(new URL(context.request.url).searchParams)
  if ("error" in prepared) {
    return jsonError(400, prepared.error)
  }

  const cacheControl = isHistoricalBlockQuery(prepared.searchParams)
    ? HISTORICAL_DATA_CACHE_CONTROL
    : LATEST_BLOCKS_CACHE_CONTROL
  return await proxyToncenterJson(context, {
    network,
    version: "v3",
    endpoint: "blocks",
    searchParams: prepared.searchParams,
    cacheControlFor: payload => (isNonEmptyBlocksResponse(payload) ? cacheControl : undefined),
  })
}

function normalizeBlocksSearchParams(
  input: URLSearchParams,
): {searchParams: URLSearchParams} | {error: string} {
  for (const key of input.keys()) {
    if (!BLOCK_PARAMETER_SET.has(key)) {
      return {error: `Unsupported query parameter: ${key}`}
    }
  }

  const searchParams = new URLSearchParams()
  for (const name of BLOCK_PARAMETER_NAMES) {
    const values = input.getAll(name)
    if (values.length > 1) {
      return {error: `Only one ${name} query parameter is allowed`}
    }
    const value = values[0]
    if (value === undefined) {
      continue
    }
    const normalized = normalizeBlockParameter(name, value)
    if (!normalized) {
      return {error: `Invalid ${name} query parameter`}
    }
    searchParams.set(name, normalized)
  }
  if (searchParams.has("shard") && !searchParams.has("workchain")) {
    return {error: "shard must be provided with workchain"}
  }
  if (searchParams.has("seqno") && (!searchParams.has("workchain") || !searchParams.has("shard"))) {
    return {error: "seqno must be provided with workchain and shard"}
  }
  return {searchParams}
}

function normalizeBlockParameter(name: string, value: string): string | undefined {
  if (value.length === 0 || value.length > 128) {
    return undefined
  }
  if (name === "sort") {
    return value === "asc" || value === "desc" ? value : undefined
  }
  if (name === "workchain") {
    return normalizeInteger(value, INT32_MIN, INT32_MAX)
  }
  if (name === "start_lt" || name === "end_lt") {
    return normalizeInteger(value, 0n, INT64_MAX)
  }
  if (name === "limit") {
    return normalizeInteger(value, 1n, 1_000n)
  }
  if (
    name === "seqno" ||
    name === "mc_seqno" ||
    name === "start_utime" ||
    name === "end_utime" ||
    name === "offset"
  ) {
    return normalizeInteger(value, 0n, INT32_MAX)
  }
  return value
}

function isHistoricalBlockQuery(searchParams: URLSearchParams): boolean {
  if (
    searchParams.has("seqno") ||
    searchParams.has("root_hash") ||
    searchParams.has("file_hash") ||
    searchParams.has("mc_seqno") ||
    searchParams.get("sort") === "asc"
  ) {
    return true
  }

  const endUtime = searchParams.get("end_utime")
  return endUtime !== null && Number(endUtime) < Date.now() / 1000 - 30
}

function isNonEmptyBlocksResponse(value: unknown): boolean {
  return getNonEmptyArray(value, "blocks") !== undefined
}
