import {useToast} from "@acton/shared-ui"
import {FileCode2, History, Search, X} from "lucide-react"
import {useCallback, useEffect, useMemo, useRef, useState} from "react"
import type {FC, KeyboardEvent as ReactKeyboardEvent, MouseEvent as ReactMouseEvent} from "react"
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
import styles from "./ExplorerSearch.module.css"

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
  const [isFocused, setIsFocused] = useState(false)
  const [isInvalid, setIsInvalid] = useState(false)
  const [showHistoryDropdown, setShowHistoryDropdown] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)
  const hasQuery = input.trim().length > 0
  const tonAssetsNameMatches = searchTonAssetsNames(input)
  const abiNameMatches = useMemo(
    () => searchAbiIndex(input, abiSearchIndex, MAX_ABI_SEARCH_MATCHES),
    [abiSearchIndex, input],
  )
  const visibleHistory = hasQuery ? [] : history
  const showDropdown =
    showHistoryDropdown &&
    (visibleHistory.length > 0 || tonAssetsNameMatches.length > 0 || abiNameMatches.length > 0)

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

  useEffect(() => {
    if (autoFocus) {
      inputRef.current?.focus()
    }
  }, [autoFocus])

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
    (event: ReactMouseEvent, value: string) => {
      event.stopPropagation()
      const nextHistory = history.filter(item => item !== value)
      persistHistory(nextHistory)
      setShowHistoryDropdown(nextHistory.length > 0)
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
            setShowHistoryDropdown,
          })
          return
        }

        const [abiMatch] = searchAbiIndex(value, abiSearchIndex, 1)
        if (abiMatch) {
          openAbiNameMatch({
            match: abiMatch,
            routes,
            navigate,
            addToHistory,
            setInput,
            setShowHistoryDropdown,
          })
          return
        }

        if (!value.trim()) return

        setIsInvalid(true)
        const opcode = normalizeOpcodeSearchQuery(value)
        if (opcode) {
          showToast({
            title: "Opcode not found",
            description: `${OPCODE_NOT_FOUND_DESCRIPTION} ${opcode}.`,
            variant: "error",
          })
          return
        }

        showToast({
          title: "Invalid search",
          description: INVALID_SEARCH_DESCRIPTION,
          variant: "error",
        })
        return
      }

      setInput("")
      setIsInvalid(false)
      addToHistory(target.displayValue)
      setShowHistoryDropdown(false)
      void navigate(target.path)
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

  const handleInputKeyDown = useCallback(
    (event: ReactKeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Enter") {
        handleSearch(input)
      }
    },
    [handleSearch, input],
  )

  const rootClassName = [
    styles.search,
    variant === "header" ? styles.searchHeader : styles.searchHero,
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ")

  return (
    <section className={rootClassName} aria-label="Explorer search">
      <div
        className={`${styles.inputWrapper} ${isFocused ? styles.focused : ""} ${
          isInvalid ? styles.inputInvalid : ""
        }`}
      >
        <div className={styles.searchIcon} aria-hidden="true">
          <Search size={variant === "header" ? 16 : 20} />
        </div>
        <input
          ref={inputRef}
          type="text"
          spellCheck="false"
          autoComplete="off"
          autoCorrect="off"
          className={styles.input}
          placeholder="Search by address or hash"
          value={input}
          aria-invalid={isInvalid}
          onChange={event => {
            const nextInput = event.target.value
            setInput(nextInput)
            if (isFocused) {
              setShowHistoryDropdown(true)
            }
            if (isInvalid) {
              setIsInvalid(false)
            }
          }}
          onKeyDown={handleInputKeyDown}
          onFocus={() => {
            setIsFocused(true)
            if (
              visibleHistory.length > 0 ||
              tonAssetsNameMatches.length > 0 ||
              abiNameMatches.length > 0
            ) {
              setShowHistoryDropdown(true)
            }
          }}
          onBlur={() => {
            setIsFocused(false)
            globalThis.setTimeout(() => setShowHistoryDropdown(false), 100)
          }}
          onClick={() => {
            if (
              isFocused &&
              (visibleHistory.length > 0 ||
                tonAssetsNameMatches.length > 0 ||
                abiNameMatches.length > 0)
            ) {
              setShowHistoryDropdown(true)
            }
          }}
        />
      </div>

      {showDropdown && (
        <div className={styles.historyDropdown} onMouseDown={event => event.preventDefault()}>
          {tonAssetsNameMatches.map(match => (
            <div key={`tonassets:${match.address}`} className={styles.historyItem}>
              <button
                type="button"
                className={`${styles.historyItemButton} ${styles.nameMatchButton}`}
                onClick={() =>
                  openTonAssetsNameMatch({
                    match,
                    addressFormat,
                    routes,
                    navigate,
                    addToHistory,
                    setInput,
                    setShowHistoryDropdown,
                  })
                }
              >
                <Search size={16} className={styles.historyItemIcon} aria-hidden="true" />
                <span className={styles.nameMatchText}>
                  <span className={styles.nameMatchName}>{match.name}</span>
                  <span className={styles.nameMatchAddress}>
                    {formatAddress(match.address, false, addressFormat)}
                  </span>
                </span>
              </button>
            </div>
          ))}
          {abiNameMatches.map(match => (
            <div
              key={`abi:${match.slug}:${match.kind}:${match.name}`}
              className={styles.historyItem}
            >
              <button
                type="button"
                className={`${styles.historyItemButton} ${styles.nameMatchButton}`}
                onClick={() =>
                  openAbiNameMatch({
                    match,
                    routes,
                    navigate,
                    addToHistory,
                    setInput,
                    setShowHistoryDropdown,
                  })
                }
              >
                <FileCode2 size={16} className={styles.historyItemIcon} aria-hidden="true" />
                <span className={styles.nameMatchText}>
                  <span className={styles.nameMatchName}>{match.name}</span>
                  <span className={styles.nameMatchAddress}>
                    ABI · {match.kind} · {match.detail}
                    {match.opcode ? ` · ${match.opcode}` : ""}
                  </span>
                </span>
              </button>
            </div>
          ))}
          {visibleHistory.map(item => (
            <div key={`history:${item}`} className={styles.historyItem}>
              <button
                type="button"
                className={styles.historyItemButton}
                onClick={() => handleSearch(item)}
              >
                <History size={16} className={styles.historyItemIcon} aria-hidden="true" />
                <span className={styles.historyValue}>
                  {formatHistoryItem(item, addressFormat)}
                </span>
              </button>
              <button
                type="button"
                className={styles.historyItemDeleteButton}
                onMouseDown={event => event.preventDefault()}
                onClick={event => removeFromHistory(event, item)}
                title="Remove from history"
                aria-label="Remove from history"
              >
                <X size={14} />
              </button>
            </div>
          ))}
        </div>
      )}
    </section>
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
  setShowHistoryDropdown,
}: {
  readonly match: TonAssetsNameMatch
  readonly addressFormat: AddressFormatOptions
  readonly routes: ExplorerRoutes
  readonly navigate: NavigateFunction
  readonly addToHistory: (value: string) => void
  readonly setInput: (value: string) => void
  readonly setShowHistoryDropdown: (value: boolean) => void
}) {
  const displayAddress = parseAddress(match.address)?.toString(addressFormat) ?? match.address
  setInput("")
  addToHistory(displayAddress)
  setShowHistoryDropdown(false)
  void navigate(routes.addressPath(displayAddress))
}

function openAbiNameMatch({
  match,
  routes,
  navigate,
  addToHistory,
  setInput,
  setShowHistoryDropdown,
}: {
  readonly match: AbiSearchIndexEntry
  readonly routes: ExplorerRoutes
  readonly navigate: NavigateFunction
  readonly addToHistory: (value: string) => void
  readonly setInput: (value: string) => void
  readonly setShowHistoryDropdown: (value: boolean) => void
}) {
  setInput("")
  addToHistory(match.name)
  setShowHistoryDropdown(false)
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
        targetHash: abiSymbolAnchorId("get-method", method.name, String(method.tvm_method_id)),
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
