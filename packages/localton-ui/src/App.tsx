import {useEffect, useMemo, useState} from "react"
import {
  Activity,
  Boxes,
  Clock3,
  CircleAlert,
  Gauge,
  Network,
  RadioTower,
  RefreshCw,
  ShieldCheck,
} from "lucide-react"
import {
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableTable,
  Duration,
  InlineLoader,
  RelativeTime,
  TechnicalValue,
  ThemeSwitch,
} from "@acton/ui"

import type {ElectionObservation, NetworkView, NodeView, ShardHead} from "./types"

const POLL_INTERVAL_MS = 2000

export function App() {
  const [network, setNetwork] = useState<NetworkView>()
  const [error, setError] = useState<string>()
  const [query, setQuery] = useState("")
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000))

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
          setError(undefined)
        }
      } catch (cause) {
        if (active) setError(cause instanceof Error ? cause.message : "Network request failed")
      } finally {
        if (active) timer = globalThis.setTimeout(load, POLL_INTERVAL_MS)
      }
    }

    void load()
    return () => {
      active = false
      if (timer !== undefined) globalThis.clearTimeout(timer)
    }
  }, [])

  const visibleNodes = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    if (!normalized) return network?.nodes ?? []
    return (network?.nodes ?? []).filter(node =>
      [node.name, node.public_ip, node.observer_id, node.validator_adnl, ...node.roles]
        .filter(Boolean)
        .some(value => value?.toLowerCase().includes(normalized)),
    )
  }, [network, query])

  if (!network && !error) {
    return (
      <main className="boot-state">
        <InlineLoader message="Reading network state" subtext="Waiting for the first signed observation" />
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
            {network ? <TechnicalValue value={network.network_id} copyLabel="network ID" /> : null}
          </div>
          <div className="refresh-state">
            <RefreshCw size={13} aria-hidden="true" />
            {network ? `Updated ${Math.max(0, now - network.generated_at)}s ago` : "Waiting"}
          </div>
        </header>

        <main className="content">
          {error ? (
            <div className="notice notice-error" role="alert">
              <CircleAlert size={16} aria-hidden="true" />
              <span>{error}</span>
            </div>
          ) : null}
          {network ? (
            <>
              <section id="overview" className="section-stack" aria-labelledby="overview-title">
                <div className="section-heading">
                  <h2 id="overview-title">Network overview</h2>
                  <ChainTrust source={network.chain_source} />
                </div>
                <div className="metric-strip">
                  <Metric label="Online nodes" value={`${network.totals.online_nodes} / ${network.totals.nodes}`} tone={network.totals.online_nodes === network.totals.nodes ? "good" : "warning"} />
                  <Metric label="Active validators" value={`${network.totals.active_validators} / ${network.totals.configured_validators}`} tone={network.totals.active_validators === network.totals.configured_validators ? "good" : "warning"} />
                  <Metric label="Masterchain" value={network.chain ? `#${network.chain.seqno.toLocaleString()}` : "Unavailable"} />
                  <Metric label="Current shards" value={String(network.chain?.shard_count ?? 0)} />
                  <Metric label="Observed blocks" value={(network.totals.masterchain_blocks + network.totals.shard_blocks).toLocaleString()} />
                </div>
                {network.chain ? (
                  <div className="chain-line">
                    <span>Latest masterchain block</span>
                    <TechnicalValue value={network.chain.root_hash} copyLabel="block root hash" />
                    <span className="chain-age">
                      <Duration value={Math.max(0, now - network.chain.gen_utime)} display="elapsed" /> old
                    </span>
                  </div>
                ) : (
                  <div className="notice">
                    <Activity size={16} aria-hidden="true" />
                    <span>No observer can currently verify the chain head</span>
                  </div>
                )}
              </section>

              <section id="elections" className="section-stack" aria-labelledby="elections-title">
                <div className="section-heading">
                  <h2 id="elections-title">Election round</h2>
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
                  <label className="search-field">
                    <span className="visually-hidden">Filter nodes</span>
                    <input value={query} onChange={event => setQuery(event.target.value)} placeholder="Filter nodes" />
                  </label>
                </div>
                <NodesTable nodes={visibleNodes} />
              </section>

              <section id="validators" className="section-stack" aria-labelledby="validators-title">
                <div className="section-heading">
                  <h2 id="validators-title">Validator production</h2>
                  <span className="section-meta">Rolling observation window</span>
                </div>
                <ValidatorsTable nodes={network.nodes.filter(node => node.roles.includes("validator"))} />
              </section>

              <section id="shards" className="section-stack" aria-labelledby="shards-title">
                <div className="section-heading">
                  <h2 id="shards-title">Shard topology</h2>
                  <span className="section-meta">{network.shards.length} current</span>
                </div>
                <ShardsTable shards={network.shards} now={now} />
              </section>

              <section className="section-stack" aria-labelledby="observers-title">
                <div className="section-heading">
                  <h2 id="observers-title">Signed observers</h2>
                  <span className="section-meta">{network.totals.online_observers} online</span>
                </div>
                <DataTable minWidth="44rem">
                  <DataTableTable>
                    <DataTableHead>
                      <DataTableRow>
                        <DataTableHeaderCell>Status</DataTableHeaderCell>
                        <DataTableHeaderCell>Observer</DataTableHeaderCell>
                        <DataTableHeaderCell>Endpoint</DataTableHeaderCell>
                        <DataTableHeaderCell align="right">Nodes</DataTableHeaderCell>
                        <DataTableHeaderCell>Last report</DataTableHeaderCell>
                      </DataTableRow>
                    </DataTableHead>
                    <DataTableBody>
                      {network.observers.map(observer => (
                        <DataTableRow key={observer.observer_id}>
                          <DataTableCell><StatusPill online={observer.online} /></DataTableCell>
                          <DataTableCell><TechnicalValue value={observer.observer_id} copyLabel="observer ID" /></DataTableCell>
                          <DataTableCell mono truncate>{observer.endpoint}</DataTableCell>
                          <DataTableCell align="right"><span className="tabular">{observer.node_count.toLocaleString()}</span></DataTableCell>
                          <DataTableCell><RelativeTime value={observer.generated_at} now={now} unit="seconds" /></DataTableCell>
                        </DataTableRow>
                      ))}
                    </DataTableBody>
                  </DataTableTable>
                </DataTable>
              </section>
            </>
          ) : null}
        </main>
      </div>
    </div>
  )
}

function NavigationLink({href, icon, label}: {readonly href: string; readonly icon: React.ReactNode; readonly label: string}) {
  return <a href={href}>{icon}<span>{label}</span></a>
}

function Metric({label, value, tone = "default"}: {readonly label: string; readonly value: string; readonly tone?: "default" | "good" | "warning"}) {
  return <div className="metric"><span>{label}</span><strong data-tone={tone}>{value}</strong></div>
}

function ChainTrust({source}: {readonly source: NetworkView["chain_source"]}) {
  const label = source === "local_verification" ? "Verified locally" : source === "peer_attestation" ? "Peer attestation" : "Unavailable"
  return <span className="trust" data-source={source}><ShieldCheck size={13} aria-hidden="true" />{label}</span>
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
  return <span className="election-stage" data-stage={stage}>{ELECTION_STAGE_LABELS[stage]}</span>
}

function ElectionDiagram({election, now}: {readonly election: ElectionObservation; readonly now: number}) {
  const start = election.validation_started_at
  const end = Math.max(start + 1, election.next_set_activation_at)
  const retrying = election.stage === "retrying"
  const chartEnd = retrying ? Math.max(end + 30, now + 30) : end
  const chartStart = 48
  const chartWidth = 904
  const x = (value: number) => chartStart + ((Math.min(chartEnd, Math.max(start, value)) - start) / (chartEnd - start)) * chartWidth
  const openX = x(election.elections_open_at)
  const closeX = x(election.elections_close_at)
  const endX = x(end)
  const chartEndX = x(chartEnd)
  const nowX = x(now)
  const boundaries = [...new Set([chartStart, openX, closeX, endX])]
  const timeMarkers = [...new Set([start, election.elections_open_at, election.elections_close_at, end])]
  const formatTime = (value: number) => new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(value * 1000)
  const stages = [
    {label: "Current set", start, end: election.elections_open_at},
    {label: "Entry window", start: election.elections_open_at, end: election.elections_close_at},
    {label: election.next_validators === null ? "Selection" : "Next set ready", start: election.elections_close_at, end},
  ].filter(stage => stage.end > stage.start)

  return (
    <div className="election-panel">
      <div className="election-summary">
        <Metric label="Round ID" value={election.round_id.toLocaleString()} />
        <Metric label="Current set" value={formatValidators(election.current_validators)} />
        <Metric label="Main subset" value={formatValidators(election.current_main_validators)} />
        <Metric label="Next set" value={election.next_validators === null ? "Pending" : formatValidators(election.next_validators)} />
        <Metric label="Stake hold" value={`${election.stake_held_for}s`} />
      </div>
      <div className="election-chart" data-stage={election.stage}>
        <svg viewBox="0 0 1000 126" role="img" aria-labelledby="election-chart-title election-chart-description">
          <title id="election-chart-title">Validator election timeline</title>
          <desc id="election-chart-description">Current validation, entry window, next validator selection, and activation</desc>
          <rect className="timeline-segment timeline-validation" x={chartStart} y="48" width={Math.max(0, openX - chartStart)} height="16" rx="8" />
          <rect className="timeline-segment timeline-entry" x={openX} y="48" width={Math.max(0, closeX - openX)} height="16" />
          <rect className="timeline-segment timeline-selection" x={closeX} y="48" width={Math.max(0, endX - closeX)} height="16" rx="8" />
          {retrying && <rect className="timeline-segment timeline-retry" x={endX} y="48" width={Math.max(0, chartEndX - endX)} height="16" rx="8" />}
          {boundaries.map(position => <circle key={position} className="timeline-boundary" cx={position} cy="56" r={position === endX ? 6 : 4} />)}
          <line className="timeline-now-line" x1={nowX} x2={nowX} y1="25" y2="87" />
          <circle className="timeline-now-dot" cx={nowX} cy="56" r="6" />
          <text className="timeline-now-label" x={nowX} y="17" textAnchor={nowX > 900 ? "end" : nowX < 100 ? "start" : "middle"}>NOW</text>
          {timeMarkers.map(value => {
            const position = x(value)
            return <text className="timeline-time" key={value} x={position} y="105" textAnchor={position === chartStart ? "start" : position === endX ? "end" : "middle"}>{formatTime(value)}</text>
          })}
        </svg>
      </div>
      <div className="election-steps">
        {stages.map(stage => {
          const state = now >= stage.end ? "complete" : now >= stage.start ? "active" : "upcoming"
          return (
            <div className="election-step" data-state={state} key={stage.label}>
              <span className="election-step-marker" aria-hidden="true" />
              <div><strong>{stage.label}</strong><span>{formatTime(stage.start)}–{formatTime(stage.end)}</span></div>
            </div>
          )
        })}
        <div className="election-step" data-state={retrying ? "waiting" : now >= end ? election.next_validators === null ? "waiting" : "overdue" : "upcoming"}>
          <span className="election-step-marker" aria-hidden="true" />
          <div>
            <strong>{retrying ? "Automatic election retry" : "Next set activation"}</strong>
            <span>{retrying ? `${Math.max(0, now - end).toLocaleString()}s since scheduled activation` : formatTime(end)}</span>
          </div>
        </div>
      </div>
    </div>
  )
}

function formatValidators(count: number) {
  return `${count.toLocaleString()} ${count === 1 ? "validator" : "validators"}`
}

function StatusPill({online}: {readonly online: boolean}) {
  return <span className="status-pill" data-online={online ? "true" : "false"}><span aria-hidden="true" />{online ? "Online" : "Offline"}</span>
}

function NodesTable({nodes}: {readonly nodes: readonly NodeView[]}) {
  return (
    <DataTable minWidth="68rem" meta={`${nodes.length} visible`}>
      <DataTableTable>
        <DataTableHead><DataTableRow>
          <DataTableHeaderCell>Status</DataTableHeaderCell><DataTableHeaderCell>Node</DataTableHeaderCell><DataTableHeaderCell>Roles</DataTableHeaderCell><DataTableHeaderCell align="right">Head</DataTableHeaderCell><DataTableHeaderCell align="right">Lag</DataTableHeaderCell><DataTableHeaderCell align="right">MC blocks</DataTableHeaderCell><DataTableHeaderCell align="right">Shard blocks</DataTableHeaderCell><DataTableHeaderCell>Observer</DataTableHeaderCell>
        </DataTableRow></DataTableHead>
        <DataTableBody>
          {nodes.length === 0 ? <DataTableEmpty colSpan={8}>No nodes match this filter</DataTableEmpty> : nodes.map(node => (
            <DataTableRow key={`${node.observer_id}:${node.name}`}>
              <DataTableCell><StatusPill online={node.online} /></DataTableCell>
              <DataTableCell><div className="node-name"><strong>{node.name}</strong><span>{node.public_ip}</span></div></DataTableCell>
              <DataTableCell><div className="role-list">{node.roles.map(role => <span key={role}>{role.replaceAll("_", " ")}</span>)}</div></DataTableCell>
              <DataTableCell align="right"><span className="tabular">{node.head_seqno?.toLocaleString() ?? "—"}</span></DataTableCell>
              <DataTableCell align="right"><span data-lag={(node.sync_lag_blocks ?? 0) > 3 ? "high" : "normal"}>{node.sync_lag_blocks ?? "—"}</span></DataTableCell>
              <DataTableCell align="right"><span className="tabular">{node.produced_masterchain_blocks.toLocaleString()}</span></DataTableCell>
              <DataTableCell align="right"><span className="tabular">{node.produced_shard_blocks.toLocaleString()}</span></DataTableCell>
              <DataTableCell><TechnicalValue value={node.observer_id} copyLabel="observer ID" /></DataTableCell>
            </DataTableRow>
          ))}
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}

function ValidatorsTable({nodes}: {readonly nodes: readonly NodeView[]}) {
  return (
    <DataTable minWidth="58rem">
      <DataTableTable><DataTableHead><DataTableRow>
        <DataTableHeaderCell>Validator</DataTableHeaderCell><DataTableHeaderCell>Activity</DataTableHeaderCell><DataTableHeaderCell>Public key</DataTableHeaderCell><DataTableHeaderCell align="right">MC blocks</DataTableHeaderCell><DataTableHeaderCell align="right">Shard blocks</DataTableHeaderCell><DataTableHeaderCell>ADNL</DataTableHeaderCell>
      </DataTableRow></DataTableHead><DataTableBody>
        {nodes.length === 0 ? <DataTableEmpty colSpan={6}>No validators have reported yet</DataTableEmpty> : nodes.map(node => (
          <DataTableRow key={`${node.observer_id}:${node.name}`}>
            <DataTableCell><strong>{node.name}</strong></DataTableCell>
            <DataTableCell><span className="validator-state" data-active={node.active_validator ? "true" : "false"}>{node.active_validator ? "Producing" : "Not observed"}</span></DataTableCell>
            <DataTableCell><TechnicalValue value={node.validator_public_key} copyLabel="validator public key" /></DataTableCell>
            <DataTableCell align="right"><span className="tabular">{node.produced_masterchain_blocks.toLocaleString()}</span></DataTableCell>
            <DataTableCell align="right"><span className="tabular">{node.produced_shard_blocks.toLocaleString()}</span></DataTableCell>
            <DataTableCell><TechnicalValue value={node.validator_adnl} copyLabel="validator ADNL" /></DataTableCell>
          </DataTableRow>
        ))}
      </DataTableBody></DataTableTable>
    </DataTable>
  )
}

function ShardsTable({shards, now}: {readonly shards: readonly ShardHead[]; readonly now: number}) {
  return (
    <DataTable minWidth="62rem">
      <DataTableTable><DataTableHead><DataTableRow>
        <DataTableHeaderCell>Workchain</DataTableHeaderCell><DataTableHeaderCell>Shard</DataTableHeaderCell><DataTableHeaderCell align="right">Seqno</DataTableHeaderCell><DataTableHeaderCell>Block age</DataTableHeaderCell><DataTableHeaderCell>Split or merge</DataTableHeaderCell><DataTableHeaderCell>Root hash</DataTableHeaderCell>
      </DataTableRow></DataTableHead><DataTableBody>
        {shards.length === 0 ? <DataTableEmpty colSpan={6}>No shard frontier is available</DataTableEmpty> : shards.map(shard => (
          <DataTableRow key={`${shard.workchain}:${shard.shard}`}>
            <DataTableCell><span className="tabular">{shard.workchain}</span></DataTableCell>
            <DataTableCell mono>{shard.shard}</DataTableCell>
            <DataTableCell align="right"><span className="tabular">{shard.seqno.toLocaleString()}</span></DataTableCell>
            <DataTableCell><Duration value={Math.max(0, now - shard.gen_utime)} display="elapsed" /></DataTableCell>
            <DataTableCell>{shard.want_split || shard.before_split ? <span className="topology-change">Split pending</span> : shard.want_merge || shard.before_merge ? <span className="topology-change">Merge pending</span> : <span className="muted">Stable</span>}</DataTableCell>
            <DataTableCell><TechnicalValue value={shard.root_hash} copyLabel="shard block root hash" /></DataTableCell>
          </DataTableRow>
        ))}
      </DataTableBody></DataTableTable>
    </DataTable>
  )
}
