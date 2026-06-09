import {Check, CircleAlert, RefreshCw} from "lucide-react"
import * as React from "react"
import {
  Button,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@acton/shared-ui"

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

export const ApiCallsPage: React.FC<ApiCallsPageProps> = ({client}) => {
  const [calls, setCalls] = React.useState<readonly ApiCallRecord[]>([])
  const [statusFilter, setStatusFilter] = React.useState<StatusFilter>(DEFAULT_STATUS_FILTER)
  const [isLoading, setIsLoading] = React.useState(true)
  const [isRefreshing, setIsRefreshing] = React.useState(false)
  const [error, setError] = React.useState<string>()

  const loadCalls = React.useCallback(
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

  React.useEffect(() => {
    void loadCalls()
  }, [loadCalls])

  const filteredCalls = React.useMemo(
    () => calls.filter(call => statusFilter[call.status]),
    [calls, statusFilter],
  )
  const successCount = React.useMemo(
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
            <label className={styles.rpcCallsFilter}>
              <input
                type="checkbox"
                checked={statusFilter.success}
                onChange={() => toggleStatusFilter("success")}
              />
              <span className={styles.rpcFilterCheckbox}>
                <Check size={13} strokeWidth={2.5} />
              </span>
              <span>Success</span>
              <span className={styles.rpcFilterCount}>{successCount}</span>
            </label>
            <label className={styles.rpcCallsFilter}>
              <input
                type="checkbox"
                checked={statusFilter.failed}
                onChange={() => toggleStatusFilter("failed")}
              />
              <span className={styles.rpcFilterCheckbox}>
                <Check size={13} strokeWidth={2.5} />
              </span>
              <span>Failed</span>
              <span className={styles.rpcFilterCount}>{failedCount}</span>
            </label>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={isRefreshing}
            onClick={() => void loadCalls(true)}
          >
            <RefreshCw size={14} className={isRefreshing ? styles.spinning : ""} />
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
          <div className={styles.rpcCallsTableFrame}>
            <Table className={styles.rpcCallsTable}>
              <TableHeader>
                <TableRow>
                  <TableHead>Status</TableHead>
                  <TableHead>Status Code</TableHead>
                  <TableHead>Call Type</TableHead>
                  <TableHead>Method</TableHead>
                  <TableHead>Duration</TableHead>
                  <TableHead>Timestamp</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {[...filteredCalls].reverse().map(call => (
                  <TableRow key={call.sequence}>
                    <TableCell className={styles.rpcStatusCell}>
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
                    </TableCell>
                    <TableCell className={styles.rpcCodeCell}>{call.status_code}</TableCell>
                    <TableCell className={styles.rpcTypeCell}>{call.call_type}</TableCell>
                    <TableCell className={styles.rpcMethodCell}>{call.method}</TableCell>
                    <TableCell className={styles.rpcDurationCell}>{call.duration_ms} ms</TableCell>
                    <TableCell className={styles.rpcTimestampCell}>
                      {formatTimestamp(call.timestamp_ms)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
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
