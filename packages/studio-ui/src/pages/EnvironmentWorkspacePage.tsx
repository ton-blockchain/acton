import {Button} from "@acton/ui"
import {CircleAlert, LoaderCircle, Play} from "lucide-react"
import {useEffect, useState} from "react"

import {LocalnetWorkspace, type LocalnetWorkspaceShellState} from "../localnet/LocalnetWorkspace"
import {type StudioEnvironment, restartStudioEnvironment} from "../studioApi"

import styles from "./EnvironmentWorkspacePage.module.css"

interface EnvironmentWorkspacePageProps {
  readonly basePath: string
  readonly environment?: StudioEnvironment
  readonly isLoading: boolean
  readonly loadError?: string
  readonly onEnvironmentChange: (environment: StudioEnvironment) => void
  readonly onRetry: () => Promise<void>
  readonly onShellChange: (state: LocalnetWorkspaceShellState) => void
}

export function EnvironmentWorkspacePage({
  basePath,
  environment,
  isLoading,
  loadError,
  onEnvironmentChange,
  onRetry,
  onShellChange,
}: EnvironmentWorkspacePageProps) {
  const [isRestarting, setIsRestarting] = useState(false)
  const [restartError, setRestartError] = useState<string>()
  const visibleError = loadError ?? restartError

  useEffect(() => {
    if (environment?.status === "running") return

    const pageDescription = visibleError
      ? "This virtual environment could not be opened"
      : environment?.status === "stopped"
        ? "Restart this virtual environment to continue"
        : environment?.status === "failed"
          ? "This virtual environment needs attention"
          : "Preparing this virtual environment"

    onShellChange({
      pageDescription,
      pageTitle: environment?.name ?? "Virtual Environment",
      rpcUrl: environment?.rpcUrl,
    })
  }, [environment?.name, environment?.rpcUrl, environment?.status, onShellChange, visibleError])

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

  if (environment?.status === "running") {
    return <LocalnetWorkspace basePath={basePath} onShellChange={onShellChange} />
  }

  const canRestart = environment?.status === "stopped" || environment?.status === "failed"
  const isStarting =
    environment?.status === "starting" ||
    environment?.status === "stopping" ||
    (isLoading && !visibleError)

  return (
    <div className={styles.statePage}>
      <main className={styles.stateContent}>
        <span className={styles.stateIcon} data-error={visibleError ? "true" : undefined}>
          {visibleError ? (
            <CircleAlert size={21} aria-hidden="true" />
          ) : isStarting ? (
            <LoaderCircle className={styles.loadingIcon} size={21} aria-hidden="true" />
          ) : (
            <Play size={20} aria-hidden="true" />
          )}
        </span>
        <strong>
          {visibleError
            ? "Unable to open environment"
            : isStarting
              ? environment?.status === "stopping"
                ? "Environment is stopping"
                : "Environment is starting"
              : "Environment is stopped"}
        </strong>
        <span>
          {visibleError ??
            (isStarting
              ? "The workspace will open when the localnet is ready"
              : "Restart the environment to continue working with it")}
        </span>
        {canRestart ? (
          <Button
            variant="primary"
            size="sm"
            leadingIcon={<Play size={15} aria-hidden="true" />}
            loading={isRestarting}
            onClick={() => void handleRestart()}
          >
            Restart environment
          </Button>
        ) : visibleError ? (
          <Button variant="outline" size="sm" onClick={() => void onRetry()}>
            Retry
          </Button>
        ) : undefined}
      </main>
    </div>
  )
}
