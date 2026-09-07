import {
  Button,
  ByteSize,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  DateTime,
  Dialog,
  DialogActions,
  Duration,
  EmptyState,
  Input,
  InlineAction,
  NumberValue,
  useToast,
} from "@acton/ui"
import {Archive, RotateCcw, Trash2} from "lucide-react"
import {useCallback, useEffect, useMemo, useRef, useState} from "react"
import type {FC, ReactNode} from "react"

import {EnvironmentStartupProgress} from "../../../components/EnvironmentStartupProgress"
import {
  createStudioEnvironmentSnapshot,
  deleteStudioEnvironmentSnapshot,
  fetchStudioEnvironmentSnapshotOperation,
  fetchStudioEnvironmentSnapshots,
  restoreStudioEnvironmentSnapshot,
} from "../../../studioApi"
import type {
  EnvironmentSnapshot,
  EnvironmentSnapshotOperation,
  EnvironmentSnapshotOperationPhase,
  StudioEnvironment,
} from "../../../studioApi"

import pageStyles from "../DashboardPage.module.css"
import styles from "./SnapshotsPage.module.css"

interface SnapshotsPageProps {
  readonly environment: StudioEnvironment
  readonly onActionsChange: (actions: ReactNode) => void
}

type DialogState =
  | {readonly kind: "create"}
  | {readonly kind: "restore" | "delete"; readonly snapshot: EnvironmentSnapshot}

const CREATE_PHASES: readonly Phase[] = [
  {id: "preparing", label: "Prepare"},
  {id: "stopping", label: "Stop network"},
  {id: "creatingArchive", label: "Create archive"},
  {id: "starting", label: "Start network"},
]

const RESTORE_PHASES: readonly Phase[] = [
  {id: "preparing", label: "Prepare"},
  {id: "stopping", label: "Stop network"},
  {id: "restoringState", label: "Restore state"},
  {id: "resettingIndexer", label: "Rebuild index"},
  {id: "starting", label: "Start network"},
]

interface Phase {
  readonly id: EnvironmentSnapshotOperationPhase
  readonly label: string
}

export const SnapshotsPage: FC<SnapshotsPageProps> = ({environment, onActionsChange}) => {
  const {showToast, updateToast} = useToast()
  const [snapshots, setSnapshots] = useState<readonly EnvironmentSnapshot[]>([])
  const [operation, setOperation] = useState<EnvironmentSnapshotOperation | null>(null)
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState(false)
  const [operationLoaded, setOperationLoaded] = useState(false)
  const [dialog, setDialog] = useState<DialogState>()
  const [snapshotName, setSnapshotName] = useState("")
  const [submitting, setSubmitting] = useState(false)
  const [deletingId, setDeletingId] = useState<string>()
  const [now, setNow] = useState(Date.now())
  const operationWasActive = useRef(false)
  const mutationRevision = useRef(0)
  const refreshList = useRef(true)
  const listPending = useRef(false)
  const listErrorShown = useRef(false)
  const pollErrorShown = useRef(false)
  const failedOperationShown = useRef<string | undefined>(undefined)

  const active =
    operation !== null && operation.phase !== "completed" && operation.phase !== "failed"

  const loadSnapshots = useCallback(
    async (signal?: AbortSignal) => {
      if (listPending.current) return
      listPending.current = true
      const revision = mutationRevision.current
      try {
        const result = await fetchStudioEnvironmentSnapshots(environment.id, signal)
        if (signal?.aborted || revision !== mutationRevision.current) return
        setSnapshots(result)
        setLoadError(false)
        refreshList.current = false
        listErrorShown.current = false
      } catch (error) {
        if (signal?.aborted) return
        setLoadError(true)
        refreshList.current = true
        if (!listErrorShown.current) {
          showToast({
            variant: "error",
            title: "Failed to load snapshots",
            description: errorMessage(error, "The snapshot request failed"),
          })
          listErrorShown.current = true
        }
      } finally {
        listPending.current = false
        if (!signal?.aborted) setLoading(false)
      }
    },
    [environment.id, showToast],
  )

  useEffect(() => {
    const controller = new AbortController()
    let timer: ReturnType<typeof setTimeout>

    // Progress discovery must work independently of inventory errors, including
    // after navigation or reload. Schedule after each response to avoid overlap.
    const poll = async () => {
      const revision = mutationRevision.current
      let next: EnvironmentSnapshotOperation | null = null
      try {
        next = await fetchStudioEnvironmentSnapshotOperation(environment.id, controller.signal)
        if (controller.signal.aborted) return
        if (revision === mutationRevision.current) {
          setOperation(next)
          setOperationLoaded(true)
          pollErrorShown.current = false
        }
      } catch (error) {
        if (controller.signal.aborted) return
        setOperationLoaded(false)
        if (!pollErrorShown.current) {
          showToast({
            variant: "error",
            title: "Failed to load snapshot progress",
            description: errorMessage(error, "The progress request failed"),
          })
          pollErrorShown.current = true
        }
      }
      if (refreshList.current) void loadSnapshots(controller.signal)
      if (!controller.signal.aborted) {
        timer = setTimeout(
          () => void poll(),
          next && next.phase !== "completed" && next.phase !== "failed" ? 1000 : 3000,
        )
      }
    }

    void loadSnapshots(controller.signal)
    void poll()
    return () => {
      controller.abort()
      clearTimeout(timer)
    }
  }, [environment.id, loadSnapshots, showToast])

  const actions = useMemo(
    () => (
      <Button
        variant="outline"
        size="sm"
        leadingIcon={<Archive size={16} />}
        disabled={!operationLoaded || active || submitting}
        onClick={() => setDialog({kind: "create"})}
      >
        Create snapshot
      </Button>
    ),
    [active, operationLoaded, submitting],
  )

  useEffect(() => {
    onActionsChange(actions)
    return () => onActionsChange(undefined)
  }, [actions, onActionsChange])

  useEffect(() => {
    if (!active) return
    setNow(Date.now())
    const timer = globalThis.setInterval(() => setNow(Date.now()), 1000)
    return () => globalThis.clearInterval(timer)
  }, [active])

  useEffect(() => {
    if (operationWasActive.current && operation?.phase === "completed") {
      mutationRevision.current += 1
      refreshList.current = true
      void loadSnapshots()
      showToast({
        variant: "success",
        title: operation.kind === "create" ? "Snapshot created" : "Snapshot restored",
        description:
          operation.kind === "create"
            ? "The archive is ready"
            : "The environment is available again",
      })
    }
    if (operation?.phase === "failed" && failedOperationShown.current !== operation.startedAt) {
      failedOperationShown.current = operation.startedAt
      refreshList.current = true
      showToast({
        variant: "error",
        title: operation.kind === "create" ? "Snapshot not created" : "Snapshot not restored",
        description: operation.error ?? "The snapshot operation failed",
      })
    }
    operationWasActive.current = active
  }, [active, loadSnapshots, operation, showToast])

  const submitDialog = useCallback(async () => {
    if (!dialog || submitting || active || !operationLoaded) return

    // A GET sent before this mutation must not replace its accepted operation.
    mutationRevision.current += 1
    refreshList.current = true

    const snapshot = dialog.kind === "create" ? undefined : dialog.snapshot
    const toastId = showToast({
      variant: "loading",
      title:
        dialog.kind === "create"
          ? "Starting snapshot creation"
          : dialog.kind === "restore"
            ? "Starting snapshot restore"
            : "Deleting snapshot",
      description: snapshot ? snapshotLabel(snapshot) : snapshotName.trim() || environment.name,
      durationMs: 0,
    })
    setSubmitting(true)
    try {
      if (dialog.kind === "create") {
        const name = snapshotName.trim()
        setOperation(await createStudioEnvironmentSnapshot(environment.id, name || undefined))
        setSnapshotName("")
        updateToast(toastId, {
          variant: "info",
          title: "Snapshot creation started",
          description: "Progress is available on this page",
          durationMs: 4000,
        })
      } else if (dialog.kind === "restore") {
        setOperation(await restoreStudioEnvironmentSnapshot(environment.id, dialog.snapshot.id))
        updateToast(toastId, {
          variant: "info",
          title: "Snapshot restore started",
          description: "Progress is available on this page",
          durationMs: 4000,
        })
      } else {
        setDeletingId(dialog.snapshot.id)
        await deleteStudioEnvironmentSnapshot(environment.id, dialog.snapshot.id)
        await loadSnapshots()
        updateToast(toastId, {
          variant: "success",
          title: "Snapshot deleted",
          description: `${snapshotLabel(dialog.snapshot)} was removed`,
          durationMs: 4000,
        })
      }
      setDialog(undefined)
    } catch (error) {
      updateToast(toastId, {
        variant: "error",
        title:
          dialog.kind === "create"
            ? "Snapshot not started"
            : dialog.kind === "restore"
              ? "Restore not started"
              : "Snapshot not deleted",
        description: errorMessage(error, "The snapshot request failed"),
        durationMs: 8000,
      })
    } finally {
      mutationRevision.current += 1
      setDeletingId(undefined)
      setSubmitting(false)
    }
  }, [
    active,
    operationLoaded,
    dialog,
    environment.id,
    environment.name,
    loadSnapshots,
    showToast,
    snapshotName,
    submitting,
    updateToast,
  ])

  return (
    <section className={`${pageStyles.settingsSection} ${styles.page}`}>
      <div className={`${pageStyles.settingsNotice} ${styles.notice}`}>
        <Archive size={17} aria-hidden="true" />
        <div>
          <strong>Snapshots can be large</strong>
          <span>
            Creation pauses all nodes and can take several minutes. Restore replaces the saved nodes
            and rebuilds the index before the environment is available again
          </span>
        </div>
      </div>

      <div className={styles.panel} aria-busy={loading}>
        {active && operation ? <OperationProgress operation={operation} now={now} /> : undefined}

        <div className={styles.tableWrap}>
          <DataTableTable aria-label="Saved snapshots">
            <DataTableHead>
              <DataTableRow>
                <DataTableHeaderCell columnWidth="26%">Snapshot</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="24%">Created</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="10%">Block</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="14%">Archive</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="14%">State</DataTableHeaderCell>
                <DataTableHeaderCell align="right" columnWidth="9rem">
                  Actions
                </DataTableHeaderCell>
              </DataTableRow>
            </DataTableHead>
            <DataTableBody>
              {loading ? (
                <DataTableSkeletonRows
                  alignments={["left", "left", "left", "left", "left", "right"]}
                  columns={6}
                  rowKeyPrefix="snapshot-row-skeleton"
                  rows={4}
                  widths={["14rem", "12rem", "5rem", "5rem", "5rem", "7rem"]}
                />
              ) : snapshots.length === 0 ? (
                <DataTableEmpty colSpan={6}>
                  <EmptyState
                    icon={<Archive size={20} aria-hidden="true" />}
                    title={loadError ? "Saved snapshots" : "No snapshots yet"}
                    description={
                      loadError
                        ? undefined
                        : "Create a snapshot to restore this environment to a known state later"
                    }
                    action={
                      loadError ? (
                        <Button size="sm" variant="outline" onClick={() => void loadSnapshots()}>
                          Retry
                        </Button>
                      ) : undefined
                    }
                  />
                </DataTableEmpty>
              ) : (
                snapshots.map(snapshot => (
                  <DataTableRow key={snapshot.id}>
                    <DataTableCell>
                      <strong className={styles.snapshotName}>{snapshotLabel(snapshot)}</strong>
                    </DataTableCell>
                    <DataTableCell tone="muted">
                      <DateTime value={snapshot.createdAt} unit="seconds" />
                    </DataTableCell>
                    <DataTableCell tone="muted" mono>
                      <NumberValue value={snapshot.masterchainSeqno} />
                    </DataTableCell>
                    <DataTableCell tone="muted" mono>
                      <ByteSize value={snapshot.archiveSizeBytes} />
                    </DataTableCell>
                    <DataTableCell tone="muted" mono>
                      <ByteSize value={snapshot.stateSizeBytes} />
                    </DataTableCell>
                    <DataTableCell align="right">
                      <span className={styles.actions}>
                        <InlineAction
                          label="Restore snapshot"
                          icon={<RotateCcw />}
                          disabled={
                            !operationLoaded || active || submitting || deletingId !== undefined
                          }
                          onClick={() => setDialog({kind: "restore", snapshot})}
                        />
                        <InlineAction
                          label={`Delete ${snapshotLabel(snapshot)}`}
                          icon={<Trash2 />}
                          disabled={
                            !operationLoaded || active || submitting || deletingId !== undefined
                          }
                          onClick={() => setDialog({kind: "delete", snapshot})}
                        />
                      </span>
                    </DataTableCell>
                  </DataTableRow>
                ))
              )}
            </DataTableBody>
          </DataTableTable>
        </div>
      </div>

      <SnapshotDialog
        state={dialog}
        name={snapshotName}
        loading={submitting}
        onNameChange={setSnapshotName}
        onClose={() => setDialog(undefined)}
        onConfirm={() => void submitDialog()}
      />
    </section>
  )
}

const OperationProgress: FC<{
  readonly now: number
  readonly operation: EnvironmentSnapshotOperation
}> = ({now, operation}) => {
  const phases = operation.kind === "create" ? CREATE_PHASES : RESTORE_PHASES
  const startupTimings = operation.startupTimings
  const currentIndex = Math.max(
    0,
    phases.findIndex(phase => phase.id === operation.phase),
  )
  return (
    <div className={styles.operation} role="status" aria-live="polite">
      <div className={styles.operationHeader}>
        <div>
          <strong>
            {operation.kind === "create" ? "Creating snapshot" : "Restoring snapshot"}
          </strong>
          <span>{operationDetail(operation.phase)}</span>
        </div>
        <span className={styles.elapsed}>
          <Duration
            display="elapsed"
            value={Math.max(0, Math.floor((now - Date.parse(operation.startedAt)) / 1000))}
          />
        </span>
      </div>
      <ol
        className={styles.phases}
        style={{gridTemplateColumns: `repeat(${phases.length}, minmax(0, 1fr))`}}
      >
        {phases.map((phase, index) => (
          <li
            key={phase.id}
            className={
              index < currentIndex
                ? styles.phaseComplete
                : index === currentIndex
                  ? styles.phaseCurrent
                  : undefined
            }
          >
            <span className={styles.phaseDot} aria-hidden="true" />
            <span>{phase.label}</span>
          </li>
        ))}
      </ol>
      {operation.phase === "starting" && startupTimings ? (
        <EnvironmentStartupProgress timings={startupTimings} />
      ) : undefined}
      <p>You can leave this page. Studio continues the operation in the background</p>
    </div>
  )
}

interface SnapshotDialogProps {
  readonly state: DialogState | undefined
  readonly name: string
  readonly loading: boolean
  readonly onNameChange: (value: string) => void
  readonly onClose: () => void
  readonly onConfirm: () => void
}

const SnapshotDialog: FC<SnapshotDialogProps> = ({
  state,
  name,
  loading,
  onNameChange,
  onClose,
  onConfirm,
}) => {
  const title = state
    ? state.kind === "create"
      ? "Create snapshot"
      : state.kind === "restore"
        ? `Restore ${snapshotLabel(state.snapshot)}`
        : `Delete ${snapshotLabel(state.snapshot)}`
    : "Snapshot"
  const description =
    state?.kind === "restore"
      ? "Studio will stop the environment, replace its chain state, rebuild the index, and start the environment again. This can take several minutes"
      : state?.kind === "delete"
        ? "This permanently deletes the snapshot archive"
        : "If the environment is running, Studio will stop it, create a compressed archive, and start it again. This can take several minutes"

  return (
    <Dialog
      open={state !== undefined}
      onOpenChange={open => {
        if (!open) onClose()
      }}
      title={title}
      description={description}
      busy={loading}
      maxWidth="30rem"
    >
      {state?.kind === "create" ? (
        <Input
          label="Name"
          description="Optional name to identify this point in the chain"
          maxLength={80}
          value={name}
          onChange={event => onNameChange(event.target.value)}
        />
      ) : undefined}
      <DialogActions className={styles.dialogActions}>
        <Button variant="secondary" onClick={onClose}>
          {loading ? "Close" : "Cancel"}
        </Button>
        <Button
          variant={state?.kind === "delete" ? "danger" : "primary"}
          loading={loading}
          onClick={onConfirm}
        >
          {state?.kind === "restore"
            ? "Restore snapshot"
            : state?.kind === "delete"
              ? "Delete snapshot"
              : "Create snapshot"}
        </Button>
      </DialogActions>
    </Dialog>
  )
}

function snapshotLabel(snapshot: EnvironmentSnapshot): string {
  return snapshot.name ?? snapshot.id
}

function operationDetail(phase: EnvironmentSnapshotOperationPhase): string {
  switch (phase) {
    case "preparing":
      return "Preparing the operation"
    case "stopping":
      return "Waiting for network processes to stop"
    case "creatingArchive":
      return "Compressing the persistent chain state"
    case "restoringState":
      return "Replacing the persistent chain state"
    case "resettingIndexer":
      return "Removing derived data so the index can be rebuilt"
    case "starting":
      return "Waiting for the network and APIs to become ready"
    case "completed":
      return "Operation complete"
    case "failed":
      return "Operation failed"
    default:
      return "Working"
  }
}

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message.trim() ? error.message : fallback
}
