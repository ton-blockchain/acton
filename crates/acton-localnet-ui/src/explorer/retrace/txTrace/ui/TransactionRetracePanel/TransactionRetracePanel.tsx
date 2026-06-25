import {type CSSProperties, useEffect, useLayoutEffect, useRef, useState} from "react"
import {X} from "lucide-react"

import {useToast} from "@acton/shared-ui"

import type {TonClient} from "../../../../api/client"
import {useNetworkInfo} from "../../../../hooks/useNetworkInfo"
import {traceTx} from "../../lib/traceTx"
import type {RetraceResultAndCode} from "../../lib/types"
import InlineLoader from "../InlineLoader"
import RetraceWorkspace from "../RetraceWorkspace"
import "../../../Retrace.tokens.css"
import styles from "./TransactionRetracePanel.module.css"

type RetracePanelState =
  | {readonly type: "loading"}
  | {readonly type: "ready"; readonly result: RetraceResultAndCode}
  | {readonly type: "error"; readonly message: string}

interface TransactionRetracePanelProps {
  readonly client: TonClient
  readonly txHash: string
  readonly onClose: () => void
  readonly onResult?: (txHash: string, result: RetraceResultAndCode) => void
}

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Failed to trace transaction"
}

export default function TransactionRetracePanel({
  client,
  txHash,
  onClose,
  onResult,
}: TransactionRetracePanelProps) {
  const {network} = useNetworkInfo()
  const {showToast} = useToast()
  const rootRef = useRef<HTMLDivElement | null>(null)
  const [flowMetrics, setFlowMetrics] = useState({offset: 0, width: 0})
  const [state, setState] = useState<RetracePanelState>({type: "loading"})

  useLayoutEffect(() => {
    const updateFlowMetrics = () => {
      const root = rootRef.current
      const anchor = root?.parentElement ?? root
      if (!anchor) {
        return
      }

      const offset = Math.max(0, Math.round(anchor.getBoundingClientRect().left))
      const width = Math.round(document.documentElement.clientWidth || window.innerWidth)
      setFlowMetrics(current =>
        current.offset === offset && current.width === width ? current : {offset, width},
      )
    }

    updateFlowMetrics()

    const resizeObserver =
      typeof ResizeObserver === "undefined" ? undefined : new ResizeObserver(updateFlowMetrics)
    const observedElement = rootRef.current?.parentElement
    if (resizeObserver && observedElement) {
      resizeObserver.observe(observedElement)
    }

    window.addEventListener("resize", updateFlowMetrics)

    return () => {
      resizeObserver?.disconnect()
      window.removeEventListener("resize", updateFlowMetrics)
    }
  }, [])

  useEffect(() => {
    let isActive = true

    const loadRetrace = async () => {
      setState({type: "loading"})

      try {
        const result = await traceTx(txHash, network, client)
        if (isActive) {
          setState({type: "ready", result})
          onResult?.(txHash, result)
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
  }, [client, network, onResult, showToast, txHash])

  const rootStyle = {
    "--retrace-flow-offset": `${flowMetrics.offset}px`,
    "--retrace-flow-width": flowMetrics.width > 0 ? `${flowMetrics.width}px` : "100vw",
  } as CSSProperties

  return (
    <div ref={rootRef} className={`${styles.root} retraceRoot`} style={rootStyle}>
      <div className={styles.header}>
        <div className={styles.title}>Retrace</div>
        <button
          type="button"
          className={styles.closeButton}
          onClick={onClose}
          aria-label="Close retrace"
        >
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
