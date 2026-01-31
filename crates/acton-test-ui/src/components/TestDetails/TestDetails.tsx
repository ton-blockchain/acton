import { 
  Address, 
  TestStatus, 
  type TestReport, 
  type Trace, 
  type ContractData,
  formatAddress,
  processTransactions,
  TransactionTree
} from "@acton/ui-shared"
import type React from "react"
import { useEffect, useMemo, useRef, useState } from "react"
import { FiCheck, FiCircle, FiCode, FiMinus, FiX } from "react-icons/fi"
import { SiIntellijidea, SiRust, SiWebstorm } from "react-icons/si"
import { VscCode } from "react-icons/vsc"
import { useContracts } from "../../hooks/useContracts"
import { DataBlock } from "../common/DataBlock/DataBlock"
import styles from "./TestDetails.module.css"

interface TestDetailsProps {
  readonly test: TestReport
  readonly trace: Trace | null
}

interface IDEConfig {
  readonly name: string
  readonly icon: React.ReactNode
  readonly getUrl: (test: TestReport) => string
}

export const TestDetails: React.FC<TestDetailsProps> = ({ test, trace }) => {
  const [activeTab, setActiveTab] = useState<"vm" | "executor" | "transactions">(() => {
    const saved = localStorage.getItem("activeTab")
    return (saved as "vm" | "executor" | "transactions") || "transactions"
  })
  const [selectedTraceIndex, setSelectedTraceIndex] = useState<number>(() => {
    const saved = localStorage.getItem(`selectedTraceIndex:${test.suite_name}::${test.name}`)
    return saved ? Number.parseInt(saved, 10) : 0
  })
  const [isIDESelectorOpen, setIsIDESelectorOpen] = useState(false)
  const dropdownRef = useRef<HTMLDivElement>(null)

  const contractNames = useMemo(() => trace?.contracts ?? [], [trace])
  const { contracts: backendContracts } = useContracts(contractNames)

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsIDESelectorOpen(false)
      }
    }
    document.addEventListener("mousedown", handleClickOutside)
    return () => document.removeEventListener("mousedown", handleClickOutside)
  }, [])

  useEffect(() => {
    if (trace) {
      const saved = localStorage.getItem(`selectedTraceIndex:${test.suite_name}::${test.name}`)
      const index = saved ? Number.parseInt(saved, 10) : 0
      // Ensure the saved index is valid for the current trace
      if (index < trace.traces.length) {
        setSelectedTraceIndex(index)
      } else {
        setSelectedTraceIndex(0)
      }
    }
  }, [trace, test.suite_name, test.name])

  const handleSelectTraceIndex = (index: number) => {
    setSelectedTraceIndex(index)
    localStorage.setItem(`selectedTraceIndex:${test.suite_name}::${test.name}`, index.toString())
  }

  const handleTabChange = (tab: "vm" | "executor" | "transactions") => {
    setActiveTab(tab)
    localStorage.setItem("activeTab", tab)
  }

  const parsedTransactions = useMemo(() => {
    if (!trace || !trace.traces[selectedTraceIndex]) return []
    try {
      return processTransactions(trace.traces[selectedTraceIndex].transactions)
    } catch (e) {
      console.error("Failed to process trace", e)
      return []
    }
  }, [trace, selectedTraceIndex])

  const ides: IDEConfig[] = [
    {
      name: "Cursor",
      icon: <VscCode />,
      getUrl: (t) => `cursor://file/${t.file_path}:${t.row + 1}:${t.column + 1}`,
    },
    {
      name: "Windsurf",
      icon: <VscCode />,
      getUrl: (t) => `windsurf://file/${t.file_path}:${t.row + 1}:${t.column + 1}`,
    },
    {
      name: "VS Code",
      icon: <VscCode />,
      getUrl: (t) => `vscode://file/${t.file_path}:${t.row + 1}:${t.column + 1}`,
    },
    {
      name: "VSCodium",
      icon: <VscCode />,
      getUrl: (t) => `vscodium://file/${t.file_path}:${t.row + 1}:${t.column + 1}`,
    },
    {
      name: "WebStorm",
      icon: <SiWebstorm />,
      getUrl: (t) => `webstorm://open?file=${t.file_path}&line=${t.row + 1}&column=${t.column + 1}`,
    },
    {
      name: "RustRover",
      icon: <SiRust />,
      getUrl: (t) =>
        `rustrover://open?file=${t.file_path}&line=${t.row + 1}&column=${t.column + 1}`,
    },
    {
      name: "IntelliJ",
      icon: <SiIntellijidea />,
      getUrl: (t) => `idea://open?file=${t.file_path}&line=${t.row + 1}&column=${t.column + 1}`,
    },
  ]

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

  const allContracts = [...Object.values(backendContracts)]

  if (!test) return null

  const getStatusIcon = (status: TestStatus) => {
    switch (status) {
      case TestStatus.Passed:
        return <FiCheck className={styles.passedIcon} />
      case TestStatus.Failed:
        return <FiX className={styles.failedIcon} />
      case TestStatus.Skipped:
        return <FiCircle className={styles.skippedIcon} />
      case TestStatus.Todo:
        return <FiMinus className={styles.todoIcon} />
      default:
        return null
    }
  }

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
          <TransactionTree
            transactions={parsedTransactions}
            contracts={contracts}
            allContracts={allContracts}
          />
        </div>
      )
    }

    const logs = currentTraceList.transactions
      .map((tx, idx) => {
        const content = activeTab === "vm" ? tx.vm_log_diff : tx.executor_logs
        if (!content) return null

        return (
          <div key={tx.lt} className={styles.txLogs}>
            <div className={styles.txHeader}>
              <span>Transaction #{idx + 1}</span>
              {tx.dest_contract_info && (
                <span className={styles.txDest}>Dest: {tx.dest_contract_info}</span>
              )}
            </div>
            <DataBlock data={content} />
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
          <span className={styles.statusIcon}>{getStatusIcon(test.status)}</span>
          <span className={styles.suiteName}>{test.suite_name} / </span>
          <span className={styles.testName}>{test.name}</span>

          <div className={styles.ideSelectorContainer} ref={dropdownRef}>
            <button
              type="button"
              className={`${styles.ideTrigger} ${isIDESelectorOpen ? styles.active : ""}`}
              onClick={() => setIsIDESelectorOpen(!isIDESelectorOpen)}
              title="Open in IDE"
            >
              <FiCode />
            </button>

            {isIDESelectorOpen && (
              <div className={styles.ideDropdown}>
                {ides.map((ide) => (
                  <a
                    key={ide.name}
                    href={ide.getUrl(test)}
                    className={styles.ideItem}
                    onClick={() => setIsIDESelectorOpen(false)}
                  >
                    <span className={styles.ideIcon}>{ide.icon}</span>
                    <span className={styles.ideName}>{ide.name}</span>
                  </a>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>

      {test.message && (
        <div className={styles.errorMessage}>
          <DataBlock data={test.message} />
        </div>
      )}

      <div className={styles.tabs}>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "transactions" ? styles.activeTab : ""}`}
          onClick={() => handleTabChange("transactions")}
        >
          Transactions
        </button>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "vm" ? styles.activeTab : ""}`}
          onClick={() => handleTabChange("vm")}
        >
          VM Log
        </button>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "executor" ? styles.activeTab : ""}`}
          onClick={() => handleTabChange("executor")}
        >
          Executor Logs
        </button>
      </div>

      {trace && trace.traces.length > 1 && (
        <div className={styles.traceSelector}>
          {trace.traces.map((_t, index) => (
            <button
              key={`${trace.name}-${index}`}
              type="button"
              className={`${styles.traceTab} ${selectedTraceIndex === index ? styles.activeTraceTab : ""}`}
              onClick={() => handleSelectTraceIndex(index)}
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
