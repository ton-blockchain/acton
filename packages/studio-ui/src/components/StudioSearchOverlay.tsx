import {Box, Boxes, CircleUserRound, File, Globe2, History, Search, Wallet} from "lucide-react"
import {useMemo, useState} from "react"
import {InlineButton, InlineLoader, SearchInput} from "@acton/ui"

import {useLocalnetRuntime} from "../localnet/LocalnetRuntimeProvider"
import {
  buildStudioSearchIndex,
  readSearchHistory,
  recentSearchResults,
  resolveSearchDestination,
  searchStudio,
  writeSearchHistory,
} from "../search/studioSearch"
import type {StudioSearchResult} from "../search/studioSearch"
import {useSearchObjects} from "../search/useSearchObjects"
import type {StudioEnvironment} from "../studioApi"

import styles from "./StudioSearch.module.css"

interface StudioSearchOverlayProps {
  readonly environments: readonly StudioEnvironment[]
  readonly open: boolean
  readonly onSelect: (path: string) => void
}

const resultIcons = {
  page: File,
  environment: Boxes,
  contract: Box,
  wallet: Wallet,
  address: CircleUserRound,
  transaction: Search,
}

/** Resolves search against the active runtime while keeping navigation and history in Studio */
export function StudioSearchOverlay({environments, open, onSelect}: StudioSearchOverlayProps) {
  const {client, environment} = useLocalnetRuntime()
  const [query, setQuery] = useState("")
  const [history, setHistory] = useState(readSearchHistory)
  const objects = useSearchObjects(client, environment, open)
  const index = useMemo(
    () => buildStudioSearchIndex(environments, environment, objects.contracts, objects.wallets),
    [environments, environment, objects.contracts, objects.wallets],
  )
  const recent = recentSearchResults(history, index, environments, environment, objects.loaded)
  const results = query.trim()
    ? searchStudio(query, index, environments, environment)
    : [
        ...recent,
        ...index
          .filter(
            result =>
              !recent.some(item => item.id === result.id) &&
              (environment
                ? result.kind === "page" &&
                  result.environmentId === environment.id &&
                  ["/explorer", "/contracts", "/wallets", "/snapshots"].includes(result.value)
                : result.kind === "environment" ||
                  (result.kind === "page" && !result.environmentId)),
          )
          .sort((a, b) => Number(b.kind === "environment") - Number(a.kind === "environment"))
          .slice(0, 8),
      ]

  const selectResult = (result: StudioSearchResult) => {
    const next = [
      result,
      ...history.filter(item => resolveSearchDestination(item, environments)?.id !== result.id),
    ].slice(0, 8)
    writeSearchHistory(next)
    setHistory(next)
    onSelect(result.href)
  }

  const removeRecent = (result: StudioSearchResult) => {
    const next = history.filter(
      item => resolveSearchDestination(item, environments)?.id !== result.id,
    )
    writeSearchHistory(next)
    setHistory(next)
  }

  return (
    <>
      <div className={styles.contextRow}>
        <span className={styles.context} aria-label="Search context">
          <Globe2 size={14} aria-hidden="true" />
          {environment?.name ?? "All environments"}
        </span>
        {!query.trim() && recent.length > 0 && (
          <InlineButton
            onClick={() => {
              writeSearchHistory([])
              setHistory([])
            }}
          >
            Clear recent
          </InlineButton>
        )}
      </div>

      <SearchInput
        ariaLabel="Search Studio"
        autoFocus
        inline
        size="md"
        value={query}
        onValueChange={setQuery}
        placeholder={
          environment
            ? "Search names, pages, or paste an address or hash"
            : "Search pages, environments, or paste an address or hash"
        }
        items={results.map(result => {
          const Icon = result.group === "Recent" ? History : resultIcons[result.kind]
          return {
            id: result.id,
            label: result.title,
            description: result.description,
            icon: <Icon size={17} aria-hidden="true" />,
            group: result.group,
            onSelect: () => selectResult(result),
            onRemove: result.group === "Recent" ? () => removeRecent(result) : undefined,
            removeLabel: `Remove ${result.title} from recent`,
          }
        })}
        onSubmit={() => {
          if (!results[0]) return false
          selectResult(results[0])
        }}
        emptyContent={
          <div className={styles.empty} role="status">
            {objects.loading ? "Looking for matches…" : "No matches"}
          </div>
        }
      />

      <div className={styles.status} aria-live="polite">
        {objects.loading ? (
          <InlineLoader message="Loading contracts and wallets" />
        ) : objects.failed ? (
          <>
            <span>Some results are unavailable</span>
            <InlineButton onClick={objects.retry}>Retry</InlineButton>
          </>
        ) : environment ? (
          environment.status === "running" ? undefined : (
            <span>Contracts and wallets are available when this environment is running</span>
          )
        ) : (
          <span>Choose an environment to search its contracts and wallets</span>
        )}
      </div>
    </>
  )
}
