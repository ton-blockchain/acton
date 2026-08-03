export type ToncenterNetwork = "mainnet" | "testnet"
type ToncenterProxyTarget =
  | {version: "v2"; endpoint: "getShards"}
  | {version: "v3"; endpoint: "blocks" | "traces" | "transactions"}

export const HISTORICAL_DATA_CACHE_CONTROL = "public, max-age=300, s-maxage=604800, immutable"
export const INT32_MIN = -2_147_483_648n
export const INT32_MAX = 2_147_483_647n
export const INT64_MAX = 9_223_372_036_854_775_807n

export type Env = {
  TONCENTER_API_V2_URL?: string
  TONCENTER_API_V3_URL?: string
  TONCENTER_API_KEY?: string
  TONCENTER_MAINNET_API_V2_URL?: string
  TONCENTER_MAINNET_API_V3_URL?: string
  TONCENTER_MAINNET_API_KEY?: string
  TONCENTER_TESTNET_API_V2_URL?: string
  TONCENTER_TESTNET_API_V3_URL?: string
  TONCENTER_TESTNET_API_KEY?: string
  VITE_EXPLORER_TONCENTER_API_V2_URL?: string
  VITE_EXPLORER_TONCENTER_API_V3_URL?: string
  VITE_EXPLORER_TONCENTER_API_KEY?: string
  VITE_EXPLORER_MAINNET_TONCENTER_API_V2_URL?: string
  VITE_EXPLORER_MAINNET_TONCENTER_API_V3_URL?: string
  VITE_EXPLORER_MAINNET_TONCENTER_API_KEY?: string
  VITE_EXPLORER_TESTNET_TONCENTER_API_V2_URL?: string
  VITE_EXPLORER_TESTNET_TONCENTER_API_V3_URL?: string
  VITE_EXPLORER_TESTNET_TONCENTER_API_KEY?: string
}

export type PagesContext = {
  request: Request
  env: Env
  params: {
    network?: string | string[]
  }
  waitUntil(promise: Promise<unknown>): void
}

type EdgeCache = {
  match(request: Request): Promise<Response | undefined>
  put(request: Request, response: Response): Promise<void>
}

type ToncenterRequestTarget = ToncenterProxyTarget & {
  network: ToncenterNetwork
  searchParams: URLSearchParams
}

type ToncenterProxyOptions = ToncenterRequestTarget & {
  cacheControlFor(payload: unknown): string | undefined
}

export async function proxyToncenterJson(
  context: PagesContext,
  options: ToncenterProxyOptions,
): Promise<Response> {
  const {network, searchParams, cacheControlFor} = options
  const requestUrl = new URL(context.request.url)
  const cacheKey = createCacheKey(requestUrl, searchParams)
  const edgeCache = defaultEdgeCache()
  const cached = await matchCache(edgeCache, cacheKey)
  if (cached) {
    return withProxyHeaders(cached, "HIT", 'edge;desc="HIT"')
  }

  const upstreamUrl = createUpstreamUrl(context.env, options)
  if (!upstreamUrl) {
    return jsonError(500, `Toncenter ${network} API URL is invalid`)
  }

  const startedAt = performance.now()
  let upstream: Response
  try {
    upstream = await fetch(upstreamUrl, {headers: toncenterHeaders(context.env, network)})
  } catch {
    return jsonError(502, "Toncenter request failed", {
      "server-timing": toncenterTiming(startedAt),
    })
  }

  const body = await upstream.text()
  let payload: unknown
  try {
    payload = JSON.parse(body)
  } catch {
    const headers = new Headers({"server-timing": toncenterTiming(startedAt)})
    const retryAfter = upstream.headers.get("retry-after")
    if (retryAfter) {
      headers.set("retry-after", retryAfter)
    }
    return jsonError(
      upstream.ok ? 502 : upstream.status,
      "Toncenter returned an invalid JSON response",
      headers,
    )
  }

  const cacheControl = upstream.ok ? cacheControlFor(payload) : undefined
  const responseHeaders = new Headers({
    "cache-control": cacheControl ?? "no-store",
    "content-type": "application/json; charset=utf-8",
  })
  const retryAfter = upstream.headers.get("retry-after")
  if (retryAfter) {
    responseHeaders.set("retry-after", retryAfter)
  }

  const response = new Response(body, {
    status: upstream.status,
    statusText: upstream.statusText,
    headers: responseHeaders,
  })
  const cacheStatus = edgeCache ? "MISS" : "BYPASS"
  const result = withProxyHeaders(response, cacheStatus, toncenterTiming(startedAt))

  if (cacheControl && edgeCache) {
    context.waitUntil(edgeCache.put(cacheKey, result.clone()).catch(() => undefined))
  }

  return result
}

export function validateToncenterRequest(context: PagesContext): ToncenterNetwork | Response {
  if (context.request.method !== "GET") {
    return jsonError(405, "Method not allowed", {allow: "GET"})
  }

  const value = context.params.network
  const network = Array.isArray(value) ? value[0] : value
  return network === "mainnet" || network === "testnet"
    ? network
    : jsonError(404, "Unknown TON network")
}

export function normalizeInteger(
  value: string,
  minimum: bigint,
  maximum: bigint,
): string | undefined {
  if (value.length === 0 || value.length > 20 || !/^-?\d+$/.test(value)) {
    return undefined
  }
  const parsed = BigInt(value)
  return parsed >= minimum && parsed <= maximum ? parsed.toString() : undefined
}

export function getNonEmptyArray(value: unknown, property: string): unknown[] | undefined {
  if (typeof value !== "object" || value === null) {
    return undefined
  }
  const items = (value as Record<string, unknown>)[property]
  return Array.isArray(items) && items.length > 0 ? items : undefined
}

export function jsonError(status: number, error: string, headers?: HeadersInit): Response {
  return Response.json(
    {error},
    {
      status,
      headers: {
        "cache-control": "no-store",
        ...Object.fromEntries(new Headers(headers)),
      },
    },
  )
}

function createCacheKey(requestUrl: URL, searchParams: URLSearchParams): Request {
  const cacheUrl = new URL(requestUrl.origin + requestUrl.pathname)
  cacheUrl.search = searchParams.toString()
  return new Request(cacheUrl)
}

function createUpstreamUrl(
  env: Env,
  {network, version, endpoint, searchParams}: ToncenterRequestTarget,
): URL | undefined {
  try {
    const baseUrl = toncenterApiUrl(env, network, version)
    const url = new URL(`${baseUrl.replace(/\/$/, "")}/${endpoint}`)
    if (url.protocol !== "https:" && url.protocol !== "http:") {
      return undefined
    }
    url.search = searchParams.toString()
    return url
  } catch {
    return undefined
  }
}

function toncenterApiUrl(
  env: Env,
  network: ToncenterNetwork,
  version: ToncenterProxyTarget["version"],
): string {
  let configuredUrl: string | undefined
  if (version === "v2") {
    configuredUrl =
      network === "testnet"
        ? firstValue(
            env.TONCENTER_TESTNET_API_V2_URL,
            env.VITE_EXPLORER_TESTNET_TONCENTER_API_V2_URL,
          )
        : firstValue(
            env.TONCENTER_MAINNET_API_V2_URL,
            env.TONCENTER_API_V2_URL,
            env.VITE_EXPLORER_MAINNET_TONCENTER_API_V2_URL,
            env.VITE_EXPLORER_TONCENTER_API_V2_URL,
          )
  } else {
    configuredUrl =
      network === "testnet"
        ? firstValue(
            env.TONCENTER_TESTNET_API_V3_URL,
            env.VITE_EXPLORER_TESTNET_TONCENTER_API_V3_URL,
          )
        : firstValue(
            env.TONCENTER_MAINNET_API_V3_URL,
            env.TONCENTER_API_V3_URL,
            env.VITE_EXPLORER_MAINNET_TONCENTER_API_V3_URL,
            env.VITE_EXPLORER_TONCENTER_API_V3_URL,
          )
  }
  const host = network === "testnet" ? "testnet.toncenter.com" : "toncenter.com"
  return configuredUrl ?? `https://${host}/api/${version}`
}

function toncenterApiKey(env: Env, network: ToncenterNetwork): string | undefined {
  return network === "testnet"
    ? firstValue(env.TONCENTER_TESTNET_API_KEY, env.VITE_EXPLORER_TESTNET_TONCENTER_API_KEY)
    : firstValue(
        env.TONCENTER_MAINNET_API_KEY,
        env.TONCENTER_API_KEY,
        env.VITE_EXPLORER_MAINNET_TONCENTER_API_KEY,
        env.VITE_EXPLORER_TONCENTER_API_KEY,
      )
}

function firstValue(...values: Array<string | undefined>): string | undefined {
  return values.find(value => typeof value === "string" && value.trim())?.trim()
}

function toncenterHeaders(env: Env, network: ToncenterNetwork): Headers {
  const headers = new Headers({accept: "application/json"})
  const apiKey = toncenterApiKey(env, network)
  if (apiKey) {
    headers.set("X-API-Key", apiKey)
  }
  return headers
}

function defaultEdgeCache(): EdgeCache | undefined {
  const cacheStorage = (
    globalThis as typeof globalThis & {
      caches?: CacheStorage & {default?: EdgeCache}
    }
  ).caches
  return cacheStorage?.default
}

function matchCache(edgeCache: EdgeCache | undefined, cacheKey: Request) {
  return edgeCache ? edgeCache.match(cacheKey).catch(() => undefined) : undefined
}

function withProxyHeaders(
  response: Response,
  cacheStatus: "HIT" | "MISS" | "BYPASS",
  serverTiming: string,
): Response {
  const headers = new Headers(response.headers)
  headers.set("server-timing", serverTiming)
  headers.set("x-actonscan-cache", cacheStatus)
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  })
}

function toncenterTiming(startedAt: number): string {
  return `toncenter;dur=${Math.max(0, performance.now() - startedAt).toFixed(1)}`
}
