import React, {memo, useMemo} from "react"

import {
  type BackendContractInfo,
  type ContractData,
  DataBlock,
  TransactionDetails,
} from "@acton/shared-ui"

import {toTransactionInfo} from "../../lib/toTransactionInfo"
import type {RetraceResultAndCode} from "../../lib/types"

interface RetraceResultViewProps {
  readonly result: RetraceResultAndCode
}

const EMPTY_CONTRACTS = new Map<string, ContractData>()
const EMPTY_CONTRACT_INFOS: readonly BackendContractInfo[] = []

const RetraceResultViewFc: React.FC<RetraceResultViewProps> = ({result}) => {
  const transactionInfo = useMemo(() => toTransactionInfo(result.result), [result.result])

  return (
    <div>
      <TransactionDetails
        tx={transactionInfo}
        contracts={EMPTY_CONTRACTS}
        allContracts={EMPTY_CONTRACT_INFOS}
      />
      {result.result.emulatedTx.vmLogs && (
        <DataBlock
          copyLabel="VM Logs"
          data={result.result.emulatedTx.vmLogs}
          defaultExpanded={false}
          label="VM Logs"
          maxHeight={420}
          collapsible={true}
          variant="standalone"
          wrap={true}
        />
      )}

      {result.result.emulatedTx.executorLogs && (
        <DataBlock
          copyLabel="Executor Logs"
          data={result.result.emulatedTx.executorLogs}
          defaultExpanded={false}
          label="Executor Logs"
          maxHeight={420}
          collapsible={true}
          variant="standalone"
          wrap={true}
        />
      )}
    </div>
  )
}

export const RetraceResultView = memo(RetraceResultViewFc)
RetraceResultView.displayName = "RetraceResultView"
