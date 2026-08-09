import {CircleDot, CircleHelp, FastForward, GitBranch, Network} from "lucide-react"
import {
  BlockChip,
  Button,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  DateTime,
  Dialog,
  Duration,
  formatDuration,
  formatNumberValue,
  humanizeIdentifier,
  Input,
  DAY_SECONDS,
  Tooltip,
  useToast,
} from "@acton/ui"
import {useNavigate} from "react-router"
import {useCallback, useEffect, useMemo, useState} from "react"
import type {FC, FormEvent} from "react"

import {supports} from "../../../environmentCapabilities"
import {useLocalnetRuntime} from "../../LocalnetRuntimeProvider"
import type {TonClient} from "@acton/explorer-core/api/client"
import {addressKey} from "@acton/explorer-core/api/compilerAbi"
import type {
  LocalnetNodeInfo,
  V3AccountState,
  V3TransactionListItem,
} from "@acton/explorer-core/api/types"
import {
  DeveloperAccountList,
  DeveloperAccountListSkeleton,
  type DeveloperAccountListItem,
} from "@acton/explorer-core/components/DeveloperAccountList"
import {
  DeveloperTransactionList,
  DeveloperTransactionListSkeleton,
} from "@acton/explorer-core/components/DeveloperTransactionList"
import {useAddressBook} from "@acton/explorer-core/hooks/useAddressBook"
import {useExplorerRoutePaths} from "@acton/explorer-core/hooks/useExplorerRoutePaths"
import {useOpenExplorerPath} from "@acton/explorer-core/hooks/useOpenExplorerPath"
import {useTransactionMessageNames} from "@acton/explorer-core/hooks/useTransactionMessageNames"
import {useLocalnetRoutes} from "../../routes"
import {EnvironmentActions} from "../components/EnvironmentActions"
import {EnvironmentConnect} from "../components/EnvironmentConnect"
import {collectRecentAccounts, formatForkNetworkLabel} from "../dashboardUtils"

import styles from "../DashboardPage.module.css"

const HOME_RECENT_TRANSACTIONS_REFRESH_MS = 2000
const HOME_NODE_INFO_REFRESH_MS = 1000
const CONNECT_PANEL_DISMISSED_STORAGE_PREFIX = "acton-studio:connect-panel-dismissed:"
const MASTERCHAIN_BLOCK_SHARD = "8000000000000000"
const DEFAULT_TIME_ADVANCE_SECONDS = "0"
const MINUTE_SECONDS = 60
const HOUR_SECONDS = 3600
const WEEK_SECONDS = 604_800
const MONTH_SECONDS = 2_592_000
const YEAR_SECONDS = 31_536_000
const TIME_ADVANCE_PRESET_SECONDS = [
  MINUTE_SECONDS,
  HOUR_SECONDS,
  DAY_SECONDS,
  WEEK_SECONDS,
  MONTH_SECONDS,
  YEAR_SECONDS,
] as const
const TIME_ADVANCE_PRESETS = TIME_ADVANCE_PRESET_SECONDS.map(seconds => ({
  label: formatDuration(seconds, {display: "readable", sign: "always"}),
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

interface NetworkNodeInfo {
  readonly lastBlockSeqno: number
  readonly latestBlockUnixTime: number
}

export const HomePage: FC<HomePageProps> = ({client}) => {
  const runtime = useLocalnetRuntime()
  const environment = runtime.environment
  const localnetConfig =
    environment?.config.kind === "actonLocalnet" ? environment.config : undefined
  const fullNetworkConfig =
    environment?.config.kind === "fullTonNetwork" ? environment.config : undefined
  const remoteNetworkConfig =
    environment?.config.kind === "remoteTonNetwork" ? environment.config : undefined
  const hasSimulatedControlApi = localnetConfig !== undefined && supports(environment, "controlApi")
  const hasNetworkNodeInfo = !hasSimulatedControlApi && supports(environment, "apiV3")
  const hasIntegration = supports(environment, "integration")
  const navigate = useNavigate()
  const routes = useExplorerRoutePaths()
  const localnetRoutes = useLocalnetRoutes()
  const openPath = useOpenExplorerPath()
  const {showToast} = useToast()
  const {prefetchNames, updateDomains} = useAddressBook()
  const [nodeInfo, setNodeInfo] = useState<LocalnetNodeInfo | undefined>()
  const [networkNodeInfo, setNetworkNodeInfo] = useState<NetworkNodeInfo | undefined>()
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
  const timeAdvanceShiftValue = formatDuration(parsedTimeAdvanceSeconds ?? 0, {
    display: "readable",
    sign: "always",
  })
  const forkNetwork = nodeInfo === undefined ? localnetConfig?.forkNetwork : nodeInfo.fork_network
  const forkBlockNumber =
    nodeInfo === undefined ? localnetConfig?.forkBlockNumber : nodeInfo.fork_block_number
  const hasFork = Boolean(forkNetwork?.trim())
  const showUptime = hasSimulatedControlApi
  const nodeInfoColumnCount = hasFork ? 6 : remoteNetworkConfig ? 3 : 4
  const forkSummary = formatForkSummary(forkNetwork, forkBlockNumber)
  const networkSummary = hasSimulatedControlApi
    ? forkSummary
    : (environment?.network.label ?? "Virtual environment")
  const forkBadgeLabel = remoteNetworkConfig
    ? undefined
    : (formatForkNetworkLabel(forkNetwork) ??
      (environment?.network.id === "localnet"
        ? "Local genesis"
        : (environment?.network.label ?? "Local genesis")))
  const connectPanelStorageKey = runtime.environment?.id
    ? `${CONNECT_PANEL_DISMISSED_STORAGE_PREFIX}${runtime.environment.id}`
    : undefined
  const [isConnectPanelDismissed, setIsConnectPanelDismissed] = useState(
    () =>
      connectPanelStorageKey !== undefined &&
      globalThis.localStorage.getItem(connectPanelStorageKey) === "true",
  )
  const latestBlockSeqno = nodeInfo?.last_block_seqno ?? networkNodeInfo?.lastBlockSeqno
  const nodeUnixTime = nodeInfo
    ? nodeInfo.current_unix_time
    : networkNodeInfo
      ? networkNodeInfo.latestBlockUnixTime
      : undefined
  const nodeTimeOffset =
    nodeInfo && nodeInfo.time_offset_seconds !== 0
      ? formatDuration(nodeInfo.time_offset_seconds, {
          display: "parts",
          maxParts: 4,
          sign: "always",
        })
      : undefined
  const forkBlockExplorerUrl = getActonscanForkBlockUrl(forkNetwork, forkBlockNumber)
  const localBlockCount =
    nodeInfo && forkBlockNumber !== undefined && forkBlockNumber !== null
      ? Math.max(0, nodeInfo.last_block_seqno - forkBlockNumber)
      : undefined
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
    setIsConnectPanelDismissed(
      connectPanelStorageKey !== undefined &&
        globalThis.localStorage.getItem(connectPanelStorageKey) === "true",
    )
  }, [connectPanelStorageKey])

  useEffect(() => {
    if (!hasSimulatedControlApi) {
      setNodeInfo(undefined)
      return
    }

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
  }, [client, hasSimulatedControlApi])

  useEffect(() => {
    if (!hasNetworkNodeInfo) {
      setNetworkNodeInfo(undefined)
      return
    }

    let cancelled = false
    let timeoutId: ReturnType<typeof setTimeout> | undefined

    const loadNetworkNodeInfo = async () => {
      try {
        const response = await client.getBlocks({workchain: -1, limit: 1, sort: "desc"})
        const latestBlock = response.blocks[0]
        const latestBlockUnixTime = latestBlock ? Number(latestBlock.gen_utime) : Number.NaN
        if (!latestBlock || !Number.isFinite(latestBlockUnixTime)) {
          throw new Error("Latest masterchain block is unavailable")
        }

        if (!cancelled) {
          setNetworkNodeInfo({
            lastBlockSeqno: latestBlock.seqno,
            latestBlockUnixTime,
          })
        }
      } catch {
        if (!cancelled) {
          setNetworkNodeInfo(undefined)
        }
      } finally {
        if (!cancelled) {
          timeoutId = globalThis.setTimeout(
            () => void loadNetworkNodeInfo(),
            HOME_NODE_INFO_REFRESH_MS,
          )
        }
      }
    }

    void loadNetworkNodeInfo()

    return () => {
      cancelled = true
      if (timeoutId !== undefined) {
        globalThis.clearTimeout(timeoutId)
      }
    }
  }, [client, hasNetworkNodeInfo])

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
        updateDomains(transactionsResponse.address_book)
        const transactions = transactionsResponse.transactions
        const accounts = collectRecentAccounts(transactions)
        let accountStatesByAddress: Record<string, V3AccountState> = {}

        if (accounts.length > 0) {
          try {
            const accountStates = await client.getAccountStates(accounts, false)
            updateDomains(accountStates.address_book)
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
  }, [client, updateDomains])

  useEffect(() => {
    void prefetchNames(displayedAddresses)
  }, [displayedAddresses, prefetchNames])

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
          description: `Node time moved by ${formatDuration(seconds, {
            display: "readable",
            sign: "always",
          })}`,
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
    <div className={styles.environmentHomePage}>
      <header className={styles.environmentHomeHeader}>
        <div className={styles.environmentHomeIdentity}>
          <h1 className={styles.environmentHomeName}>
            {runtime.environment?.name ?? "Virtual environment"}
          </h1>
          {forkBadgeLabel ? (
            <span className={styles.workspaceForkBadge} title={networkSummary}>
              {forkBadgeLabel}
            </span>
          ) : undefined}
        </div>
        <EnvironmentActions
          client={client}
          environment={environment}
          isAdvanceTimeOpen={isTimeModalOpen}
          latestBlockSeqno={latestBlockSeqno}
          onAdvanceTime={openTimeAdvanceModal}
          onOpenMiningSettings={() => void navigate(localnetRoutes.path("/settings"))}
          onFund={() => void navigate(localnetRoutes.path("/faucet"))}
          onSend={() => void navigate(localnetRoutes.path("/simulator"))}
          onSnapshots={() => void navigate(localnetRoutes.path("/snapshots"))}
          onStateChanged={() => setNodeInfo(undefined)}
        />
      </header>

      <div className={styles.environmentHomeScroll}>
        <section className={styles.environmentHomeContent}>
          <div className={styles.homeLayout}>
            {!hasIntegration || isConnectPanelDismissed ? undefined : (
              <EnvironmentConnect
                onDismiss={() => {
                  setIsConnectPanelDismissed(true)
                  if (connectPanelStorageKey !== undefined) {
                    globalThis.localStorage.setItem(connectPanelStorageKey, "true")
                  }
                }}
              />
            )}

            {hasSimulatedControlApi || hasNetworkNodeInfo ? (
              <DataTable title="Node info" minWidth="42rem">
                <DataTableTable aria-label="Node info">
                  <DataTableHead>
                    <DataTableRow>
                      <DataTableHeaderCell columnWidth={hasFork ? "16%" : "25%"}>
                        Latest block
                      </DataTableHeaderCell>
                      <DataTableHeaderCell columnWidth={hasFork ? "17%" : "27%"}>
                        State source
                      </DataTableHeaderCell>
                      {hasFork && (
                        <>
                          <DataTableHeaderCell columnWidth="18%">Fork block</DataTableHeaderCell>
                          <DataTableHeaderCell columnWidth="13%">Local blocks</DataTableHeaderCell>
                        </>
                      )}
                      {fullNetworkConfig ? (
                        <DataTableHeaderCell columnWidth="20%">Validators</DataTableHeaderCell>
                      ) : showUptime ? (
                        <DataTableHeaderCell columnWidth={hasFork ? "12%" : "20%"}>
                          Uptime
                        </DataTableHeaderCell>
                      ) : undefined}
                      <DataTableHeaderCell align="right">
                        {fullNetworkConfig ? "Latest block time" : "Node time"}
                      </DataTableHeaderCell>
                    </DataTableRow>
                  </DataTableHead>
                  <DataTableBody>
                    {latestBlockSeqno !== undefined && nodeUnixTime !== undefined ? (
                      <DataTableRow>
                        <DataTableCell>
                          <BlockChip
                            workchain={-1}
                            shard={MASTERCHAIN_BLOCK_SHARD}
                            seqno={latestBlockSeqno}
                            href={localnetRoutes.path(getMasterchainBlockPath(latestBlockSeqno))}
                            onClick={event => {
                              event.preventDefault()
                              openPath(
                                localnetRoutes.path(getMasterchainBlockPath(latestBlockSeqno)),
                                event,
                              )
                            }}
                          />
                        </DataTableCell>
                        <DataTableCell>
                          <span className={styles.nodeInfoStateSource}>
                            {remoteNetworkConfig ? (
                              <>
                                <Network size={15} aria-hidden="true" />
                                <span>{environment?.network.label ?? "Remote network"}</span>
                              </>
                            ) : (
                              <>
                                {fullNetworkConfig ? (
                                  <Network size={15} aria-hidden="true" />
                                ) : nodeInfo?.fork_network ? (
                                  <GitBranch size={15} aria-hidden="true" />
                                ) : (
                                  <CircleDot size={15} aria-hidden="true" />
                                )}
                                <span>
                                  {fullNetworkConfig ? "Full localnet" : "Simulated localnet"}
                                </span>
                                <Tooltip
                                  content={
                                    fullNetworkConfig
                                      ? "Runs local TON validators and a full indexer, produces blocks through validator nodes, supports actions, and reproduces full-node API behavior, but starts more slowly and uses more memory and disk space"
                                      : nodeInfo?.fork_network
                                        ? "Uses the Acton emulator and starts from a TON network snapshot, supports manual mining, time travel, and network controls, but can behave differently from a real TON network in edge cases"
                                        : "Uses the Acton emulator instead of TON validators, starts quickly, uses little disk space, and supports manual mining, time travel, and network controls, but can behave differently from a real TON network in edge cases"
                                  }
                                >
                                  <button
                                    type="button"
                                    className={styles.settingsSectionHelp}
                                    aria-label={`About ${fullNetworkConfig ? "Full localnet" : "Simulated localnet"}`}
                                  >
                                    <CircleHelp size={14} aria-hidden="true" />
                                  </button>
                                </Tooltip>
                              </>
                            )}
                          </span>
                        </DataTableCell>
                        {hasFork && (
                          <>
                            <DataTableCell>
                              {forkBlockNumber !== undefined && forkBlockNumber !== null ? (
                                <BlockChip
                                  workchain={-1}
                                  shard={MASTERCHAIN_BLOCK_SHARD}
                                  seqno={forkBlockNumber}
                                  href={forkBlockExplorerUrl}
                                  title={
                                    forkBlockExplorerUrl
                                      ? `Open fork block ${forkBlockNumber} in Actonscan`
                                      : `Fork block ${forkBlockNumber}`
                                  }
                                />
                              ) : (
                                "—"
                              )}
                            </DataTableCell>
                            <DataTableCell>{localBlockCount ?? "—"}</DataTableCell>
                          </>
                        )}
                        {fullNetworkConfig ? (
                          <DataTableCell>{fullNetworkConfig.validators}</DataTableCell>
                        ) : showUptime ? (
                          <DataTableCell>
                            <Duration value={nodeInfo?.uptime_seconds} />
                          </DataTableCell>
                        ) : undefined}
                        <DataTableCell align="right">
                          <span className={styles.nodeInfoTime}>
                            <DateTime
                              display="date-time-numeric-seconds"
                              unit="seconds"
                              value={nodeUnixTime}
                            />
                            {nodeTimeOffset && (
                              <span className={styles.nodeInfoValueMeta}>{nodeTimeOffset}</span>
                            )}
                          </span>
                        </DataTableCell>
                      </DataTableRow>
                    ) : (
                      <DataTableSkeletonRows
                        alignments={
                          hasFork
                            ? ["left", "left", "left", "left", "left", "right"]
                            : remoteNetworkConfig
                              ? ["left", "left", "right"]
                              : ["left", "left", "left", "right"]
                        }
                        columns={nodeInfoColumnCount}
                        rows={1}
                        widths={
                          hasFork
                            ? ["3rem", "5rem", "5rem", "3rem", "4rem", "8rem"]
                            : remoteNetworkConfig
                              ? ["3rem", "5rem", "8rem"]
                              : ["3rem", "5rem", "4rem", "8rem"]
                        }
                      />
                    )}
                  </DataTableBody>
                </DataTableTable>
              </DataTable>
            ) : undefined}

            {homeState.isLoading ? (
              <DeveloperTransactionListSkeleton
                className={styles.homeTransactionsCard}
                title="Recent transactions"
              />
            ) : (
              <DeveloperTransactionList
                className={styles.homeTransactionsCard}
                title="Recent transactions"
                transactions={homeState.transactions}
                emptyState={
                  homeState.error ? "Recent transactions are unavailable" : "No transactions yet"
                }
                messageNamesByAddress={messageNamesByAddress}
                onTransactionClick={(hashHex, _transaction, event) => {
                  openPath(routes.transactionPath(hashHex), event)
                }}
                onAddressClick={(address, event) => {
                  openPath(routes.addressPath(address), event)
                }}
              />
            )}

            <div className={styles.homeMainColumn}>
              {homeState.isLoading ? (
                <DeveloperAccountListSkeleton title="Recent accounts" />
              ) : (
                <DeveloperAccountList
                  title="Recent accounts"
                  accounts={recentAccountItems}
                  emptyState={
                    homeState.error ? "Recent accounts are unavailable" : "No accounts yet"
                  }
                  onAddressClick={(address, event) => {
                    openPath(routes.addressPath(address), event)
                  }}
                />
              )}
            </div>
          </div>
        </section>
      </div>

      {supports(environment, "timeTravel") ? (
        <Dialog
          open={isTimeModalOpen}
          title="Advance time"
          className={styles.dashboardDialog}
          maxWidth={420}
          dismissible={!isAdvancingTime}
          closeLabel="Close time control"
          onOpenChange={open => {
            if (!open) closeTimeAdvanceModal()
          }}
        >
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
                <strong>
                  <DateTime
                    display="date-time-numeric-seconds"
                    unit="seconds"
                    value={nodeInfo?.current_unix_time}
                  />
                </strong>
              </div>
              <div className={styles.timeAdvancePreviewRow}>
                <span>After</span>
                <strong>
                  <DateTime
                    display="date-time-numeric-seconds"
                    unit="seconds"
                    value={
                      nodeInfo
                        ? nodeInfo.current_unix_time + (parsedTimeAdvanceSeconds ?? 0)
                        : undefined
                    }
                  />
                </strong>
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
              <Button
                type="submit"
                variant="primary"
                trailingIcon={<FastForward size={15} />}
                disabled={isAdvancingTime || !parsedTimeAdvanceSeconds}
              >
                {isAdvancingTime ? "Advancing..." : "Advance"}
              </Button>
            </div>
          </form>
        </Dialog>
      ) : undefined}
    </div>
  )
}

function formatForkSummary(
  network: string | null | undefined,
  block: number | null | undefined,
): string {
  const networkValue = network?.trim()
  const networkLabel = networkValue
    ? `${humanizeIdentifier(networkValue, {capitalize: true})} fork`
    : undefined

  if (block === undefined || block === null) {
    return networkLabel ?? "Clean network"
  }

  const blockLabel = `Block ${formatNumberValue(block)}`
  return networkLabel ? `${networkLabel} · ${blockLabel}` : `Fork · ${blockLabel}`
}

function getActonscanForkBlockUrl(
  network: string | null | undefined,
  block: number | null | undefined,
): string | undefined {
  if (block === undefined || block === null) {
    return undefined
  }

  const networkId = network?.trim().toLocaleLowerCase()
  if (networkId !== "mainnet" && networkId !== "testnet") {
    return undefined
  }

  const blockUrl = `https://actonscan.com${getMasterchainBlockPath(block)}`
  return networkId === "testnet" ? `${blockUrl}?network=testnet` : blockUrl
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

function getMasterchainBlockPath(seqno: number): string {
  return `/block/-1/${encodeURIComponent(MASTERCHAIN_BLOCK_SHARD)}/${seqno}`
}
