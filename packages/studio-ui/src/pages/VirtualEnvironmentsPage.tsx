import {
  Button,
  CopyInlineAction,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  EmptyState,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  InlineActions,
  InlineButton,
  useToast,
} from "@acton/ui"
import {Boxes, Plus, RotateCcw, Square} from "lucide-react"
import {useState} from "react"

import {TablePage} from "../components/TablePage"
import {EnvironmentInfoValue} from "../components/EnvironmentInfoValue"
import {environmentStatusLabels} from "../environmentPresentation"
import {
  type EnvironmentStatus,
  type StudioEnvironment,
  restartStudioEnvironment,
  stopStudioEnvironment,
} from "../studioApi"
import {CreateEnvironmentDialog} from "./CreateEnvironmentDialog"
import {StopEnvironmentDialog} from "./StopEnvironmentDialog"

import styles from "./VirtualEnvironmentsPage.module.css"

interface VirtualEnvironmentsPageProps {
  readonly createOpen: boolean
  readonly environments: readonly StudioEnvironment[]
  readonly importSourceEnvironments: readonly StudioEnvironment[]
  readonly isLoading: boolean
  readonly loadError?: string
  readonly walletNames: readonly string[]
  readonly onCreateOpenChange: (open: boolean) => void
  readonly onEnvironmentChange: (environment: StudioEnvironment) => void
  readonly onOpenEnvironment: (environment: StudioEnvironment) => void
  readonly onRefresh: () => Promise<void>
}

export function VirtualEnvironmentsPage({
  createOpen,
  environments,
  importSourceEnvironments,
  isLoading,
  loadError,
  walletNames,
  onCreateOpenChange,
  onEnvironmentChange,
  onOpenEnvironment,
  onRefresh,
}: VirtualEnvironmentsPageProps) {
  const {showToast, updateToast} = useToast()
  const [stoppingIds, setStoppingIds] = useState<ReadonlySet<string>>(new Set())
  const [restartingIds, setRestartingIds] = useState<ReadonlySet<string>>(new Set())
  const [stopTarget, setStopTarget] = useState<StudioEnvironment>()

  const handleCreated = (environment: StudioEnvironment) => {
    onEnvironmentChange(environment)
  }

  const handleStop = async () => {
    const environment = stopTarget
    if (!environment || stoppingIds.has(environment.id)) return

    const toastId = showToast({
      title: `Stopping ${environment.name}`,
      variant: "loading",
      durationMs: 0,
    })
    setStoppingIds(current => new Set(current).add(environment.id))
    try {
      const stopped = await stopStudioEnvironment(environment.id)
      onEnvironmentChange(stopped)
      updateToast(toastId, {
        title: `${environment.name} stopped`,
        variant: "success",
        durationMs: 4000,
      })
      setStopTarget(undefined)
    } catch (error) {
      updateToast(toastId, {
        title: `Failed to stop ${environment.name}`,
        description: getErrorMessage(error),
        variant: "error",
        durationMs: 8000,
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
      onEnvironmentChange(restarted)
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
    <>
      <TablePage
        error={loadError}
        errorTitle="Unable to load environments"
        hasContent={environments.length > 0}
        onRetry={onRefresh}
      >
        <DataTable minWidth="62rem">
          <DataTableTable aria-label="Virtual environments">
            <DataTableHead>
              <DataTableRow>
                <DataTableHeaderCell columnWidth="22%">Name</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="12%">Status</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="17%">Type</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="14%">Network</DataTableHeaderCell>
                <DataTableHeaderCell>ID</DataTableHeaderCell>
                <DataTableHeaderCell align="right" columnWidth="9rem">
                  Actions
                </DataTableHeaderCell>
              </DataTableRow>
            </DataTableHead>
            <DataTableBody>
              {isLoading ? (
                <DataTableSkeletonRows
                  columns={6}
                  rows={3}
                  widths={["58%", "44%", "62%", "48%", "74%", "60%"]}
                  alignments={["left", "left", "left", "left", "left", "right"]}
                />
              ) : environments.length === 0 ? (
                <DataTableEmpty colSpan={6}>
                  <EmptyState
                    icon={<Boxes size={21} aria-hidden="true" />}
                    title="No virtual environments"
                    description="Create an isolated TON network for this workspace"
                    action={
                      <Button
                        size="sm"
                        variant="primary"
                        leadingIcon={<Plus size={15} aria-hidden="true" />}
                        onClick={() => onCreateOpenChange(true)}
                      >
                        Create environment
                      </Button>
                    }
                  />
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
                    <DataTableRow
                      key={environment.id}
                      hover
                      interactive
                      onClick={event => {
                        const target = event.target
                        if (
                          target instanceof Element &&
                          target.closest("button, a, input, select, textarea")
                        ) {
                          return
                        }
                        onOpenEnvironment(environment)
                      }}
                    >
                      <DataTableCell>
                        <div className={styles.environmentName}>
                          <button
                            type="button"
                            className={styles.environmentLink}
                            onClick={() => onOpenEnvironment(environment)}
                          >
                            {environment.name}
                          </button>
                          {environment.error ? (
                            <span title={environment.error}>{environment.error}</span>
                          ) : null}
                        </div>
                      </DataTableCell>
                      <DataTableCell>
                        <EnvironmentStatusLabel status={environment.status} />
                      </DataTableCell>
                      <DataTableCell tone="muted">
                        <EnvironmentInfoValue environment={environment} property="type" />
                      </DataTableCell>
                      <DataTableCell tone="muted">
                        <EnvironmentInfoValue environment={environment} property="network" />
                      </DataTableCell>
                      <DataTableCell>
                        <InlineActions
                          visibility="always"
                          actions={
                            <CopyInlineAction
                              value={environment.id}
                              label="Copy environment ID"
                              copiedLabel="Environment ID copied"
                            />
                          }
                        >
                          <span className={styles.environmentId}>{environment.id}</span>
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
      </TablePage>

      <CreateEnvironmentDialog
        environmentCount={environments.length}
        importSourceEnvironments={importSourceEnvironments}
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
    </>
  )
}

function EnvironmentStatusLabel({status}: {readonly status: EnvironmentStatus}) {
  return (
    <span className={styles.status} data-status={status}>
      <span className={styles.statusDot} aria-hidden="true" />
      {environmentStatusLabels[status]}
    </span>
  )
}

function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
