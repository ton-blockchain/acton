import {useEffect, useState} from "react"
import {X} from "lucide-react"

import {useToast} from "@acton/shared-ui"
import type {RetraceResultAndCode} from "@retrace/txTrace/lib/types"
import {traceTx} from "@retrace/txTrace/lib/traceTx"
import InlineLoader from "@retrace/txTrace/ui/InlineLoader"
import RetraceWorkspace from "@retrace/txTrace/ui/RetraceWorkspace"

import {useNetworkInfo} from "../../../../hooks/useNetworkInfo"
import "@retrace/RetracePage.tokens.css"
import styles from "./TransactionRetracePanel.module.css"

type RetracePanelState =
  | {readonly type: "loading"}
  | {readonly type: "ready"; readonly result: RetraceResultAndCode}
  | {readonly type: "error"; readonly message: string}

interface TransactionRetracePanelProps {
  readonly txHash: string
  readonly onClose: () => void
}

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Failed to trace transaction"
}

export default function TransactionRetracePanel({txHash, onClose}: TransactionRetracePanelProps) {
  const {network} = useNetworkInfo()
  const {showToast} = useToast()
  const [state, setState] = useState<RetracePanelState>({type: "loading"})

  useEffect(() => {
    let isActive = true

    const loadRetrace = async () => {
      setState({type: "loading"})

      try {
        const result = await traceTx(txHash, network)
        if (isActive) {
          setState({type: "ready", result})
        }
      } catch (error) {
        if (!isActive) {
          return
        }

        const message = getErrorMessage(error)
        setState({type: "error", message})
        showToast({
          title: "Failed to trace transaction",
          description: message,
          variant: "error",
        })
      }
    }

    void loadRetrace()

    return () => {
      isActive = false
    }
  }, [network, showToast, txHash])

  return (
    <div className={`${styles.root} retraceRoot`}>
      <div className={styles.header}>
        <div className={styles.title}>Retrace</div>
        <button type="button" className={styles.closeButton} onClick={onClose} aria-label="Close retrace">
          <X size={16} />
        </button>
      </div>

      <div className={styles.content}>
        {state.type === "loading" && (
          <div className={styles.loadingState}>
            <InlineLoader
              message="Tracing transaction"
              subtext="This may take a few moments"
              loading={true}
            />
          </div>
        )}

        {state.type === "error" && (
          <div className={styles.errorState}>
            <div className={styles.errorTitle}>Failed to trace transaction</div>
            <div className={styles.errorMessage}>{state.message}</div>
          </div>
        )}

        {state.type === "ready" && <RetraceWorkspace result={state.result} />}
      </div>
    </div>
  )
}
