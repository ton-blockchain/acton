import {
  Button,
  CopyInlineAction,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  InlineActions,
  InlineButton,
  useToast,
} from "@acton/ui"
import {Boxes, CircleAlert, Plus, RotateCcw, Square} from "lucide-react"
import {useCallback, useEffect, useState} from "react"

import {
  type EnvironmentStatus,
  type StudioEnvironment,
  fetchStudioEnvironments,
  restartStudioEnvironment,
  stopStudioEnvironment,
} from "../studioApi"
import {CreateEnvironmentDialog} from "./CreateEnvironmentDialog"
import {StopEnvironmentDialog} from "./StopEnvironmentDialog"

import styles from "./VirtualEnvironmentsPage.module.css"

const ENVIRONMENT_POLL_INTERVAL_MS = 1500

interface VirtualEnvironmentsPageProps {
  readonly createOpen: boolean
  readonly walletNames: readonly string[]
  readonly onCreateOpenChange: (open: boolean) => void
}

const statusLabels = {
  starting: "Starting",
  running: "Running",
  stopping: "Stopping",
  stopped: "Stopped",
  failed: "Failed",
} satisfies Record<EnvironmentStatus, string>

export function VirtualEnvironmentsPage({
  createOpen,
  walletNames,
  onCreateOpenChange,
}: VirtualEnvironmentsPageProps) {
  const {showToast} = useToast()
  const [environments, setEnvironments] = useState<StudioEnvironment[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [loadError, setLoadError] = useState<string>()
  const [stoppingIds, setStoppingIds] = useState<ReadonlySet<string>>(new Set())
  const [restartingIds, setRestartingIds] = useState<ReadonlySet<string>>(new Set())
  const [stopTarget, setStopTarget] = useState<StudioEnvironment>()

  const refresh = useCallback(async (signal?: AbortSignal) => {
    const nextEnvironments = await fetchStudioEnvironments(signal)
    setEnvironments(nextEnvironments)
    setLoadError(undefined)
    setIsLoading(false)
  }, [])

  useEffect(() => {
    const controller = new AbortController()
    let pollTimer: ReturnType<typeof globalThis.setTimeout> | undefined

    const poll = async () => {
      try {
        await refresh(controller.signal)
      } catch (error) {
        if (controller.signal.aborted) return
        setLoadError(getErrorMessage(error))
        setIsLoading(false)
      }
      if (!controller.signal.aborted) {
        pollTimer = globalThis.setTimeout(() => void poll(), ENVIRONMENT_POLL_INTERVAL_MS)
      }
    }

    void poll()
    return () => {
      controller.abort()
      if (pollTimer !== undefined) globalThis.clearTimeout(pollTimer)
    }
  }, [refresh])

  const handleCreated = (environment: StudioEnvironment) => {
    setEnvironments(current => [
      environment,
      ...current.filter(candidate => candidate.id !== environment.id),
    ])
  }

  const handleStop = async () => {
    const environment = stopTarget
    if (!environment) return

    setStoppingIds(current => new Set(current).add(environment.id))
    try {
      const stopped = await stopStudioEnvironment(environment.id)
      setEnvironments(current =>
        current.map(candidate => (candidate.id === stopped.id ? stopped : candidate)),
      )
      showToast({
        title: `${environment.name} stopped`,
        variant: "success",
      })
      setStopTarget(undefined)
    } catch (error) {
      showToast({
        title: `Failed to stop ${environment.name}`,
        description: getErrorMessage(error),
        variant: "error",
      })
    } finally {
      setStoppingIds(current => {
        const next = new Set(current)
        next.delete(environment.id)
        return next
      })
    }
  }

  const handleRestart = async (environment: StudioEnvironment) => {
    setRestartingIds(current => new Set(current).add(environment.id))
    try {
      const restarted = await restartStudioEnvironment(environment.id)
      setEnvironments(current =>
        current.map(candidate => (candidate.id === restarted.id ? restarted : candidate)),
      )
      showToast({
        title: `${environment.name} is restarting`,
        variant: "success",
      })
    } catch (error) {
      showToast({
        title: `Failed to restart ${environment.name}`,
        description: getErrorMessage(error),
        variant: "error",
      })
    } finally {
      setRestartingIds(current => {
        const next = new Set(current)
        next.delete(environment.id)
        return next
      })
    }
  }

  return (
    <div className={styles.page}>
      {loadError && environments.length === 0 ? (
        <section className={styles.errorPanel} aria-live="polite">
          <CircleAlert size={18} aria-hidden="true" />
          <div>
            <strong>Unable to load environments</strong>
            <span>{loadError}</span>
          </div>
          <Button size="sm" variant="outline" onClick={() => void refresh()}>
            Retry
          </Button>
        </section>
      ) : (
        <DataTable minWidth="50rem">
          <DataTableTable aria-label="Virtual environments">
            <DataTableHead>
              <DataTableRow>
                <DataTableHeaderCell columnWidth="28%">Name</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="14%">Status</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="15%">Network</DataTableHeaderCell>
                <DataTableHeaderCell>RPC endpoint</DataTableHeaderCell>
                <DataTableHeaderCell align="right" columnWidth="9rem">
                  Actions
                </DataTableHeaderCell>
              </DataTableRow>
            </DataTableHead>
            <DataTableBody>
              {isLoading ? (
                <DataTableSkeletonRows
                  columns={5}
                  rows={3}
                  widths={["58%", "44%", "48%", "74%", "60%"]}
                  alignments={["left", "left", "left", "left", "right"]}
                />
              ) : environments.length === 0 ? (
                <DataTableEmpty colSpan={5}>
                  <div className={styles.emptyState}>
                    <span className={styles.emptyIcon}>
                      <Boxes size={21} aria-hidden="true" />
                    </span>
                    <strong>No virtual environments</strong>
                    <span>Create an isolated TON network for this workspace</span>
                    <Button
                      size="sm"
                      variant="primary"
                      leadingIcon={<Plus size={15} aria-hidden="true" />}
                      onClick={() => onCreateOpenChange(true)}
                    >
                      Create environment
                    </Button>
                  </div>
                </DataTableEmpty>
              ) : (
                environments.map(environment => {
                  const canStop =
                    environment.status === "starting" || environment.status === "running"
                  const isStopping =
                    stoppingIds.has(environment.id) || environment.status === "stopping"
                  const canRestart =
                    environment.status === "stopped" || environment.status === "failed"
                  const isRestarting = restartingIds.has(environment.id)

                  return (
                    <DataTableRow key={environment.id} hover>
                      <DataTableCell>
                        <div className={styles.environmentName}>
                          <strong>{environment.name}</strong>
                          {environment.error ? (
                            <span title={environment.error}>{environment.error}</span>
                          ) : null}
                        </div>
                      </DataTableCell>
                      <DataTableCell>
                        <EnvironmentStatusLabel status={environment.status} />
                      </DataTableCell>
                      <DataTableCell tone="muted">
                        {formatNetwork(environment.config.forkNetwork)}
                      </DataTableCell>
                      <DataTableCell>
                        <InlineActions
                          visibility="always"
                          actions={
                            <CopyInlineAction
                              value={environment.rpcUrl}
                              label="Copy RPC endpoint"
                              copiedLabel="RPC endpoint copied"
                            />
                          }
                        >
                          <span className={styles.rpcUrl}>{environment.rpcUrl}</span>
                        </InlineActions>
                      </DataTableCell>
                      <DataTableCell align="right">
                        {canRestart ? (
                          <InlineButton
                            leadingIcon={<RotateCcw size={14} aria-hidden="true" />}
                            disabled={isRestarting}
                            onClick={() => void handleRestart(environment)}
                          >
                            Restart
                          </InlineButton>
                        ) : (
                          <InlineButton
                            variant="danger"
                            leadingIcon={<Square size={13} aria-hidden="true" />}
                            disabled={!canStop}
                            aria-busy={isStopping || undefined}
                            onClick={() => setStopTarget(environment)}
                          >
                            Stop
                          </InlineButton>
                        )}
                      </DataTableCell>
                    </DataTableRow>
                  )
                })
              )}
            </DataTableBody>
          </DataTableTable>
        </DataTable>
      )}

      <CreateEnvironmentDialog
        environmentCount={environments.length}
        open={createOpen}
        walletNames={walletNames}
        onCreated={handleCreated}
        onOpenChange={onCreateOpenChange}
      />
      <StopEnvironmentDialog
        environment={stopTarget}
        loading={stopTarget ? stoppingIds.has(stopTarget.id) : false}
        onConfirm={() => void handleStop()}
        onOpenChange={() => setStopTarget(undefined)}
      />
    </div>
  )
}

function EnvironmentStatusLabel({status}: {readonly status: EnvironmentStatus}) {
  return (
    <span className={styles.status} data-status={status}>
      <span className={styles.statusDot} aria-hidden="true" />
      {statusLabels[status]}
    </span>
  )
}

function formatNetwork(forkNetwork: string | undefined) {
  if (!forkNetwork) return "Clean state"
  return `${forkNetwork.charAt(0).toUpperCase()}${forkNetwork.slice(1)} fork`
}

function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
