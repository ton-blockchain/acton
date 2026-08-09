import {Check, ChevronDown, ChevronLeft, ChevronRight, CircleAlert, RefreshCw} from "lucide-react"
import {
  Button,
  Checkbox,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  DateTime,
  Duration,
  HighlightedCode,
  InlineAction,
  RawDataBlock,
  Select,
} from "@acton/ui"
import {Fragment, useCallback, useEffect, useMemo, useState} from "react"
import type {FC} from "react"

import {
  fetchStudioApiCalls,
  type ApiCallRecord,
  type ApiCallStatus,
  type ApiCallType,
} from "../../../studioApi"

import styles from "../DashboardPage.module.css"

interface ApiCallsPageProps {
  readonly environmentId: string
}

type StatusFilter = Readonly<Record<ApiCallStatus, boolean>>
type CallTypeFilter = Readonly<Record<ApiCallType, boolean>>

const DEFAULT_STATUS_FILTER: StatusFilter = {
  success: true,
  failed: true,
}
const DEFAULT_CALL_TYPE_FILTER: CallTypeFilter = {
  read: true,
  write: true,
}
const ALL_ENDPOINTS = "all"
const DEFAULT_CALLS_PER_PAGE = 20
const CALLS_PER_PAGE_OPTIONS = [10, 20, 50, 100, 500] as const
const CALLS_PER_PAGE_STORAGE_KEY = "acton-studio.api-calls-per-page"

export const ApiCallsPage: FC<ApiCallsPageProps> = ({environmentId}) => {
  const [calls, setCalls] = useState<readonly ApiCallRecord[]>([])
  const [statusFilter, setStatusFilter] = useState<StatusFilter>(DEFAULT_STATUS_FILTER)
  const [callTypeFilter, setCallTypeFilter] = useState<CallTypeFilter>(DEFAULT_CALL_TYPE_FILTER)
  const [endpointFilter, setEndpointFilter] = useState(ALL_ENDPOINTS)
  const [showStudioRequests, setShowStudioRequests] = useState(false)
  const [callsPerPage, setCallsPerPage] = useState(readCallsPerPage)
  const [currentPage, setCurrentPage] = useState(1)
  const [expandedCalls, setExpandedCalls] = useState<ReadonlySet<number>>(() => new Set())
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
        const response = await fetchStudioApiCalls(environmentId)
        setCalls(response.calls)
      } catch (loadError) {
        setError(loadError instanceof Error ? loadError.message : "Failed to load API calls")
      } finally {
        setIsLoading(false)
        setIsRefreshing(false)
      }
    },
    [environmentId],
  )

  useEffect(() => {
    void loadCalls()
  }, [loadCalls])

  useEffect(() => {
    globalThis.localStorage.setItem(CALLS_PER_PAGE_STORAGE_KEY, String(callsPerPage))
  }, [callsPerPage])

  const endpoints = useMemo(
    () =>
      [...new Set(calls.map(call => call.method))].sort((left, right) => left.localeCompare(right)),
    [calls],
  )
  const sourceAndEndpointCalls = useMemo(() => {
    return calls.filter(
      call =>
        (showStudioRequests || call.source !== "studio_ui") &&
        (endpointFilter === ALL_ENDPOINTS || call.method === endpointFilter),
    )
  }, [calls, endpointFilter, showStudioRequests])
  const statusFilteredCalls = useMemo(
    () => sourceAndEndpointCalls.filter(call => statusFilter[call.status]),
    [sourceAndEndpointCalls, statusFilter],
  )
  const typeFilteredCalls = useMemo(
    () => sourceAndEndpointCalls.filter(call => callTypeFilter[call.call_type]),
    [callTypeFilter, sourceAndEndpointCalls],
  )
  const filteredCalls = useMemo(
    () => statusFilteredCalls.filter(call => callTypeFilter[call.call_type]),
    [callTypeFilter, statusFilteredCalls],
  )
  const orderedCalls = useMemo(() => [...filteredCalls].reverse(), [filteredCalls])
  const totalPages = Math.max(1, Math.ceil(orderedCalls.length / callsPerPage))
  const safeCurrentPage = Math.min(currentPage, totalPages)
  const firstCallIndex = (safeCurrentPage - 1) * callsPerPage
  const paginatedCalls = orderedCalls.slice(firstCallIndex, firstCallIndex + callsPerPage)
  const successCount = useMemo(
    () => typeFilteredCalls.filter(call => call.status === "success").length,
    [typeFilteredCalls],
  )
  const failedCount = typeFilteredCalls.length - successCount
  const readCount = useMemo(
    () => statusFilteredCalls.filter(call => call.call_type === "read").length,
    [statusFilteredCalls],
  )
  const writeCount = statusFilteredCalls.length - readCount
  const studioRequestCount = useMemo(
    () =>
      calls.filter(
        call =>
          call.source === "studio_ui" &&
          statusFilter[call.status] &&
          callTypeFilter[call.call_type] &&
          (endpointFilter === ALL_ENDPOINTS || call.method === endpointFilter),
      ).length,
    [callTypeFilter, calls, endpointFilter, statusFilter],
  )

  useEffect(() => {
    setCurrentPage(1)
  }, [callTypeFilter, endpointFilter, showStudioRequests, statusFilter])

  const toggleStatusFilter = (status: ApiCallStatus) => {
    setStatusFilter(current => ({...current, [status]: !current[status]}))
  }
  const toggleCallTypeFilter = (callType: ApiCallType) => {
    setCallTypeFilter(current => ({...current, [callType]: !current[callType]}))
  }
  const toggleCallDetails = (sequence: number) => {
    setExpandedCalls(current => {
      const next = new Set(current)
      if (next.has(sequence)) {
        next.delete(sequence)
      } else {
        next.add(sequence)
      }
      return next
    })
  }

  return (
    <>
      <section className={styles.rpcCallsLayout}>
        <div className={styles.rpcCallsToolbar}>
          <div className={styles.rpcCallsFilterControls}>
            <div className={styles.rpcEndpointFilter}>
              <Select
                aria-label="Filter by endpoint"
                size="sm"
                value={endpointFilter}
                onChange={event => setEndpointFilter(event.target.value)}
              >
                <option value={ALL_ENDPOINTS}>All endpoints</option>
                {endpoints.map(endpoint => (
                  <option key={endpoint} value={endpoint}>
                    {endpoint}
                  </option>
                ))}
              </Select>
            </div>
            <div className={styles.rpcCallsFilters}>
              <Checkbox
                label="Read"
                count={readCount}
                checked={callTypeFilter.read}
                onChange={() => toggleCallTypeFilter("read")}
              />
              <Checkbox
                label="Write"
                count={writeCount}
                checked={callTypeFilter.write}
                onChange={() => toggleCallTypeFilter("write")}
              />
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
              <Checkbox
                label="Studio requests"
                count={studioRequestCount}
                checked={showStudioRequests}
                onChange={() => setShowStudioRequests(current => !current)}
              />
            </div>
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

        <DataTable
          aria-busy={isLoading}
          aria-label={isLoading ? "Loading API calls" : undefined}
          className={styles.rpcCallsTable}
          minWidth="46.875rem"
        >
          <DataTableTable aria-label="API calls" layout="fixed">
            <DataTableHead>
              <DataTableRow>
                <DataTableHeaderCell
                  aria-label="Details"
                  className={styles.rpcExpandHeader}
                  columnWidth="1.875rem"
                />
                <DataTableHeaderCell align="center" columnWidth="4rem">
                  Status
                </DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="7rem">Status Code</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="7rem">Type</DataTableHeaderCell>
                <DataTableHeaderCell>Endpoint</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="7rem">Duration</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="12rem">Timestamp</DataTableHeaderCell>
              </DataTableRow>
            </DataTableHead>
            <DataTableBody>
              {error ? (
                <DataTableEmpty colSpan={7}>
                  <span role="alert">{error}</span>
                </DataTableEmpty>
              ) : isLoading ? (
                <DataTableSkeletonRows
                  alignments={["center", "center", "left", "left", "left", "left", "left"]}
                  columns={7}
                  rowKeyPrefix="api-call-row-skeleton"
                  rows={5}
                  widths={["1.875rem", "4rem", "7rem", "7rem", "16rem", "7rem", "12rem"]}
                />
              ) : filteredCalls.length === 0 ? (
                <DataTableEmpty colSpan={7}>
                  {calls.length === 0 ? "No API calls yet" : "No calls match the selected filters"}
                </DataTableEmpty>
              ) : (
                paginatedCalls.map(call => {
                  const expanded = expandedCalls.has(call.sequence)
                  const queryParams = formatRequestData(call.query_params)
                  const requestBody = formatRequestData(
                    call.request_body,
                    call.request_body_truncated,
                  )
                  const responseBody = formatRequestData(
                    call.response_body,
                    call.response_body_truncated,
                  )
                  return (
                    <Fragment key={call.sequence}>
                      <DataTableRow
                        hover
                        interactive
                        selected={expanded}
                        tabIndex={0}
                        onClick={() => toggleCallDetails(call.sequence)}
                        onKeyDown={event => {
                          if (
                            event.target === event.currentTarget &&
                            (event.key === "Enter" || event.key === " ")
                          ) {
                            event.preventDefault()
                            toggleCallDetails(call.sequence)
                          }
                        }}
                      >
                        <DataTableCell align="center" className={styles.rpcExpandCell}>
                          <InlineAction
                            aria-controls={`api-call-details-${call.sequence}`}
                            aria-expanded={expanded}
                            className={styles.rpcExpandAction}
                            icon={expanded ? <ChevronDown /> : <ChevronRight />}
                            label={expanded ? "Hide API call details" : "Show API call details"}
                            size="compact"
                          />
                        </DataTableCell>
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
                        <DataTableCell tone="muted">
                          {call.call_type === "read" ? "Read" : "Write"}
                        </DataTableCell>
                        <DataTableCell truncate title={call.method}>
                          {call.method}
                        </DataTableCell>
                        <DataTableCell className={styles.rpcDurationCell} tone="muted">
                          <Duration
                            display="latency"
                            fallback="Unknown"
                            unit="nanoseconds"
                            value={
                              Number.isFinite(call.duration_ns) && call.duration_ns >= 0
                                ? call.duration_ns
                                : undefined
                            }
                          />
                        </DataTableCell>
                        <DataTableCell className={styles.rpcTimestampCell} tone="muted">
                          <DateTime
                            display="date-time-seconds"
                            fallback="Unknown"
                            value={
                              Number.isFinite(call.timestamp_ms) && call.timestamp_ms > 0
                                ? call.timestamp_ms
                                : undefined
                            }
                          />
                        </DataTableCell>
                      </DataTableRow>
                      {expanded ? (
                        <DataTableRow className={styles.rpcDetailsRow} groupChild>
                          <DataTableCell
                            className={styles.rpcDetailsCell}
                            colSpan={7}
                            id={`api-call-details-${call.sequence}`}
                          >
                            <div className={styles.rpcDetails}>
                              <div className={styles.rpcRequestMeta}>
                                <span>{call.http_method}</span>
                                <span className={styles.rpcRequestPath}>{call.path}</span>
                              </div>
                              {queryParams === undefined ? undefined : (
                                <RawDataBlock
                                  copyLabel="Copy query parameters"
                                  customContent={
                                    <HighlightedCode
                                      className={styles.rpcRequestCode}
                                      language="json"
                                      maxHeight="18rem"
                                      value={queryParams}
                                      wrap
                                    />
                                  }
                                  title="Query parameters"
                                  value={queryParams}
                                />
                              )}
                              {requestBody === undefined ? undefined : (
                                <RawDataBlock
                                  copyLabel="Copy request body"
                                  customContent={
                                    <HighlightedCode
                                      className={styles.rpcRequestCode}
                                      language="json"
                                      maxHeight="18rem"
                                      value={requestBody}
                                      wrap
                                    />
                                  }
                                  title="Request body"
                                  value={requestBody}
                                />
                              )}
                              {queryParams === undefined && requestBody === undefined ? (
                                <div className={styles.rpcEmptyRequest}>
                                  This request has no parameters
                                </div>
                              ) : undefined}
                              {call.request_body_truncated ? (
                                <p className={styles.rpcBodyNotice}>
                                  Request body preview is limited to 64 KB
                                </p>
                              ) : undefined}
                              {responseBody === undefined ? undefined : (
                                <RawDataBlock
                                  copyLabel="Copy response body"
                                  customContent={
                                    <HighlightedCode
                                      className={styles.rpcRequestCode}
                                      language="json"
                                      maxHeight="18rem"
                                      value={responseBody}
                                      wrap
                                    />
                                  }
                                  title="Response"
                                  value={responseBody}
                                />
                              )}
                              {call.response_body_truncated ? (
                                <p className={styles.rpcBodyNotice}>
                                  Response preview is limited to 64 KB
                                </p>
                              ) : undefined}
                            </div>
                          </DataTableCell>
                        </DataTableRow>
                      ) : undefined}
                    </Fragment>
                  )
                })
              )}
            </DataTableBody>
          </DataTableTable>
        </DataTable>
        {!error && !isLoading && filteredCalls.length > 0 ? (
          <div className={styles.rpcCallsPagination}>
            <div className={styles.rpcCallsPaginationOverview}>
              <span className={styles.rpcCallsPaginationSummary}>
                {firstCallIndex + 1}–{Math.min(firstCallIndex + callsPerPage, orderedCalls.length)}{" "}
                of {orderedCalls.length}
              </span>
              <div className={styles.rpcCallsPageSize}>
                <Select
                  aria-label="Calls per page"
                  size="sm"
                  value={callsPerPage}
                  onChange={event => {
                    setCallsPerPage(Number(event.target.value))
                    setCurrentPage(1)
                  }}
                >
                  {CALLS_PER_PAGE_OPTIONS.map(option => (
                    <option key={option} value={option}>
                      {option} per page
                    </option>
                  ))}
                </Select>
              </div>
            </div>
            <div className={styles.rpcCallsPaginationActions}>
              <Button
                type="button"
                variant="outline"
                size="sm"
                leadingIcon={<ChevronLeft size={14} />}
                disabled={safeCurrentPage === 1}
                onClick={() => setCurrentPage(safeCurrentPage - 1)}
              >
                Previous
              </Button>
              <span className={styles.rpcCallsPaginationPage}>
                Page {safeCurrentPage} of {totalPages}
              </span>
              <Button
                type="button"
                variant="outline"
                size="sm"
                trailingIcon={<ChevronRight size={14} />}
                disabled={safeCurrentPage === totalPages}
                onClick={() => setCurrentPage(safeCurrentPage + 1)}
              >
                Next
              </Button>
            </div>
          </div>
        ) : undefined}
      </section>
    </>
  )
}

function formatRequestData(data: unknown | null, truncated = false): string | undefined {
  if (data === null || data === undefined) {
    return undefined
  }

  if (truncated && typeof data === "string") {
    return data
  }

  return JSON.stringify(data, null, 2)
}

function readCallsPerPage(): number {
  const storedValue = Number(globalThis.localStorage.getItem(CALLS_PER_PAGE_STORAGE_KEY))
  return CALLS_PER_PAGE_OPTIONS.some(option => option === storedValue)
    ? storedValue
    : DEFAULT_CALLS_PER_PAGE
}
