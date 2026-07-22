import {Check, CircleAlert, RefreshCw} from "lucide-react"
import {
  Button,
  Checkbox,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableTable,
} from "@acton/ui"
import {useCallback, useEffect, useMemo, useState} from "react"
import type {FC} from "react"

import type {TonClient} from "../../explorer/api/client"
import type {ApiCallRecord, ApiCallStatus} from "../../explorer/api/types"

import styles from "../DashboardPage.module.css"

interface ApiCallsPageProps {
  readonly client: TonClient
}

type StatusFilter = Readonly<Record<ApiCallStatus, boolean>>

const DEFAULT_STATUS_FILTER: StatusFilter = {
  success: true,
  failed: true,
}

const NANOSECONDS_PER_MICROSECOND = 1000
const NANOSECONDS_PER_MILLISECOND = NANOSECONDS_PER_MICROSECOND * 1000

export const ApiCallsPage: FC<ApiCallsPageProps> = ({client}) => {
  const [calls, setCalls] = useState<readonly ApiCallRecord[]>([])
  const [statusFilter, setStatusFilter] = useState<StatusFilter>(DEFAULT_STATUS_FILTER)
  const [isLoading, setIsLoading] = useState(true)
  const [isRefreshing, setIsRefreshing] = useState(false)
  const [error, setError] = useState<string>()

  const loadCalls = useCallback(
    async (refreshing = false) => {
      if (refreshing) {
        setIsRefreshing(true)
      } else {
        setIsLoading(true)
      }
      setError(undefined)

      try {
        const response = await client.getApiCalls(200)
        setCalls(response.calls)
      } catch (loadError) {
        setError(loadError instanceof Error ? loadError.message : "Failed to load API calls")
      } finally {
        setIsLoading(false)
        setIsRefreshing(false)
      }
    },
    [client],
  )

  useEffect(() => {
    void loadCalls()
  }, [loadCalls])

  const filteredCalls = useMemo(
    () => calls.filter(call => statusFilter[call.status]),
    [calls, statusFilter],
  )
  const successCount = useMemo(
    () => calls.filter(call => call.status === "success").length,
    [calls],
  )
  const failedCount = calls.length - successCount

  const toggleStatusFilter = (status: ApiCallStatus) => {
    setStatusFilter(current => ({...current, [status]: !current[status]}))
  }

  return (
    <>
      <section className={styles.hero}>
        <div>
          <h1 className={styles.title}>API Calls</h1>
        </div>
      </section>

      <section className={styles.rpcCallsLayout}>
        <div className={styles.rpcCallsToolbar}>
          <div className={styles.rpcCallsFilters}>
            <Checkbox
              label="Success"
              count={successCount}
              checked={statusFilter.success}
              onChange={() => toggleStatusFilter("success")}
            />
            <Checkbox
              label="Failed"
              count={failedCount}
              checked={statusFilter.failed}
              onChange={() => toggleStatusFilter("failed")}
            />
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            leadingIcon={<RefreshCw size={14} className={isRefreshing ? styles.spinning : ""} />}
            disabled={isRefreshing}
            onClick={() => void loadCalls(true)}
          >
            Refresh
          </Button>
        </div>

        {error ? (
          <div className={styles.emptyState}>{error}</div>
        ) : isLoading ? (
          <div className={styles.emptyState}>Loading API calls...</div>
        ) : calls.length === 0 ? (
          <div className={styles.emptyState}>No API calls yet.</div>
        ) : filteredCalls.length === 0 ? (
          <div className={styles.emptyState}>No calls match the selected status filters.</div>
        ) : (
          <DataTable className={styles.rpcCallsTable} minWidth="42.5rem">
            <DataTableTable aria-label="API calls" layout="fixed">
              <DataTableHead>
                <DataTableRow>
                  <DataTableHeaderCell align="center" columnWidth="4rem">
                    Status
                  </DataTableHeaderCell>
                  <DataTableHeaderCell columnWidth="7rem">Status Code</DataTableHeaderCell>
                  <DataTableHeaderCell columnWidth="8rem">Call Type</DataTableHeaderCell>
                  <DataTableHeaderCell>Method</DataTableHeaderCell>
                  <DataTableHeaderCell columnWidth="7rem">Duration</DataTableHeaderCell>
                  <DataTableHeaderCell columnWidth="12rem">Timestamp</DataTableHeaderCell>
                </DataTableRow>
              </DataTableHead>
              <DataTableBody>
                {[...filteredCalls].reverse().map(call => (
                  <DataTableRow key={call.sequence} hover>
                    <DataTableCell align="center">
                      <span
                        aria-label={call.status === "success" ? "Success" : "Failed"}
                        className={`${styles.rpcStatusIcon} ${
                          call.status === "success"
                            ? styles.rpcStatusSuccess
                            : styles.rpcStatusFailed
                        }`}
                        role="img"
                        title={call.status === "success" ? "Success" : "Failed"}
                      >
                        {call.status === "success" ? (
                          <Check size={17} />
                        ) : (
                          <CircleAlert size={17} />
                        )}
                      </span>
                    </DataTableCell>
                    <DataTableCell tone="subtle">{call.status_code}</DataTableCell>
                    <DataTableCell className={styles.rpcTypeCell} tone="muted">
                      {call.call_type}
                    </DataTableCell>
                    <DataTableCell mono truncate title={call.method}>
                      {call.method}
                    </DataTableCell>
                    <DataTableCell className={styles.rpcDurationCell} tone="muted">
                      {formatApiCallDuration(call.duration_ns)}
                    </DataTableCell>
                    <DataTableCell
                      className={styles.rpcTimestampCell}
                      tone="muted"
                      data-visual-dynamic="time"
                      data-visual-placeholder="<time>"
                    >
                      {formatTimestamp(call.timestamp_ms)}
                    </DataTableCell>
                  </DataTableRow>
                ))}
              </DataTableBody>
            </DataTableTable>
          </DataTable>
        )}
      </section>
    </>
  )
}

function formatTimestamp(timestampMs: number): string {
  if (!Number.isFinite(timestampMs) || timestampMs <= 0) {
    return "Unknown"
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "medium",
  }).format(new Date(timestampMs))
}

function formatApiCallDuration(durationNs: number): string {
  if (!Number.isFinite(durationNs) || durationNs < 0) {
    return "Unknown"
  }

  if (durationNs < NANOSECONDS_PER_MICROSECOND) {
    return `${Math.round(durationNs)} ns`
  }

  if (durationNs < NANOSECONDS_PER_MILLISECOND) {
    return `${formatDurationValue(durationNs / NANOSECONDS_PER_MICROSECOND)} µs`
  }

  const durationMs = durationNs / NANOSECONDS_PER_MILLISECOND
  if (durationMs < 10) {
    return `${formatDurationValue(durationMs)} ms`
  }

  return `${Math.round(durationMs)} ms`
}

function formatDurationValue(value: number): string {
  return value.toLocaleString(undefined, {
    maximumFractionDigits: value < 10 ? 2 : value < 100 ? 1 : 0,
  })
}
