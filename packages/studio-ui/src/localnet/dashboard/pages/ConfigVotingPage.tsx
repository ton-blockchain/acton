import {Fragment, useCallback, useEffect, useMemo, useState} from "react"
import {Link, useNavigate, useParams, useSearchParams} from "react-router"
import {
  ArrowUpRight,
  Box,
  Check,
  ChevronRight,
  CircleHelp,
  Clock3,
  Info,
  RefreshCw,
  Settings2,
  ShieldAlert,
  X,
} from "lucide-react"
import {
  Breadcrumbs,
  Button,
  ContentTabs,
  CopyButton,
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
  DateTime,
  InlineButton,
  InlineLoader,
  RawDataBlock,
  TechnicalValue,
  useToast,
} from "@acton/ui"
import type {TonClient} from "@acton/explorer-core/api/client"
import {
  ConfigVotingReader,
  configVoteTally,
  weightPercent,
  type ConfigVote,
  type ConfigVoteResult,
  type ConfigVotingSnapshot,
} from "@acton/explorer-core/api/configVoting"
import {getConfigParameterMetadata} from "@acton/explorer-core/api/config"
import {ConfigParameterHeader, ConfigParameterValue} from "@acton/explorer-core/pages/ConfigPage"
import {useExplorerRoutePaths} from "@acton/explorer-core/hooks/useExplorerRoutePaths"
import {useLocalnetRoutes} from "../../routes"

import styles from "./ConfigVotingPage.module.css"

const ACTIVE_REFRESH_MS = 30_000

const outcomeLabels = {
  accepted: "Accepted",
  passed: "Passed · not applied",
  expired: "Expired",
  rejected: "Not passed",
  closed: "Closed · outcome unavailable",
  active: "Active",
  verifying: "Verifying",
} as const

const outcomeIcons = {
  accepted: Check,
  passed: Check,
  expired: Clock3,
  rejected: X,
  closed: CircleHelp,
  active: Clock3,
  verifying: Clock3,
}

function ProposalStatus({outcome}: {readonly outcome: keyof typeof outcomeLabels}) {
  const Icon = outcomeIcons[outcome]

  return (
    <span className={styles.status} data-outcome={outcome}>
      <Icon size={14} aria-hidden="true" />
      {outcomeLabels[outcome]}
    </span>
  )
}

function ProposalKind({critical}: {readonly critical: boolean}) {
  return critical ? (
    <span className={styles.critical}>
      <ShieldAlert size={14} aria-hidden="true" />
      Critical
    </span>
  ) : (
    <span className={styles.muted}>Normal</span>
  )
}

// Keep archive lookups out of the polling loop once a proposal is completed.
// Schedule after each request settles so slow RPC responses cannot overlap.
function useVotingAutoRefresh(enabled: boolean, refresh: (signal: AbortSignal) => Promise<void>) {
  const {showToast} = useToast()

  useEffect(() => {
    if (!enabled) return

    const controller = new AbortController()
    let pending = false
    let errorReported = false
    let timer: ReturnType<typeof setTimeout>

    const refreshWhenVisible = async () => {
      if (controller.signal.aborted || pending || document.visibilityState !== "visible") return

      clearTimeout(timer)
      pending = true

      try {
        await refresh(controller.signal)
        errorReported = false
      } catch (reason) {
        if (!controller.signal.aborted && !errorReported) {
          errorReported = true
          showToast({
            title: "Could not refresh active proposals",
            description: errorMessage(reason),
            variant: "error",
          })
        }
      } finally {
        pending = false
        if (!controller.signal.aborted) timer = setTimeout(refreshWhenVisible, ACTIVE_REFRESH_MS)
      }
    }

    timer = setTimeout(refreshWhenVisible, ACTIVE_REFRESH_MS)
    globalThis.addEventListener("focus", refreshWhenVisible)
    document.addEventListener("visibilitychange", refreshWhenVisible)

    return () => {
      controller.abort()
      clearTimeout(timer)
      globalThis.removeEventListener("focus", refreshWhenVisible)
      document.removeEventListener("visibilitychange", refreshWhenVisible)
    }
  }, [enabled, refresh, showToast])
}

/** A shareable, network-scoped proposal view reuses the same details as the voting list. */
export function ConfigProposalPage({client}: {readonly client: TonClient}) {
  const {proposalHash = ""} = useParams<{proposalHash: string}>()
  const reader = useMemo(() => new ConfigVotingReader(client), [client])
  const {path} = useLocalnetRoutes()
  const {showToast} = useToast()
  const [selection, setSelection] = useState<{
    readonly vote: ConfigVote
    readonly snapshot: ConfigVotingSnapshot
  }>()
  const [loading, setLoading] = useState(true)
  const [progress, setProgress] = useState("Loading proposal")
  const [reload, setReload] = useState(0)

  useEffect(() => {
    const controller = new AbortController()
    setLoading(true)
    setSelection(undefined)
    setProgress("Loading proposal")

    reader
      .proposal(proposalHash, controller.signal, setProgress)
      .then(value => {
        if (!controller.signal.aborted) setSelection(value)
      })
      .catch(reason => {
        if (controller.signal.aborted) return

        showToast({
          title: "Could not load proposal",
          description: errorMessage(reason),
          variant: "error",
        })
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false)
      })

    return () => controller.abort()
  }, [reader, proposalHash, reload, showToast])

  const completed = selection ? (selection.vote.expires ?? 0) <= selection.snapshot.time : false
  const refreshActiveProposal = useCallback(
    async (signal: AbortSignal) => {
      const value = await reader.proposal(proposalHash, signal, setProgress)
      if (!signal.aborted) setSelection(value)
    },
    [reader, proposalHash],
  )
  useVotingAutoRefresh(Boolean(selection) && !completed && !loading, refreshActiveProposal)

  const listUrl = selection
    ? `${path("/voting")}?tab=${completed ? "completed" : "active"}&proposal=${selection.vote.hash}`
    : path("/voting")

  return (
    <div className={styles.page}>
      <div className={styles.toolbar}>
        <div className={styles.proposalBreadcrumbs}>
          <Breadcrumbs
            items={[
              {
                label: "Voting",
                truncate: false,
                link: (children, className) => (
                  <Link to={listUrl} className={className}>
                    {children}
                  </Link>
                ),
              },
              {
                label: selection
                  ? `${selection.vote.parameterId} ${getConfigParameterMetadata(selection.vote.parameterId).title}`
                  : proposalHash,
                truncate: selection ? "end" : "middle",
                preserveStart: 16,
                preserveEnd: 12,
              },
            ]}
          />
        </div>
        <Button
          size="sm"
          variant="outline"
          leadingIcon={<RefreshCw size={15} />}
          loading={loading}
          onClick={() => setReload(value => value + 1)}
        >
          Refresh
        </Button>
      </div>

      {selection ? (
        <ProposalDetails
          reader={reader}
          vote={selection.vote}
          snapshot={selection.snapshot}
          completed={completed}
          standalone
        />
      ) : (
        <div className={styles.lookup} aria-live="polite">
          {loading ? (
            <InlineLoader message={progress} />
          ) : (
            <Button size="sm" variant="outline" onClick={() => setReload(value => value + 1)}>
              Retry
            </Button>
          )}
        </div>
      )}
    </div>
  )
}

/** Studio owns network selection; the reader and all page state are recreated for that environment. */
export function ConfigVotingPage({client}: {readonly client: TonClient}) {
  const reader = useMemo(() => new ConfigVotingReader(client), [client])
  const {path} = useLocalnetRoutes()
  const {showToast} = useToast()
  const navigate = useNavigate()
  const [searchParams, setSearchParams] = useSearchParams()
  const tab = searchParams.get("tab") === "completed" ? "completed" : "active"
  const [snapshot, setSnapshot] = useState<ConfigVotingSnapshot>()
  const [history, setHistory] = useState<readonly ConfigVote[]>([])
  const [nextOffset, setNextOffset] = useState(0)
  const [hasMore, setHasMore] = useState(true)
  const [loading, setLoading] = useState(true)
  const [historyLoading, setHistoryLoading] = useState(false)
  const [error, setError] = useState<string>()
  const [historyError, setHistoryError] = useState<string>()
  const [reload, setReload] = useState(0)
  const [historyRequest, setHistoryRequest] = useState<{readonly offset: number} | undefined>(() =>
    tab === "completed" ? {offset: 0} : undefined,
  )
  const [results, setResults] = useState<ReadonlyMap<string, ConfigVoteResult>>(new Map())
  const selected = searchParams.get("proposal") ?? undefined

  const refreshActiveProposals = useCallback(
    async (signal: AbortSignal) => {
      const value = await reader.current(signal)
      if (signal.aborted) return

      setSnapshot(value)
      setError(undefined)
    },
    [reader],
  )
  useVotingAutoRefresh(tab === "active" && !loading, refreshActiveProposals)

  const selectProposal = (hash: string | undefined) => {
    setSearchParams(
      current => {
        const next = new URLSearchParams(current)
        if (hash) next.set("proposal", hash)
        else next.delete("proposal")
        return next
      },
      {preventScrollReset: true},
    )
  }

  useEffect(() => {
    const controller = new AbortController()
    setLoading(true)
    setError(undefined)

    reader
      .current(controller.signal)
      .then(value => {
        if (!controller.signal.aborted) setSnapshot(value)
      })
      .catch(reason => {
        if (controller.signal.aborted) return

        setError(errorMessage(reason))
        showToast({
          title: "Could not load proposals",
          description: errorMessage(reason),
          variant: "error",
        })
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false)
      })

    return () => controller.abort()
  }, [reader, reload, showToast])

  useEffect(() => {
    if (tab === "completed") setHistoryRequest(current => current ?? {offset: 0})
  }, [tab])

  const address = snapshot?.address

  useEffect(() => {
    if (historyRequest === undefined || !address) return

    const controller = new AbortController()
    setHistoryLoading(true)
    setHistoryError(undefined)

    reader
      .history(address, historyRequest.offset, controller.signal)
      .then(page => {
        if (controller.signal.aborted) return

        // A later submission may extend the same cell hash. Keep its latest registration.
        setHistory(current => {
          const merged = new Map(current.map(vote => [vote.hash, vote]))
          for (const vote of page.proposals) {
            if (!merged.has(vote.hash)) merged.set(vote.hash, vote)
          }
          return [...merged.values()]
        })
        setNextOffset(page.nextOffset)
        setHasMore(page.hasMore)
      })
      .catch(reason => {
        if (controller.signal.aborted) return

        setHistoryError(errorMessage(reason))
        showToast({
          title: "Could not load proposal history",
          description: errorMessage(reason),
          variant: "error",
        })
      })
      .finally(() => {
        if (!controller.signal.aborted) setHistoryLoading(false)
      })

    return () => controller.abort()
  }, [reader, address, historyRequest, showToast])

  const recordResult = useCallback((result: ConfigVoteResult) => {
    setResults(current => new Map(current).set(result.vote.hash, result))
  }, [])

  const activeVotes = snapshot?.proposals.filter(vote => (vote.expires ?? 0) > snapshot.time) ?? []
  const expiredVotes =
    snapshot?.proposals.filter(vote => (vote.expires ?? 0) <= snapshot.time) ?? []
  const activeHashes = new Set(activeVotes.map(vote => vote.hash))
  const completedVotes = new Map(
    history.filter(vote => !activeHashes.has(vote.hash)).map(vote => [vote.hash, vote]),
  )
  for (const vote of expiredVotes) {
    completedVotes.set(vote.hash, {...completedVotes.get(vote.hash), ...vote})
  }

  const votes = tab === "active" ? activeVotes : [...completedVotes.values()]

  // A bookmarked proposal may be beyond the first history page. Follow the
  // normal pagination cursor until it is found, the history ends, or the URL changes.
  const selectedFound = votes.some(vote => vote.hash === selected)

  useEffect(() => {
    if (
      tab !== "completed" ||
      !selected ||
      selectedFound ||
      historyLoading ||
      historyError ||
      !hasMore
    )
      return
    if (historyRequest && nextOffset > historyRequest.offset) {
      setHistoryRequest({offset: nextOffset})
    }
  }, [
    tab,
    selected,
    selectedFound,
    historyLoading,
    historyError,
    hasMore,
    historyRequest,
    nextOffset,
  ])

  const selectTab = (value: "active" | "completed") => {
    setSearchParams(current => {
      const next = new URLSearchParams(current)
      next.set("tab", value)
      next.delete("proposal")
      return next
    })
    if (value === "completed" && historyRequest === undefined) setHistoryRequest({offset: 0})
  }

  const refresh = () => {
    setHistory([])
    setResults(new Map())
    setNextOffset(0)
    setHasMore(true)
    setHistoryRequest(tab === "completed" ? {offset: 0} : undefined)
    setReload(value => value + 1)
  }

  return (
    <div className={styles.page}>
      <div className={styles.toolbar}>
        {snapshot ? (
          <span className={styles.updated}>
            As of <DateTime value={snapshot.time} unit="seconds" display="compact" />
          </span>
        ) : undefined}
        <Button
          size="sm"
          variant="outline"
          leadingIcon={<Settings2 size={15} />}
          onClick={() => navigate(path("/explorer/config"))}
        >
          Network config
        </Button>
        <Button
          size="sm"
          variant="outline"
          leadingIcon={<RefreshCw size={15} />}
          loading={loading}
          onClick={refresh}
        >
          Refresh
        </Button>
      </div>

      <ContentTabs
        ariaLabel="Configuration proposals"
        variant="standalone"
        value={tab}
        onValueChange={selectTab}
        tabs={[
          {value: "active", label: `Active${snapshot ? ` (${activeVotes.length})` : ""}`},
          {value: "completed", label: "Completed"},
        ]}
      >
        <DataTable minWidth={0}>
          <DataTableTable
            aria-label={`${tab === "active" ? "Active" : "Completed"} configuration proposals`}
          >
            <DataTableHead>
              <DataTableRow>
                <DataTableHeaderCell className={styles.parameterColumn}>
                  Parameter
                </DataTableHeaderCell>
                <DataTableHeaderCell className={styles.kindColumn}>Kind</DataTableHeaderCell>
                <DataTableHeaderCell className={styles.resultColumn}>
                  {tab === "active" ? "Voting weight" : "Result"}
                </DataTableHeaderCell>
                <DataTableHeaderCell className={styles.dateColumn}>
                  {tab === "active" ? "Expires" : "Last registered"}
                </DataTableHeaderCell>
              </DataTableRow>
            </DataTableHead>
            <DataTableBody>
              {(loading && !snapshot) || (historyLoading && history.length === 0) ? (
                <DataTableSkeletonRows columns={4} rows={3} />
              ) : votes.length === 0 ? (
                <DataTableEmpty colSpan={4}>
                  {error || historyError ? (
                    <Button size="sm" variant="secondary" onClick={refresh}>
                      Retry
                    </Button>
                  ) : tab === "active" ? (
                    "No active proposals in this network"
                  ) : (
                    "No completed proposals in the loaded history"
                  )}
                </DataTableEmpty>
              ) : (
                votes.map(vote => {
                  const tally = snapshot && configVoteTally(vote, snapshot)
                  const result = results.get(vote.hash)
                  const timestamp = tab === "active" ? vote.expires : vote.registeredAt

                  return (
                    <Fragment key={vote.hash}>
                      <DataTableRow selected={vote.hash === selected} hover>
                        <DataTableCell className={styles.parameterCell}>
                          <InlineButton
                            className={styles.proposalButton}
                            leadingIcon={
                              <ChevronRight
                                size={16}
                                className={selected === vote.hash ? styles.expanded : undefined}
                              />
                            }
                            aria-expanded={selected === vote.hash}
                            aria-controls={`proposal-${vote.hash}`}
                            onClick={() => {
                              selectProposal(selected === vote.hash ? undefined : vote.hash)
                            }}
                          >
                            <span className={styles.parameterId}>{vote.parameterId}</span>{" "}
                            {getConfigParameterMetadata(vote.parameterId).title}
                          </InlineButton>
                          <span className={styles.mobileMeta}>
                            <ProposalKind critical={vote.critical} />
                            {timestamp === undefined ? undefined : (
                              <>
                                {" · "}
                                <DateTime value={timestamp} unit="seconds" display="compact" />
                              </>
                            )}
                          </span>
                        </DataTableCell>
                        <DataTableCell tone="muted" className={styles.kindColumn}>
                          <ProposalKind critical={vote.critical} />
                        </DataTableCell>
                        <DataTableCell className={styles.resultCell}>
                          {tab === "completed" ? (
                            <span>
                              {result ? (
                                <ProposalStatus outcome={result.outcome} />
                              ) : (
                                <InlineButton
                                  variant="accent"
                                  aria-expanded={selected === vote.hash}
                                  aria-controls={`proposal-${vote.hash}`}
                                  onClick={() => {
                                    selectProposal(selected === vote.hash ? undefined : vote.hash)
                                  }}
                                >
                                  View result
                                </InlineButton>
                              )}
                            </span>
                          ) : tally ? (
                            <div className={styles.weight}>
                              <span>
                                {tally.percent.toLocaleString(undefined, {
                                  maximumFractionDigits: 2,
                                })}
                                %
                              </span>
                              <progress
                                max={100}
                                value={tally.percent}
                                aria-label={`Voting weight for parameter ${vote.parameterId}`}
                              />
                            </div>
                          ) : (
                            "Validator set unavailable"
                          )}
                        </DataTableCell>
                        <DataTableCell tone="muted" className={styles.dateColumn}>
                          {timestamp === undefined ? (
                            "—"
                          ) : (
                            <DateTime value={timestamp} unit="seconds" display="compact" />
                          )}
                        </DataTableCell>
                      </DataTableRow>
                      {selected === vote.hash && snapshot ? (
                        <DataTableRow>
                          <DataTableCell colSpan={4} className={styles.detailCell}>
                            <ProposalDetails
                              reader={reader}
                              vote={vote}
                              snapshot={snapshot}
                              completed={tab === "completed"}
                              cachedResult={results.get(vote.hash)}
                              onResult={recordResult}
                            />
                          </DataTableCell>
                        </DataTableRow>
                      ) : undefined}
                    </Fragment>
                  )
                })
              )}
            </DataTableBody>
            {tab === "completed" && hasMore && snapshot ? (
              <DataTableFooter>
                <DataTableRow>
                  <DataTableCell colSpan={4} className={styles.loadMoreCell}>
                    <Button
                      size="sm"
                      variant="outline"
                      loading={historyLoading}
                      onClick={() => setHistoryRequest({offset: nextOffset})}
                    >
                      {historyError ? "Retry" : "Load older proposals"}
                    </Button>
                  </DataTableCell>
                </DataTableRow>
              </DataTableFooter>
            ) : undefined}
          </DataTableTable>
        </DataTable>

        {tab === "completed" ? (
          <div className={styles.historyFooter}>
            <Info size={15} aria-hidden="true" />
            <span>Results are verified from archived network states when you open a proposal</span>
          </div>
        ) : undefined}
        {selected && !selectedFound && !loading && !error ? (
          <div className={styles.lookup}>
            {tab === "completed" && hasMore && !historyError ? (
              <InlineLoader message="Finding selected proposal" />
            ) : (
              <span className={styles.muted}>The selected proposal is not in this list</span>
            )}
            <InlineButton onClick={() => selectProposal(undefined)}>Clear selection</InlineButton>
          </div>
        ) : undefined}
      </ContentTabs>
    </div>
  )
}

function ProposalDetails({
  reader,
  vote,
  snapshot,
  completed,
  cachedResult,
  onResult,
  standalone = false,
}: {
  readonly reader: ConfigVotingReader
  readonly vote: ConfigVote
  readonly snapshot: ConfigVotingSnapshot
  readonly completed: boolean
  readonly cachedResult?: ConfigVoteResult
  readonly onResult?: (result: ConfigVoteResult) => void
  readonly standalone?: boolean
}) {
  const [result, setResult] = useState(cachedResult)
  const [progress, setProgress] = useState("Loading voting result")
  const [error, setError] = useState<string>()
  const [retry, setRetry] = useState(0)
  const [view, setView] = useState<"changes" | "raw">("changes")
  const [votersExpanded, setVotersExpanded] = useState(false)
  const {blockPath} = useExplorerRoutePaths()
  const navigate = useNavigate()
  const {path} = useLocalnetRoutes()
  const {showToast} = useToast()

  useEffect(() => {
    if (!completed || result) return

    const controller = new AbortController()
    setError(undefined)

    reader
      .result(vote, snapshot, controller.signal, setProgress)
      .then(value => {
        if (!controller.signal.aborted) {
          setResult(value)
          onResult?.(value)
        }
      })
      .catch(reason => {
        if (controller.signal.aborted) return

        setError(errorMessage(reason))
        showToast({
          title: "Could not load voting result",
          description: errorMessage(reason),
          variant: "error",
        })
      })

    return () => controller.abort()
  }, [reader, vote, snapshot, completed, result, retry, onResult, showToast])

  const observedVote = result?.vote ?? vote
  const observedSnapshot = result?.snapshot ?? snapshot
  const state = observedVote.state
  const tally = configVoteTally(observedVote, observedSnapshot)
  const rule = observedVote.critical
    ? observedSnapshot.rules.critical
    : observedSnapshot.rules.normal
  const before = observedSnapshot.config.parameters.find(
    parameter => parameter.id === vote.parameterId,
  )
  const showChanges = !completed || Boolean(result)

  // Match the config page: a short table preview with the shared Show more control.
  const votedValidators =
    tally?.set.validators.filter(validator => state?.voters.has(validator.index)) ?? []
  const hasMoreVoters = votedValidators.length > 7
  const visibleVoters = votersExpanded
    ? votedValidators
    : votedValidators.slice(0, hasMoreVoters ? 8 : 7)

  return (
    <section
      id={`proposal-${vote.hash}`}
      className={styles.details}
      aria-label={`Proposal details for parameter ${vote.parameterId}`}
    >
      <div className={styles.detailLinks}>
        {standalone ? undefined : (
          <Button
            size="sm"
            variant="outline"
            leadingIcon={<ArrowUpRight size={15} />}
            title="Open proposal in a new tab"
            onClick={() =>
              window.open(path(`/voting/${vote.hash}`), "_blank", "noopener,noreferrer")
            }
          >
            Open proposal
          </Button>
        )}
        <Button
          size="sm"
          variant="outline"
          leadingIcon={<Settings2 size={15} />}
          disabled={completed && !result}
          onClick={() =>
            navigate(
              `${path(`/explorer/config/${result?.seqno ?? snapshot.seqno}`)}#config-parameter-${vote.parameterId}`,
            )
          }
        >
          View in config
        </Button>
        {result ? (
          <Button
            size="sm"
            variant="outline"
            leadingIcon={<Box size={15} />}
            onClick={() => navigate(blockPath(-1, "8000000000000000", result.seqno))}
          >
            {result.removed ? "Completion" : "Snapshot"} block {result.seqno}
          </Button>
        ) : undefined}
        <CopyButton value={vote.hash} label="Copy proposal hash" size="sm" variant="outline">
          Copy hash
        </CopyButton>
      </div>

      {completed && !result ? (
        <div className={styles.lookup} aria-live="polite">
          {error ? (
            <Button size="sm" variant="secondary" onClick={() => setRetry(value => value + 1)}>
              Retry result lookup
            </Button>
          ) : (
            <InlineLoader message={progress} />
          )}
        </div>
      ) : undefined}

      {state ? (
        <>
          <dl className={styles.metrics}>
            <div>
              <dt>Status</dt>
              <dd>
                <ProposalStatus outcome={result?.outcome ?? (completed ? "verifying" : "active")} />
              </dd>
            </div>
            <div>
              <dt>Winning rounds</dt>
              <dd>
                {state.wins} / {rule.min_wins}
              </dd>
            </div>
            <div>
              <dt>Losing rounds</dt>
              <dd>
                {state.losses} / {rule.max_losses}
              </dd>
            </div>
            <div>
              <dt>Rounds remaining</dt>
              <dd>{state.rounds_remaining}</dd>
            </div>
            <div>
              <dt>{result ? "Completed" : "Expires"}</dt>
              <dd>
                <DateTime
                  value={result?.time ?? state.expires}
                  unit="seconds"
                  display="date-time-numeric-seconds"
                />
              </dd>
            </div>
            <div>
              <dt>Kind</dt>
              <dd>
                <ProposalKind critical={vote.critical} />
              </dd>
            </div>
          </dl>

          {tally ? (
            <div className={styles.round}>
              <div className={styles.roundHeading}>
                <h3>
                  {result && !result.finalVoteRecorded
                    ? "Last recorded voting round"
                    : "Voting round"}
                </h3>
                <span>
                  {state.voters.size} / {tally.set.total} validators voted
                </span>
              </div>
              <progress max={100} value={tally.percent} aria-label="Proposal voting weight" />
              <div className={styles.roundCaption}>
                <strong>
                  {tally.percent.toLocaleString(undefined, {maximumFractionDigits: 4})}% voting
                  weight
                </strong>
                <span>{tally.won ? "Round won" : "More than 75% required"}</span>
              </div>
              {!result && !tally.won ? (
                <p>
                  At least {tally.neededValidators} more{" "}
                  {tally.neededValidators === 1 ? "validator" : "validators"} and{" "}
                  {tally.remainingPercent.toLocaleString(undefined, {maximumFractionDigits: 4})}%
                  voting weight needed
                </p>
              ) : undefined}
              {tally.currentSet ? undefined : <p>This tally belongs to a previous validator set</p>}
              {result && !result.finalVoteRecorded ? (
                <p>Snapshot before the completion block</p>
              ) : undefined}
            </div>
          ) : (
            <p className={styles.muted}>
              The validator set for this voting round is not available in this snapshot
            </p>
          )}
        </>
      ) : undefined}

      {result?.outcome === "passed" ? (
        <p className={styles.muted}>
          The vote passed, but the proposed value was not applied by the contract
        </p>
      ) : undefined}
      {result?.outcome === "closed" ? (
        <p className={styles.muted}>
          The proposal was removed, but the archive does not prove whether it passed
        </p>
      ) : undefined}

      <section id={`config-parameter-${vote.parameterId}`} className={styles.parameterChange}>
        <ConfigParameterHeader
          parameter={{id: vote.parameterId, ...getConfigParameterMetadata(vote.parameterId)}}
        />
        <ContentTabs
          ariaLabel="Proposed parameter"
          className={styles.parameterTabs}
          value={view}
          onValueChange={setView}
          tabs={[
            {label: "Changes", value: "changes"},
            {label: "Raw cell", value: "raw"},
          ]}
        >
          <div className={styles.changes}>
            {view === "raw" ? (
              vote.proposed ? (
                <RawDataBlock
                  value={vote.proposed.rawHex}
                  title="Proposed value"
                  className={styles.rawValue}
                  copyLabel="proposed cell"
                  wrap
                />
              ) : (
                <span>Remove this parameter</span>
              )
            ) : showChanges ? (
              vote.proposed?.parsedValue || vote.proposed?.contractBytecode ? (
                <ConfigParameterValue parameter={vote.proposed} comparison={{before}} />
              ) : vote.proposed ? (
                <RawDataBlock
                  value={vote.proposed.rawHex}
                  title="Proposed value"
                  className={styles.rawValue}
                  copyLabel="proposed cell"
                  wrap
                />
              ) : (
                <>
                  <p>Remove this parameter</p>
                  {before ? <ConfigParameterValue parameter={before} /> : undefined}
                </>
              )
            ) : (
              <div className={styles.pendingComparison}>
                The parameter comparison will appear when its archived state is loaded
              </div>
            )}
            {vote.requiredHash ? (
              <RawDataBlock
                title="Required current value hash"
                className={styles.rawValue}
                value={vote.requiredHash}
                copyLabel="required value hash"
                wrap
              />
            ) : undefined}
          </div>
        </ContentTabs>
      </section>

      {state && tally ? (
        <DataTable
          title={`Voted validators (${state.voters.size})`}
          variant="nested"
          minWidth={0}
          preview={
            hasMoreVoters
              ? {
                  expanded: votersExpanded,
                  itemLabel: "voted validators",
                  onExpandedChange: setVotersExpanded,
                }
              : undefined
          }
        >
          <DataTableTable aria-label="Voted validators">
            <DataTableHead>
              <DataTableRow>
                <DataTableHeaderCell columnWidth="4rem">Index</DataTableHeaderCell>
                <DataTableHeaderCell>Public key</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="6rem" align="right">
                  Weight
                </DataTableHeaderCell>
              </DataTableRow>
            </DataTableHead>
            <DataTableBody>
              {visibleVoters.length === 0 ? (
                <DataTableEmpty colSpan={3}>No votes in this round</DataTableEmpty>
              ) : (
                visibleVoters.map(validator => (
                  <DataTableRow key={validator.index}>
                    <DataTableCell tone="muted">{validator.index}</DataTableCell>
                    <DataTableCell mono>
                      <TechnicalValue
                        value={validator.publicKey}
                        copyLabel="validator public key"
                        startLength={6}
                        endLength={6}
                      />
                    </DataTableCell>
                    <DataTableCell align="right">
                      {weightPercent(validator.weight, tally.total).toLocaleString(undefined, {
                        maximumFractionDigits: 4,
                      })}
                      %
                    </DataTableCell>
                  </DataTableRow>
                ))
              )}
            </DataTableBody>
          </DataTableTable>
        </DataTable>
      ) : undefined}
    </section>
  )
}

function errorMessage(reason: unknown) {
  return reason instanceof Error ? reason.message : String(reason)
}
