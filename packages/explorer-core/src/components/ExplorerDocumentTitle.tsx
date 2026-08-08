import {useLocation} from "react-router"
import {shortenMiddle} from "@acton/ui"

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
  if (relativePath === "/tokens") return "Tokens"
  if (relativePath === "/abi") return "ABI"
  if (relativePath === "/cell") return "Cell Inspector"
  if (relativePath === "/emulate") return "Emulate Transaction"
  if (relativePath === "/sources") return "Verified Sources"
  if (relativePath === "/favorites") return "Favorites"
  if (relativePath === "/suspended") return "Suspended Addresses"

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
  if (transactionHash) return `Transaction ${shortenMiddle(transactionHash, {start: 6, end: 6})}`

  const blockMatch = relativePath.match(/^\/block\/-?\d+\/[^/]+\/(\d+)$/)
  if (blockMatch?.[1]) return `Block ${blockMatch[1]}`

  return undefined
}

function explorerRelativePath(pathname: string, rootPath: string): string | undefined {
  if (rootPath === "/") return pathname
  if (pathname === rootPath) return "/"
  if (pathname.startsWith(`${rootPath}/`)) return pathname.slice(rootPath.length)
  const localnetRoot = rootPath.endsWith("/explorer") ? rootPath.slice(0, -"/explorer".length) : ""
  if (pathname.startsWith(`${localnetRoot}/block/`)) {
    return pathname.slice(localnetRoot.length)
  }
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
