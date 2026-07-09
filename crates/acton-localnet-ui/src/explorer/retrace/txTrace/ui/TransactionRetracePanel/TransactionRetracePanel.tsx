import {useEffect, useState} from "react"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"

import {useToast} from "@acton/ui"
import type {ContractData} from "@acton/shared-ui"

import {useNetworkInfo} from "../../../../hooks/useNetworkInfo"
import type {ExplorerMetadataRegistry} from "../../../../metadata/types"
import {traceTx} from "../../lib/traceTx"
import type {RetraceResultAndCode} from "../../lib/types"
import RetraceWorkspace from "../RetraceWorkspace"
import {
  TraceDebugPanel,
  TraceDebugPanelError,
  TraceDebugPanelLoading,
} from "../TraceDebugPanel/TraceDebugPanel"

type RetracePanelState =
  | {readonly type: "loading"}
  | {readonly type: "ready"; readonly result: RetraceResultAndCode}
  | {readonly type: "error"; readonly message: string}

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
  const [state, setState] = useState<RetracePanelState>({type: "loading"})

  useEffect(() => {
    let isActive = true

    const loadRetrace = async () => {
      setState({type: "loading"})

      try {
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

  return (
    <TraceDebugPanel className={className} onClose={onClose}>
      {state.type === "loading" && <TraceDebugPanelLoading />}

      {state.type === "error" && (
        <TraceDebugPanelError title="Failed to trace transaction" message={state.message} />
      )}

      {state.type === "ready" && (
        <RetraceWorkspace
          result={state.result}
          contractAbi={contractAbi}
          contracts={contracts}
          onContractClick={onContractClick}
        />
      )}
    </TraceDebugPanel>
  )
}
