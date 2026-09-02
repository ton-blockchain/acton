import {lazy, Suspense, useState} from "react"
import {
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
  TechnicalValue,
  Tooltip,
} from "@acton/ui"

import type {InitialSyncProgress, NodeView} from "../types"
import {StatusPill} from "./StatusPill"
import styles from "./NodesSection.module.css"

const NetworkMap = lazy(() => import("./NetworkMap"))

interface NodesSectionProps {
  readonly nodes: readonly NodeView[]
  readonly now: number
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

/** Owns node synchronization, role, production, and observer columns for the network view */
export function NodesSection({nodes, now}: NodesSectionProps) {
  const [locationsOpen, setLocationsOpen] = useState(false)
  const locatedNodes = nodes.filter(node => node.location.kind === "country").length

  return (
    <section id="nodes" className={styles.sectionStack} aria-labelledby="nodes-title">
      <div className={styles.sectionHeading}>
        <h2 id="nodes-title">Nodes and synchronization</h2>
      </div>
      <NodesTable nodes={nodes} now={now} />
      <Disclosure
        className={styles.locationDisclosure}
        label={
          <span className={styles.locationDisclosureLabel}>
            <span>Node locations by public IP</span>
            <span>{locatedNodes.toLocaleString()} located</span>
          </span>
        }
        contentClassName={styles.locationDisclosureContent}
        onToggle={event => setLocationsOpen(event.currentTarget.open)}
      >
        {locationsOpen ? (
          <Suspense
            fallback={
              <div className={styles.mapLoading}>
                <InlineLoader message="Loading node locations" />
              </div>
            }
          >
            <NetworkMap nodes={nodes} />
          </Suspense>
        ) : null}
      </Disclosure>
    </section>
  )
}

function SyncState({state}: {readonly state: NodeView["sync_status"]}) {
  return (
    <span className={styles.syncState} data-state={state}>
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
      <div className={styles.syncProgress} data-state={node.sync_status}>
        <SyncState state={node.sync_status} />
        <span className={styles.syncProgressPending}>Waiting for synchronization data</span>
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
      className={styles.syncProgress}
      data-state={node.sync_status}
      title={progressAge === undefined ? undefined : `Last progress ${progressAge}s ago`}
    >
      <div className={styles.syncProgressHeading}>
        <SyncState state={node.sync_status} />
        {percent === undefined ? null : (
          <span className={styles.syncProgressPercent}>
            <Percentage value={percent} maximumFractionDigits={1} />
          </span>
        )}
      </div>
      {percent === undefined ? null : (
        <div
          className={styles.syncProgressTrack}
          role="progressbar"
          aria-label={`${node.name} synchronization`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Number(percent.toFixed(1))}
        >
          <span style={{width: `${percent}%`}} />
        </div>
      )}
      <div className={styles.syncProgressDetail}>
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
        <div className={styles.syncProgressDetail}>
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
        <span className={styles.syncProgressSample}>
          No progress for <Duration value={progressAge} display="elapsed" tooltip={false} />
        </span>
      ) : null}
    </div>
  )
}

function NodeRoleBadge({role}: {readonly role: NodeRole}) {
  const presentation = NODE_ROLE_PRESENTATION[role]
  const tooltip = `${presentation.label} — ${presentation.description}`

  return (
    <Tooltip content={tooltip} delay={0} width="wide">
      <span className={styles.roleBadge} data-role={role} aria-label={tooltip}>
        {presentation.letter}
      </span>
    </Tooltip>
  )
}

function NodesTable({nodes, now}: NodesSectionProps) {
  return (
    <DataTable className={styles.nodesTable} minWidth="60rem">
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
                  <div className={styles.nodeName}>
                    <strong>{node.name}</strong>
                    <span>{node.public_ip}</span>
                  </div>
                </DataTableCell>
                <DataTableCell>
                  <SynchronizationProgress node={node} now={now} />
                </DataTableCell>
                <DataTableCell>
                  <div className={styles.roleList} aria-label={`${node.name} roles`}>
                    {node.roles
                      .filter(role => role !== "validator" || node.active_validator)
                      .map(role => (
                        <NodeRoleBadge role={role} key={role} />
                      ))}
                  </div>
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
