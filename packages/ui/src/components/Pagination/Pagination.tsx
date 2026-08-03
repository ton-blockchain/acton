import {ChevronLeft, ChevronRight} from "lucide-react"
import {useEffect, useMemo, useState} from "react"
import type {Dispatch, SetStateAction} from "react"

import {cx} from "../../lib/cx"
import {Button} from "../Button"
import styles from "./Pagination.module.css"

export const DEFAULT_PAGE_SIZE = 20

type PaginationItem = number | "ellipsis-left" | "ellipsis-right"

export interface PaginationProps {
  readonly bordered?: boolean
  readonly className?: string
  readonly currentPage: number
  readonly disabled?: boolean
  readonly label?: string
  readonly onPageChange: (page: number) => void
  readonly pageSize?: number
  readonly totalItems: number
}

export interface PaginationState {
  readonly currentPage: number
  readonly endIndex: number
  readonly pageSize: number
  readonly startIndex: number
  readonly totalItems: number
  readonly totalPages: number
}

export interface ClientPagination<T> extends PaginationState {
  readonly currentItems: readonly T[]
  readonly setCurrentPage: Dispatch<SetStateAction<number>>
}

export function Pagination({
  bordered = true,
  className,
  currentPage,
  disabled = false,
  label = "Pagination",
  onPageChange,
  pageSize = DEFAULT_PAGE_SIZE,
  totalItems,
}: PaginationProps) {
  const state = getPaginationState(totalItems, currentPage, pageSize)
  const items = getPaginationItems(state.currentPage, state.totalPages)

  if (state.totalPages <= 1) {
    return null
  }

  return (
    <nav
      className={cx(styles.pagination, bordered && styles.bordered, className)}
      aria-label={label}
    >
      <span className={styles.summary}>
        {state.startIndex + 1}–{state.endIndex} of {state.totalItems.toLocaleString()}
      </span>
      <div className={styles.controls}>
        <Button
          size="sm"
          variant="outline"
          leadingIcon={<ChevronLeft size={15} />}
          disabled={disabled || state.currentPage === 1}
          aria-label="Previous page"
          onClick={() => onPageChange(state.currentPage - 1)}
        >
          Previous
        </Button>
        <div className={styles.pages}>
          {items.map(item =>
            typeof item === "number" ? (
              <Button
                key={item}
                className={styles.page}
                size="sm"
                variant={item === state.currentPage ? "secondary" : "ghost"}
                disabled={disabled}
                aria-current={item === state.currentPage ? "page" : undefined}
                aria-label={`Go to page ${item}`}
                onClick={() => onPageChange(item)}
              >
                {item}
              </Button>
            ) : (
              <span key={item} className={styles.ellipsis} aria-hidden="true">
                …
              </span>
            ),
          )}
        </div>
        <Button
          size="sm"
          variant="outline"
          trailingIcon={<ChevronRight size={15} />}
          disabled={disabled || state.currentPage === state.totalPages}
          aria-label="Next page"
          onClick={() => onPageChange(state.currentPage + 1)}
        >
          Next
        </Button>
      </div>
    </nav>
  )
}

export function useClientPagination<T>(
  items: readonly T[],
  pageSize = DEFAULT_PAGE_SIZE,
): ClientPagination<T> {
  const [requestedPage, setCurrentPage] = useState(1)
  const state = getPaginationState(items.length, requestedPage, pageSize)
  const currentItems = useMemo(
    () => items.slice(state.startIndex, state.endIndex),
    [items, state.endIndex, state.startIndex],
  )

  useEffect(() => {
    if (requestedPage !== state.currentPage) {
      setCurrentPage(state.currentPage)
    }
  }, [requestedPage, state.currentPage])

  return {
    ...state,
    currentItems,
    setCurrentPage,
  }
}

export function getPaginationState(
  totalItems: number,
  currentPage: number,
  pageSize = DEFAULT_PAGE_SIZE,
): PaginationState {
  const normalizedTotalItems = Number.isFinite(totalItems) ? Math.max(0, Math.trunc(totalItems)) : 0
  const normalizedPageSize = Number.isFinite(pageSize)
    ? Math.max(1, Math.trunc(pageSize))
    : DEFAULT_PAGE_SIZE
  const totalPages = Math.max(1, Math.ceil(normalizedTotalItems / normalizedPageSize))
  const normalizedCurrentPage = Number.isFinite(currentPage)
    ? Math.min(totalPages, Math.max(1, Math.trunc(currentPage)))
    : 1
  const startIndex = (normalizedCurrentPage - 1) * normalizedPageSize

  return {
    currentPage: normalizedCurrentPage,
    endIndex: Math.min(normalizedTotalItems, startIndex + normalizedPageSize),
    pageSize: normalizedPageSize,
    startIndex,
    totalItems: normalizedTotalItems,
    totalPages,
  }
}

export function getPaginationItems(
  currentPage: number,
  totalPages: number,
): readonly PaginationItem[] {
  if (totalPages <= 7) {
    return Array.from({length: totalPages}, (_, index) => index + 1)
  }

  if (currentPage <= 4) {
    return [1, 2, 3, 4, 5, "ellipsis-right", totalPages]
  }

  if (currentPage >= totalPages - 3) {
    return [
      1,
      "ellipsis-left",
      totalPages - 4,
      totalPages - 3,
      totalPages - 2,
      totalPages - 1,
      totalPages,
    ]
  }

  return [
    1,
    "ellipsis-left",
    currentPage - 1,
    currentPage,
    currentPage + 1,
    "ellipsis-right",
    totalPages,
  ]
}
