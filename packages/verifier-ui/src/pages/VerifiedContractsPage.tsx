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
  Pagination,
} from "@acton/ui"
import {ChartPie} from "lucide-react"
import {useEffect, useMemo, useRef, useState} from "react"
import type {MouseEvent as ReactMouseEvent} from "react"

import type {LastVerifiedItem, VerifierApi} from "../lib/api"
import {shortenMiddle} from "../lib/target"
import styles from "./VerifiedPage.module.css"

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

function handleLinkClick(event: ReactMouseEvent<HTMLAnchorElement>, onOpen: () => void): void {
  event.stopPropagation()
  if (event.button !== 0 || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) {
    return
  }

  event.preventDefault()
  onOpen()
}

export interface VerifiedContractsPageProps {
  readonly api: VerifierApi
  readonly getContractHref: (item: LastVerifiedItem) => string
  readonly onOpenContract: (item: LastVerifiedItem) => void
  readonly page?: number
  readonly onPageChange?: (page: number) => void
  readonly onContentReady?: () => void
  readonly statisticsHref?: string
  readonly onOpenStatistics?: () => void
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
  statisticsHref,
  onOpenStatistics,
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

      <DataTable
        title="Contracts"
        actions={
          statisticsHref ? (
            <a
              className={styles.statisticsLink}
              href={statisticsHref}
              target={statisticsHref.startsWith("http") ? "_blank" : undefined}
              rel={statisticsHref.startsWith("http") ? "noreferrer" : undefined}
              onClick={
                onOpenStatistics ? event => handleLinkClick(event, onOpenStatistics) : undefined
              }
            >
              <ChartPie size={16} aria-hidden="true" />
              Statistics
            </a>
          ) : undefined
        }
        minWidth="53.75rem"
      >
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
                      onClick={event => handleLinkClick(event, () => onOpenContract(item))}
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
                        onClick={event => handleLinkClick(event, () => onOpenContract(item))}
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
                  <Pagination
                    bordered={false}
                    currentPage={page + 1}
                    totalItems={total}
                    pageSize={pageSize}
                    disabled={isLoading || Boolean(error)}
                    onPageChange={nextPage => changePage(nextPage - 1)}
                    label="Verified contracts pagination"
                  />
                </DataTableCell>
              </DataTableRow>
            </DataTableFooter>
          )}
        </DataTableTable>
      </DataTable>
    </section>
  )
}
