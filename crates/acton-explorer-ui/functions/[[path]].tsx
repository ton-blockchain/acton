import {ImageResponse} from "@cloudflare/pages-plugin-vercel-og/api"
import {AccountOgImage, type AccountOgPreview} from "../src/og/AccountOgImage"

const OG_IMAGE_VERSION = "5"
const OG_IMAGE_WIDTH = 1200
const OG_IMAGE_HEIGHT = 630

type AssetFetcher = {
  fetch(request: Request): Promise<Response>
}

type Env = {
  ASSETS: AssetFetcher
  TONCENTER_API_V3_URL?: string
  TONCENTER_API_KEY?: string
  VITE_EXPLORER_TONCENTER_API_V3_URL?: string
  VITE_EXPLORER_TONCENTER_API_KEY?: string
}

type PagesContext = {
  request: Request
  env: Env
  next(): Promise<Response>
}

type RouteMetadata = {
  title: string
  description: string
  image: string
  url: string
}

export async function onRequest(context: PagesContext) {
  const url = new URL(context.request.url)

  if (url.pathname === "/og/account.png") {
    return renderAccountOgPng(context)
  }

  const metadata = await getRouteMetadata(url, context.env)
  const assetResponse = await context.next()
  const response =
    metadata && assetResponse.status === 404
      ? await context.env.ASSETS.fetch(new Request(new URL("/index.html", url), context.request))
      : assetResponse

  if (!shouldInjectHtml(context.request, response)) {
    return assetResponse
  }

  if (!metadata) {
    return response
  }

  const html = await response.text()
  return new Response(injectMetadata(html, metadata), {
    headers: withHeader(response.headers, "content-type", "text/html; charset=utf-8"),
    status: 200,
    statusText: "OK",
  })
}

async function renderAccountOgPng(context: PagesContext) {
  const url = new URL(context.request.url)
  const preview = await getAccountPreview(url.searchParams.get("address") || "", context.env, true)
  const image = new ImageResponse(<AccountOgImage preview={preview} />, {
    width: OG_IMAGE_WIDTH,
    height: OG_IMAGE_HEIGHT,
    headers: {
      "cache-control": "public, max-age=14400",
    },
  })
  const bytes = await image.arrayBuffer()

  return new Response(bytes, {
    headers: {
      "content-type": "image/png",
      "cache-control": "public, max-age=14400",
    },
  })
}

function shouldInjectHtml(request: Request, response: Response) {
  if (request.method !== "GET") {
    return false
  }
  return response.headers.get("content-type")?.includes("text/html") ?? false
}

async function getRouteMetadata(url: URL, env: Env): Promise<RouteMetadata | undefined> {
  const address = addressFromPath(url.pathname)
  if (!address) {
    return undefined
  }

  const preview = await getAccountPreview(address, env)
  const title = `${preview.title} · actonscan`
  const description = `${preview.subtitle} ${preview.shortAddress} on actonscan.`
  const image = absoluteUrl(
    url,
    `/og/account.png?address=${encodeURIComponent(address)}&v=${OG_IMAGE_VERSION}`,
  )
  return {title, description, image, url: url.href}
}

function addressFromPath(pathname: string) {
  const match = pathname.match(/^\/address\/([^/?#]+)$/)
  if (!match) {
    return undefined
  }

  try {
    return decodeURIComponent(match[1] || "").trim()
  } catch {
    return match[1]?.trim()
  }
}

function injectMetadata(html: string, metadata: RouteMetadata) {
  return html
    .replace(/<title>.*?<\/title>/, `<title>${escapeHtml(metadata.title)}</title>`)
    .replace(
      /<meta\s+name="description"\s+content="[^"]*"\s*\/>/,
      `<meta name="description" content="${escapeHtml(metadata.description)}" />`,
    )
    .replace(
      /<meta\s+property="og:title"\s+content="[^"]*"\s*\/>/,
      `<meta property="og:title" content="${escapeHtml(metadata.title)}" />`,
    )
    .replace(
      /<meta\s+property="og:url"\s+content="[^"]*"\s*\/>/,
      `<meta property="og:url" content="${escapeHtml(metadata.url)}" />`,
    )
    .replace(
      /<meta\s+property="og:description"\s+content="[^"]*"\s*\/>/,
      `<meta property="og:description" content="${escapeHtml(metadata.description)}" />`,
    )
    .replace(
      /<meta\s+property="og:image"\s+content="[^"]*"\s*\/>/,
      `<meta property="og:image" content="${escapeHtml(metadata.image)}" />`,
    )
    .replace(
      /<meta\s+name="twitter:title"\s+content="[^"]*"\s*\/>/,
      `<meta name="twitter:title" content="${escapeHtml(metadata.title)}" />`,
    )
    .replace(
      /<meta\s+name="twitter:description"\s+content="[^"]*"\s*\/>/,
      `<meta name="twitter:description" content="${escapeHtml(metadata.description)}" />`,
    )
    .replace(
      /<meta\s+name="twitter:image"\s+content="[^"]*"\s*\/>/,
      `<meta name="twitter:image" content="${escapeHtml(metadata.image)}" />`,
    )
}

async function getAccountPreview(address: string, env: Env, includeImage = false) {
  const fallback = fallbackAccountPreview(address)
  if (!address) {
    return fallback
  }

  try {
    const accountStates = await fetchToncenterJson(
      "/accountStates",
      {address, include_boc: "false"},
      env,
    )
    const jettonMasters = await fetchToncenterJson("/jetton/masters", {address}, env).catch(
      () => undefined,
    )
    const preview = previewFromResponses(address, accountStates, jettonMasters)
    if (!includeImage || !preview.image) {
      return preview
    }

    return {
      ...preview,
      image: await inlineImage(preview.image),
    }
  } catch {
    return fallback
  }
}

function fallbackAccountPreview(address: string): AccountOgPreview {
  const shortAddress = address ? formatAddress(address) : "actonscan"
  return {
    title: shortAddress,
    subtitle: "TON account",
    shortAddress,
    rawAddress: address || "Open-source TON explorer",
    status: undefined,
    type: undefined,
    detail: undefined,
    image: undefined,
    avatarText: "A",
  }
}

async function fetchToncenterJson(
  pathname: string,
  searchParams: Record<string, string>,
  env: Env,
) {
  const baseUrl =
    env.TONCENTER_API_V3_URL?.trim() ||
    env.VITE_EXPLORER_TONCENTER_API_V3_URL?.trim() ||
    "https://toncenter.com/api/v3"
  const url = new URL(`${baseUrl.replace(/\/$/, "")}/${pathname.replace(/^\//, "")}`)
  for (const [key, value] of Object.entries(searchParams)) {
    url.searchParams.append(key, value)
  }

  const headers = new Headers()
  const apiKey = env.TONCENTER_API_KEY?.trim() || env.VITE_EXPLORER_TONCENTER_API_KEY?.trim()
  if (apiKey) {
    headers.set("X-API-Key", apiKey)
  }

  const response = await fetch(url, {headers})
  if (!response.ok) {
    throw new Error(`Toncenter request failed: ${response.status}`)
  }
  return response.json()
}

function previewFromResponses(
  address: string,
  accountStates: Record<string, unknown>,
  jettonMasters: Record<string, unknown> | undefined,
): AccountOgPreview {
  const fallback = fallbackAccountPreview(address)
  const accounts = arrayValue(accountStates.accounts)
  const account = recordValue(accounts?.[0])
  const accountAddress = stringValue(account?.address) || address
  const addressBookRecords = recordValue(accountStates.address_book)
  const addressBook =
    recordValue(addressBookRecords?.[accountAddress]) || recordValue(addressBookRecords?.[address])
  const interfaces = arrayValue(account?.interfaces) || arrayValue(addressBook?.interfaces) || []
  const jettonMasterRecords = arrayValue(jettonMasters?.jetton_masters)
  const jettonMaster = recordValue(jettonMasterRecords?.[0])
  const tokenInfo =
    tokenInfoForAddress(recordValue(accountStates.metadata), accountAddress) ||
    tokenInfoForAddress(recordValue(jettonMasters?.metadata), accountAddress)
  const jettonContent = {
    ...(isRecord(jettonMaster?.jetton_content) ? jettonMaster.jetton_content : {}),
    ...(isRecord(tokenInfo?.extra) ? tokenInfo.extra : {}),
  }

  const name =
    stringValue(jettonContent.name) ||
    stringValue(tokenInfo?.name) ||
    stringValue(addressBook?.domain) ||
    fallback.title
  const symbol = stringValue(jettonContent.symbol) || stringValue(tokenInfo?.symbol)
  const image = tokenImage(jettonContent, tokenInfo)
  const status = formatStatus(
    stringValue(account?.status) ||
      stringValue(account?.account_status) ||
      stringValue(addressBook?.status),
  )
  const type = formatAccountType(interfaces, jettonMaster)
  return {
    title: name,
    subtitle: type || "TON account",
    shortAddress: fallback.shortAddress,
    rawAddress: address,
    status,
    type,
    detail: undefined,
    image,
    avatarText: avatarText(name, symbol),
  }
}

function tokenInfoForAddress(metadata: Record<string, unknown> | undefined, address: string) {
  const tokenInfoRecords = recordValue(metadata?.[address])?.token_info
  const records = arrayValue(tokenInfoRecords)
  return records?.find(info => recordValue(info)?.type === "jetton_masters") as
    | Record<string, unknown>
    | undefined
}

function formatAddress(address: string) {
  if (address.length <= 18) {
    return address
  }
  return `${address.slice(0, 8)}…${address.slice(-6)}`
}

function formatStatus(status: string | undefined) {
  const value = status?.toLowerCase()
  if (!value) {
    return undefined
  }
  return value === "active" ? "Active" : value.charAt(0).toUpperCase() + value.slice(1)
}

function formatAccountType(values: unknown[], jettonMaster: Record<string, unknown> | undefined) {
  const interfaces = values.filter((value): value is string => typeof value === "string")
  if (interfaces.includes("dedust_pool")) {
    return "DeDust Pool"
  }
  if (interfaces.includes("moon_pool")) {
    return "Moon Pool"
  }
  if (jettonMaster || interfaces.includes("jetton_master")) {
    return "Jetton Master"
  }
  if (interfaces.includes("jetton_wallet")) {
    return "Jetton Wallet"
  }
  if (interfaces.includes("nft_item")) {
    return "NFT Item"
  }
  if (interfaces.includes("nft_collection")) {
    return "NFT Collection"
  }
  const wallet = interfaces.find(iface => /^wallet_v\d+r\d+$/i.test(iface))
  if (wallet) {
    return wallet.replace(/^wallet_/i, "wallet ")
  }
  return undefined
}

function avatarText(name: string, symbol: string | undefined) {
  return (symbol || name || "A").trim().charAt(0).toUpperCase()
}

function tokenImage(
  content: Record<string, unknown>,
  tokenInfo: Record<string, unknown> | undefined,
) {
  const candidates = [
    stringValue(tokenInfo?.image),
    stringValue(content.image),
    stringValue(content._image_big),
    stringValue(content._image_medium),
    stringValue(content._image_small),
  ].filter((value): value is string => Boolean(value))
  return candidates.find(isDataImage) || candidates[0]
}

async function inlineImage(url: string) {
  if (isDataImage(url)) {
    return url
  }

  try {
    const response = await fetch(url)
    if (!response.ok) {
      return undefined
    }

    const contentType = response.headers.get("content-type") || "image/png"
    const bytes = new Uint8Array(await response.arrayBuffer())
    let binary = ""
    for (const byte of bytes) {
      binary += String.fromCharCode(byte)
    }
    return `data:${contentType};base64,${btoa(binary)}`
  } catch {
    return undefined
  }
}

function isDataImage(value: string) {
  return value.startsWith("data:image/")
}

function recordValue(value: unknown): Record<string, unknown> | undefined {
  return isRecord(value) ? value : undefined
}

function arrayValue(value: unknown): unknown[] | undefined {
  return Array.isArray(value) ? value : undefined
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function stringValue(value: unknown) {
  return typeof value === "string" && value.length > 0 ? value : undefined
}

function absoluteUrl(url: URL, pathname: string) {
  return `${url.protocol}//${url.host}${pathname}`
}

function withHeader(headers: Headers, name: string, value: string) {
  const next = new Headers(headers)
  next.set(name, value)
  return next
}

function escapeHtml(value: string) {
  return value.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;")
}
