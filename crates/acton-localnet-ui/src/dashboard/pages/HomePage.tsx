import {BookOpen, Check, Copy, FastForward, X} from "lucide-react"
import {Button, Card, CardContent, CardHeader, CardTitle, Input, useToast} from "@acton/shared-ui"
import {Link, useNavigate} from "react-router-dom"
import {useCallback, useEffect, useMemo, useState} from "react"
import type {FC, FormEvent} from "react"

import type {TonClient} from "../../explorer/api/client"
import {addressKey} from "../../explorer/api/compilerAbi"
import type {
  LocalnetNodeInfo,
  V3AccountState,
  V3TransactionListItem,
} from "../../explorer/api/types"
import {
  DeveloperAccountList,
  DeveloperAccountListSkeleton,
  type DeveloperAccountListItem,
} from "../../explorer/components/DeveloperAccountList"
import {
  DeveloperTransactionList,
  DeveloperTransactionListSkeleton,
} from "../../explorer/components/DeveloperTransactionList"
import {formatDuration} from "../../explorer/components/utils"
import {useAddressBook} from "../../explorer/hooks/useAddressBook"
import {useOpenExplorerPath} from "../../explorer/hooks/useOpenExplorerPath"
import {useTransactionMessageNames} from "../../explorer/hooks/useTransactionMessageNames"
import {collectRecentAccounts} from "../dashboardUtils"

import styles from "../DashboardPage.module.css"

const HOME_RECENT_TRANSACTIONS_REFRESH_MS = 2000
const HOME_NODE_INFO_REFRESH_MS = 1000
const MASTERCHAIN_BLOCK_SHARD = "8000000000000000"
const DEFAULT_TIME_ADVANCE_SECONDS = "0"
const MINUTE_SECONDS = 60
const HOUR_SECONDS = 3600
const DAY_SECONDS = 86_400
const WEEK_SECONDS = 604_800
const MONTH_SECONDS = 2_592_000
const YEAR_SECONDS = 31_536_000
const TIME_UNITS = [
  {seconds: YEAR_SECONDS, compact: "y", name: "year"},
  {seconds: MONTH_SECONDS, compact: "mo", name: "month"},
  {seconds: WEEK_SECONDS, compact: "w", name: "week"},
  {seconds: DAY_SECONDS, compact: "d", name: "day"},
  {seconds: HOUR_SECONDS, compact: "h", name: "hour"},
  {seconds: MINUTE_SECONDS, compact: "min", name: "minute"},
  {seconds: 1, compact: "s", name: "second"},
] as const
const TIME_ADVANCE_PRESET_SECONDS = [
  MINUTE_SECONDS,
  HOUR_SECONDS,
  DAY_SECONDS,
  WEEK_SECONDS,
  MONTH_SECONDS,
  YEAR_SECONDS,
] as const
const TIME_ADVANCE_PRESETS = TIME_ADVANCE_PRESET_SECONDS.map(seconds => ({
  label: formatReadableDuration(seconds),
  seconds,
}))

interface HomePageProps {
  readonly client: TonClient
}

interface HomeState {
  readonly transactions: readonly V3TransactionListItem[]
  readonly accountStatesByAddress: Readonly<Record<string, V3AccountState>>
  readonly isLoading: boolean
  readonly error?: string
}

interface NodeInfoRow {
  readonly label: string
  readonly value?: string
  readonly secondaryValue?: string
  readonly to?: string
  readonly isLoading?: boolean
  readonly title?: string
  readonly variant?: "time"
}

export const HomePage: FC<HomePageProps> = ({client}) => {
  const navigate = useNavigate()
  const openPath = useOpenExplorerPath()
  const {showToast} = useToast()
  const {prefetchNames} = useAddressBook()
  const [nodeInfo, setNodeInfo] = useState<LocalnetNodeInfo | undefined>()
  const [copiedEndpoint, setCopiedEndpoint] = useState<string>()
  const [isTimeModalOpen, setIsTimeModalOpen] = useState(false)
  const [timeAdvanceSeconds, setTimeAdvanceSeconds] = useState(DEFAULT_TIME_ADVANCE_SECONDS)
  const [timeAdvanceError, setTimeAdvanceError] = useState<string>()
  const [isAdvancingTime, setIsAdvancingTime] = useState(false)
  const [homeState, setHomeState] = useState<HomeState>({
    transactions: [],
    accountStatesByAddress: {},
    isLoading: true,
  })
  const parsedTimeAdvanceSeconds = parseTimeAdvanceSeconds(timeAdvanceSeconds)
  const timeAdvanceShiftValue = formatReadableDuration(parsedTimeAdvanceSeconds ?? 0)
  const timeAdvanceCurrentValue = nodeInfo ? formatNodeDateTime(nodeInfo.current_unix_time) : "—"
  const timeAdvanceTargetValue =
    nodeInfo
      ? formatNodeDateTime(nodeInfo.current_unix_time + (parsedTimeAdvanceSeconds ?? 0))
      : "—"
  const endpoints = useMemo(() => client.getEndpoints(), [client])
  const endpointRows = useMemo(
    () =>
      [
        {
          label: "V2 API",
          value: endpoints.apiV2,
          referencePath: "/api-reference/v2",
        },
        {
          label: "V3 API",
          value: endpoints.apiV3,
          referencePath: "/api-reference/v3",
        },
        {
          label: "Control API",
          value: endpoints.admin,
          referencePath: "/api-reference/control",
        },
      ].filter(endpoint => endpoint.value.length > 0),
    [endpoints],
  )
  const nodeInfoRows = useMemo<readonly NodeInfoRow[]>(() => {
    const isLoading = nodeInfo === undefined
    const nodeTime = nodeInfo ? formatNodeDateTime(nodeInfo.current_unix_time) : undefined
    const nodeTimeOffset =
      nodeInfo && nodeInfo.time_offset_seconds !== 0
        ? formatTimeOffset(nodeInfo.time_offset_seconds)
        : undefined

    return [
      {
        label: "Latest block",
        value: nodeInfo?.last_block_seqno.toString(),
        to: nodeInfo ? getMasterchainBlockPath(nodeInfo.last_block_seqno) : undefined,
        isLoading,
      },
      {
        label: "Uptime",
        value: nodeInfo ? formatDuration(nodeInfo.uptime_seconds) : undefined,
        isLoading,
      },
      {
        label: "State source",
        value: nodeInfo ? formatNodeInfoValue(nodeInfo.state_source) : undefined,
        isLoading,
      },
      {
        label: "Fork",
        value: nodeInfo
          ? formatForkInfo(nodeInfo.fork_network, nodeInfo.fork_block_number)
          : undefined,
        isLoading,
      },
      {
        label: "Node time",
        value: nodeTime,
        secondaryValue: nodeTimeOffset,
        title: nodeTimeOffset ? `${nodeTime} (${nodeTimeOffset})` : nodeTime,
        isLoading,
        variant: "time",
      },
    ]
  }, [nodeInfo])
  const recentAccounts = useMemo(
    () => collectRecentAccounts(homeState.transactions),
    [homeState.transactions],
  )
  const recentAccountItems = useMemo<readonly DeveloperAccountListItem[]>(
    () =>
      recentAccounts.map(address => ({
        address,
        state: homeState.accountStatesByAddress[addressKey(address)],
      })),
    [homeState.accountStatesByAddress, recentAccounts],
  )
  const {addresses: displayedAddresses, messageNamesByAddress} = useTransactionMessageNames(
    client,
    homeState.transactions,
  )

  useEffect(() => {
    let cancelled = false
    let timeoutId: ReturnType<typeof setTimeout> | undefined

    const loadNodeInfo = async () => {
      try {
        const nextNodeInfo = await client.getNodeInfo()
        if (!cancelled) {
          setNodeInfo(nextNodeInfo)
        }
      } catch {
        if (!cancelled) {
          setNodeInfo(undefined)
        }
      } finally {
        if (!cancelled) {
          timeoutId = globalThis.setTimeout(() => void loadNodeInfo(), HOME_NODE_INFO_REFRESH_MS)
        }
      }
    }

    void loadNodeInfo()

    return () => {
      cancelled = true
      if (timeoutId !== undefined) {
        globalThis.clearTimeout(timeoutId)
      }
    }
  }, [client])

  useEffect(() => {
    let cancelled = false
    let timeoutId: ReturnType<typeof setTimeout> | undefined

    const loadHomeState = async (showLoading: boolean) => {
      if (showLoading) {
        setHomeState(current => ({
          ...current,
          isLoading: true,
          error: undefined,
        }))
      }

      try {
        const transactionsResponse = await client.getRecentTransactions(8)
        const transactions = transactionsResponse.transactions
        const accounts = collectRecentAccounts(transactions)
        let accountStatesByAddress: Record<string, V3AccountState> = {}

        if (accounts.length > 0) {
          try {
            const accountStates = await client.getAccountStates(accounts, false)
            accountStatesByAddress = Object.fromEntries(
              accountStates.accounts.map(account => [addressKey(account.address), account]),
            )
          } catch (error) {
            console.error("Failed to fetch recent account states", error)
          }
        }

        if (!cancelled) {
          setHomeState({
            transactions,
            accountStatesByAddress,
            isLoading: false,
          })
        }
      } catch (error) {
        if (!cancelled) {
          const message = error instanceof Error ? error.message : "Failed to load dashboard"
          setHomeState(current => ({
            transactions: current.transactions,
            accountStatesByAddress: current.accountStatesByAddress,
            isLoading: false,
            error: current.transactions.length === 0 ? message : undefined,
          }))
        }
      } finally {
        if (!cancelled) {
          timeoutId = globalThis.setTimeout(
            () => void loadHomeState(false),
            HOME_RECENT_TRANSACTIONS_REFRESH_MS,
          )
        }
      }
    }

    void loadHomeState(true)

    return () => {
      cancelled = true
      if (timeoutId !== undefined) {
        globalThis.clearTimeout(timeoutId)
      }
    }
  }, [client])

  useEffect(() => {
    void prefetchNames(displayedAddresses)
  }, [displayedAddresses, prefetchNames])

  useEffect(() => {
    if (!copiedEndpoint) {
      return
    }

    const timeoutId = globalThis.setTimeout(() => setCopiedEndpoint(undefined), 2000)
    return () => {
      globalThis.clearTimeout(timeoutId)
    }
  }, [copiedEndpoint])

  useEffect(() => {
    if (!isTimeModalOpen) {
      return
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !isAdvancingTime) {
        setIsTimeModalOpen(false)
      }
    }

    document.addEventListener("keydown", handleKeyDown)
    return () => {
      document.removeEventListener("keydown", handleKeyDown)
    }
  }, [isAdvancingTime, isTimeModalOpen])

  const openTimeAdvanceModal = useCallback(() => {
    setTimeAdvanceSeconds(DEFAULT_TIME_ADVANCE_SECONDS)
    setTimeAdvanceError(undefined)
    setIsTimeModalOpen(true)
  }, [])

  const closeTimeAdvanceModal = useCallback(() => {
    if (!isAdvancingTime) {
      setIsTimeModalOpen(false)
    }
  }, [isAdvancingTime])

  const copyEndpoint = useCallback(
    async (endpoint: string) => {
      try {
        await navigator.clipboard.writeText(endpoint)
        setCopiedEndpoint(endpoint)
      } catch (error) {
        console.error("Failed to copy endpoint", error)
        showToast({
          variant: "error",
          title: "Copy failed",
          description: "Failed to copy endpoint URL.",
        })
      }
    },
    [showToast],
  )

  const handleTimeAdvanceSubmit = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault()

      const seconds = parseTimeAdvanceSeconds(timeAdvanceSeconds)
      if (!seconds) {
        setTimeAdvanceError("Enter a positive number of seconds.")
        return
      }

      setIsAdvancingTime(true)
      setTimeAdvanceError(undefined)
      try {
        const nextTimeInfo = await client.increaseTime(seconds)
        setNodeInfo(current => (current ? {...current, ...nextTimeInfo} : current))
        setIsTimeModalOpen(false)
        showToast({
          variant: "success",
          title: "Time advanced",
          description: `Node time moved by ${formatReadableDuration(seconds)}.`,
        })
      } catch (error) {
        const message = error instanceof Error ? error.message : "Failed to advance node time."
        setTimeAdvanceError(message)
        showToast({
          variant: "error",
          title: "Time not advanced",
          description: message,
        })
      } finally {
        setIsAdvancingTime(false)
      }
    },
    [client, showToast, timeAdvanceSeconds],
  )

  return (
    <>
      <section className={styles.hero}>
        <div>
          <h1 className={styles.title}>Home</h1>
          <p className={styles.subtitle}>
            A quick snapshot of your local node and recent activity.
          </p>
        </div>
      </section>

      <section className={styles.homeLayout}>
        <div className={styles.homeTopRow}>
          <Card className={`${styles.dashboardCard} ${styles.homeCard}`}>
            <CardHeader className={styles.dashboardCardHeader}>
              <CardTitle className={styles.dashboardCardTitle}>Node info</CardTitle>
            </CardHeader>
            <CardContent className={`${styles.dashboardCardContent} ${styles.nodeInfoList}`}>
              {nodeInfoRows.map(row => {
                const value = row.value ?? "—"
                const title = row.title ?? value
                const rowClassName = `${styles.nodeInfoRow} ${
                  row.variant === "time" ? styles.nodeInfoTimeRow : ""
                }`

                return (
                  <div key={row.label} className={rowClassName}>
                    <span className={styles.nodeInfoLabel}>{row.label}</span>
                    {row.isLoading ? (
                      <span
                        className={`${styles.skeletonLine} ${styles.nodeInfoValueSkeleton}`}
                        aria-label={`Loading ${row.label}`}
                      />
                    ) : row.to ? (
                      <Link className={styles.nodeInfoValueLink} to={row.to} title={title}>
                        {value}
                      </Link>
                    ) : row.variant === "time" ? (
                      <div className={styles.nodeInfoTimeControl} title={title}>
                        <span className={styles.nodeInfoTimeText}>
                          <span className={styles.nodeInfoValueText}>{value}</span>
                          {row.secondaryValue && (
                            <span className={styles.nodeInfoValueMeta}>{row.secondaryValue}</span>
                          )}
                        </span>
                        <button
                          type="button"
                          className={styles.nodeInfoTimeButton}
                          aria-label="Advance node time"
                          aria-haspopup="dialog"
                          aria-expanded={isTimeModalOpen}
                          title="Advance time"
                          onClick={openTimeAdvanceModal}
                        >
                          <FastForward size={14} />
                          <span>Advance</span>
                        </button>
                      </div>
                    ) : (
                      <span className={styles.nodeInfoValue} title={title}>
                        <span className={styles.nodeInfoValueText}>{value}</span>
                        {row.secondaryValue && (
                          <span className={styles.nodeInfoValueMeta}>{row.secondaryValue}</span>
                        )}
                      </span>
                    )}
                  </div>
                )
              })}
            </CardContent>
          </Card>

          <Card className={`${styles.dashboardCard} ${styles.homeCard}`}>
            <CardHeader className={styles.dashboardCardHeader}>
              <CardTitle className={styles.dashboardCardTitle}>Endpoints</CardTitle>
            </CardHeader>
            <CardContent className={`${styles.dashboardCardContent} ${styles.endpointList}`}>
              {endpointRows.map(endpoint => {
                const isCopied = copiedEndpoint === endpoint.value

                return (
                  <div key={endpoint.label} className={styles.endpointRow}>
                    <span className={styles.endpointLabel}>{endpoint.label}</span>
                    <span className={styles.endpointValueRow}>
                      <span className={styles.endpointValue}>{endpoint.value}</span>
                      <span className={styles.endpointActions}>
                        <button
                          type="button"
                          className={`${styles.endpointButton} ${isCopied ? styles.endpointButtonCopied : ""}`}
                          aria-label={
                            isCopied ? "Endpoint copied" : `Copy ${endpoint.label} endpoint`
                          }
                          title={isCopied ? "Copied" : "Copy endpoint"}
                          onClick={() => void copyEndpoint(endpoint.value)}
                        >
                          {isCopied ? <Check size={14} /> : <Copy size={14} />}
                        </button>
                        <button
                          type="button"
                          className={styles.endpointButton}
                          aria-label={`Open ${endpoint.label} reference`}
                          title="Open API reference"
                          onClick={() => void navigate(endpoint.referencePath)}
                        >
                          <BookOpen size={14} />
                        </button>
                      </span>
                    </span>
                  </div>
                )
              })}
            </CardContent>
          </Card>
        </div>

        {homeState.error ? (
          <div className={`${styles.homeTransactionsCard} ${styles.emptyState}`}>
            {homeState.error}
          </div>
        ) : homeState.isLoading ? (
          <DeveloperTransactionListSkeleton
            className={styles.homeTransactionsCard}
            title="Recent transactions"
          />
        ) : homeState.transactions.length === 0 ? (
          <div className={`${styles.homeTransactionsCard} ${styles.emptyState}`}>
            No transactions yet
          </div>
        ) : (
          <DeveloperTransactionList
            className={styles.homeTransactionsCard}
            title="Recent transactions"
            transactions={homeState.transactions}
            messageNamesByAddress={messageNamesByAddress}
            onTransactionClick={(hashHex, _transaction, event) => {
              openPath(`/explorer/tx/${encodeURIComponent(hashHex)}`, event)
            }}
            onAddressClick={(address, event) => {
              openPath(`/explorer/address/${encodeURIComponent(address)}`, event)
            }}
          />
        )}

        <div className={styles.homeMainColumn}>
          {homeState.error ? (
            <div className={styles.emptyState}>{homeState.error}</div>
          ) : homeState.isLoading ? (
            <DeveloperAccountListSkeleton title="Recent accounts" />
          ) : (
            <DeveloperAccountList
              title="Recent accounts"
              accounts={recentAccountItems}
              onAddressClick={(address, event) => {
                openPath(`/explorer/address/${encodeURIComponent(address)}`, event)
              }}
            />
          )}
        </div>
      </section>

      {isTimeModalOpen && (
        <div
          className={styles.timeModalBackdrop}
          onMouseDown={event => {
            if (event.target === event.currentTarget) {
              closeTimeAdvanceModal()
            }
          }}
        >
          <section
            className={styles.timeModal}
            role="dialog"
            aria-modal="true"
            aria-labelledby="node-time-modal-title"
          >
            <div className={styles.timeModalHeader}>
              <h2 id="node-time-modal-title" className={styles.timeModalTitle}>
                Advance time
              </h2>
              <button
                type="button"
                className={styles.timeModalCloseButton}
                aria-label="Close time control"
                disabled={isAdvancingTime}
                onClick={closeTimeAdvanceModal}
              >
                <X size={18} />
              </button>
            </div>

            <form className={styles.timeModalContent} onSubmit={handleTimeAdvanceSubmit}>
              <div className={styles.fieldBlock}>
                <label className={styles.label} htmlFor="node-time-advance-seconds">
                  Seconds
                </label>
                <Input
                  id="node-time-advance-seconds"
                  className={styles.fieldInput}
                  type="number"
                  min="0"
                  step="1"
                  inputMode="numeric"
                  value={timeAdvanceSeconds}
                  disabled={isAdvancingTime}
                  onChange={event => {
                    setTimeAdvanceSeconds(event.target.value)
                    setTimeAdvanceError(undefined)
                  }}
                />
              </div>

              <div className={styles.timeAdvancePresets}>
                {TIME_ADVANCE_PRESETS.map(preset => (
                  <button
                    key={preset.seconds}
                    type="button"
                    className={styles.timeAdvancePresetButton}
                    aria-label={`Add ${preset.label} to time shift`}
                    disabled={isAdvancingTime}
                    onClick={() => {
                      setTimeAdvanceSeconds(currentSeconds =>
                        addTimeAdvanceSeconds(currentSeconds, preset.seconds),
                      )
                      setTimeAdvanceError(undefined)
                    }}
                  >
                    {preset.label}
                  </button>
                ))}
              </div>

              <div className={styles.timeAdvancePreview}>
                <div className={styles.timeAdvancePreviewRow}>
                  <span>Shift</span>
                  <strong>{timeAdvanceShiftValue}</strong>
                </div>
                <div className={styles.timeAdvancePreviewRow}>
                  <span>Current</span>
                  <strong>{timeAdvanceCurrentValue}</strong>
                </div>
                <div className={styles.timeAdvancePreviewRow}>
                  <span>After</span>
                  <strong>{timeAdvanceTargetValue}</strong>
                </div>
              </div>

              {timeAdvanceError && (
                <div className={styles.timeAdvanceError} role="alert">
                  {timeAdvanceError}
                </div>
              )}

              <div className={styles.timeModalActions}>
                <Button
                  type="button"
                  variant="outline"
                  disabled={isAdvancingTime}
                  onClick={closeTimeAdvanceModal}
                >
                  Cancel
                </Button>
                <Button type="submit" disabled={isAdvancingTime || !parsedTimeAdvanceSeconds}>
                  {isAdvancingTime ? "Advancing..." : "Advance"}
                  <FastForward size={15} />
                </Button>
              </div>
            </form>
          </section>
        </div>
      )}
    </>
  )
}

function formatNodeInfoValue(value: string): string {
  const normalized = value.trim()
  if (normalized.length === 0) {
    return "—"
  }

  return normalized.replace(/_/g, " ")
}

function formatOptionalNodeInfoValue(value: string | null | undefined): string {
  if (value === undefined || value === null) {
    return "—"
  }

  return formatNodeInfoValue(value)
}

function formatForkInfo(network: string | null | undefined, block: number | null | undefined): string {
  const networkValue = formatOptionalNodeInfoValue(network)
  if (block === undefined || block === null) {
    return networkValue
  }

  const blockValue = block.toLocaleString()
  return networkValue === "—" ? blockValue : `${networkValue} · ${blockValue}`
}

function formatNodeDateTime(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000)

  return `${formatDateTimePart(date.getDate())}.${formatDateTimePart(
    date.getMonth() + 1,
  )}.${date.getFullYear()}, ${formatDateTimePart(date.getHours())}:${formatDateTimePart(
    date.getMinutes(),
  )}:${formatDateTimePart(date.getSeconds())}`
}

function formatDateTimePart(value: number): string {
  return value.toString().padStart(2, "0")
}

function parseTimeAdvanceSeconds(value: string): number | undefined {
  const seconds = Number(value)
  if (!Number.isSafeInteger(seconds) || seconds <= 0) {
    return undefined
  }

  return seconds
}

function addTimeAdvanceSeconds(currentValue: string, secondsToAdd: number): string {
  const currentSeconds = parseTimeAdvanceSeconds(currentValue) ?? 0
  return (currentSeconds + secondsToAdd).toString()
}

function formatReadableDuration(totalSeconds: number): string {
  return formatDurationWithTimeUnits(totalSeconds, {style: "readable"})
}

function formatTimeOffset(offsetSeconds: number): string {
  return formatDurationWithTimeUnits(offsetSeconds, {style: "compact", maxParts: 4})
}

function formatDurationWithTimeUnits(
  totalSeconds: number,
  options: {readonly style: "compact" | "readable"; readonly maxParts?: number},
): string {
  const sign = totalSeconds < 0 ? "-" : "+"
  let remainingSeconds = Math.abs(totalSeconds)
  const parts: string[] = []

  for (const unit of TIME_UNITS) {
    const value = Math.floor(remainingSeconds / unit.seconds)
    if (value === 0) {
      continue
    }

    parts.push(
      options.style === "compact"
        ? `${value}${unit.compact}`
        : `${value} ${value === 1 ? unit.name : `${unit.name}s`}`,
    )
    remainingSeconds %= unit.seconds
    if (options.maxParts !== undefined && parts.length === options.maxParts) {
      break
    }
  }

  const zeroValue = options.style === "compact" ? "0s" : "0 seconds"
  return `${sign}${parts.length > 0 ? parts.join(" ") : zeroValue}`
}

function getMasterchainBlockPath(seqno: number): string {
  return `/block/-1/${encodeURIComponent(MASTERCHAIN_BLOCK_SHARD)}/${seqno}`
}
