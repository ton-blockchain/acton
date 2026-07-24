import {
  CopyInlineAction,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableFooter,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  Button,
} from "@acton/ui"
import {useEffect, useMemo, useRef, useState} from "react"
import type {MouseEvent as ReactMouseEvent} from "react"

import type {LastVerifiedItem, VerifierApi} from "../lib/api"
import {shortenMiddle} from "../lib/target"
import styles from "./VerifiedPage.module.css"

type PaginationItem = number | "ellipsis-start" | "ellipsis-end"

const PAGINATION_BUTTON_COUNT = 7

function paginationItems(currentPage: number, totalPages: number): readonly PaginationItem[] {
  if (totalPages <= PAGINATION_BUTTON_COUNT) {
    return Array.from({length: totalPages}, (_, index) => index)
  }

  const lastPage = totalPages - 1
  if (currentPage <= 3) {
    return [0, 1, 2, 3, 4, "ellipsis-end", lastPage]
  }
  if (currentPage >= lastPage - 3) {
    return [0, "ellipsis-start", lastPage - 4, lastPage - 3, lastPage - 2, lastPage - 1, lastPage]
  }

  return [
    0,
    "ellipsis-start",
    currentPage - 1,
    currentPage,
    currentPage + 1,
    "ellipsis-end",
    lastPage,
  ]
}

function formatVerifiedAt(timestamp: number): string {
  if (!Number.isFinite(timestamp) || timestamp <= 0) {
    return "Unknown"
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(timestamp * 1000))
}

function compilerLabel(item: LastVerifiedItem): string {
  const language = item.compiler.language || "unknown"
  const version = item.compiler.version || "unknown"
  return `${language} ${version}`
}

function sourceName(item: LastVerifiedItem): string {
  const abiName = item.abi_name?.trim()
  if (abiName) {
    return abiName
  }

  return item.entrypoint || "Unknown"
}

function handleContractLinkClick(
  event: ReactMouseEvent<HTMLAnchorElement>,
  item: LastVerifiedItem,
  onOpenContract: (item: LastVerifiedItem) => void,
): void {
  event.stopPropagation()
  if (event.button !== 0 || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) {
    return
  }

  event.preventDefault()
  onOpenContract(item)
}

export interface VerifiedContractsPageProps {
  readonly api: VerifierApi
  readonly getContractHref: (item: LastVerifiedItem) => string
  readonly onOpenContract: (item: LastVerifiedItem) => void
  readonly page?: number
  readonly onPageChange?: (page: number) => void
  readonly onContentReady?: () => void
  readonly limit?: number
  readonly className?: string
}

export function VerifiedContractsPage({
  api,
  getContractHref,
  onOpenContract,
  page: controlledPage,
  onPageChange,
  onContentReady,
  limit = 25,
  className,
}: VerifiedContractsPageProps) {
  const pageSize = Number.isFinite(limit) ? Math.min(Math.max(Math.trunc(limit), 1), 100) : 25
  const [internalPage, setInternalPage] = useState(0)
  const isPageControlled = controlledPage !== undefined
  const page =
    controlledPage !== undefined && Number.isFinite(controlledPage)
      ? Math.max(0, Math.trunc(controlledPage))
      : internalPage
  const [items, setItems] = useState<readonly LastVerifiedItem[]>([])
  const [total, setTotal] = useState(0)
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()
  const onPageChangeRef = useRef(onPageChange)

  useEffect(() => {
    onPageChangeRef.current = onPageChange
  }, [onPageChange])

  useEffect(() => {
    let cancelled = false

    setIsLoading(true)
    setError(undefined)

    api
      .fetchLastVerified(pageSize, page * pageSize)
      .then(response => {
        if (!cancelled) {
          const nextTotal = Math.max(0, Math.trunc(response.total))
          const lastPage = Math.max(0, Math.ceil(nextTotal / pageSize) - 1)

          setTotal(nextTotal)
          if (page > lastPage) {
            setItems([])
            if (!isPageControlled) {
              setInternalPage(lastPage)
            }
            onPageChangeRef.current?.(lastPage)
            return
          }

          setItems(response.items)
          setError(undefined)
        }
      })
      .catch(error => {
        if (!cancelled) {
          setItems([])
          setError(error instanceof Error ? error.message : String(error))
        }
      })
      .finally(() => {
        if (!cancelled) {
          setIsLoading(false)
        }
      })

    return () => {
      cancelled = true
    }
  }, [api, isPageControlled, page, pageSize])

  useEffect(() => {
    const contentReady = !isLoading && (items.length > 0 || Boolean(error) || total === 0)
    if (contentReady) {
      onContentReady?.()
    }
  }, [error, isLoading, items.length, onContentReady, total])

  const sortedItems = useMemo(
    () => [...items].sort((left, right) => right.verified_at - left.verified_at),
    [items],
  )
  const totalPages = Math.ceil(total / pageSize)
  const visiblePages = useMemo(() => paginationItems(page, totalPages), [page, totalPages])
  const changePage = (nextPage: number) => {
    const normalizedPage = Math.max(0, Math.trunc(nextPage))
    if (!isPageControlled) {
      setInternalPage(normalizedPage)
    }
    onPageChange?.(normalizedPage)
  }

  return (
    <section className={`${styles.container} ${className ?? ""}`}>
      <section className={styles.hero}>
        <h1 className={styles.title}>Verified contracts</h1>
      </section>

      <DataTable title="Contracts" minWidth="53.75rem">
        <DataTableTable aria-label="Verified contracts">
          <DataTableHead>
            <DataTableRow>
              <DataTableHeaderCell columnWidth="32%">Code hash</DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="20%">Name</DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="18%">Compiler</DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="10%">Files</DataTableHeaderCell>
              <DataTableHeaderCell>Verified at</DataTableHeaderCell>
            </DataTableRow>
          </DataTableHead>
          <DataTableBody>
            {isLoading ? (
              <DataTableSkeletonRows
                columns={5}
                rows={8}
                widths={["72%", "54%", "48%", "2.5rem", "68%"]}
              />
            ) : error ? (
              <DataTableEmpty colSpan={5}>{error}</DataTableEmpty>
            ) : sortedItems.length === 0 ? (
              <DataTableEmpty colSpan={5}>No verified contracts indexed yet</DataTableEmpty>
            ) : (
              sortedItems.map(item => (
                <DataTableRow key={item.code_hash} className={styles.contractRow} interactive>
                  <DataTableCell>
                    <a
                      className={styles.rowOverlayLink}
                      href={getContractHref(item)}
                      aria-label={`Open verified contract ${item.code_hash}`}
                      onClick={event => handleContractLinkClick(event, item, onOpenContract)}
                    >
                      <span className={styles.visuallyHidden}>
                        Open verified contract {item.code_hash}
                      </span>
                    </a>
                    <div className={styles.codeHashCell}>
                      <a
                        className={styles.codeHash}
                        href={getContractHref(item)}
                        title={item.code_hash}
                        aria-label={`Open code hash ${item.code_hash}`}
                        onClick={event => handleContractLinkClick(event, item, onOpenContract)}
                      >
                        {shortenMiddle(item.code_hash, 18, 12)}
                      </a>
                      <CopyInlineAction
                        className={styles.hashCopyButton}
                        value={item.code_hash}
                        label="Copy code hash"
                        copiedLabel="Code hash copied"
                      />
                    </div>
                  </DataTableCell>
                  <DataTableCell>
                    <span className={styles.sourceName} title={sourceName(item)}>
                      {sourceName(item)}
                    </span>
                  </DataTableCell>
                  <DataTableCell truncate title={compilerLabel(item)}>
                    {compilerLabel(item)}
                  </DataTableCell>
                  <DataTableCell>{item.file_count}</DataTableCell>
                  <DataTableCell truncate>{formatVerifiedAt(item.verified_at)}</DataTableCell>
                </DataTableRow>
              ))
            )}
          </DataTableBody>
          {(page > 0 || totalPages > 1) && (
            <DataTableFooter>
              <DataTableRow>
                <DataTableCell className={styles.paginationCell} colSpan={5}>
                  <div className={styles.pagination}>
                    <div className={styles.paginationActions}>
                      <Button
                        size="sm"
                        variant="outline"
                        disabled={isLoading || page === 0}
                        onClick={() => changePage(page - 1)}
                      >
                        Previous
                      </Button>
                      {totalPages > 0 && (
                        <nav className={styles.paginationNumbers} aria-label="Pagination">
                          {visiblePages.map(item =>
                            typeof item === "string" ? (
                              <span
                                key={item}
                                className={styles.paginationEllipsis}
                                aria-hidden="true"
                              >
                                …
                              </span>
                            ) : (
                              <Button
                                key={item}
                                className={styles.paginationNumber}
                                size="sm"
                                variant={item === page ? "secondary" : "ghost"}
                                disabled={isLoading}
                                aria-current={item === page ? "page" : undefined}
                                aria-label={`Go to page ${item + 1}`}
                                onClick={() => changePage(item)}
                              >
                                {item + 1}
                              </Button>
                            ),
                          )}
                        </nav>
                      )}
                      <Button
                        size="sm"
                        variant="outline"
                        disabled={isLoading || Boolean(error) || page + 1 >= totalPages}
                        onClick={() => changePage(page + 1)}
                      >
                        Next
                      </Button>
                    </div>
                  </div>
                </DataTableCell>
              </DataTableRow>
            </DataTableFooter>
          )}
        </DataTableTable>
      </DataTable>
    </section>
  )
}
