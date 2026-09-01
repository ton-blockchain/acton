import {useEffect, useRef, useState} from "react"
import {
  Activity,
  Boxes,
  Clock3,
  Gauge,
  Network,
  RadioTower,
  RefreshCw,
  ShieldCheck,
} from "lucide-react"
import {
  BooleanValue,
  ByteSize,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableTable,
  Disclosure,
  Duration,
  InlineLoader,
  Percentage,
  RelativeTime,
  TechnicalValue,
  ThemeSwitch,
  Tooltip,
  useToast,
} from "@acton/ui"

import type {
  ElectionObservation,
  InitialSyncProgress,
  NetworkView,
  NodeView,
  ShardHead,
  ValidatorObservation,
  ValidatorSetObservation,
} from "./types"

const POLL_INTERVAL_MS = 2000

export function App() {
  const [network, setNetwork] = useState<NetworkView>()
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000))
  const errorToastId = useRef<string | undefined>(undefined)
  const {dismissToast, showToast} = useToast()

  useEffect(() => {
    const timer = globalThis.setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000)
    return () => globalThis.clearInterval(timer)
  }, [])

  useEffect(() => {
    let active = true
    let timer: ReturnType<typeof setTimeout> | undefined

    const load = async () => {
      try {
        const response = await fetch("/api/v1/network", {cache: "no-store"})
        if (!response.ok) throw new Error(`Request failed with status ${response.status}`)
        const next = (await response.json()) as NetworkView
        if (active) {
          setNetwork(next)

          if (errorToastId.current !== undefined) {
            dismissToast(errorToastId.current)
            errorToastId.current = undefined
          }
        }
      } catch (cause) {
        if (active && errorToastId.current === undefined) {
          errorToastId.current = showToast({
            title: "Unable to refresh network data",
            description:
              cause instanceof Error
                ? cause.message
                : "The observability service could not be reached",
            variant: "error",
          })
        }
      } finally {
        if (active) timer = globalThis.setTimeout(load, POLL_INTERVAL_MS)
      }
    }

    void load()
    return () => {
      active = false
      if (timer !== undefined) globalThis.clearTimeout(timer)
    }
  }, [dismissToast, showToast])

  if (!network) {
    return (
      <main className="boot-state">
        <InlineLoader
          message="Reading network state"
          subtext="Waiting for the local observability service"
        />
      </main>
    )
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <a className="brand" href="#overview" aria-label="Localton network overview">
          <span className="brand-mark" aria-hidden="true">
            <Network size={17} strokeWidth={1.8} />
          </span>
          <span>Localton</span>
        </a>
        <nav className="navigation" aria-label="Network sections">
          <NavigationLink href="#overview" icon={<Gauge size={15} />} label="Overview" />
          <NavigationLink href="#elections" icon={<Clock3 size={15} />} label="Elections" />
          <NavigationLink href="#nodes" icon={<RadioTower size={15} />} label="Nodes" />
          <NavigationLink href="#validators" icon={<ShieldCheck size={15} />} label="Validators" />
          <NavigationLink href="#shards" icon={<Boxes size={15} />} label="Shards" />
        </nav>
        <div className="sidebar-footer">
          <ThemeSwitch />
        </div>
      </aside>

      <div className="workspace">
        <header className="topbar">
          <div className="network-title">
            <h1>Network health</h1>
            <TechnicalValue value={network.network_id} copyLabel="network ID" />
          </div>
          <div className="refresh-state">
            <RefreshCw size={13} aria-hidden="true" />
            {`Updated ${Math.max(0, now - network.generated_at)}s ago`}
          </div>
        </header>

        <main className="content">
          <>
            <section id="overview" className="section-stack" aria-labelledby="overview-title">
              <div className="section-heading">
                <h2 id="overview-title">Network overview</h2>
              </div>
              <div className="metric-strip">
                <Metric
                  label="Online nodes"
                  value={`${network.totals.online_nodes} / ${network.totals.nodes}`}
                  tone={network.totals.online_nodes === network.totals.nodes ? "good" : "warning"}
                />
                <Metric
                  label="Synchronized"
                  value={`${network.totals.synchronized_nodes} / ${network.totals.nodes}`}
                  tone={
                    network.totals.synchronized_nodes === network.totals.nodes ? "good" : "warning"
                  }
                />
                <Metric
                  label="Active validators"
                  value={`${network.totals.active_validators} / ${network.totals.configured_validators}`}
                  tone={
                    network.totals.active_validators === network.totals.configured_validators
                      ? "good"
                      : "warning"
                  }
                />
                <Metric
                  label="Masterchain"
                  value={network.chain ? `#${network.chain.seqno.toLocaleString()}` : "Waiting"}
                />
                <Metric label="Current shards" value={String(network.chain?.shard_count ?? 0)} />
              </div>
              {network.chain ? null : (
                <div className="notice">
                  <Activity size={16} aria-hidden="true" />
                  <span>Waiting for TON network data</span>
                </div>
              )}
            </section>

            <section id="elections" className="section-stack" aria-labelledby="elections-title">
              <div className="section-heading">
                <h2 id="elections-title">Validator elections</h2>
                {network.election ? <ElectionStage stage={network.election.stage} /> : null}
              </div>
              {network.election ? (
                <ElectionDiagram election={network.election} now={now} />
              ) : (
                <div className="notice">
                  <Clock3 size={16} aria-hidden="true" />
                  <span>Election data is not available from the current chain view</span>
                </div>
              )}
            </section>

            <section id="nodes" className="section-stack" aria-labelledby="nodes-title">
              <div className="section-heading">
                <h2 id="nodes-title">Nodes and synchronization</h2>
              </div>
              <NodesTable nodes={network.nodes} now={now} />
            </section>

            <section id="validators" className="section-stack" aria-labelledby="validators-title">
              <div className="section-heading">
                <h2 id="validators-title">Validator production</h2>
              </div>
              <ValidatorsTable
                nodes={network.nodes.filter(node => node.roles.includes("validator"))}
              />
            </section>

            <section id="shards" className="section-stack" aria-labelledby="shards-title">
              <div className="section-heading">
                <h2 id="shards-title">Shard topology</h2>
              </div>
              <ShardsTable shards={network.shards} now={now} />
            </section>

            <section className="section-stack" aria-labelledby="observers-title">
              <div className="section-heading">
                <h2 id="observers-title">Signed observers</h2>
              </div>
              <DataTable minWidth="44rem">
                <DataTableTable>
                  <DataTableHead>
                    <DataTableRow>
                      <DataTableHeaderCell>Status</DataTableHeaderCell>
                      <DataTableHeaderCell>Observer</DataTableHeaderCell>
                      <DataTableHeaderCell>Endpoint</DataTableHeaderCell>
                      <DataTableHeaderCell>Last report</DataTableHeaderCell>
                    </DataTableRow>
                  </DataTableHead>
                  <DataTableBody>
                    {network.observers.map(observer => (
                      <DataTableRow key={observer.observer_id}>
                        <DataTableCell>
                          <StatusPill online={observer.online} />
                        </DataTableCell>
                        <DataTableCell>
                          <TechnicalValue value={observer.observer_id} copyLabel="observer ID" />
                        </DataTableCell>
                        <DataTableCell>
                          <div className="observer-endpoint">
                            <TechnicalValue
                              value={observer.endpoint}
                              copyLabel="observability endpoint"
                              shorten={false}
                            />
                            <span>{observer.software}</span>
                          </div>
                        </DataTableCell>
                        <DataTableCell>
                          <RelativeTime value={observer.generated_at} now={now} unit="seconds" />
                        </DataTableCell>
                      </DataTableRow>
                    ))}
                  </DataTableBody>
                </DataTableTable>
              </DataTable>
            </section>
          </>
        </main>
      </div>
    </div>
  )
}

function NavigationLink({
  href,
  icon,
  label,
}: {
  readonly href: string
  readonly icon: React.ReactNode
  readonly label: string
}) {
  return (
    <a href={href}>
      {icon}
      <span>{label}</span>
    </a>
  )
}

function Metric({
  label,
  value,
  tone = "default",
}: {
  readonly label: string
  readonly value: string
  readonly tone?: "default" | "good" | "warning"
}) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong data-tone={tone}>{value}</strong>
    </div>
  )
}

const ELECTION_STAGE_LABELS: Record<ElectionObservation["stage"], string> = {
  validation: "Validation in progress",
  accepting_entries: "Entries are open",
  finalizing: "Selecting next set",
  next_set_ready: "Next set is ready",
  retrying: "Election retrying",
  activation_overdue: "Activation overdue",
}

function ElectionStage({stage}: {readonly stage: ElectionObservation["stage"]}) {
  return (
    <span className="election-stage" data-stage={stage}>
      {ELECTION_STAGE_LABELS[stage]}
    </span>
  )
}

function ElectionDiagram({
  election,
  now,
}: {
  readonly election: ElectionObservation
  readonly now: number
}) {
  const previousCurrentRoundId = useRef(election.current.round_id)
  const rollingOver = previousCurrentRoundId.current !== election.current.round_id

  useEffect(() => {
    previousCurrentRoundId.current = election.current.round_id
  }, [election.current.round_id])

  const duration = Math.max(1, election.validators_elected_for)
  const previous =
    election.previous ??
    inferredValidatorSet(election.current.validation_started_at - duration, duration)
  const next = election.next ?? inferredValidatorSet(election.current.validation_ended_at, duration)
  const rounds = [
    {
      kind: "previous",
      label: "Previous round",
      set: previous,
      available: election.previous !== null,
      unavailableLabel: "Set unavailable",
    },
    {
      kind: "current",
      label: "Current round",
      set: election.current,
      available: true,
      unavailableLabel: "Set unavailable",
    },
    {
      kind: "next",
      label: "Next round",
      set: next,
      available: election.next !== null,
      unavailableLabel: "Pending",
    },
  ]
  const entryStart = (set: ValidatorSetObservation) =>
    set.validation_started_at - (election.current.validation_ended_at - election.elections_open_at)
  const entryEnd = (set: ValidatorSetObservation) =>
    set.validation_started_at - (election.current.validation_ended_at - election.elections_close_at)
  const rangeStart = Math.min(...rounds.map(round => entryStart(round.set)), now)
  const rangeEnd = Math.max(
    ...rounds.map(round => round.set.validation_ended_at + election.stake_held_for),
    now + Math.floor(duration / 4),
  )
  const range = Math.max(1, rangeEnd - rangeStart)
  const position = (value: number) =>
    Math.min(100, Math.max(0, ((value - rangeStart) / range) * 100))
  const width = (start: number, end: number) => Math.max(0.4, position(end) - position(start))
  const nowPosition = position(now)

  return (
    <div className="election-panel">
      <div className="election-summary">
        <Metric label="Round ID" value={election.current.round_id.toLocaleString()} />
        <Metric label="Current set" value={formatValidators(election.current.validators)} />
        <Metric label="Main subset" value={formatValidators(election.current.main_validators)} />
        <Metric
          label="Next set"
          value={election.next === null ? "Pending" : formatValidators(election.next.validators)}
        />
        <Metric label="Stake hold" value={`${election.stake_held_for}s`} />
      </div>
      <div className="election-chart" data-stage={election.stage}>
        <div
          aria-label="Validator election timeline"
          className="election-timeline"
          data-rollover={rollingOver}
          role="img"
        >
          <div className="timeline-now" style={{left: `${nowPosition}%`}}>
            <strong>NOW</strong>
          </div>
          {rounds.map(round => {
            const openedAt = entryStart(round.set)
            const closedAt = entryEnd(round.set)
            const validationEndedAt = round.set.validation_ended_at
            const holdingEndedAt = validationEndedAt + election.stake_held_for
            const phases = [
              {name: "Election", className: "timeline-entry", start: openedAt, end: closedAt},
              {
                name: "Selection",
                className: "timeline-selection",
                start: closedAt,
                end: round.set.validation_started_at,
              },
              {
                name: "Validation",
                className: "timeline-validation",
                start: round.set.validation_started_at,
                end: validationEndedAt,
              },
              {
                name: "Stake hold",
                className: "timeline-holding",
                start: validationEndedAt,
                end: holdingEndedAt,
              },
            ]
            const activePhase = phases.find(phase => phase.start <= now && now < phase.end)

            return (
              <div
                className="election-round"
                data-active={activePhase !== undefined}
                data-current={round.kind === "current"}
                key={round.set.round_id}
              >
                <div className="election-round-heading">
                  <strong>{round.label}</strong>
                  <span>#{round.set.round_id.toLocaleString()}</span>
                  <span>
                    {round.available
                      ? formatValidators(round.set.validators)
                      : round.unavailableLabel}
                  </span>
                </div>
                <div className="election-round-track">
                  {phases.map(phase => {
                    const tooltip = `${phase.name} · ${formatTimestamp(phase.start)}–${formatTimestamp(phase.end)}`

                    return (
                      <Tooltip content={tooltip} delay={0} key={phase.name}>
                        <span
                          aria-current={phase === activePhase ? "true" : undefined}
                          aria-label={tooltip}
                          className={`timeline-segment ${phase.className}`}
                          data-active={phase === activePhase}
                          style={{
                            left: `${position(phase.start)}%`,
                            width: `${width(phase.start, phase.end)}%`,
                          }}
                        />
                      </Tooltip>
                    )
                  })}
                  {activePhase ? (
                    <span
                      className="timeline-now-dot"
                      style={{left: `${nowPosition}%`}}
                      aria-hidden="true"
                    />
                  ) : null}
                </div>
                <div className="election-round-phases" aria-hidden="true">
                  <span style={{left: `${position((openedAt + closedAt) / 2)}%`}}>Election</span>
                  <span
                    style={{
                      left: `${position((closedAt + round.set.validation_started_at) / 2)}%`,
                    }}
                  >
                    Selection
                  </span>
                  <span
                    style={{
                      left: `${position((round.set.validation_started_at + validationEndedAt) / 2)}%`,
                    }}
                  >
                    Validation
                  </span>
                  <span style={{left: `${position((validationEndedAt + holdingEndedAt) / 2)}%`}}>
                    Holding
                  </span>
                </div>
              </div>
            )
          })}
        </div>
      </div>
      <ValidatorSetTables election={election} />
    </div>
  )
}

const VALIDATOR_PREVIEW_COUNT = 7

function ValidatorSetTables({election}: {readonly election: ElectionObservation}) {
  const sets = [
    {label: "Previous set", set: election.previous, unavailableLabel: "Set unavailable"},
    {label: "Current set", set: election.current, unavailableLabel: "Set unavailable"},
    {label: "Next set", set: election.next, unavailableLabel: "Pending"},
  ]

  return (
    <div className="validator-set-disclosures">
      {sets.map(({label, set, unavailableLabel}) => (
        <Disclosure
          className="validator-set-disclosure"
          contentClassName="validator-set-content"
          key={label}
          label={
            <span className="validator-set-summary">
              <span>{label}</span>
              <span className="validator-set-summary-meta">
                {set
                  ? `${formatValidators(set.validators)} · ${formatTimestamp(set.validation_started_at)}–${formatTimestamp(set.validation_ended_at)}`
                  : unavailableLabel}
              </span>
            </span>
          }
        >
          {set ? (
            <ValidatorSetTable label={label} set={set} />
          ) : (
            <div className="validator-set-unavailable">{unavailableLabel}</div>
          )}
        </Disclosure>
      ))}
    </div>
  )
}

function ValidatorSetTable({
  label,
  set,
}: {
  readonly label: string
  readonly set: ValidatorSetObservation
}) {
  const [expanded, setExpanded] = useState(false)
  const validators = set.members ?? []
  const hasMore = validators.length > VALIDATOR_PREVIEW_COUNT
  const visibleValidators = expanded
    ? validators
    : validators.slice(0, VALIDATOR_PREVIEW_COUNT + (hasMore ? 1 : 0))

  return (
    <DataTable
      minWidth="48rem"
      preview={
        hasMore
          ? {
              expanded,
              itemLabel: "validators",
              onExpandedChange: setExpanded,
            }
          : undefined
      }
      variant="embedded"
    >
      <DataTableTable aria-label={`${label} validators`}>
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell columnWidth="3.5rem">#</DataTableHeaderCell>
            <DataTableHeaderCell>Public key</DataTableHeaderCell>
            <DataTableHeaderCell>ADNL</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="8rem">Masterchain</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="10rem">Weight share</DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {set.members === undefined ? (
            <DataTableEmpty colSpan={5}>Validator identities are not available</DataTableEmpty>
          ) : visibleValidators.length === 0 ? (
            <DataTableEmpty colSpan={5}>No validators in this set</DataTableEmpty>
          ) : (
            visibleValidators.map((validator, index) => (
              <ValidatorSetRow
                index={index}
                key={`${validator.public_key}:${index}`}
                mainValidatorCount={set.main_validators}
                totalWeight={set.total_weight ?? "0"}
                validator={validator}
              />
            ))
          )}
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}

function ValidatorSetRow({
  index,
  mainValidatorCount,
  totalWeight,
  validator,
}: {
  readonly index: number
  readonly mainValidatorCount: number
  readonly totalWeight: string
  readonly validator: ValidatorObservation
}) {
  const weightParts = validatorWeightParts(validator.weight, totalWeight)

  return (
    <DataTableRow hover>
      <DataTableCell tone="muted">{index + 1}</DataTableCell>
      <DataTableCell truncate>
        <TechnicalValue
          copyLabel="validator public key"
          endLength={10}
          startLength={10}
          value={validator.public_key}
        />
      </DataTableCell>
      <DataTableCell truncate>
        <TechnicalValue
          copyLabel="validator ADNL address"
          endLength={10}
          fallback="—"
          startLength={10}
          value={validator.adnl_address}
        />
      </DataTableCell>
      <DataTableCell>
        <BooleanValue value={index < mainValidatorCount} />
      </DataTableCell>
      <DataTableCell>
        <Tooltip
          content={`${BigInt(validator.weight).toLocaleString()} of ${BigInt(totalWeight).toLocaleString()}`}
        >
          <span className="validator-weight-share">
            <Percentage
              maximumFractionDigits={3}
              minimumFractionDigits={2}
              total={1_000_000}
              value={weightParts}
            />
          </span>
        </Tooltip>
      </DataTableCell>
    </DataTableRow>
  )
}

function validatorWeightParts(weight: string, totalWeight: string) {
  const total = BigInt(totalWeight)
  if (total === 0n) return 0

  return Number((BigInt(weight) * 1_000_000n) / total)
}

function inferredValidatorSet(start: number, duration: number): ValidatorSetObservation {
  return {
    round_id: start,
    validation_started_at: start,
    validation_ended_at: start + duration,
    validators: 0,
    main_validators: 0,
    total_weight: "0",
    members: [],
  }
}

function formatTimestamp(value: number) {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(value * 1000)
}

function formatValidators(count: number) {
  return `${count.toLocaleString()} ${count === 1 ? "validator" : "validators"}`
}

function StatusPill({online}: {readonly online: boolean}) {
  return (
    <span className="status-pill" data-online={online ? "true" : "false"}>
      <span aria-hidden="true" />
      {online ? "Online" : "Offline"}
    </span>
  )
}

const SYNC_LABELS: Record<NodeView["sync_status"], string> = {
  synced: "Synced",
  catching_up: "Catching up",
  unknown: "Unknown",
  offline: "Offline",
}

const INITIAL_SYNC_LABELS: Record<InitialSyncProgress["stage"], string> = {
  starting: "Loading initial block and proof",
  discovering_key_blocks: "Discovering recent key blocks",
  downloading_masterchain_state: "Downloading masterchain state",
  downloading_shard_states: "Downloading shard states",
  preparing: "Preparing initial synchronization",
}

function SyncState({state}: {readonly state: NodeView["sync_status"]}) {
  return (
    <span className="sync-state" data-state={state}>
      <span aria-hidden="true" />
      {SYNC_LABELS[state]}
    </span>
  )
}

function SynchronizationProgress({node, now}: {readonly node: NodeView; readonly now: number}) {
  const localHead = node.head_seqno
  const targetHead = node.network_head_seqno
  const initialBlockTime = node.sync_initial_masterchain_block_time
  const blockTime = node.sync_masterchain_block_time
  const targetTime = node.sync_target_time
  const initialSync = node.initial_sync_progress
  const sample =
    typeof localHead === "number" && typeof targetHead === "number"
      ? {
          kind: "blocks" as const,
          local: localHead,
          target: Math.max(localHead, targetHead),
          lag: node.sync_lag_blocks ?? Math.max(localHead, targetHead) - localHead,
        }
      : typeof initialBlockTime === "number" &&
          typeof blockTime === "number" &&
          typeof targetTime === "number"
        ? {
            kind: "time" as const,
            progress: blockTime - Math.min(initialBlockTime, blockTime),
            range: Math.max(blockTime, targetTime) - Math.min(initialBlockTime, blockTime),
            lag: Math.max(blockTime, targetTime) - blockTime,
          }
        : initialSync
          ? {kind: "initial" as const, progress: initialSync}
          : undefined
  if (!sample) {
    return (
      <div className="sync-progress" data-state={node.sync_status}>
        <SyncState state={node.sync_status} />
        <span className="sync-progress-pending">Waiting for synchronization data</span>
      </div>
    )
  }

  const stateDownload = sample.kind === "initial" ? sample.progress.state_download : null
  let initialPercent: number | undefined
  if (stateDownload && stateDownload.total_bytes > 0) {
    initialPercent = (stateDownload.downloaded_bytes / stateDownload.total_bytes) * 100
  } else if (
    sample.kind === "initial" &&
    typeof sample.progress.current_part === "number" &&
    typeof sample.progress.total_parts === "number" &&
    sample.progress.total_parts > 0
  ) {
    initialPercent = (sample.progress.current_part / sample.progress.total_parts) * 100
  }
  const percent =
    sample.kind === "initial"
      ? initialPercent
      : node.sync_status === "synced"
        ? 100
        : sample.kind === "blocks"
          ? Math.min(99.9, sample.target === 0 ? 100 : (sample.local / sample.target) * 100)
          : Math.min(
              99.9,
              sample.range === 0
                ? sample.lag === 0
                  ? 100
                  : 0
                : (sample.progress / sample.range) * 100,
            )
  const progressAge =
    typeof node.sync_progressed_at === "number"
      ? Math.max(0, now - node.sync_progressed_at)
      : undefined
  const stalled =
    node.sync_status === "catching_up" && progressAge !== undefined && progressAge > 120

  return (
    <div
      className="sync-progress"
      data-state={node.sync_status}
      title={progressAge === undefined ? undefined : `Last progress ${progressAge}s ago`}
    >
      <div className="sync-progress-heading">
        <SyncState state={node.sync_status} />
        {percent === undefined ? null : (
          <span className="sync-progress-percent">
            <Percentage value={percent} maximumFractionDigits={1} />
          </span>
        )}
      </div>
      {percent === undefined ? null : (
        <div
          className="sync-progress-track"
          role="progressbar"
          aria-label={`${node.name} synchronization`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Number(percent.toFixed(1))}
        >
          <span style={{width: `${percent}%`}} />
        </div>
      )}
      <div className="sync-progress-detail">
        {sample.kind === "blocks" ? (
          <>
            <span>
              {sample.local.toLocaleString()} / {sample.target.toLocaleString()}
            </span>
            <span>{sample.lag === 0 ? "At head" : `${sample.lag.toLocaleString()} behind`}</span>
          </>
        ) : sample.kind === "time" ? (
          <>
            <span>Time-based estimate</span>
            <span>
              <Duration value={sample.lag} display="elapsed" tooltip={false} /> behind
            </span>
          </>
        ) : (
          <>
            <span>{INITIAL_SYNC_LABELS[sample.progress.stage]}</span>
            <span>
              {stateDownload ? (
                <>
                  <ByteSize value={stateDownload.downloaded_bytes} /> /{" "}
                  <ByteSize value={stateDownload.total_bytes} />
                </>
              ) : typeof sample.progress.current_part === "number" &&
                typeof sample.progress.total_parts === "number" ? (
                `Part ${sample.progress.current_part.toLocaleString()} / ${sample.progress.total_parts.toLocaleString()}`
              ) : typeof sample.progress.masterchain_seqno === "number" ? (
                `Masterchain #${sample.progress.masterchain_seqno.toLocaleString()}`
              ) : (
                "Preparing"
              )}
            </span>
          </>
        )}
      </div>
      {stateDownload ? (
        <div className="sync-progress-detail">
          <span>
            <ByteSize value={stateDownload.bytes_per_second} />
            /s
          </span>
          <span>
            <Duration value={stateDownload.remaining_seconds} display="elapsed" tooltip={false} />{" "}
            remaining
          </span>
        </div>
      ) : null}
      {stalled ? (
        <span className="sync-progress-sample">
          No progress for <Duration value={progressAge} display="elapsed" tooltip={false} />
        </span>
      ) : null}
    </div>
  )
}

const VALIDATOR_LABELS: Record<NodeView["validator_status"], string> = {
  not_configured: "Not configured",
  validating: "Validating",
  leaving: "Leaving after round",
  joining: "Joining next set",
  waiting: "Waiting for election",
  inactive: "Not participating",
  unknown: "Set unavailable",
}

function ValidatorLifecycle({state}: {readonly state: NodeView["validator_status"]}) {
  return (
    <span className="validator-state" data-state={state}>
      <span aria-hidden="true" />
      {VALIDATOR_LABELS[state]}
    </span>
  )
}

type NodeRole = NodeView["roles"][number]

const NODE_ROLE_PRESENTATION: Record<
  NodeRole,
  {readonly letter: string; readonly label: string; readonly description: string}
> = {
  full_node: {
    letter: "F",
    label: "Full node",
    description: "Stores and synchronizes the current blockchain state",
  },
  validator: {
    letter: "V",
    label: "Validator",
    description: "Participates in the current validator set",
  },
  liteserver: {
    letter: "L",
    label: "Liteserver",
    description: "Serves TON data to lite clients over ADNL",
  },
}

function NodeRoleBadge({role}: {readonly role: NodeRole}) {
  const presentation = NODE_ROLE_PRESENTATION[role]
  const tooltip = `${presentation.label} — ${presentation.description}`

  return (
    <Tooltip content={tooltip} delay={0} width="wide">
      <span className="role-badge" data-role={role} aria-label={tooltip}>
        {presentation.letter}
      </span>
    </Tooltip>
  )
}

function ProductionState({node}: {readonly node: NodeView}) {
  const produced = node.produced_masterchain_blocks + node.produced_shard_blocks
  const state = node.active_validator
    ? produced > 0
      ? "producing"
      : "silent"
    : produced > 0
      ? "recent"
      : "inactive"
  const label =
    state === "producing"
      ? "Producing"
      : state === "silent"
        ? "No blocks observed"
        : state === "recent"
          ? "Produced recently"
          : "Not active"
  return (
    <span
      className="production-state"
      data-state={state}
      title={`${node.produced_masterchain_blocks.toLocaleString()} masterchain and ${node.produced_shard_blocks.toLocaleString()} shard blocks in the rolling window`}
    >
      {label}
    </span>
  )
}

function NodesTable({nodes, now}: {readonly nodes: readonly NodeView[]; readonly now: number}) {
  return (
    <DataTable className="nodes-table" minWidth="60rem">
      <DataTableTable>
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell>Status</DataTableHeaderCell>
            <DataTableHeaderCell>Node</DataTableHeaderCell>
            <DataTableHeaderCell>Synchronization</DataTableHeaderCell>
            <DataTableHeaderCell>Roles</DataTableHeaderCell>
            <DataTableHeaderCell align="right">MC blocks</DataTableHeaderCell>
            <DataTableHeaderCell align="right">Shard blocks</DataTableHeaderCell>
            <DataTableHeaderCell>Observer</DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {nodes.length === 0 ? (
            <DataTableEmpty colSpan={7}>No nodes match this filter</DataTableEmpty>
          ) : (
            nodes.map(node => (
              <DataTableRow key={`${node.observer_id}:${node.name}`}>
                <DataTableCell>
                  <StatusPill online={node.online} />
                </DataTableCell>
                <DataTableCell>
                  <div className="node-name">
                    <strong>{node.name}</strong>
                    <span>{node.public_ip}</span>
                  </div>
                </DataTableCell>
                <DataTableCell>
                  <SynchronizationProgress node={node} now={now} />
                </DataTableCell>
                <DataTableCell>
                  <div className="role-list" aria-label={`${node.name} roles`}>
                    {node.roles
                      .filter(role => role !== "validator" || node.active_validator)
                      .map(role => (
                        <NodeRoleBadge role={role} key={role} />
                      ))}
                  </div>
                </DataTableCell>
                <DataTableCell align="right">
                  <span className="tabular">
                    {node.produced_masterchain_blocks.toLocaleString()}
                  </span>
                </DataTableCell>
                <DataTableCell align="right">
                  <span className="tabular">{node.produced_shard_blocks.toLocaleString()}</span>
                </DataTableCell>
                <DataTableCell>
                  <TechnicalValue value={node.observer_id} copyLabel="observer ID" />
                </DataTableCell>
              </DataTableRow>
            ))
          )}
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}

function ValidatorsTable({nodes}: {readonly nodes: readonly NodeView[]}) {
  return (
    <DataTable minWidth="68rem">
      <DataTableTable>
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell>Validator</DataTableHeaderCell>
            <DataTableHeaderCell>Participation</DataTableHeaderCell>
            <DataTableHeaderCell>Production</DataTableHeaderCell>
            <DataTableHeaderCell>Public key</DataTableHeaderCell>
            <DataTableHeaderCell align="right">MC blocks</DataTableHeaderCell>
            <DataTableHeaderCell align="right">Shard blocks</DataTableHeaderCell>
            <DataTableHeaderCell>ADNL</DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {nodes.length === 0 ? (
            <DataTableEmpty colSpan={7}>No validators have reported yet</DataTableEmpty>
          ) : (
            nodes.map(node => (
              <DataTableRow key={`${node.observer_id}:${node.name}`}>
                <DataTableCell>
                  <strong>{node.name}</strong>
                </DataTableCell>
                <DataTableCell>
                  <ValidatorLifecycle state={node.validator_status} />
                </DataTableCell>
                <DataTableCell>
                  <ProductionState node={node} />
                </DataTableCell>
                <DataTableCell>
                  <TechnicalValue
                    value={node.validator_public_key ?? undefined}
                    copyLabel="validator public key"
                  />
                </DataTableCell>
                <DataTableCell align="right">
                  <span className="tabular">
                    {node.produced_masterchain_blocks.toLocaleString()}
                  </span>
                </DataTableCell>
                <DataTableCell align="right">
                  <span className="tabular">{node.produced_shard_blocks.toLocaleString()}</span>
                </DataTableCell>
                <DataTableCell>
                  <TechnicalValue
                    value={node.validator_adnl ?? undefined}
                    copyLabel="validator ADNL"
                  />
                </DataTableCell>
              </DataTableRow>
            ))
          )}
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}

function ShardsTable({shards, now}: {readonly shards: readonly ShardHead[]; readonly now: number}) {
  return (
    <DataTable minWidth="62rem">
      <DataTableTable>
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell>Workchain</DataTableHeaderCell>
            <DataTableHeaderCell>Shard</DataTableHeaderCell>
            <DataTableHeaderCell align="right">Seqno</DataTableHeaderCell>
            <DataTableHeaderCell>Block age</DataTableHeaderCell>
            <DataTableHeaderCell>Split or merge</DataTableHeaderCell>
            <DataTableHeaderCell>Root hash</DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {shards.length === 0 ? (
            <DataTableEmpty colSpan={6}>No shard frontier is available</DataTableEmpty>
          ) : (
            shards.map(shard => (
              <DataTableRow key={`${shard.workchain}:${shard.shard}`}>
                <DataTableCell>
                  <span className="tabular">{shard.workchain}</span>
                </DataTableCell>
                <DataTableCell mono>{shard.shard}</DataTableCell>
                <DataTableCell align="right">
                  <span className="tabular">{shard.seqno.toLocaleString()}</span>
                </DataTableCell>
                <DataTableCell>
                  <Duration value={Math.max(0, now - shard.gen_utime)} display="elapsed" />
                </DataTableCell>
                <DataTableCell>
                  {shard.want_split || shard.before_split ? (
                    <span className="topology-change">Split pending</span>
                  ) : shard.want_merge || shard.before_merge ? (
                    <span className="topology-change">Merge pending</span>
                  ) : (
                    <span className="muted">Stable</span>
                  )}
                </DataTableCell>
                <DataTableCell>
                  <TechnicalValue value={shard.root_hash} copyLabel="shard block root hash" />
                </DataTableCell>
              </DataTableRow>
            ))
          )}
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}
