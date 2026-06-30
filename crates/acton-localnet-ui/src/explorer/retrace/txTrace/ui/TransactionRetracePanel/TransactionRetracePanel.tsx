import {Suspense, lazy, type CSSProperties, useEffect, useState} from "react"
import {X} from "lucide-react"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"

import {useToast, type ContractData} from "@acton/shared-ui"

import {useNetworkInfo} from "../../../../hooks/useNetworkInfo"
import {useAvailableFlowMetrics} from "../../../../hooks/useAvailableFlowMetrics"
import type {ExplorerMetadataRegistry} from "../../../../metadata/types"
import type {RetraceResultAndCode} from "../../lib/types"
import InlineLoader from "../InlineLoader"
import "../../../Retrace.tokens.css"
import styles from "./TransactionRetracePanel.module.css"

const RetraceWorkspace = lazy(() => import("../RetraceWorkspace"))

type RetracePanelState =
  | {readonly type: "loading"}
  | {readonly type: "ready"; readonly result: RetraceResultAndCode}
  | {readonly type: "error"; readonly message: string}

const MAX_RETRACE_FLOW_WIDTH = 1800

interface TransactionRetracePanelProps {
  readonly metadataRegistry: ExplorerMetadataRegistry
  readonly txHash: string
  readonly codeHash?: string
  readonly contractAbi?: ContractABI
  readonly contracts?: Map<string, ContractData>
  readonly className?: string
  readonly onClose: () => void
  readonly onContractClick?: (address: string) => void
  readonly onResult?: (txHash: string, result: RetraceResultAndCode) => void
}

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Failed to trace transaction"
}

export default function TransactionRetracePanel({
  metadataRegistry,
  txHash,
  codeHash,
  contractAbi,
  contracts,
  className,
  onClose,
  onContractClick,
  onResult,
}: TransactionRetracePanelProps) {
  const {network} = useNetworkInfo()
  const {showToast} = useToast()
  const {flowMetrics, rootRef} = useAvailableFlowMetrics<HTMLDivElement>(MAX_RETRACE_FLOW_WIDTH)
  const [state, setState] = useState<RetracePanelState>({type: "loading"})

  useEffect(() => {
    let isActive = true

    const loadRetrace = async () => {
      setState({type: "loading"})

      try {
        const {traceTx} = await import("../../lib/traceTx")
        const result = await traceTx(txHash, network, metadataRegistry, {codeHash})
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
  }, [codeHash, metadataRegistry, network, onResult, showToast, txHash])

  const rootStyle = {
    "--retrace-flow-offset": `${flowMetrics.offset}px`,
    "--retrace-flow-width": flowMetrics.width > 0 ? `${flowMetrics.width}px` : "100vw",
  } as CSSProperties

  return (
    <div
      ref={rootRef}
      className={`${styles.root} ${className ?? ""} retraceRoot`}
      style={rootStyle}
    >
      <div className={styles.header}>
        <div className={styles.title}>Debug</div>
        <button
          type="button"
          className={styles.closeButton}
          onClick={onClose}
          aria-label="Close debug panel"
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

        {state.type === "ready" && (
          <Suspense
            fallback={
              <div className={styles.loadingState}>
                <InlineLoader
                  message="Loading debug workspace"
                  subtext="Preparing trace view"
                  loading={true}
                />
              </div>
            }
          >
            <RetraceWorkspace
              result={state.result}
              contractAbi={contractAbi}
              contracts={contracts}
              onContractClick={onContractClick}
            />
          </Suspense>
        )}
      </div>
    </div>
  )
}
