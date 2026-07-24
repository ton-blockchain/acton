import {SearchInput} from "@acton/ui"
import {History} from "lucide-react"
import {useCallback, useEffect, useState} from "react"
import type {FC} from "react"

import {lookupPath, parseLookupTarget, shortenMiddle} from "../lib/target"

interface SearchBoxProps {
  readonly autoFocus?: boolean
  readonly className?: string
  readonly initialValue?: string
  readonly variant?: "hero" | "header"
}

interface SearchTarget {
  readonly displayValue: string
  readonly path: string
}

const MAX_HISTORY_ITEMS = 5
const VERIFIER_HISTORY_STORAGE_KEY = "verifier-search-history"

export const SearchBox: FC<SearchBoxProps> = ({
  autoFocus = false,
  className,
  initialValue = "",
  variant = "hero",
}) => {
  const [value, setValue] = useState(initialValue)
  const [history, setHistory] = useState<readonly string[]>([])
  const [isInvalid, setIsInvalid] = useState(false)
  const [showHistoryDropdown, setShowHistoryDropdown] = useState(false)
  const hasQuery = value.trim().length > 0
  const visibleHistory = hasQuery ? [] : history

  useEffect(() => {
    setHistory(readSearchHistory())
  }, [])

  const persistHistory = useCallback((nextHistory: readonly string[]) => {
    setHistory(nextHistory)
    localStorage.setItem(VERIFIER_HISTORY_STORAGE_KEY, JSON.stringify(nextHistory))
  }, [])

  const addToHistory = useCallback(
    (nextValue: string) => {
      const nextHistory = [nextValue, ...history.filter(item => item !== nextValue)].slice(
        0,
        MAX_HISTORY_ITEMS,
      )
      persistHistory(nextHistory)
    },
    [history, persistHistory],
  )

  const removeFromHistory = useCallback(
    (nextValue: string) => {
      const nextHistory = history.filter(item => item !== nextValue)
      persistHistory(nextHistory)
      setShowHistoryDropdown(nextHistory.length > 0)
    },
    [history, persistHistory],
  )

  const handleSearch = useCallback(
    (nextValue: string) => {
      const target = resolveSearchTarget(nextValue)
      if (!target) {
        if (!nextValue.trim()) return false

        setIsInvalid(true)
        return false
      }

      setValue("")
      setIsInvalid(false)
      addToHistory(target.displayValue)
      setShowHistoryDropdown(false)
      globalThis.location.assign(target.path)
      return true
    },
    [addToHistory],
  )

  return (
    <SearchInput
      ariaLabel="Verifier search"
      autoFocus={autoFocus}
      className={className}
      invalid={isInvalid}
      items={visibleHistory.map(item => ({
        id: `history:${item}`,
        label: formatHistoryItem(item),
        icon: <History size={16} />,
        onSelect: () => handleSearch(item),
        onRemove: () => removeFromHistory(item),
        removeLabel: "Remove from history",
      }))}
      open={showHistoryDropdown}
      placeholder="Search by address or hash"
      size={variant === "header" ? "sm" : "lg"}
      value={value}
      onOpenChange={setShowHistoryDropdown}
      onSubmit={handleSearch}
      onValueChange={nextValue => {
        setValue(nextValue)
        if (isInvalid) setIsInvalid(false)
      }}
    />
  )
}

function resolveSearchTarget(rawValue: string): SearchTarget | undefined {
  const trimmed = rawValue.trim()
  if (!trimmed) {
    return undefined
  }

  try {
    const target = parseLookupTarget(trimmed)
    return {
      displayValue: target.value,
      path: lookupPath(target.value),
    }
  } catch {
    return undefined
  }
}

function formatHistoryItem(value: string): string {
  try {
    const target = parseLookupTarget(value)
    return target.kind === "code_hash" ? shortenMiddle(target.value, 14, 10) : target.value
  } catch {
    return value
  }
}

function readSearchHistory(): readonly string[] {
  const savedHistory = localStorage.getItem(VERIFIER_HISTORY_STORAGE_KEY)
  if (!savedHistory) {
    return []
  }

  try {
    const parsed = JSON.parse(savedHistory)
    return Array.isArray(parsed)
      ? parsed.filter((item): item is string => typeof item === "string")
      : []
  } catch (error) {
    console.error("Failed to parse verifier search history", error)
    return []
  }
}
