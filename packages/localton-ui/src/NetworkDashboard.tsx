import {lazy, Suspense, useEffect, useState} from "react"
import type {MouseEvent, ReactNode} from "react"
import {Activity} from "lucide-react"
import {
  AddressChip,
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
  GramAmount,
  Percentage,
  RelativeTime,
  Skeleton,
  TechnicalValue,
  Tooltip,
} from "@acton/ui"

import {ElectionSection, validatorWeightParts} from "./components/ElectionSection"
import {Metric} from "./components/Metric"
import {NodesSection} from "./components/NodesSection"
import {StatusPill} from "./components/StatusPill"
import type {ObservabilityClient} from "./observability"
import {useObservability} from "./observability"
import styles from "./App.module.css"
import type {
  NetworkView,
  NodeView,
  ShardHead,
  TpsView,
  ValidatorEfficiencyObservation,
  ValidatorObservation,
} from "./types"

const TpsSection = lazy(() => import("./components/TpsSection"))

export type NetworkDashboardView = "all" | "overview" | "nodes" | "validators"

export interface NetworkDashboardProps {
  readonly client: ObservabilityClient
  /** Keeps host-managed offline nodes accessible before the collector has a report */
  readonly fallbackNodes?: readonly NodeView[]
  /** Host-owned controls rendered after the node table without coupling them to Localton */
  readonly nodesFooter?: ReactNode
  /** Opens validator wallet addresses in the host application's account view */
  readonly onAddressClick?: (address: string, event?: MouseEvent<HTMLElement>) => void
  readonly onNetworkChange?: (network: NetworkView) => void
  /** Host-owned row controls; standalone Localton omits this callback */
  readonly renderNodeActions?: (node: NodeView) => ReactNode
  readonly view?: NetworkDashboardView
}

/** Renders collector-backed network pages without owning product navigation or page chrome */
export function NetworkDashboard({
  client,
  fallbackNodes = [],
  nodesFooter,
  onAddressClick,
  onNetworkChange,
  renderNodeActions,
  view = "all",
}: NetworkDashboardProps) {
  const {network, now, tps} = useObservability(client)

  useEffect(() => {
    if (network) onNetworkChange?.(network)
  }, [network, onNetworkChange])

  if (!network) {
    return <NetworkDashboardSkeleton nodesFooter={nodesFooter} view={view} />
  }

  return (
    <NetworkDashboardContent
      network={{
        ...network,
        nodes: [
          ...network.nodes,
          ...fallbackNodes.filter(
            node =>
              !network.nodes.some(
                observed => observed.name.toLowerCase() === node.name.toLowerCase(),
              ),
          ),
        ],
      }}
      nodesFooter={nodesFooter}
      now={now}
      onAddressClick={onAddressClick}
      renderNodeActions={renderNodeActions}
      tps={tps}
      view={view}
    />
  )
}

interface LoadingTableColumn {
  readonly align?: "left" | "right"
  readonly label: string
  readonly width?: string
}

function NetworkDashboardSkeleton({
  nodesFooter,
  view,
}: {
  readonly nodesFooter?: ReactNode
  readonly view: NetworkDashboardView
}) {
  return (
    <div className={styles.dashboardContent} aria-label="Loading network state" aria-busy="true">
      {view === "all" || view === "overview" ? (
        <>
          <NetworkOverviewSkeleton showTitle={view === "all"} />
          <TpsSkeleton />
        </>
      ) : null}

      {view === "all" || view === "validators" ? (
        <>
          <LoadingTableSection
            ariaLabel="Current validation round"
            columns={[
              {label: "Metric", width: "12rem"},
              {label: "Value", width: "22rem"},
            ]}
            minWidth="36rem"
            rows={3}
            title="Current validation round"
          />
          <ElectionSkeleton />
          <LoadingTableSection
            ariaLabel="Validator production"
            columns={[
              {label: "Validator", width: "8rem"},
              {label: "Participation", width: "8rem"},
              {label: "Production", width: "7rem"},
              {label: "Public key", width: "9rem"},
              {label: "MC blocks", align: "right", width: "4rem"},
              {label: "Shard blocks", align: "right", width: "4rem"},
              {label: "ADNL", width: "9rem"},
            ]}
            minWidth="68rem"
            rows={1}
            title="Validator production"
          />
          <LoadingTableSection
            ariaLabel="Current round validator performance"
            columns={[
              {label: "#", width: "2rem"},
              {label: "Validator", width: "7rem"},
              {label: "Efficiency", align: "right", width: "6rem"},
              {label: "Weight", align: "right", width: "5rem"},
              {label: "Stake", align: "right", width: "8rem"},
              {label: "Node version", width: "6rem"},
              {label: "Wallet", width: "13rem"},
              {label: "Type", width: "4rem"},
            ]}
            minWidth="56rem"
            rows={1}
            title="Current round validator performance"
          />
        </>
      ) : null}

      {view === "all" || view === "nodes" ? (
        <>
          <LoadingTableSection
            ariaLabel="Nodes and synchronization"
            columns={[
              {label: "Status", width: "5rem"},
              {label: "Node", width: "8rem"},
              {label: "Synchronization", width: "14rem"},
              {label: "Roles", width: "5rem"},
              {label: "MC blocks", align: "right", width: "4rem"},
              {label: "Shard blocks", align: "right", width: "4rem"},
            ]}
            footer={nodesFooter}
            minWidth="50rem"
            rows={1}
            rowSize="node"
            showTitle={view === "all"}
            title="Nodes and synchronization"
          />
          <LoadingTableSection
            ariaLabel="Collector diagnostics"
            columns={[
              {label: "Status", width: "5rem"},
              {label: "Observer", width: "9rem"},
              {label: "Endpoint", width: "13rem"},
              {label: "Last report", width: "7rem"},
            ]}
            minWidth="44rem"
            rows={1}
            title="Collector diagnostics"
          />
        </>
      ) : null}

      {view === "all" || view === "overview" ? (
        <LoadingTableSection
          ariaLabel="Shard topology"
          columns={[
            {label: "Workchain", width: "6rem"},
            {label: "Shard", width: "10rem"},
            {label: "Seqno", align: "right", width: "5rem"},
            {label: "Block age", width: "6rem"},
            {label: "Split or merge", width: "8rem"},
            {label: "Root hash", width: "12rem"},
          ]}
          minWidth="62rem"
          rows={1}
          title="Shard topology"
        />
      ) : null}
    </div>
  )
}

function NetworkOverviewSkeleton({showTitle}: {readonly showTitle: boolean}) {
  return (
    <section className={styles.sectionStack} aria-label="Loading network overview">
      {showTitle ? (
        <div className={styles.sectionHeading}>
          <h2>Network overview</h2>
        </div>
      ) : null}
      <div className={styles.metricStrip}>
        {["Online nodes", "Synchronized", "Active validators", "Masterchain", "Current shards"].map(
          label => (
            <Metric
              key={label}
              label={label}
              value={<Skeleton width="4.5rem" height="1.375rem" />}
            />
          ),
        )}
      </div>
    </section>
  )
}

function ElectionSkeleton() {
  return (
    <section className={styles.sectionStack} aria-label="Loading validator elections">
      <div className={styles.electionSkeletonPanel}>
        <div className={styles.electionSkeletonSummary}>
          {["Round ID", "Current set", "Main subset", "Next set", "Stake hold"].map(label => (
            <Metric
              key={label}
              density="compact"
              label={label}
              value={<Skeleton width="5.5rem" height="1.0625rem" />}
            />
          ))}
        </div>
        <div className={styles.electionSkeletonChart}>
          {Array.from({length: 3}, (_, index) => (
            <div className={styles.electionSkeletonRound} key={index}>
              <Skeleton width={index === 1 ? "11rem" : "9rem"} />
              <Skeleton shape="rect" width="100%" height="0.7rem" radius="round" />
            </div>
          ))}
        </div>
        <div className={styles.electionSkeletonDisclosures}>
          {Array.from({length: 3}, (_, index) => (
            <div className={styles.electionSkeletonDisclosure} key={index}>
              <Skeleton width={index === 2 ? "8rem" : "14rem"} />
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}

function LoadingTableSection({
  ariaLabel,
  columns,
  footer,
  minWidth,
  rows = 3,
  rowSize = "default",
  showTitle = true,
  title,
}: {
  readonly ariaLabel: string
  readonly columns: readonly LoadingTableColumn[]
  readonly footer?: ReactNode
  readonly minWidth: string
  readonly rows?: number
  readonly rowSize?: "default" | "node"
  readonly showTitle?: boolean
  readonly title: string
}) {
  return (
    <section className={styles.sectionStack} aria-label={`Loading ${ariaLabel}`}>
      {showTitle ? (
        <div className={styles.sectionHeading}>
          <h2>{title}</h2>
        </div>
      ) : null}
      <DataTable
        className={rowSize === "node" ? styles.nodeSkeletonTable : undefined}
        minWidth={minWidth}
      >
        <DataTableTable aria-label={ariaLabel}>
          <DataTableHead>
            <DataTableRow>
              {columns.map(column => (
                <DataTableHeaderCell key={column.label} align={column.align}>
                  {column.label}
                </DataTableHeaderCell>
              ))}
            </DataTableRow>
          </DataTableHead>
          <DataTableBody>
            <DataTableSkeletonRows
              alignments={columns.map(column => column.align ?? "left")}
              columns={columns.length}
              rowKeyPrefix={`network-${ariaLabel.toLowerCase().replaceAll(" ", "-")}`}
              rows={rows}
              widths={columns.map(column => column.width)}
            />
          </DataTableBody>
        </DataTableTable>
      </DataTable>
      {footer ? <div className={styles.loadingTableFooter}>{footer}</div> : null}
    </section>
  )
}

interface NetworkDashboardContentProps {
  readonly network: NetworkView
  readonly nodesFooter?: ReactNode
  readonly now: number
  readonly onAddressClick?: (address: string, event?: MouseEvent<HTMLElement>) => void
  readonly renderNodeActions?: (node: NodeView) => ReactNode
  readonly tps: TpsView | undefined
  readonly view?: NetworkDashboardView
}

/** Presents a supplied snapshot so shells can share polling with their own live status chrome */
export function NetworkDashboardContent({
  network,
  nodesFooter,
  now,
  onAddressClick,
  renderNodeActions,
  tps,
  view = "all",
}: NetworkDashboardContentProps) {
  return (
    <div className={styles.dashboardContent}>
      {view === "all" || view === "overview" ? (
        <>
          <NetworkOverviewSection network={network} showTitle={view === "all"} />
          <DeferredTpsSection series={tps} />
        </>
      ) : null}

      {view === "all" || view === "validators" ? (
        <>
          <ValidationRoundSection network={network} now={now} />
          <ElectionSection election={network.election} now={now} />
          <ValidatorsSection nodes={network.nodes} />
          <ValidatorPerformanceSection network={network} onAddressClick={onAddressClick} />
        </>
      ) : null}

      {view === "all" || view === "nodes" ? (
        <>
          <NodesSection
            footer={nodesFooter}
            nodes={network.nodes}
            now={now}
            renderNodeActions={renderNodeActions}
            showLocations={view === "all"}
            showTitle={view === "all"}
          />
          <ObserverDiagnostics network={network} now={now} />
        </>
      ) : null}

      {view === "all" || view === "overview" ? (
        <ShardsSection shards={network.shards} now={now} showTitle />
      ) : null}
    </div>
  )
}

function DeferredTpsSection({series}: {readonly series: TpsView | undefined}) {
  const [ready, setReady] = useState(false)

  useEffect(() => {
    if (typeof globalThis.requestIdleCallback === "function") {
      const request = globalThis.requestIdleCallback(() => setReady(true), {timeout: 800})
      return () => globalThis.cancelIdleCallback(request)
    }

    const request = globalThis.setTimeout(() => setReady(true), 0)
    return () => globalThis.clearTimeout(request)
  }, [])

  if (!ready) return <TpsSkeleton />

  return (
    <Suspense fallback={<TpsSkeleton />}>
      <TpsSection series={series} />
    </Suspense>
  )
}

function TpsSkeleton() {
  return (
    <>
      <PerformanceSectionSkeleton title="Transaction throughput" />
      <PerformanceSectionSkeleton title="Masterchain block time" />
    </>
  )
}

function PerformanceSectionSkeleton({title}: {readonly title: string}) {
  return (
    <section className={styles.tpsSkeleton} aria-label={`Loading ${title.toLowerCase()}`} aria-busy>
      <div className={styles.tpsSkeletonHeading}>
        <h2>{title}</h2>
      </div>
      <Skeleton shape="rect" width="100%" height="22.375rem" radius="md" />
    </section>
  )
}

function NetworkOverviewSection({
  network,
  showTitle,
}: {
  readonly network: NetworkView
  readonly showTitle: boolean
}) {
  return (
    <section
      id="overview"
      className={styles.sectionStack}
      aria-label={showTitle ? undefined : "Network overview"}
      aria-labelledby={showTitle ? "overview-title" : undefined}
    >
      {showTitle ? (
        <div className={styles.sectionHeading}>
          <h2 id="overview-title">Network overview</h2>
        </div>
      ) : null}
      <div className={styles.metricStrip}>
        <Metric
          label="Online nodes"
          value={`${network.totals.online_nodes} / ${network.totals.nodes}`}
          tone={network.totals.online_nodes === network.totals.nodes ? "good" : "warning"}
        />
        <Metric
          label="Synchronized"
          value={`${network.totals.synchronized_nodes} / ${network.totals.nodes}`}
          tone={network.totals.synchronized_nodes === network.totals.nodes ? "good" : "warning"}
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

function ValidatorsSection({nodes}: {readonly nodes: readonly NodeView[]}) {
  const validators = nodes.filter(node => node.roles.includes("validator"))

  return (
    <section id="validators" className={styles.sectionStack} aria-labelledby="validators-title">
      <div className={styles.sectionHeading}>
        <h2 id="validators-title">Validator production</h2>
      </div>
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
            {validators.length === 0 ? (
              <DataTableEmpty colSpan={7}>No validators have reported yet</DataTableEmpty>
            ) : (
              validators.map(node => (
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
    </section>
  )
}

function ValidationRoundSection({
  network,
  now,
}: {
  readonly network: NetworkView
  readonly now: number
}) {
  const election = network.election
  const members = election?.current.members ?? []
  const stakes = members.flatMap(validator => {
    const stake = nodeForValidator(network.nodes, validator)?.validator_stake_nano

    return stake ? [BigInt(stake)] : []
  })
  const completeStakeSet = members.length > 0 && stakes.length === members.length
  const totalStake = completeStakeSet
    ? stakes.reduce((total, stake) => total + stake, 0n).toString()
    : undefined
  const minimumStake = completeStakeSet
    ? stakes.reduce((minimum, stake) => (stake < minimum ? stake : minimum)).toString()
    : undefined
  const maximumStake = completeStakeSet
    ? stakes.reduce((maximum, stake) => (stake > maximum ? stake : maximum)).toString()
    : undefined

  return (
    <section className={styles.sectionStack} aria-labelledby="validation-round-title">
      <div className={styles.sectionHeading}>
        <h2 id="validation-round-title">Current validation round</h2>
      </div>
      {election ? (
        <div className={styles.validationRoundPanel} aria-label="Current validation round">
          <ValidationRoundGroup label="Round" columns={4}>
            <ValidationRoundMetric
              label="Number"
              value={election.current.round_id.toLocaleString()}
            />
            <ValidationRoundMetric
              label="Start"
              value={
                <ValidationRoundTime now={now} timestamp={election.current.validation_started_at} />
              }
            />
            <ValidationRoundMetric
              label="End"
              value={
                <ValidationRoundTime now={now} timestamp={election.current.validation_ended_at} />
              }
            />
            <ValidationRoundMetric
              label="Unfreezing stakes"
              value={
                <ValidationRoundTime
                  now={now}
                  timestamp={election.current.validation_ended_at + election.stake_held_for}
                />
              }
            />
          </ValidationRoundGroup>
          <ValidationRoundGroup label="Stake" columns={4}>
            <ValidationRoundMetric
              label="Validators"
              value={`${election.current.validators.toLocaleString()} / ${election.max_validators.toLocaleString()}`}
            />
            <ValidationRoundMetric label="Total" value={<GramAmount value={totalStake} />} />
            <ValidationRoundMetric label="Actual min" value={<GramAmount value={minimumStake} />} />
            <ValidationRoundMetric label="Actual max" value={<GramAmount value={maximumStake} />} />
          </ValidationRoundGroup>
          <ValidationRoundGroup label="Network config" columns={4}>
            <ValidationRoundMetric
              label="Min stake"
              value={<GramAmount value={election.min_stake_nano} />}
            />
            <ValidationRoundMetric
              label="Max stake"
              value={<GramAmount value={election.max_stake_nano} />}
            />
            <ValidationRoundMetric
              label="Min validators"
              value={election.min_validators.toLocaleString()}
            />
            <ValidationRoundMetric
              label="Max validators"
              value={election.max_validators.toLocaleString()}
            />
            <ValidationRoundMetric
              label="Max masterchain validators"
              value={election.max_main_validators.toLocaleString()}
            />
          </ValidationRoundGroup>
        </div>
      ) : (
        <div className={styles.validationRoundEmpty}>Current validation round is not available</div>
      )}
    </section>
  )
}

function ValidationRoundGroup({
  children,
  columns,
  label,
}: {
  readonly children: ReactNode
  readonly columns: number
  readonly label: string
}) {
  return (
    <div className={styles.validationRoundGroup}>
      <div className={styles.validationRoundGroupLabel}>{label}</div>
      <div className={styles.validationRoundGrid} data-columns={columns}>
        {children}
      </div>
    </div>
  )
}

function ValidationRoundMetric({
  label,
  value,
}: {
  readonly label: string
  readonly value: ReactNode
}) {
  return (
    <div className={styles.validationRoundMetric}>
      <span className={styles.validationRoundMetricLabel}>{label}</span>
      <span className={styles.validationRoundMetricValue}>{value}</span>
    </div>
  )
}

function ValidationRoundTime({now, timestamp}: {readonly now: number; readonly timestamp: number}) {
  return (
    <span className={styles.validationRoundTime}>
      <DateTime display="date-time-numeric-seconds" unit="seconds" value={timestamp} />
      <span className={styles.validationRoundRelativeTime}>
        (<RelativeTime mode="relative" now={now} unit="seconds" value={timestamp} />)
      </span>
    </span>
  )
}

function ValidatorPerformanceSection({
  network,
  onAddressClick,
}: {
  readonly network: NetworkView
  readonly onAddressClick?: (address: string, event?: MouseEvent<HTMLElement>) => void
}) {
  const currentSet = network.election?.current
  const validators = currentSet?.members ?? []

  return (
    <section className={styles.sectionStack} aria-labelledby="validator-performance-title">
      <div className={styles.sectionHeading}>
        <h2 id="validator-performance-title">Current round validator performance</h2>
      </div>
      <DataTable className={styles.validatorPerformanceTable} minWidth="56rem">
        <DataTableTable aria-label="Current round validator performance" layout="fixed">
          <DataTableHead>
            <DataTableRow>
              <DataTableHeaderCell columnWidth="3rem">#</DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="9rem">Validator</DataTableHeaderCell>
              <DataTableHeaderCell align="right" columnWidth="8rem">
                Efficiency
              </DataTableHeaderCell>
              <DataTableHeaderCell align="right" columnWidth="7rem">
                Weight
              </DataTableHeaderCell>
              <DataTableHeaderCell align="right" columnWidth="8rem">
                Stake
              </DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="8rem">Node version</DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="14rem">Wallet</DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="5rem">Type</DataTableHeaderCell>
            </DataTableRow>
          </DataTableHead>
          <DataTableBody>
            {currentSet ? (
              currentSet.members === undefined ? (
                <DataTableEmpty colSpan={8}>Validator identities are not available</DataTableEmpty>
              ) : validators.length === 0 ? (
                <DataTableEmpty colSpan={8}>No validators in the current round</DataTableEmpty>
              ) : (
                validators.map((validator, index) => {
                  const node = nodeForValidator(network.nodes, validator)

                  return (
                    <ValidatorPerformanceRow
                      index={index}
                      key={`${validator.public_key}:${index}`}
                      node={node}
                      onAddressClick={onAddressClick}
                      totalWeight={currentSet.total_weight ?? "0"}
                      validator={validator}
                    />
                  )
                })
              )
            ) : (
              <DataTableEmpty colSpan={8}>Current validator set is not available</DataTableEmpty>
            )}
          </DataTableBody>
        </DataTableTable>
      </DataTable>
    </section>
  )
}

function ValidatorPerformanceRow({
  index,
  node,
  onAddressClick,
  totalWeight,
  validator,
}: {
  readonly index: number
  readonly node: NodeView | undefined
  readonly onAddressClick?: (address: string, event?: MouseEvent<HTMLElement>) => void
  readonly totalWeight: string
  readonly validator: ValidatorObservation
}) {
  const weightParts = validatorWeightParts(validator.weight, totalWeight)

  return (
    <DataTableRow hover>
      <DataTableCell tone="muted">{index + 1}</DataTableCell>
      <DataTableCell>
        <strong>{node?.name ?? `Validator ${index + 1}`}</strong>
      </DataTableCell>
      <DataTableCell align="right">
        <ValidatorEfficiency efficiency={validator.efficiency} />
      </DataTableCell>
      <DataTableCell align="right">
        <Tooltip
          content={`${BigInt(validator.weight).toLocaleString()} of ${BigInt(totalWeight).toLocaleString()}`}
        >
          <span className={styles.tabular}>
            <Percentage
              maximumFractionDigits={3}
              minimumFractionDigits={2}
              total={1_000_000}
              value={weightParts}
            />
          </span>
        </Tooltip>
      </DataTableCell>
      <DataTableCell align="right">
        <GramAmount value={node?.validator_stake_nano} />
      </DataTableCell>
      <DataTableCell>
        <span className={styles.tabular}>{node?.ton_release || "—"}</span>
      </DataTableCell>
      <DataTableCell>
        <AddressChip
          address={node?.validator_wallet_address ?? undefined}
          fallback="—"
          onAddressClick={onAddressClick}
        />
      </DataTableCell>
      <DataTableCell tone="muted">{node?.validator_wallet_version ?? "—"}</DataTableCell>
    </DataTableRow>
  )
}

function ValidatorEfficiency({
  efficiency,
}: {
  readonly efficiency: ValidatorEfficiencyObservation | null
}) {
  if (!efficiency) return <span className={styles.muted}>Collecting</span>

  const masterchainExpected = Number(efficiency.masterchain_blocks_expected)
  const usesMasterchain = masterchainExpected > 0
  const created = usesMasterchain
    ? efficiency.masterchain_blocks_created
    : efficiency.shard_blocks_created
  const expected = usesMasterchain
    ? efficiency.masterchain_blocks_expected
    : efficiency.shard_blocks_expected
  const chain = usesMasterchain ? "masterchain" : "shard"
  const percent = Number(efficiency.percent)
  const tone = percent >= 90 ? "good" : percent >= 60 ? "warning" : "bad"

  return (
    <Tooltip content={`${created.toLocaleString()} of ${expected} expected ${chain} blocks`}>
      <span className={styles.validatorEfficiency} data-tone={tone}>
        <Percentage maximumFractionDigits={2} minimumFractionDigits={2} value={percent} />
      </span>
    </Tooltip>
  )
}

/** Finds the host report that owns a key from the on-chain validator set */
function nodeForValidator(
  nodes: readonly NodeView[],
  validator: ValidatorObservation,
): NodeView | undefined {
  const publicKey = validator.public_key.toLowerCase()

  return nodes.find(
    node =>
      node.validator_public_key?.toLowerCase() === publicKey ||
      node.validator_public_keys.some(key => key.toLowerCase() === publicKey),
  )
}

function ShardsSection({
  shards,
  now,
  showTitle,
}: {
  readonly shards: readonly ShardHead[]
  readonly now: number
  readonly showTitle: boolean
}) {
  return (
    <section
      id="shards"
      className={styles.sectionStack}
      aria-label={showTitle ? undefined : "Shard topology"}
      aria-labelledby={showTitle ? "shards-title" : undefined}
    >
      {showTitle ? (
        <div className={styles.sectionHeading}>
          <h2 id="shards-title">Shard topology</h2>
        </div>
      ) : null}
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
    </section>
  )
}

function ObserverDiagnostics({
  network,
  now,
}: {
  readonly network: NetworkView
  readonly now: number
}) {
  return (
    <section className={styles.sectionStack} aria-label="Collector diagnostics">
      <div className={styles.sectionHeading}>
        <h2>Collector diagnostics</h2>
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
  )
}
