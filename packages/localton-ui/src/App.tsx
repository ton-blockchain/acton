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
  useToast,
} from "@acton/ui"

import {ElectionSection} from "./components/ElectionSection"
import {Metric} from "./components/Metric"
import {NodesSection} from "./components/NodesSection"
import {StatusPill} from "./components/StatusPill"
import styles from "./App.module.css"
import type {NetworkView, NodeView, ShardHead} from "./types"

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
      <main className={styles.bootState}>
        <InlineLoader
          message="Reading network state"
          subtext="Waiting for the local observability service"
        />
      </main>
    )
  }

  return (
    <div className={styles.appShell}>
      <aside className={styles.sidebar}>
        <a className={styles.brand} href="#overview" aria-label="Localton network overview">
          <span className={styles.brandMark} aria-hidden="true">
            <Network size={17} strokeWidth={1.8} />
          </span>
          <span>Localton</span>
        </a>
        <nav className={styles.navigation} aria-label="Network sections">
          <NavigationLink href="#overview" icon={<Gauge size={15} />} label="Overview" />
          <NavigationLink href="#elections" icon={<Clock3 size={15} />} label="Elections" />
          <NavigationLink href="#nodes" icon={<RadioTower size={15} />} label="Nodes" />
          <NavigationLink href="#validators" icon={<ShieldCheck size={15} />} label="Validators" />
          <NavigationLink href="#shards" icon={<Boxes size={15} />} label="Shards" />
        </nav>
        <div className={styles.sidebarFooter}>
          <ThemeSwitch />
        </div>
      </aside>

      <div className={styles.workspace}>
        <header className={styles.topbar}>
          <div className={styles.networkTitle}>
            <h1>Network health</h1>
            <TechnicalValue value={network.network_id} copyLabel="network ID" />
          </div>
          <div className={styles.refreshState}>
            <RefreshCw size={13} aria-hidden="true" />
            {`Updated ${Math.max(0, now - network.generated_at)}s ago`}
          </div>
        </header>

        <main className={styles.content}>
          <>
            <section id="overview" className={styles.sectionStack} aria-labelledby="overview-title">
              <div className={styles.sectionHeading}>
                <h2 id="overview-title">Network overview</h2>
              </div>
              <div className={styles.metricStrip}>
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
                <div className={styles.notice}>
                  <Activity size={16} aria-hidden="true" />
                  <span>Waiting for TON network data</span>
                </div>
              )}
            </section>

            <ElectionSection election={network.election} now={now} />

            <NodesSection nodes={network.nodes} now={now} />

            <section
              id="validators"
              className={styles.sectionStack}
              aria-labelledby="validators-title"
            >
              <div className={styles.sectionHeading}>
                <h2 id="validators-title">Validator production</h2>
              </div>
              <ValidatorsTable
                nodes={network.nodes.filter(node => node.roles.includes("validator"))}
              />
            </section>

            <section id="shards" className={styles.sectionStack} aria-labelledby="shards-title">
              <div className={styles.sectionHeading}>
                <h2 id="shards-title">Shard topology</h2>
              </div>
              <ShardsTable shards={network.shards} now={now} />
            </section>

            <section className={styles.sectionStack} aria-labelledby="observers-title">
              <div className={styles.sectionHeading}>
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
                          <div className={styles.observerEndpoint}>
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
    <span className={styles.validatorState} data-state={state}>
      <span aria-hidden="true" />
      {VALIDATOR_LABELS[state]}
    </span>
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
      className={styles.productionState}
      data-state={state}
      title={`${node.produced_masterchain_blocks.toLocaleString()} masterchain and ${node.produced_shard_blocks.toLocaleString()} shard blocks in the rolling window`}
    >
      {label}
    </span>
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
                  <span className={styles.tabular}>
                    {node.produced_masterchain_blocks.toLocaleString()}
                  </span>
                </DataTableCell>
                <DataTableCell align="right">
                  <span className={styles.tabular}>
                    {node.produced_shard_blocks.toLocaleString()}
                  </span>
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
                  <span className={styles.tabular}>{shard.workchain}</span>
                </DataTableCell>
                <DataTableCell mono>{shard.shard}</DataTableCell>
                <DataTableCell align="right">
                  <span className={styles.tabular}>{shard.seqno.toLocaleString()}</span>
                </DataTableCell>
                <DataTableCell>
                  <Duration value={Math.max(0, now - shard.gen_utime)} display="elapsed" />
                </DataTableCell>
                <DataTableCell>
                  {shard.want_split || shard.before_split ? (
                    <span className={styles.topologyChange}>Split pending</span>
                  ) : shard.want_merge || shard.before_merge ? (
                    <span className={styles.topologyChange}>Merge pending</span>
                  ) : (
                    <span className={styles.muted}>Stable</span>
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
