import {ArchiveRestore, Download, FileJson, Plus, RotateCcw, Trash2, Upload} from "lucide-react"
import {Button, Dialog, InlineButton, Input, useToast} from "@acton/ui"
import {useCallback, useRef, useState} from "react"
import type {ChangeEvent, FC, FormEvent} from "react"

import type {TonClient} from "../../explorer/api/client"
import type {LocalnetCheckpoint} from "../../explorer/api/types"

import styles from "../DashboardPage.module.css"

interface NodeStateControlsProps {
  readonly client: TonClient
  readonly latestBlockSeqno?: number
  readonly onStateChanged: () => void
}

interface StateFileDetails {
  readonly size: string
  readonly blockSeqno?: number
  readonly isInspecting?: boolean
  readonly error?: string
}

export const NodeStateControls: FC<NodeStateControlsProps> = ({
  client,
  latestBlockSeqno,
  onStateChanged,
}) => {
  const {showToast} = useToast()
  const stateFileInputRef = useRef<HTMLInputElement>(null)
  const [isCheckpointsOpen, setIsCheckpointsOpen] = useState(false)
  const [checkpoints, setCheckpoints] = useState<readonly LocalnetCheckpoint[]>([])
  const [checkpointName, setCheckpointName] = useState("")
  const [checkpointError, setCheckpointError] = useState<string>()
  const [isLoadingCheckpoints, setIsLoadingCheckpoints] = useState(false)
  const [busyAction, setBusyAction] = useState<string>()
  const [stateFile, setStateFile] = useState<File>()
  const [stateFileDetails, setStateFileDetails] = useState<StateFileDetails>()
  const [checkpointToRestore, setCheckpointToRestore] = useState<LocalnetCheckpoint>()

  const loadCheckpoints = useCallback(async () => {
    setIsLoadingCheckpoints(true)
    setCheckpointError(undefined)
    try {
      setCheckpoints(await client.listCheckpoints())
    } catch (error) {
      setCheckpointError(errorMessage(error, "Failed to load checkpoints"))
    } finally {
      setIsLoadingCheckpoints(false)
    }
  }, [client])

  const openCheckpoints = useCallback(() => {
    setIsCheckpointsOpen(true)
    void loadCheckpoints()
  }, [loadCheckpoints])

  const downloadState = useCallback(async () => {
    setBusyAction("download-state")
    try {
      const state = await client.downloadState()
      downloadBlob(state, `acton-localnet-state-${latestBlockSeqno ?? "latest"}.json`)
      showToast({
        variant: "success",
        title: "State downloaded",
        description: "The current localnet state was saved as a JSON file",
      })
    } catch (error) {
      showToast({
        variant: "error",
        title: "State not downloaded",
        description: errorMessage(error, "Failed to download localnet state"),
      })
    } finally {
      setBusyAction(undefined)
    }
  }, [client, latestBlockSeqno, showToast])

  const selectStateFile = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    const [file] = Array.from(event.target.files ?? [])
    if (!file) return

    setStateFile(file)
    setStateFileDetails({size: formatFileSize(file.size), isInspecting: true})
    void file
      .text()
      .then(text => {
        const document = JSON.parse(text) as {
          readonly globals?: {readonly head_seqno?: unknown}
        }
        if (stateFileInputRef.current?.files?.[0] !== file) return
        setStateFileDetails({
          size: formatFileSize(file.size),
          blockSeqno:
            typeof document.globals?.head_seqno === "number"
              ? document.globals.head_seqno
              : undefined,
        })
      })
      .catch(() => {
        if (stateFileInputRef.current?.files?.[0] !== file) return
        setStateFileDetails({
          size: formatFileSize(file.size),
          error: "This file is not valid JSON",
        })
      })
  }, [])

  const closeStateConfirmation = useCallback(() => {
    if (busyAction === "load-state") return
    setStateFile(undefined)
    setStateFileDetails(undefined)
    if (stateFileInputRef.current) stateFileInputRef.current.value = ""
  }, [busyAction])

  const loadState = useCallback(async () => {
    if (!stateFile) return
    setBusyAction("load-state")
    try {
      await client.loadState(stateFile)
      setCheckpoints([])
      setStateFile(undefined)
      setStateFileDetails(undefined)
      if (stateFileInputRef.current) stateFileInputRef.current.value = ""
      onStateChanged()
      showToast({
        variant: "success",
        title: "State loaded",
        description: `${stateFile.name} replaced the current localnet state`,
      })
    } catch (error) {
      showToast({
        variant: "error",
        title: "State not loaded",
        description: errorMessage(error, "Failed to load localnet state"),
      })
    } finally {
      setBusyAction(undefined)
    }
  }, [client, onStateChanged, showToast, stateFile])

  const createCheckpoint = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault()
      const name = checkpointName.trim()
      if (!name) {
        setCheckpointError("Enter a checkpoint name")
        return
      }

      setBusyAction("create-checkpoint")
      setCheckpointError(undefined)
      try {
        const checkpoint = await client.createCheckpoint(name)
        setCheckpoints(current => [...current, checkpoint])
        setCheckpointName("")
        showToast({
          variant: "success",
          title: "Checkpoint created",
          description: `${checkpoint.name} stores block ${checkpoint.block_seqno}`,
        })
      } catch (error) {
        setCheckpointError(errorMessage(error, "Failed to create checkpoint"))
      } finally {
        setBusyAction(undefined)
      }
    },
    [checkpointName, client, showToast],
  )

  const restoreCheckpoint = useCallback(async () => {
    if (!checkpointToRestore) return
    setBusyAction("restore-checkpoint")
    try {
      await client.restoreCheckpoint(checkpointToRestore.name)
      setCheckpointToRestore(undefined)
      onStateChanged()
      showToast({
        variant: "success",
        title: "Checkpoint restored",
        description: `${checkpointToRestore.name} is now the current localnet state`,
      })
    } catch (error) {
      showToast({
        variant: "error",
        title: "Checkpoint not restored",
        description: errorMessage(error, "Failed to restore checkpoint"),
      })
    } finally {
      setBusyAction(undefined)
    }
  }, [checkpointToRestore, client, onStateChanged, showToast])

  const downloadCheckpoint = useCallback(
    async (checkpoint: LocalnetCheckpoint) => {
      setBusyAction(`download-${checkpoint.name}`)
      try {
        const state = await client.downloadCheckpoint(checkpoint.name)
        downloadBlob(state, `acton-checkpoint-${safeFilename(checkpoint.name)}.json`)
      } catch (error) {
        showToast({
          variant: "error",
          title: "Checkpoint not downloaded",
          description: errorMessage(error, "Failed to download checkpoint"),
        })
      } finally {
        setBusyAction(undefined)
      }
    },
    [client, showToast],
  )

  const deleteCheckpoint = useCallback(
    async (checkpoint: LocalnetCheckpoint) => {
      setBusyAction(`delete-${checkpoint.name}`)
      try {
        await client.deleteCheckpoint(checkpoint.name)
        setCheckpoints(current => current.filter(item => item.name !== checkpoint.name))
      } catch (error) {
        showToast({
          variant: "error",
          title: "Checkpoint not deleted",
          description: errorMessage(error, "Failed to delete checkpoint"),
        })
      } finally {
        setBusyAction(undefined)
      }
    },
    [client, showToast],
  )

  return (
    <>
      <div className={styles.nodeStateActions}>
        <InlineButton leadingIcon={<ArchiveRestore size={15} />} onClick={openCheckpoints}>
          Checkpoints
        </InlineButton>
        <InlineButton
          leadingIcon={<Upload size={15} />}
          onClick={() => stateFileInputRef.current?.click()}
        >
          Load state
        </InlineButton>
        <InlineButton
          leadingIcon={<Download size={15} />}
          disabled={busyAction === "download-state"}
          onClick={() => void downloadState()}
        >
          {busyAction === "download-state" ? "Downloading" : "Download state"}
        </InlineButton>
        <input
          ref={stateFileInputRef}
          className={styles.visuallyHiddenInput}
          type="file"
          accept="application/json,.json"
          tabIndex={-1}
          onChange={selectStateFile}
        />
      </div>

      <Dialog
        open={isCheckpointsOpen}
        title="Checkpoints"
        description="Keep reusable restore points in this localnet process"
        className={styles.dashboardDialog}
        maxWidth={620}
        closeLabel="Close checkpoints"
        onOpenChange={setIsCheckpointsOpen}
      >
        <div className={styles.checkpointDialogContent}>
          <form
            className={styles.checkpointCreateRow}
            onSubmit={event => void createCheckpoint(event)}
          >
            <Input
              aria-label="Checkpoint name"
              placeholder="Checkpoint name"
              value={checkpointName}
              disabled={busyAction === "create-checkpoint"}
              onChange={event => {
                setCheckpointName(event.target.value)
                setCheckpointError(undefined)
              }}
            />
            <Button
              type="submit"
              size="sm"
              variant="primary"
              leadingIcon={<Plus size={15} />}
              loading={busyAction === "create-checkpoint"}
              disabled={!checkpointName.trim()}
            >
              Create
            </Button>
          </form>

          {checkpointError && (
            <div className={styles.checkpointError} role="alert">
              {checkpointError}
            </div>
          )}

          <div className={styles.checkpointList}>
            {isLoadingCheckpoints ? (
              <div className={styles.checkpointEmpty}>Loading checkpoints…</div>
            ) : checkpoints.length === 0 ? (
              <div className={styles.checkpointEmpty}>No checkpoints yet</div>
            ) : (
              checkpoints.map(checkpoint => (
                <div key={checkpoint.name} className={styles.checkpointRow}>
                  <div className={styles.checkpointIdentity}>
                    <strong>{checkpoint.name}</strong>
                    <span>Block {checkpoint.block_seqno}</span>
                  </div>
                  <div className={styles.checkpointActions}>
                    <Button
                      size="sm"
                      variant="ghost"
                      leadingIcon={<RotateCcw size={14} />}
                      disabled={busyAction !== undefined}
                      onClick={() => {
                        setIsCheckpointsOpen(false)
                        setCheckpointToRestore(checkpoint)
                      }}
                    >
                      Restore
                    </Button>
                    <Button
                      size="icon"
                      variant="ghost"
                      leadingIcon={<Download size={15} />}
                      aria-label={`Download ${checkpoint.name}`}
                      title="Download checkpoint"
                      loading={busyAction === `download-${checkpoint.name}`}
                      disabled={busyAction !== undefined}
                      onClick={() => void downloadCheckpoint(checkpoint)}
                    />
                    <Button
                      size="icon"
                      variant="ghost"
                      leadingIcon={<Trash2 size={15} />}
                      aria-label={`Delete ${checkpoint.name}`}
                      title="Delete checkpoint"
                      loading={busyAction === `delete-${checkpoint.name}`}
                      disabled={busyAction !== undefined}
                      onClick={() => void deleteCheckpoint(checkpoint)}
                    />
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      </Dialog>

      <Dialog
        open={stateFile !== undefined}
        title="Load localnet state?"
        description="This replaces the current node state and clears all checkpoints"
        className={styles.dashboardDialog}
        maxWidth={460}
        dismissible={busyAction !== "load-state"}
        closeLabel="Cancel loading state"
        onOpenChange={open => {
          if (!open) closeStateConfirmation()
        }}
      >
        <div className={styles.stateConfirmationContent}>
          <div className={styles.stateFileSummary}>
            <FileJson size={20} aria-hidden="true" />
            <div className={styles.stateFileIdentity}>
              <strong>{stateFile?.name}</strong>
              <span className={stateFileDetails?.error ? styles.stateFileError : undefined}>
                {formatStateFileDetails(stateFileDetails)}
              </span>
            </div>
          </div>
          <div className={styles.timeModalActions}>
            <Button
              variant="outline"
              disabled={busyAction === "load-state"}
              onClick={closeStateConfirmation}
            >
              Cancel
            </Button>
            <Button
              variant="danger"
              loading={busyAction === "load-state"}
              disabled={stateFileDetails?.isInspecting || stateFileDetails?.error !== undefined}
              onClick={() => void loadState()}
            >
              Load state
            </Button>
          </div>
        </div>
      </Dialog>

      <Dialog
        open={checkpointToRestore !== undefined}
        title="Restore checkpoint?"
        description="This replaces the current node state; the checkpoint remains available"
        className={styles.dashboardDialog}
        maxWidth={460}
        dismissible={busyAction !== "restore-checkpoint"}
        closeLabel="Cancel checkpoint restore"
        onOpenChange={open => {
          if (!open && busyAction !== "restore-checkpoint") setCheckpointToRestore(undefined)
        }}
      >
        <div className={styles.stateConfirmationContent}>
          <div className={styles.stateFileSummary}>
            <ArchiveRestore size={20} aria-hidden="true" />
            <div className={styles.stateFileIdentity}>
              <strong>{checkpointToRestore?.name}</strong>
              <span>Block {checkpointToRestore?.block_seqno}</span>
            </div>
          </div>
          <div className={styles.timeModalActions}>
            <Button
              variant="outline"
              disabled={busyAction === "restore-checkpoint"}
              onClick={() => setCheckpointToRestore(undefined)}
            >
              Cancel
            </Button>
            <Button
              variant="primary"
              leadingIcon={<RotateCcw size={15} />}
              loading={busyAction === "restore-checkpoint"}
              onClick={() => void restoreCheckpoint()}
            >
              Restore
            </Button>
          </div>
        </div>
      </Dialog>
    </>
  )
}

function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob)
  const link = document.createElement("a")
  link.href = url
  link.download = filename
  link.click()
  URL.revokeObjectURL(url)
}

function safeFilename(value: string): string {
  return value.replace(/[^a-zA-Z0-9._-]+/g, "-")
}

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback
}

function formatStateFileDetails(details: StateFileDetails | undefined): string {
  if (!details) return "Reading file"
  if (details.error) return details.error
  if (details.isInspecting) return `${details.size} · Reading state metadata`

  const metadata = [details.size]
  if (details.blockSeqno !== undefined) metadata.push(`Block ${details.blockSeqno}`)
  return metadata.join(" · ")
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}
