import React, {memo, useMemo} from "react"

import {type TraceInfo} from "ton-assembly/dist/trace"
import type {TraceResult} from "txtracer-core/dist/types"

import {type BackendContractInfo, type ContractData, TransactionDetails} from "@acton/shared-ui"

import type {ExplorerNetworkInfo} from "../../../hooks/useNetworkInfo"
import type {ExitCode} from "@retrace/txTrace/lib/traceTx"
import {toTransactionInfo} from "@retrace/txTrace/lib/toTransactionInfo"
import {VMLogsView} from "@retrace/txTrace/ui/index"

import {RetraceExtraDetails} from "./RetraceExtraDetails"

export interface RetraceResultAndCode {
  readonly result: TraceResult
  readonly code: string
  readonly trace: TraceInfo
  readonly exitCode: ExitCode | undefined
  readonly network: ExplorerNetworkInfo
}

interface RetraceResultViewProps {
  readonly result: RetraceResultAndCode
}

const EMPTY_CONTRACTS = new Map<string, ContractData>()
const EMPTY_CONTRACT_INFOS: readonly BackendContractInfo[] = []

const RetraceResultViewFc: React.FC<RetraceResultViewProps> = ({result}) => {
  const transactionInfo = useMemo(() => toTransactionInfo(result.result), [result.result])

  return (
    <div className="retrace-result-container">
      <TransactionDetails
        tx={transactionInfo}
        contracts={EMPTY_CONTRACTS}
        allContracts={EMPTY_CONTRACT_INFOS}
      />
      <RetraceExtraDetails result={result} />

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
