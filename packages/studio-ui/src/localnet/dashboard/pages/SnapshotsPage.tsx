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
import {Archive, Download, RotateCcw, Trash2, TriangleAlert, Upload} from "lucide-react"
import {useCallback, useEffect, useMemo, useRef, useState} from "react"
import type {FC, ReactNode} from "react"

import {EnvironmentStartupProgress} from "../../../components/EnvironmentStartupProgress"
import {
  createStudioEnvironmentSnapshot,
  deleteStudioEnvironmentSnapshot,
  downloadStudioEnvironmentSnapshot,
  fetchStudioEnvironmentSnapshotOperation,
  fetchStudioEnvironmentSnapshots,
  importStudioEnvironmentSnapshot,
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
  const simulated = environment.config.kind === "actonSimulatedLocalnet"
  const fileInput = useRef<HTMLInputElement>(null)
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
  const operationToast = useRef<string | undefined>(undefined)
  const mutationRevision = useRef(0)
  const refreshList = useRef(true)
  const listErrorShown = useRef(false)
  const pollErrorShown = useRef(false)
  const failedOperationShown = useRef<string | undefined>(undefined)

  const active =
    operation !== null && operation.phase !== "completed" && operation.phase !== "failed"

  const loadSnapshots = useCallback(
    async (signal?: AbortSignal) => {
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
        if (!signal?.aborted) setLoading(false)
      }
    },
    [environment.id, showToast],
  )

  useEffect(() => {
    const controller = new AbortController()
    let polling = false

    // Progress discovery must work independently of inventory errors, including
    // after navigation or reload. Guard the interval so slow requests never overlap.
    const poll = async () => {
      if (polling) return

      polling = true
      try {
        const revision = mutationRevision.current
        try {
          const next = await fetchStudioEnvironmentSnapshotOperation(
            environment.id,
            controller.signal,
          )
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
      } finally {
        polling = false
      }
    }

    void poll()
    const timer = setInterval(() => void poll(), active ? 1000 : 3000)

    return () => {
      controller.abort()
      clearInterval(timer)
    }
  }, [active, environment.id, loadSnapshots, showToast])

  const importSnapshot = useCallback(
    async (file: File) => {
      if (active || submitting || !operationLoaded) return

      setSubmitting(true)
      mutationRevision.current += 1

      try {
        await importStudioEnvironmentSnapshot(environment.id, file)
        refreshList.current = true
        await loadSnapshots()
        showToast({
          variant: "success",
          title: "Snapshot imported",
          description: "The snapshot is saved and available to restore",
        })
      } catch (error) {
        showToast({
          variant: "error",
          title: "Snapshot not imported",
          description: errorMessage(error, "The snapshot could not be imported"),
        })
      } finally {
        setSubmitting(false)
      }
    },
    [active, environment.id, loadSnapshots, operationLoaded, showToast, submitting],
  )

  const downloadSnapshot = async (snapshot: EnvironmentSnapshot) => {
    try {
      const blob = await downloadStudioEnvironmentSnapshot(environment.id, snapshot.id)
      const url = URL.createObjectURL(blob)
      const link = document.createElement("a")
      link.href = url
      link.download = `${snapshot.id}.json`
      link.click()
      URL.revokeObjectURL(url)
    } catch (error) {
      showToast({
        variant: "error",
        title: "Snapshot not downloaded",
        description: errorMessage(error, "The snapshot could not be downloaded"),
      })
    }
  }

  const actions = useMemo(
    () => (
      <>
        {simulated ? (
          <Button
            variant="outline"
            size="sm"
            leadingIcon={<Upload size={16} />}
            disabled={!operationLoaded || active || submitting}
            onClick={() => fileInput.current?.click()}
          >
            Import snapshot
          </Button>
        ) : undefined}
        <Button
          variant="primary"
          size="sm"
          leadingIcon={<Archive size={16} />}
          disabled={!operationLoaded || active || submitting}
          onClick={() => {
            setSnapshotName(nextSnapshotName(snapshots))
            setDialog({kind: "create"})
          }}
        >
          Create snapshot
        </Button>
      </>
    ),
    [active, operationLoaded, simulated, snapshots, submitting],
  )

  useEffect(() => {
    onActionsChange(actions)
    return () => onActionsChange(undefined)
  }, [actions, onActionsChange])

  useEffect(
    () => () => {
      if (operationToast.current) {
        updateToast(operationToast.current, {
          variant: "info",
          title: "Snapshot operation continues in the background",
          durationMs: 4000,
        })
        operationToast.current = undefined
      }
    },
    [updateToast],
  )

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
      void loadSnapshots().then(() => {
        const message = {
          variant: "success" as const,
          title: operation.kind === "create" ? "Snapshot created" : "Snapshot restored",
          durationMs: 4000,
        }

        if (operationToast.current) {
          updateToast(operationToast.current, message)
          operationToast.current = undefined
        } else {
          showToast(message)
        }
      })
    }
    if (operation?.phase === "failed" && failedOperationShown.current !== operation.startedAt) {
      failedOperationShown.current = operation.startedAt
      refreshList.current = true
      const message = {
        variant: "error",
        title: operation.kind === "create" ? "Snapshot not created" : "Snapshot not restored",
        description: operation.error ?? "The snapshot operation failed",
        durationMs: 8000,
      } as const

      if (operationToast.current) {
        updateToast(operationToast.current, message)
        operationToast.current = undefined
      } else {
        showToast(message)
      }
    }
    operationWasActive.current = active
  }, [active, loadSnapshots, operation, showToast, updateToast])

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
          ? "Creating snapshot"
          : dialog.kind === "restore"
            ? "Restoring snapshot"
            : "Deleting snapshot",
      description: snapshot ? snapshotLabel(snapshot) : snapshotName.trim() || environment.name,
      durationMs: 0,
    })
    setSubmitting(true)
    try {
      if (dialog.kind === "create") {
        const name = snapshotName.trim() || nextSnapshotName(snapshots)
        setOperation(await createStudioEnvironmentSnapshot(environment.id, name))
        setSnapshotName("")
        operationWasActive.current = true
        operationToast.current = toastId
      } else if (dialog.kind === "restore") {
        setOperation(await restoreStudioEnvironmentSnapshot(environment.id, dialog.snapshot.id))
        operationWasActive.current = true
        operationToast.current = toastId
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
    snapshots,
    submitting,
    updateToast,
  ])

  return (
    <section className={`${pageStyles.settingsSection} ${styles.page}`}>
      {simulated ? (
        <input
          ref={fileInput}
          type="file"
          accept="application/json,.json"
          aria-label="Import snapshot file"
          hidden
          onChange={event => {
            const file = event.target.files?.[0]
            event.target.value = ""
            if (file) void importSnapshot(file)
          }}
        />
      ) : (
        <div className={`${pageStyles.settingsNotice} ${styles.notice}`}>
          <Archive size={17} aria-hidden="true" />
          <div>
            <strong>Snapshots can be large</strong>
            <span>
              Creation pauses all nodes and can take several minutes. Restore replaces the saved
              nodes and rebuilds the index before the environment is available again
            </span>
          </div>
        </div>
      )}

      <div className={styles.panel} aria-busy={loading}>
        {!simulated && active && operation ? (
          <OperationProgress operation={operation} now={now} />
        ) : undefined}

        <div className={styles.tableWrap}>
          <DataTableTable aria-label="Saved snapshots">
            <DataTableHead>
              <DataTableRow>
                <DataTableHeaderCell columnWidth="26%">Snapshot</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="24%">Created</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="10%">Block</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="14%">
                  {simulated ? "Size" : "Archive"}
                </DataTableHeaderCell>
                {simulated ? undefined : (
                  <DataTableHeaderCell columnWidth="14%">State</DataTableHeaderCell>
                )}
                <DataTableHeaderCell align="right" columnWidth="9rem">
                  Actions
                </DataTableHeaderCell>
              </DataTableRow>
            </DataTableHead>
            <DataTableBody>
              {loading ? (
                <DataTableSkeletonRows
                  alignments={
                    simulated
                      ? ["left", "left", "left", "left", "right"]
                      : ["left", "left", "left", "left", "left", "right"]
                  }
                  columns={simulated ? 5 : 6}
                  rowKeyPrefix="snapshot-row-skeleton"
                  rows={4}
                  widths={
                    simulated
                      ? ["14rem", "12rem", "5rem", "5rem", "7rem"]
                      : ["14rem", "12rem", "5rem", "5rem", "5rem", "7rem"]
                  }
                />
              ) : snapshots.length === 0 ? (
                <DataTableEmpty colSpan={simulated ? 5 : 6}>
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
                      <ByteSize value={snapshot.sizeBytes} />
                    </DataTableCell>
                    {simulated ? undefined : (
                      <DataTableCell tone="muted" mono>
                        <ByteSize value={snapshot.stateSizeBytes} />
                      </DataTableCell>
                    )}
                    <DataTableCell align="right">
                      <span className={styles.actions}>
                        {simulated ? (
                          <InlineAction
                            label={`Download ${snapshotLabel(snapshot)}`}
                            icon={<Download />}
                            onClick={() => void downloadSnapshot(snapshot)}
                          />
                        ) : undefined}
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
        simulated={simulated}
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
  readonly simulated: boolean
  readonly state: DialogState | undefined
  readonly name: string
  readonly loading: boolean
  readonly onNameChange: (value: string) => void
  readonly onClose: () => void
  readonly onConfirm: () => void
}

const SnapshotDialog: FC<SnapshotDialogProps> = ({
  simulated,
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
    state?.kind === "delete"
      ? "This permanently deletes the saved snapshot"
      : state?.kind === "restore"
        ? "Restore the network state from this snapshot"
        : simulated
          ? "Save accounts, transactions, pending messages, and virtual time"
          : "Save accounts, block history, and the chain state of all network nodes"

  return (
    <Dialog
      open={state !== undefined}
      onOpenChange={open => {
        if (!open) onClose()
      }}
      title={title}
      description={description}
      className={styles.dialog}
      busy={loading}
      maxWidth="30rem"
    >
      {!simulated && state?.kind !== "delete" ? (
        <div className={styles.warning} role="alert">
          <TriangleAlert size={18} aria-hidden="true" />
          <span>
            {state?.kind === "restore"
              ? "Studio will stop the environment, restore the saved state, rebuild the index, and start it again. This can take several minutes"
              : "Studio will pause the running environment while saving the snapshot, then start it again. This can take several minutes"}
          </span>
        </div>
      ) : undefined}
      {state?.kind === "create" ? (
        <Input
          label="Name"
          description="Optional name to identify this point in the chain"
          maxLength={80}
          value={name}
          onChange={event => onNameChange(event.target.value)}
        />
      ) : undefined}
      <DialogActions className={state?.kind === "create" ? styles.dialogActions : undefined}>
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
  return snapshot.name ?? "Snapshot"
}

function nextSnapshotName(snapshots: readonly EnvironmentSnapshot[]): string {
  const lastNumber = snapshots.reduce((latest, snapshot) => {
    const number = /^Snapshot (\d+)$/.exec(snapshot.name ?? "")?.[1]
    return Math.max(latest, number ? Number(number) : 0)
  }, 0)

  return `Snapshot ${lastNumber + 1}`
}

function operationDetail(phase: EnvironmentSnapshotOperationPhase): string {
  switch (phase) {
    case "preparing":
      return "Preparing the operation"
    case "stopping":
      return "Waiting for network processes to stop"
    case "creatingArchive":
      return "Compressing the persistent chain state"
    case "savingState":
      return "Saving the network state to JSON"
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
