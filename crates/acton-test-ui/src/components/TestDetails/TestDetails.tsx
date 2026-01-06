import { Address } from "@ton/core"
import type React from "react"
import { useEffect, useMemo, useState } from "react"
import { useContracts } from "../../hooks/useContracts"
import type { TestReport, Trace } from "../../types"
import type { ContractData } from "../../types/transaction"
import { formatAddress } from "../../utils/format"
import { processTransactions } from "../../utils/transaction"
import { TransactionTree } from "../TransactionView/TransactionTree/TransactionTree"
import styles from "./TestDetails.module.css"

interface TestDetailsProps {
  readonly test: TestReport
  readonly trace: Trace | null
}

export const TestDetails: React.FC<TestDetailsProps> = ({ test, trace }) => {
  const [activeTab, setActiveTab] = useState<"vm" | "executor" | "transactions">("transactions")
  const [selectedTraceIndex, setSelectedTraceIndex] = useState<number>(0)

  const contractNames = useMemo(() => trace?.contracts ?? [], [trace])
  const { contracts: backendContracts, loading: contractsLoading } = useContracts(contractNames)

  useEffect(() => {
    setSelectedTraceIndex(0)
  }, [trace])

  const parsedTransactions = useMemo(() => {
    if (!trace || !trace.traces[selectedTraceIndex]) return []
    try {
      return processTransactions(trace.traces[selectedTraceIndex].transactions)
    } catch (e) {
      console.error("Failed to process trace", e)
      return []
    }
  }, [trace, selectedTraceIndex])

  const contracts = useMemo(() => {
    const map = new Map<string, ContractData>()
    if (!trace) return map

    // 1. Add all known wallets from treasury/external
    if (trace.wallets) {
      for (const [address, name] of Object.entries(trace.wallets)) {
        try {
          map.set(address, {
            displayName: name,
            address: Address.parse(address),
            letter: String.fromCharCode(65 + (map.size % 26)),
          })
        } catch (e) {
          console.error("Failed to parse wallet address", address, e)
        }
      }
    }

    // 2. Add contracts from transactions
    if (parsedTransactions) {
      for (const tx of parsedTransactions) {
        if (tx.address && !map.has(tx.address.toString())) {
          const addressStr = tx.address.toString()
          const backendContract = tx.contractName ? backendContracts[tx.contractName] : undefined
          map.set(addressStr, {
            displayName: tx.contractName ?? formatAddress(addressStr),
            address: tx.address,
            letter: String.fromCharCode(65 + (map.size % 26)),
            abi: backendContract?.abi,
          } as ContractData)
        }
      }
    }
    return map
  }, [parsedTransactions, trace, backendContracts])

  if (!test) return null

  const renderLogs = () => {
    if (!trace) return <div className={styles.empty}>No trace data available</div>
    const currentTraceList = trace.traces[selectedTraceIndex]
    if (!currentTraceList) return <div className={styles.empty}>Trace not found</div>

    if (activeTab === "transactions") {
      if (parsedTransactions.length === 0) {
        return <div className={styles.empty}>No transaction data available for this trace</div>
      }
      return (
        <div className={styles.treeWrapper}>
          <TransactionTree transactions={parsedTransactions} contracts={contracts} />
        </div>
      )
    }

    const logs = currentTraceList.transactions
      .map((tx, idx) => {
        const content = activeTab === "vm" ? tx.vm_log_diff : tx.executor_logs
        if (!content) return null

        return (
          <div key={idx} className={styles.txLogs}>
            <div className={styles.txHeader}>
              <span>Transaction #{idx + 1}</span>
              {tx.dest_contract_info && (
                <span className={styles.txDest}>Dest: {tx.dest_contract_info}</span>
              )}
            </div>
            <pre className={styles.logContent}>{content}</pre>
          </div>
        )
      })
      .filter(Boolean)

    if (logs.length === 0) {
      return (
        <div className={styles.empty}>
          No {activeTab === "vm" ? "VM" : "executor"} logs for this trace
        </div>
      )
    }

    return logs
  }

  return (
    <div className={styles.details}>
      <div className={styles.header}>
        <div className={styles.titleInfo}>
          <span className={styles.suiteName}>{test.suite_name} / </span>
          <span className={styles.testName}>{test.name}</span>
        </div>
        <div className={`${styles.status} ${styles[test.status.toLowerCase()]}`}>{test.status}</div>
      </div>

      {test.message && (
        <div className={styles.errorMessage}>
          <pre>{test.message}</pre>
        </div>
      )}

      <div className={styles.tabs}>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "transactions" ? styles.activeTab : ""}`}
          onClick={() => setActiveTab("transactions")}
        >
          Transactions
        </button>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "vm" ? styles.activeTab : ""}`}
          onClick={() => setActiveTab("vm")}
        >
          VM Log
        </button>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "executor" ? styles.activeTab : ""}`}
          onClick={() => setActiveTab("executor")}
        >
          Executor Logs
        </button>
      </div>

      {trace && trace.traces.length > 1 && (
        <div className={styles.traceSelector}>
          {trace.traces.map((_, index) => (
            <button
              key={index}
              type="button"
              className={`${styles.traceTab} ${selectedTraceIndex === index ? styles.activeTraceTab : ""}`}
              onClick={() => setSelectedTraceIndex(index)}
            >
              Trace #{index + 1}
            </button>
          ))}
        </div>
      )}

      <div className={styles.tabContent}>{renderLogs()}</div>
    </div>
  )
}
