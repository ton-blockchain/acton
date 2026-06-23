import React, {memo} from "react"

import {type TraceInfo} from "ton-assembly/dist/trace"
import type {TraceResult} from "txtracer-core/dist/types"

import type {ExitCode} from "@retrace/txTrace/lib/traceTx"
import {VMLogsView} from "@retrace/txTrace/ui/index"

import TransactionDetailsTable from "./TransactionDetailsTable"

export type NetworkType = "mainnet" | "testnet"

export interface RetraceResultAndCode {
  readonly result: TraceResult
  readonly code: string
  readonly trace: TraceInfo
  readonly exitCode: ExitCode | undefined
  readonly network: NetworkType
}

interface RetraceResultViewProps {
  readonly result: RetraceResultAndCode
}

const RetraceResultViewFc: React.FC<RetraceResultViewProps> = ({result}) => {
  return (
    <div className="retrace-result-container">
      <TransactionDetailsTable result={result.result} />

      {result.result.emulatedTx.vmLogs && (
        <VMLogsView
          title="VM Logs"
          logs={result.result.emulatedTx.vmLogs}
          isExpandable={true}
          defaultExpanded={false}
        />
      )}

      {result.result.emulatedTx.executorLogs && (
        <VMLogsView
          title="Executor Logs"
          logs={result.result.emulatedTx.executorLogs}
          isExpandable={true}
          defaultExpanded={false}
        />
      )}
    </div>
  )
}

export const RetraceResultView = memo(RetraceResultViewFc)
RetraceResultView.displayName = "RetraceResultView"
