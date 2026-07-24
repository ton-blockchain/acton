import {useLocation} from "react-router-dom"

import {useAddressName} from "../hooks/useAddressBook"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useNetworkInfo} from "../hooks/useNetworkInfo"

import {formatAddress, parseAddress} from "./utils"

interface ExplorerDocumentTitleProps {
  readonly productName: string
}

interface ExplorerPageTitleState {
  readonly explorerPageTitle?: unknown
}

export function ExplorerDocumentTitle({productName}: ExplorerDocumentTitleProps) {
  const pageTitle = useExplorerPageTitle()
  return <title>{pageTitle ? `${pageTitle} · ${productName}` : productName}</title>
}

export function useExplorerPageTitle(): string | undefined {
  const location = useLocation()
  const routes = useExplorerRoutePaths()
  const {addressFormat} = useNetworkInfo()
  const relativePath = explorerRelativePath(location.pathname, routes.rootPath)
  const address = matchPathSegment(relativePath, /^\/address\/([^/]+)$/)
  const addressName = useAddressName(address ?? "")

  if (!relativePath) return undefined
  if (relativePath === "/") return "Explore TON"
  if (relativePath === "/blocks") return "Blocks"
  if (relativePath === "/abi") return "ABI"
  if (relativePath === "/cell") return "Cell Inspector"
  if (relativePath === "/emulate") return "Emulate Transaction"
  if (relativePath === "/sources") return "Verified Sources"
  if (relativePath === "/favorites") return "Favorites"

  if (address) {
    const preferredTitle = pageTitleFromState(location.state) ?? addressName
    if (!preferredTitle) return formatAddress(address, true, addressFormat)
    return parseAddress(preferredTitle)
      ? formatAddress(preferredTitle, true, addressFormat)
      : preferredTitle
  }

  const abiSlug = matchPathSegment(relativePath, /^\/abi\/([^/]+)$/)
  if (abiSlug) return `${abiSlug} ABI`

  const transactionHash = matchPathSegment(relativePath, /^\/tx\/([^/]+)(?:\/trace)?$/)
  if (transactionHash) return `Transaction ${shortenIdentifier(transactionHash)}`

  const blockMatch = relativePath.match(/^\/block\/-?\d+\/[^/]+\/(\d+)$/)
  if (blockMatch?.[1]) return `Block ${blockMatch[1]}`

  return undefined
}

function explorerRelativePath(pathname: string, rootPath: string): string | undefined {
  if (rootPath === "/") return pathname
  if (pathname === rootPath) return "/"
  if (pathname.startsWith(`${rootPath}/`)) return pathname.slice(rootPath.length)
  if (pathname.startsWith("/block/")) return pathname
  return undefined
}

function matchPathSegment(pathname: string | undefined, pattern: RegExp): string | undefined {
  const value = pathname?.match(pattern)?.[1]
  if (!value) return undefined

  try {
    return decodeURIComponent(value)
  } catch {
    return value
  }
}

function pageTitleFromState(state: unknown): string | undefined {
  if (!state || typeof state !== "object") return undefined
  const title = (state as ExplorerPageTitleState).explorerPageTitle
  return typeof title === "string" && title.trim() ? title.trim() : undefined
}

function shortenIdentifier(value: string): string {
  return value.length > 13 ? `${value.slice(0, 6)}…${value.slice(-6)}` : value
}
