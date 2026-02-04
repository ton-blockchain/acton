import path from "node:path"
import { Address } from "@ton/core"
import type React from "react"
import { useEffect, useMemo, useRef, useState } from "react"
import { FiCheck, FiChevronDown, FiCircle, FiMinus, FiX } from "react-icons/fi"
import { SiIntellijidea, SiRust, SiWebstorm } from "react-icons/si"
import { VscCode } from "react-icons/vsc"
import { useContracts } from "../../hooks/useContracts"
import { type TestReport, TestStatus, type Trace } from "../../types"
import type { ContractData } from "../../types/transaction"
import { formatAddress } from "../../utils/format"
import { processTransactions } from "../../utils/transaction"
import { CodeSnippet } from "../common/CodeSnippet/CodeSnippet"
import { DataBlock } from "../common/DataBlock/DataBlock"
import { TransactionTree } from "../TransactionView/TransactionTree/TransactionTree"
import styles from "./TestDetails.module.css"

interface TestDetailsProps {
  readonly test: TestReport
  readonly trace: Trace | null
  readonly projectRoot?: string
}

interface IDEConfig {
  readonly name: string
  readonly icon: React.ReactNode
  readonly getUrl: (test: TestReport) => string
}

export const TestDetails: React.FC<TestDetailsProps> = ({ test, trace, projectRoot }) => {
  const [activeTab, setActiveTab] = useState<"info" | "logs" | "transactions">(() => {
    const saved = localStorage.getItem("activeTab")
    if (saved === "vm" || saved === "executor") return "logs"
    return (saved as "info" | "logs" | "transactions") || "info"
  })
  const [selectedTraceIndex, setSelectedTraceIndex] = useState<number>(() => {
    const saved = localStorage.getItem(`selectedTraceIndex:${test.suite_name}::${test.name}`)
    return saved ? Number.parseInt(saved, 10) : 0
  })
  const [selectedIdeName, setSelectedIdeName] = useState<string | null>(() => {
    return localStorage.getItem("selectedIde")
  })
  const [isHeaderIDESelectorOpen, setIsHeaderIDESelectorOpen] = useState(false)
  const [isGridIDESelectorOpen, setIsGridIDESelectorOpen] = useState(false)
  const headerDropdownRef = useRef<HTMLDivElement | null>(null)
  const gridDropdownRef = useRef<HTMLDivElement | null>(null)

  const contractNames = useMemo(() => trace?.contracts ?? [], [trace])
  const { contracts: backendContracts } = useContracts(contractNames)

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

  const selectedIde = useMemo(() => {
    return ides.find((i) => i.name === selectedIdeName) || ides[0]
  }, [selectedIdeName])

  const handleSelectIde = (ide: IDEConfig) => {
    setSelectedIdeName(ide.name)
    localStorage.setItem("selectedIde", ide.name)
    setIsHeaderIDESelectorOpen(false)
    setIsGridIDESelectorOpen(false)
  }

  const errorLocation = useMemo(() => {
    if (test.status !== TestStatus.Failed || !test.details) {
      return { filePath: test.file_path, row: test.row, column: test.column }
    }

    const parts = test.details.split(":")
    if (parts.length >= 3) {
      const colStr = parts.pop() ?? "0"
      const rowStr = parts.pop() ?? "0"
      const col = Number.parseInt(colStr, 10)
      const row = Number.parseInt(rowStr, 10)
      const filePathRaw = parts.join(":")
      const filePath =
        path.isAbsolute(filePathRaw) || projectRoot === undefined
          ? filePathRaw
          : path.join(projectRoot, filePathRaw)
      return { filePath, row: row - 1, column: col - 2 }
    }
    return { filePath: test.file_path, row: test.row, column: test.column }
  }, [test, projectRoot])

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Don't trigger if user is typing in an input or textarea
      if (
        document.activeElement instanceof HTMLInputElement ||
        document.activeElement instanceof HTMLTextAreaElement
      ) {
        return
      }

      if (e.key === ".") {
        window.location.href = selectedIde.getUrl({
          ...test,
          file_path: errorLocation.filePath,
          row: errorLocation.row,
          column: errorLocation.column,
        })
      }

      if (e.key === "Escape") {
        setIsHeaderIDESelectorOpen(false)
        setIsGridIDESelectorOpen(false)
      }
    }

    window.addEventListener("keydown", handleKeyDown)
    return () => window.removeEventListener("keydown", handleKeyDown)
  }, [test, selectedIde, errorLocation])

  const getRelativePath = (path: string) => {
    if (projectRoot && path.startsWith(projectRoot)) {
      const rel = path.substring(projectRoot.length)
      return rel || path
    }
    const parts = path.split("/")
    if (parts.length > 3) {
      return `.../${parts.slice(-3).join("/")}`
    }
    return path
  }

  const formatDuration = (duration: { secs: number; nanos: number }) => {
    const ms = duration.secs * 1000 + duration.nanos / 1000000
    if (ms < 1) return `${(ms * 1000).toFixed(0)}µs`
    if (ms < 1000) return `${ms.toFixed(1)}ms`
    return `${(ms / 1000).toFixed(2)}s`
  }

  const transactionCount = useMemo(() => {
    if (!trace) return 0
    return trace.traces.reduce((acc, t) => acc + t.transactions.length, 0)
  }, [trace])

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

  const handleTabChange = (tab: "info" | "logs" | "transactions") => {
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

  const renderTabContent = () => {
    if (activeTab === "info") {
      return (
        <div className={styles.infoTab}>
          <div className={styles.infoGrid}>
            <div className={styles.infoItem}>
              <div className={styles.infoLabel}>Status</div>
              <div className={`${styles.infoValue} ${styles[test.status.toLowerCase()]}`}>
                {test.status}
              </div>
            </div>
            <div className={styles.infoItem}>
              <div className={styles.infoLabel}>Suite</div>
              <div className={styles.infoValue}>{test.suite_name}</div>
            </div>
            <div className={styles.infoItem}>
              <div className={styles.infoLabel}>Location</div>
              <div className={styles.infoValue}>
                <span title={errorLocation.filePath}>
                  {getRelativePath(errorLocation.filePath)}:{errorLocation.row + 1}:
                  {errorLocation.column + 1}
                </span>
              </div>
            </div>
            <div className={styles.infoItem}>
              <div className={styles.infoLabel}>Stats</div>
              <div className={styles.infoValue}>
                {formatDuration(test.duration)} • {transactionCount} transactions
              </div>
            </div>
          </div>

          {test.status === TestStatus.Failed && (
            <div className={styles.errorSection}>
              <div className={styles.errorTitle}>Error Message</div>
              <DataBlock
                data={test.message ?? "No error message available"}
                className={styles.errorMessageBlock}
              />

              <CodeSnippet
                filePath={errorLocation.filePath}
                line={errorLocation.row + 1}
                projectRoot={projectRoot}
                ideOpener={
                  <div className={styles.gridIdeSelector} ref={gridDropdownRef}>
                    <a
                      href={selectedIde.getUrl({
                        ...test,
                        file_path: errorLocation.filePath,
                        row: errorLocation.row,
                        column: errorLocation.column,
                      })}
                      className={styles.gridIdeQuickLink}
                      title={`Open in ${selectedIde.name} (or press \`.\`)`}
                    >
                      {selectedIde.icon}
                    </a>
                    <button
                      type="button"
                      className={`${styles.gridIdeTrigger} ${isGridIDESelectorOpen ? styles.active : ""}`}
                      onClick={() => setIsGridIDESelectorOpen(!isGridIDESelectorOpen)}
                      title="Change IDE"
                    >
                      <FiChevronDown />
                    </button>
                    {isGridIDESelectorOpen && (
                      <div className={styles.gridIdeDropdown}>
                        {ides.map((ide) => (
                          <button
                            key={ide.name}
                            type="button"
                            className={`${styles.ideItem} ${selectedIdeName === ide.name ? styles.selectedIde : ""}`}
                            onClick={() => handleSelectIde(ide)}
                          >
                            <span className={styles.ideIcon}>{ide.icon}</span>
                            <span className={styles.ideName}>{ide.name}</span>
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                }
              />
            </div>
          )}
        </div>
      )
    }

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
        const hasVmLog = tx.vm_log_diff && tx.vm_log_diff.trim().length > 0
        const hasExecutorLog = tx.executor_logs && tx.executor_logs.trim().length > 0

        if (!hasVmLog && !hasExecutorLog) return null

        return (
          <div key={tx.lt} className={styles.txLogs}>
            <div className={styles.txHeader}>
              <span>Transaction #{idx + 1}</span>
            </div>
            {hasExecutorLog && (
              <div className={styles.logSection}>
                <div className={styles.logSectionTitle}>Executor Log</div>
                <DataBlock data={tx.executor_logs} />
              </div>
            )}
            {hasVmLog && (
              <div className={styles.logSection}>
                <div className={styles.logSectionTitle}>VM Log</div>
                <DataBlock data={tx.vm_log_diff} />
              </div>
            )}
          </div>
        )
      })
      .filter(Boolean)

    if (logs.length === 0) {
      return <div className={styles.empty}>No logs for this trace</div>
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

          <div className={styles.ideSelectorContainer} ref={headerDropdownRef}>
            <a
              href={selectedIde.getUrl(test)}
              className={styles.ideQuickLink}
              title={`Open in ${selectedIde.name} (or press \`.\`)`}
            >
              {selectedIde.icon}
            </a>
            <button
              type="button"
              className={`${styles.ideTrigger} ${isHeaderIDESelectorOpen ? styles.active : ""}`}
              onClick={() => setIsHeaderIDESelectorOpen(!isHeaderIDESelectorOpen)}
              title="Change IDE"
            >
              <FiChevronDown />
            </button>

            {isHeaderIDESelectorOpen && (
              <div className={styles.ideDropdown}>
                {ides.map((ide) => (
                  <button
                    key={ide.name}
                    type="button"
                    className={`${styles.ideItem} ${selectedIdeName === ide.name ? styles.selectedIde : ""}`}
                    onClick={() => handleSelectIde(ide)}
                  >
                    <span className={styles.ideIcon}>{ide.icon}</span>
                    <span className={styles.ideName}>{ide.name}</span>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>

      <div className={styles.tabsContainer}>
        <div className={styles.tabsList}>
          <button
            type="button"
            className={`${styles.tabTrigger} ${activeTab === "info" ? styles.activeTabTrigger : ""}`}
            onClick={() => handleTabChange("info")}
          >
            Info
          </button>
          <button
            type="button"
            className={`${styles.tabTrigger} ${activeTab === "transactions" ? styles.activeTabTrigger : ""}`}
            onClick={() => handleTabChange("transactions")}
          >
            Transactions
          </button>
          <button
            type="button"
            className={`${styles.tabTrigger} ${activeTab === "logs" ? styles.activeTabTrigger : ""}`}
            onClick={() => handleTabChange("logs")}
          >
            Logs
          </button>
        </div>
      </div>

      {activeTab !== "info" && trace && trace.traces.length > 1 && (
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

      <div className={styles.tabContent}>{renderTabContent()}</div>
    </div>
  )
}
