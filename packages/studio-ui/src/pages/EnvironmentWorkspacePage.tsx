import {Button, RawDataBlock} from "@acton/ui"
import {CircleAlert, LoaderCircle, Play} from "lucide-react"
import {useEffect, useState} from "react"
import {useLocation} from "react-router"

import {EnvironmentStartupProgress} from "../components/EnvironmentStartupProgress"
import {supports} from "../environmentCapabilities"
import {LocalnetWorkspace, type LocalnetWorkspaceShellState} from "../localnet/LocalnetWorkspace"
import {type StudioEnvironment, restartStudioEnvironment} from "../studioApi"

import styles from "./EnvironmentWorkspacePage.module.css"

interface EnvironmentWorkspacePageProps {
  readonly basePath: string
  readonly environment?: StudioEnvironment
  readonly isLoading: boolean
  readonly loadError?: string
  readonly onEnvironmentChange: (environment: StudioEnvironment) => void
  readonly onEnvironmentDelete: (environmentId: string) => void
  readonly onRetry: () => Promise<void>
  readonly onShellChange: (state: LocalnetWorkspaceShellState) => void
}

export function EnvironmentWorkspacePage({
  basePath,
  environment,
  isLoading,
  loadError,
  onEnvironmentChange,
  onEnvironmentDelete,
  onRetry,
  onShellChange,
}: EnvironmentWorkspacePageProps) {
  const location = useLocation()
  const [isRestarting, setIsRestarting] = useState(false)
  const [restartError, setRestartError] = useState<string>()
  const requestError = loadError ?? restartError
  const environmentError = environment?.error?.trim()
  const visibleError = requestError ?? environmentError
  // Runtime errors keep recovery copy before the full diagnostic paragraph.
  const [failureTitle, ...recoverySteps] = visibleError?.split("\n\n")[0].split("\n") ?? []
  const hasRecoverySteps = recoverySteps.length > 0
  const hasFailure = Boolean(visibleError || environment?.status === "failed")
  const isManaged = environment?.lifecycle === "managed"
  const isSnapshotsPage =
    location.pathname === `${basePath}/snapshots` && supports(environment, "snapshots")
  const isSettingsPage = location.pathname === `${basePath}/settings` && isManaged
  const isAdminPage =
    location.pathname === `${basePath}/admin` &&
    isManaged &&
    environment?.config.kind === "fullTonNetwork"
  const canOpenWorkspace =
    environment?.status === "running" || isSnapshotsPage || isSettingsPage || isAdminPage

  useEffect(() => {
    if (canOpenWorkspace) return

    const subject = isManaged ? "virtual environment" : "network"
    const pageDescription = visibleError
      ? `This ${subject} could not be opened`
      : environment?.status === "stopped"
        ? `Restart this ${subject} to continue`
        : environment?.status === "failed"
          ? `This ${subject} needs attention`
          : `Preparing this ${subject}`

    onShellChange({
      pageDescription,
      pageTitle: environment?.name ?? (isManaged ? "Virtual Environment" : "Network"),
    })
  }, [
    environment?.name,
    environment?.status,
    canOpenWorkspace,
    isManaged,
    onShellChange,
    visibleError,
  ])

  const handleRestart = async () => {
    if (!environment) return

    setIsRestarting(true)
    setRestartError(undefined)
    try {
      const restarted = await restartStudioEnvironment(environment.id)
      onEnvironmentChange(restarted)
    } catch (error) {
      setRestartError(error instanceof Error ? error.message : String(error))
    } finally {
      setIsRestarting(false)
    }
  }

  if (canOpenWorkspace) {
    return (
      <LocalnetWorkspace
        basePath={basePath}
        onEnvironmentChange={onEnvironmentChange}
        onEnvironmentDelete={onEnvironmentDelete}
        onShellChange={onShellChange}
      />
    )
  }

  const canRestart =
    isManaged && (environment?.status === "stopped" || environment?.status === "failed")
  const isStarting = environment?.status === "starting" || environment?.status === "stopping"
  const isPending = isStarting || (isLoading && !visibleError)

  return (
    <div className={styles.statePage}>
      <main className={styles.stateContent}>
        <div className={styles.stateMessage}>
          <span className={styles.stateIcon} data-error={hasFailure ? "true" : undefined}>
            {hasFailure ? (
              <CircleAlert size={21} aria-hidden="true" />
            ) : isPending ? (
              <LoaderCircle className={styles.loadingIcon} size={21} aria-hidden="true" />
            ) : (
              <Play size={20} aria-hidden="true" />
            )}
          </span>
          <strong className={styles.stateTitle}>
            {hasFailure
              ? hasRecoverySteps
                ? failureTitle
                : "Unable to open environment"
              : isLoading && !environment
                ? "Loading environment"
                : isStarting
                  ? environment?.status === "stopping"
                    ? "Environment is stopping"
                    : "Environment is starting"
                  : "Environment is stopped"}
          </strong>
          <span className={styles.stateDescription}>
            {hasFailure
              ? hasRecoverySteps
                ? recoverySteps.join("\n")
                : "Review the diagnostic details, resolve the reported cause, then retry"
              : isLoading && !environment
                ? "Fetching its current state"
                : isStarting
                  ? "The workspace will open when the localnet is ready"
                  : "Restart the environment to continue working with it"}
          </span>
        </div>
        {isStarting && environment?.startupTimings ? (
          <EnvironmentStartupProgress
            environmentId={environment.id}
            timings={environment.startupTimings}
          />
        ) : undefined}
        {visibleError ? (
          <RawDataBlock
            className={styles.errorDetails}
            title="Diagnostic details"
            value={visibleError}
            wrap
            copyLabel="error details"
            maxHeight="24rem"
          />
        ) : undefined}
        <div className={styles.stateActions}>
          {canRestart ? (
            <Button
              variant="primary"
              size="sm"
              leadingIcon={<Play size={15} aria-hidden="true" />}
              loading={isRestarting}
              onClick={() => void handleRestart()}
            >
              {hasFailure ? "Retry" : "Restart environment"}
            </Button>
          ) : visibleError ? (
            <Button variant="outline" size="sm" onClick={() => void onRetry()}>
              Retry
            </Button>
          ) : undefined}
        </div>
      </main>
    </div>
  )
}
