import {ConfigPage} from "@acton/explorer-core/pages/ConfigPage"
import {ParameterEditor, type ParameterUpdate} from "@acton/explorer-core/config/ParameterEditor"
import type {NetworkConfigParameter} from "@acton/explorer-core/api/config"
import type {TonClient} from "@acton/explorer-core/api/client"
import {useExplorerRoutePaths} from "@acton/explorer-core/hooks/useExplorerRoutePaths"
import {Button, Dialog, useToast} from "@acton/ui"
import {Plus} from "lucide-react"
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react"
import {createPortal} from "react-dom"
import {Link} from "react-router"

import {requestJson, type StudioEnvironment} from "../../../studioApi"
import styles from "./NetworkConfigPage.module.css"

interface LocalnetOperation {
  readonly id: string
  readonly status: "running" | "completed" | "failed"
  readonly phase: string
  readonly durationMs: number
  readonly error?: string
  readonly result?: {readonly index: number; readonly masterchainSeqno: number}
}

type NetworkConfigUpdate =
  | {readonly status: "pending"; readonly operation: LocalnetOperation}
  | {readonly status: "applied"; readonly index: number; readonly masterchainSeqno: number}

/** Studio supplies environment routing; encoding and chain mutations have separate owners. */
export function NetworkConfigPage({
  environment,
  client,
  onActionsChange,
}: {
  readonly environment: StudioEnvironment
  readonly client: TonClient
  readonly onActionsChange: (actions: ReactNode) => void
}) {
  const {showToast, updateToast, dismissToast} = useToast()
  const {blockPath} = useExplorerRoutePaths()
  const operationToast = useRef<string | undefined>(undefined)
  const storageKey = `acton-config-operation:${environment.id}`
  const [editing, setEditing] = useState<{parameter?: NetworkConfigParameter}>()
  const [editorOpen, setEditorOpen] = useState(false)
  const [actionsContainer, setActionsContainer] = useState<HTMLDivElement | null>(null)
  const [reloadKey, setReloadKey] = useState(0)
  const [submitting, setSubmitting] = useState(false)
  const [pendingId, setPendingId] = useState<string | undefined>(
    () => sessionStorage.getItem(storageKey) ?? undefined,
  )
  const base = `/api/v1/environments/${encodeURIComponent(environment.id)}`
  const busy = submitting || Boolean(pendingId)

  const headerActions = useMemo(
    () => (
      <Button
        variant="primary"
        size="sm"
        disabled={busy}
        leadingIcon={<Plus size={15} />}
        onClick={() => {
          setEditing({})
          setEditorOpen(true)
        }}
      >
        Add parameter
      </Button>
    ),
    [busy],
  )

  useLayoutEffect(() => {
    onActionsChange(headerActions)
    return () => onActionsChange(undefined)
  }, [headerActions, onActionsChange])

  const showLoadError = useCallback(
    (description: string) => {
      showToast({title: "Could not load configuration", description, variant: "error"})
    },
    [showToast],
  )

  const showApplied = useCallback(
    (toastId: string, result: NonNullable<LocalnetOperation["result"]>) => {
      setReloadKey(value => value + 1)
      setEditorOpen(false)
      updateToast(toastId, {
        title: `Parameter ${result.index} applied`,
        description: (
          <>
            Confirmed in masterchain block{" "}
            <Link
              className={styles.blockLink}
              to={blockPath(-1, "8000000000000000", result.masterchainSeqno)}
            >
              #{result.masterchainSeqno}
            </Link>
          </>
        ),
        variant: "success",
        durationMs: 4000,
      })
      operationToast.current = undefined
    },
    [blockPath, updateToast],
  )

  // The page owns operation tracking so closing the editor does not interrupt
  // confirmation or hide the eventual result from the notification stack.
  useEffect(() => {
    if (!pendingId) return
    const controller = new AbortController()
    let inFlight = false
    let finished = false
    const toastId =
      operationToast.current ??
      showToast({
        title: "Applying configuration",
        description: "Waiting for masterchain confirmation",
        variant: "loading",
        durationMs: 0,
      })
    operationToast.current = toastId

    updateToast(toastId, {
      description: "Waiting for masterchain confirmation",
    })

    const poll = async () => {
      if (inFlight || finished || controller.signal.aborted) return
      inFlight = true
      try {
        const current = await requestJson<LocalnetOperation>(
          `${base}/localnet-operations/${encodeURIComponent(pendingId)}`,
          {signal: controller.signal},
        )
        if (controller.signal.aborted) return
        if (current.status !== "running") {
          finished = true
          sessionStorage.removeItem(storageKey)
          setPendingId(undefined)
          if (current.status === "failed") {
            setReloadKey(value => value + 1)
            updateToast(toastId, {
              title: "Configuration update failed",
              description: current.error,
              variant: "error",
              durationMs: 8000,
            })
          } else if (current.result) {
            showApplied(toastId, current.result)
          }
          operationToast.current = undefined
          return
        }

        updateToast(toastId, {
          title: "Applying configuration",
          description: "Waiting for masterchain confirmation",
          variant: "loading",
          durationMs: 0,
        })
      } catch (cause) {
        if (controller.signal.aborted) return
        // Polling retries must not fill the notification stack with the same failure.
        updateToast(toastId, {
          title: "Cannot read operation progress — reconnecting",
          description: cause instanceof Error ? cause.message : String(cause),
          variant: "error",
          durationMs: 0,
        })
      } finally {
        inFlight = false
      }
    }
    const timer = setInterval(() => void poll(), 750)
    void poll()
    return () => {
      controller.abort()
      clearInterval(timer)
      if (operationToast.current === toastId) {
        dismissToast(toastId)
        operationToast.current = undefined
      }
    }
  }, [base, pendingId, storageKey, showToast, updateToast, dismissToast, showApplied])

  const apply = async (update: ParameterUpdate) => {
    setSubmitting(true)
    const toastId = showToast({
      title: "Applying configuration",
      description: "Submitting configuration change",
      variant: "loading",
      durationMs: 0,
    })
    operationToast.current = toastId
    try {
      const accepted = await requestJson<NetworkConfigUpdate>(`${base}/network/config`, {
        method: "POST",
        headers: {"content-type": "application/json"},
        body: JSON.stringify(update),
      })
      if (accepted.status === "applied") {
        showApplied(toastId, accepted)
      } else {
        // Only the operation ID is retained; reconnecting never resubmits a change.
        sessionStorage.setItem(storageKey, accepted.operation.id)
        setPendingId(accepted.operation.id)
      }
    } catch (cause) {
      setReloadKey(value => value + 1)
      updateToast(toastId, {
        title: "Configuration update failed",
        description: cause instanceof Error ? cause.message : String(cause),
        variant: "error",
        durationMs: 8000,
      })
      operationToast.current = undefined
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <>
      <ConfigPage
        client={client}
        embedded
        navigationPosition="right"
        showBreadcrumbs={false}
        onError={showLoadError}
        reloadKey={reloadKey}
        onEdit={
          busy
            ? undefined
            : parameter => {
                setEditing({parameter})
                setEditorOpen(true)
              }
        }
      />
      <Dialog
        open={editorOpen}
        onOpenChange={setEditorOpen}
        onOpenChangeComplete={open => {
          if (!open) setEditing(undefined)
        }}
        title={
          editing?.parameter
            ? `Edit parameter ${editing.parameter.id} — ${editing.parameter.title}`
            : "Add configuration parameter"
        }
        maxWidth="64rem"
        footer={
          <div className={styles.footer}>
            <div ref={setActionsContainer} />
          </div>
        }
      >
        {editing && (
          <ParameterEditor
            parameter={editing.parameter}
            busy={busy}
            onApply={update => void apply(update)}
            onCancel={() => setEditorOpen(false)}
            renderActions={actions => actionsContainer && createPortal(actions, actionsContainer)}
          />
        )}
      </Dialog>
    </>
  )
}
