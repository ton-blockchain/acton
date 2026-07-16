import {SearchInput, useToast} from "@acton/ui"
import type {SearchInputItem} from "@acton/ui"
import {FileCode2, History, Search} from "lucide-react"
import {useCallback, useEffect, useMemo, useState} from "react"
import type {FC} from "react"
import {useNavigate} from "react-router-dom"
import type {NavigateFunction} from "react-router-dom"

import {
  getBundledCompilerAbiCatalog,
  type BundledCompilerAbiCatalogEntry,
} from "../api/compilerAbiCatalog"
import {EXPLORER_HISTORY_STORAGE_KEY} from "../explorerResume"
import {useAddressBook} from "../hooks/useAddressBook"
import type {TonAssetsNameMatch} from "../hooks/useAddressBook"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import type {ExplorerRoutes} from "../hooks/explorerRoutesContext"
import {useAddressFormat} from "../hooks/useNetworkInfo"

import {formatAddress, hashToHex, parseAddress} from "./utils"
import type {AddressFormatOptions} from "./utils"

import {abiSymbolAnchorId} from "./abiAnchors"
type ExplorerSearchVariant = "hero" | "header"

interface ExplorerSearchProps {
  readonly autoFocus?: boolean
  readonly className?: string
  readonly variant?: ExplorerSearchVariant
}

interface SearchTarget {
  readonly displayValue: string
  readonly path: string
}

interface AbiSearchIndexEntry {
  readonly slug: string
  readonly name: string
  readonly detail: string
  readonly kind: "Contract" | "Declaration" | "Get method"
  readonly opcode?: string
  readonly targetHash?: string
  readonly searchText: string
}

const MAX_HISTORY_ITEMS = 5
const MAX_ABI_SEARCH_MATCHES = 6
const INVALID_SEARCH_DESCRIPTION = "Paste a valid TON address, transaction hash, or ABI name."
const OPCODE_NOT_FOUND_DESCRIPTION = "No ABI declaration found for opcode"

export const ExplorerSearch: FC<ExplorerSearchProps> = ({
  autoFocus = false,
  className,
  variant = "hero",
}) => {
  const addressFormat = useAddressFormat()
  const routes = useExplorerRoutePaths()
  const navigate = useNavigate()
  const {showToast} = useToast()
  const {searchTonAssetsNames} = useAddressBook()
  const [input, setInput] = useState("")
  const [history, setHistory] = useState<readonly string[]>([])
  const [abiSearchIndex, setAbiSearchIndex] = useState<readonly AbiSearchIndexEntry[]>([])
  const [isInvalid, setIsInvalid] = useState(false)
  const hasQuery = input.trim().length > 0
  const tonAssetsNameMatches = searchTonAssetsNames(input)
  const abiNameMatches = useMemo(
    () => searchAbiIndex(input, abiSearchIndex, MAX_ABI_SEARCH_MATCHES),
    [abiSearchIndex, input],
  )
  const visibleHistory = hasQuery ? [] : history

  useEffect(() => {
    setHistory(readSearchHistory())
  }, [])

  useEffect(() => {
    let isActive = true

    void getBundledCompilerAbiCatalog().then(entries => {
      if (isActive) {
        setAbiSearchIndex(buildAbiSearchIndex(entries))
      }
    })

    return () => {
      isActive = false
    }
  }, [])

  const persistHistory = useCallback((nextHistory: readonly string[]) => {
    setHistory(nextHistory)
    localStorage.setItem(EXPLORER_HISTORY_STORAGE_KEY, JSON.stringify(nextHistory))
  }, [])

  const addToHistory = useCallback(
    (value: string) => {
      const nextHistory = [value, ...history.filter(item => item !== value)].slice(
        0,
        MAX_HISTORY_ITEMS,
      )
      persistHistory(nextHistory)
    },
    [history, persistHistory],
  )

  const removeFromHistory = useCallback(
    (value: string) => {
      const nextHistory = history.filter(item => item !== value)
      persistHistory(nextHistory)
    },
    [history, persistHistory],
  )

  const handleSearch = useCallback(
    (value: string) => {
      const target = resolveSearchTarget(value, addressFormat, routes)
      if (!target) {
        const [nameMatch] = searchTonAssetsNames(value, 1)
        if (nameMatch) {
          openTonAssetsNameMatch({
            match: nameMatch,
            addressFormat,
            routes,
            navigate,
            addToHistory,
            setInput,
          })
          return true
        }

        const [abiMatch] = searchAbiIndex(value, abiSearchIndex, 1)
        if (abiMatch) {
          openAbiNameMatch({
            match: abiMatch,
            routes,
            navigate,
            addToHistory,
            setInput,
          })
          return true
        }

        if (!value.trim()) return false

        setIsInvalid(true)
        const opcode = normalizeOpcodeSearchQuery(value)
        if (opcode) {
          showToast({
            title: "Opcode not found",
            description: `${OPCODE_NOT_FOUND_DESCRIPTION} ${opcode}.`,
            variant: "error",
          })
          return false
        }

        showToast({
          title: "Invalid search",
          description: INVALID_SEARCH_DESCRIPTION,
          variant: "error",
        })
        return false
      }

      setInput("")
      setIsInvalid(false)
      addToHistory(target.displayValue)
      void navigate(target.path)
      return true
    },
    [
      abiSearchIndex,
      addToHistory,
      addressFormat,
      navigate,
      routes,
      searchTonAssetsNames,
      showToast,
    ],
  )

  const dropdownItems: readonly SearchInputItem[] = [
    ...tonAssetsNameMatches.map(match => ({
      id: `tonassets:${match.address}`,
      label: match.name,
      description: formatAddress(match.address, false, addressFormat),
      icon: <Search size={16} />,
      onSelect: () =>
        openTonAssetsNameMatch({
          match,
          addressFormat,
          routes,
          navigate,
          addToHistory,
          setInput,
        }),
    })),
    ...abiNameMatches.map(match => ({
      id: `abi:${match.slug}:${match.kind}:${match.name}`,
      label: match.name,
      description: `ABI · ${match.kind} · ${match.detail}${match.opcode ? ` · ${match.opcode}` : ""}`,
      icon: <FileCode2 size={16} />,
      onSelect: () =>
        openAbiNameMatch({
          match,
          routes,
          navigate,
          addToHistory,
          setInput,
        }),
    })),
    ...visibleHistory.map(item => ({
      id: `history:${item}`,
      label: formatHistoryItem(item, addressFormat),
      icon: <History size={16} />,
      onSelect: () => handleSearch(item),
      onRemove: () => removeFromHistory(item),
      removeLabel: "Remove from history",
    })),
  ]

  return (
    <SearchInput
      ariaLabel="Explorer search"
      autoFocus={autoFocus}
      className={className}
      invalid={isInvalid}
      items={dropdownItems}
      onSubmit={handleSearch}
      onValueChange={nextInput => {
        setInput(nextInput)
        if (isInvalid) {
          setIsInvalid(false)
        }
      }}
      placeholder="Search by address or hash"
      size={variant === "header" ? "sm" : "lg"}
      value={input}
    />
  )
}

function resolveSearchTarget(
  value: string,
  addressFormat: AddressFormatOptions,
  routes: ExplorerRoutes,
): SearchTarget | undefined {
  const trimmed = value.trim()
  if (!trimmed) {
    return undefined
  }

  const parsedAddress = parseAddress(trimmed)
  if (parsedAddress) {
    const displayAddress = parsedAddress.toString(addressFormat)
    return {
      displayValue: displayAddress,
      path: routes.addressPath(displayAddress),
    }
  }

  const transactionHash = hashToHex(trimmed)
  if (transactionHash) {
    return {
      displayValue: transactionHash,
      path: routes.transactionPath(transactionHash),
    }
  }

  return undefined
}

function formatHistoryItem(value: string, addressFormat: AddressFormatOptions): string {
  const parsedAddress = parseAddress(value)
  if (parsedAddress) {
    return formatAddress(parsedAddress.toString(addressFormat), false, addressFormat)
  }

  return hashToHex(value) ?? value
}

function openTonAssetsNameMatch({
  match,
  addressFormat,
  routes,
  navigate,
  addToHistory,
  setInput,
}: {
  readonly match: TonAssetsNameMatch
  readonly addressFormat: AddressFormatOptions
  readonly routes: ExplorerRoutes
  readonly navigate: NavigateFunction
  readonly addToHistory: (value: string) => void
  readonly setInput: (value: string) => void
}) {
  const displayAddress = parseAddress(match.address)?.toString(addressFormat) ?? match.address
  setInput("")
  addToHistory(displayAddress)
  void navigate(routes.addressPath(displayAddress))
}

function openAbiNameMatch({
  match,
  routes,
  navigate,
  addToHistory,
  setInput,
}: {
  readonly match: AbiSearchIndexEntry
  readonly routes: ExplorerRoutes
  readonly navigate: NavigateFunction
  readonly addToHistory: (value: string) => void
  readonly setInput: (value: string) => void
}) {
  setInput("")
  addToHistory(match.name)
  const path = routes.abiDetailsPath(match.slug)
  void navigate(match.targetHash ? `${path}#${match.targetHash}` : path)
}

function buildAbiSearchIndex(
  entries: readonly BundledCompilerAbiCatalogEntry[],
): readonly AbiSearchIndexEntry[] {
  const index = new Map<string, AbiSearchIndexEntry>()

  const addEntry = (entry: AbiSearchIndexEntry) => {
    const key = `${entry.slug}:${entry.kind}:${entry.name}:${entry.targetHash ?? ""}`
    if (!index.has(key)) {
      index.set(key, entry)
    }
  }

  for (const entry of entries) {
    const contractName = entry.compiler_abi.contract_name
    const displayName = entry.display_name ?? contractName
    addEntry({
      slug: entry.slug,
      name: displayName,
      detail: contractName,
      kind: "Contract",
      searchText: `${displayName} ${contractName}`,
    })

    for (const declaration of entry.compiler_abi.declarations) {
      const opcode = declarationOpcode(declaration)
      addEntry({
        slug: entry.slug,
        name: declaration.name,
        detail: displayName,
        kind: "Declaration",
        opcode,
        targetHash: abiSymbolAnchorId("declaration", declaration.name),
        searchText: `${declaration.name} ${displayName} ${contractName}`,
      })
    }

    for (const method of entry.compiler_abi.get_methods) {
      addEntry({
        slug: entry.slug,
        name: method.name,
        detail: contractName,
        kind: "Get method",
        targetHash: abiSymbolAnchorId("get-method", method.name),
        searchText: `${method.name} ${displayName} ${contractName}`,
      })
    }
  }

  return [...index.values()]
}

function declarationOpcode(
  declaration: BundledCompilerAbiCatalogEntry["compiler_abi"]["declarations"][number],
): string | undefined {
  if (!("prefix" in declaration) || declaration.prefix?.prefix_len !== 32) {
    return undefined
  }

  const opcode = declaration.prefix.prefix_num >>> 0
  return `0x${opcode.toString(16).padStart(8, "0")}`
}

function searchAbiIndex(
  query: string,
  index: readonly AbiSearchIndexEntry[],
  limit: number,
): readonly AbiSearchIndexEntry[] {
  const normalizedQuery = normalizeSearchText(query)
  if (normalizedQuery.length < 2 || limit <= 0) {
    return []
  }

  return index
    .map(entry => {
      const normalizedName = normalizeSearchText(entry.name)
      const normalizedSearchText = normalizeSearchText(entry.searchText)
      const normalizedOpcode = entry.opcode ? normalizeSearchText(entry.opcode) : undefined
      const matchesText = normalizedSearchText.includes(normalizedQuery)
      const matchesOpcode =
        normalizedQuery.startsWith("0x") && normalizedOpcode?.includes(normalizedQuery)

      if (!matchesText && !matchesOpcode) {
        return undefined
      }

      return {
        entry,
        score: getSearchMatchScore(normalizedName, normalizedSearchText, normalizedQuery),
      }
    })
    .filter((item): item is {readonly entry: AbiSearchIndexEntry; readonly score: number} =>
      Boolean(item),
    )
    .sort(
      (left, right) =>
        left.score - right.score ||
        left.entry.name.localeCompare(right.entry.name) ||
        left.entry.detail.localeCompare(right.entry.detail),
    )
    .slice(0, limit)
    .map(item => item.entry)
}

function normalizeOpcodeSearchQuery(value: string): string | undefined {
  const trimmed = value.trim()
  return /^0x[0-9a-f]{8}$/i.test(trimmed) ? trimmed.toLowerCase() : undefined
}

function normalizeSearchText(value: string): string {
  return value.trim().toLocaleLowerCase()
}

function getSearchMatchScore(
  normalizedName: string,
  normalizedSearchText: string,
  normalizedQuery: string,
): number {
  if (normalizedName === normalizedQuery) {
    return 0
  }
  if (normalizedName.startsWith(normalizedQuery)) {
    return 1
  }
  if (normalizedSearchText.startsWith(normalizedQuery)) {
    return 2
  }
  return 3
}

function readSearchHistory(): readonly string[] {
  const savedHistory = localStorage.getItem(EXPLORER_HISTORY_STORAGE_KEY)
  if (!savedHistory) {
    return []
  }

  try {
    const parsed = JSON.parse(savedHistory)
    return Array.isArray(parsed)
      ? parsed.filter((item): item is string => typeof item === "string")
      : []
  } catch (error) {
    console.error("Failed to parse explorer search history", error)
    return []
  }
}
