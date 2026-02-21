import path from "node:path"

import {Address} from "@ton/core"
import type React from "react"
import {useEffect, useMemo, useRef, useState} from "react"
import {FiArrowUpRight, FiCheck, FiChevronDown, FiCircle, FiMinus, FiX} from "react-icons/fi"
import {SiIntellijidea, SiRust, SiWebstorm} from "react-icons/si"
import {VscCode} from "react-icons/vsc"

import {
  type MatcherEvent,
  type TestReport,
  TestStatus,
  type Trace,
  ContractData,
  type TransactionInfo,
} from "@acton/shared-ui"
import {
  fmt,
  getTransactionOpcode,
  processTransactions,
  CodeSnippet,
  DataBlock,
  TransactionTree,
  ContractChip,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@acton/shared-ui"

import {useContracts} from "../../hooks/useContracts"

import styles from "./TestDetails.module.css"

interface TestDetailsProps {
  readonly test: TestReport
  readonly trace: Trace | undefined
  readonly projectRoot?: string
}

interface IDEConfig {
  readonly name: string
  readonly icon: React.ReactNode
  readonly getUrl: (test: TestReport) => string
}

interface TraceFeeSummary {
  readonly traceIndex: number
  readonly firstMessageName: string
  readonly transactionCount: number
  readonly transactionFees: readonly bigint[]
  readonly totalGasUsed: bigint
  readonly totalGasFees: bigint
  readonly totalForwardFees: bigint
  readonly totalFees: bigint
}

export const TestDetails: React.FC<TestDetailsProps> = ({test, trace, projectRoot}) => {
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
  const {contracts: backendContracts} = useContracts(contractNames)

  const ides: IDEConfig[] = useMemo(
    () => [
      {
        name: "Cursor",
        icon: <VscCode />,
        getUrl: t => `cursor://file/${t.file_path}:${t.row + 1}:${t.column + 1}`,
      },
      {
        name: "Windsurf",
        icon: <VscCode />,
        getUrl: t => `windsurf://file/${t.file_path}:${t.row + 1}:${t.column + 1}`,
      },
      {
        name: "VS Code",
        icon: <VscCode />,
        getUrl: t => `vscode://file/${t.file_path}:${t.row + 1}:${t.column + 1}`,
      },
      {
        name: "VSCodium",
        icon: <VscCode />,
        getUrl: t => `vscodium://file/${t.file_path}:${t.row + 1}:${t.column + 1}`,
      },
      {
        name: "WebStorm",
        icon: <SiWebstorm />,
        getUrl: t => `webstorm://open?file=${t.file_path}&line=${t.row + 1}&column=${t.column + 1}`,
      },
      {
        name: "RustRover",
        icon: <SiRust />,
        getUrl: t =>
          `rustrover://open?file=${t.file_path}&line=${t.row + 1}&column=${t.column + 1}`,
      },
      {
        name: "IntelliJ",
        icon: <SiIntellijidea />,
        getUrl: t => `idea://open?file=${t.file_path}&line=${t.row + 1}&column=${t.column + 1}`,
      },
    ],
    [],
  )

  const selectedIde = useMemo(() => {
    return ides.find(i => i.name === selectedIdeName) || ides[0]
  }, [ides, selectedIdeName])

  const handleSelectIde = (ide: IDEConfig) => {
    setSelectedIdeName(ide.name)
    localStorage.setItem("selectedIde", ide.name)
    setIsHeaderIDESelectorOpen(false)
    setIsGridIDESelectorOpen(false)
  }

  const errorLocation = useMemo(() => {
    if (test.status !== TestStatus.Failed || !test.details) {
      return {filePath: test.file_path, row: test.row, column: test.column}
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
      return {filePath, row: row - 1, column: col - 2}
    }
    return {filePath: test.file_path, row: test.row, column: test.column}
  }, [test, projectRoot])

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (
        document.activeElement instanceof HTMLInputElement ||
        document.activeElement instanceof HTMLTextAreaElement
      ) {
        return
      }

      if (e.key === ".") {
        globalThis.location.href = selectedIde.getUrl({
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

    globalThis.addEventListener("keydown", handleKeyDown)
    return () => globalThis.removeEventListener("keydown", handleKeyDown)
  }, [test, selectedIde, errorLocation])

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (headerDropdownRef.current && !headerDropdownRef.current.contains(event.target as Node)) {
        setIsHeaderIDESelectorOpen(false)
      }
      if (gridDropdownRef.current && !gridDropdownRef.current.contains(event.target as Node)) {
        setIsGridIDESelectorOpen(false)
      }
    }
    document.addEventListener("mousedown", handleClickOutside)
    return () => document.removeEventListener("mousedown", handleClickOutside)
  }, [])

  const getRelativePath = (path: string) => {
    if (projectRoot && path.startsWith(projectRoot)) {
      const rel = path.slice(projectRoot.length)
      return rel || path
    }
    const parts = path.split("/")
    if (parts.length > 3) {
      return `.../${parts.slice(-3).join("/")}`
    }
    return path
  }

  const formatDuration = (duration: {secs: number; nanos: number}) => {
    const ms = duration.secs * 1000 + duration.nanos / 1_000_000
    if (ms < 1) return `${(ms * 1000).toFixed(0)}µs`
    if (ms < 1000) return `${ms.toFixed(1)}ms`
    return `${(ms / 1000).toFixed(2)}s`
  }

  const compactValue = (value: unknown, maxLen = 180) => {
    try {
      const raw = typeof value === "string" ? value : JSON.stringify(value)
      if (!raw) return "-"
      return raw.length > maxLen ? `${raw.slice(0, maxLen)}...` : raw
    } catch {
      return String(value)
    }
  }

  const transactionCount = useMemo(() => {
    if (!trace) return 0
    return trace.traces.reduce((acc, t) => acc + t.transactions.length, 0)
  }, [trace])

  const parsedTraceTransactions = useMemo((): TransactionInfo[][] => {
    if (!trace) return []
    return trace.traces.map((traceItem, index) => {
      try {
        return processTransactions(traceItem.transactions)
      } catch (error) {
        console.error(`Failed to process trace #${index + 1}`, error)
        return []
      }
    })
  }, [trace])

  const parsedTransactions = useMemo(() => {
    return parsedTraceTransactions[selectedTraceIndex] ?? []
  }, [parsedTraceTransactions, selectedTraceIndex])

  const allContracts = useMemo(() => Object.values(backendContracts), [backendContracts])

  const traceFeeSummaries = useMemo((): TraceFeeSummary[] => {
    const getFirstTraceTransaction = (transactions: readonly TransactionInfo[]) => {
      const roots = transactions.filter(tx => !tx.parent)
      const candidates = roots.length > 0 ? roots : transactions
      if (candidates.length === 0) return

      return [...candidates].sort((a, b) => {
        try {
          const aLt = BigInt(a.lt)
          const bLt = BigInt(b.lt)
          if (aLt < bLt) return -1
          if (aLt > bLt) return 1
          return 0
        } catch {
          return a.lt.localeCompare(b.lt)
        }
      })[0]
    }

    const resolveFirstMessageName = (tx: TransactionInfo | undefined): string => {
      if (!tx) return "unknown"

      const opcode = getTransactionOpcode(tx.transaction)
      if (opcode === undefined) return "empty"

      const targetContract = tx.contractName ? backendContracts[tx.contractName] : undefined
      let opcodeName = targetContract?.abi?.messages.find(it => it.opcode === opcode)?.name

      if (!opcodeName) {
        for (const contract of allContracts) {
          const found = contract.abi?.messages.find(it => it.opcode === opcode)?.name
          if (found) {
            opcodeName = found
            break
          }
        }
      }

      return opcodeName ?? `0x${opcode.toString(16)}`
    }

    return parsedTraceTransactions.map((transactions, traceIndex) => {
      let totalGasUsed = 0n
      let totalGasFees = 0n
      let totalForwardFees = 0n
      let totalFees = 0n
      const transactionFees: bigint[] = []
      const firstTraceTransaction = getFirstTraceTransaction(transactions)
      const firstMessageName = resolveFirstMessageName(firstTraceTransaction)

      for (const tx of transactions) {
        const description = tx.transaction.description
        const computePhase = description.type === "generic" ? description.computePhase : undefined

        const transactionFee = tx.transaction.totalFees.coins
        transactionFees.push(transactionFee)
        totalFees += transactionFee

        if (computePhase?.type === "vm") {
          totalGasUsed += computePhase.gasUsed
          totalGasFees += computePhase.gasFees
        }

        if (tx.transaction.inMessage?.info.type === "internal") {
          totalForwardFees += tx.transaction.inMessage.info.forwardFee
        }
      }

      return {
        traceIndex,
        firstMessageName,
        transactionCount: transactions.length,
        transactionFees,
        totalGasUsed,
        totalGasFees,
        totalForwardFees,
        totalFees,
      }
    })
  }, [allContracts, backendContracts, parsedTraceTransactions])

  const failedTransactions = useMemo(() => {
    if (!test.failed_transactions) return []
    try {
      return processTransactions(test.failed_transactions)
    } catch (error) {
      console.error("Failed to process failed transactions", error)
      return []
    }
  }, [test.failed_transactions])

  const matcherEvents = useMemo<readonly MatcherEvent[]>(() => {
    return test.matcher_events ?? []
  }, [test.matcher_events])
  const visibleMatcherEvents = useMemo<readonly MatcherEvent[]>(() => {
    return matcherEvents.filter(event => event.transaction_query === undefined)
  }, [matcherEvents])

  const contracts = useMemo(() => {
    const map = new Map<string, ContractData>()

    const addContract = (address: Address, name?: string) => {
      const addrStr = address.toString()
      if (map.has(addrStr)) return

      const backendContract = name ? backendContracts[name] : undefined
      map.set(addrStr, {
        displayName: name ?? fmt.formatAddress(addrStr),
        address: address,
        letter: String.fromCodePoint(65 + (map.size % 26)),
        abi: backendContract?.abi,
      } as ContractData)
    }

    if (trace?.wallets) {
      for (const [address, name] of Object.entries(trace.wallets)) {
        try {
          addContract(Address.parse(address), name)
        } catch (error) {
          console.error("Failed to parse wallet address", address, error)
        }
      }
    }

    if (parsedTransactions) {
      for (const tx of parsedTransactions) {
        if (tx.address) {
          addContract(tx.address, tx.contractName)
        }
      }
    }

    if (failedTransactions) {
      for (const tx of failedTransactions) {
        if (tx.address) {
          addContract(tx.address, tx.contractName)
        }
      }
    }

    return map
  }, [parsedTransactions, failedTransactions, trace, backendContracts])

  const normalizeAddress = (addr: string | undefined) => {
    if (!addr) return
    try {
      return Address.parse(addr).toString()
    } catch {
      return addr
    }
  }

  useEffect(() => {
    if (trace) {
      const saved = localStorage.getItem(`selectedTraceIndex:${test.suite_name}::${test.name}`)
      const index = saved ? Number.parseInt(saved, 10) : 0
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

  const handleOpenTraceTransactions = (index: number) => {
    handleSelectTraceIndex(index)
    handleTabChange("transactions")
  }

  if (!test) return

  const getStatusIcon = (status: TestStatus) => {
    switch (status) {
      case TestStatus.Passed: {
        return <FiCheck className={styles.passedIcon} />
      }
      case TestStatus.Failed: {
        return <FiX className={styles.failedIcon} />
      }
      case TestStatus.Skipped: {
        return <FiCircle className={styles.skippedIcon} />
      }
      case TestStatus.Todo: {
        return <FiMinus className={styles.todoIcon} />
      }
      default: {
        return
      }
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
                data={
                  test.failed_transaction_context
                    ? (test.message ?? "expect(actual).toHaveTx(expected)")
                    : (test.detailed_message ?? test.message ?? "No error message available")
                }
                className={styles.errorMessageBlock}
              />

              {failedTransactions.length > 0 && (
                <div className={styles.failedTransactionsSection}>
                  <TransactionTree
                    transactions={failedTransactions}
                    contracts={contracts}
                    allContracts={allContracts}
                  />
                </div>
              )}

              {test.failed_transaction_context && (
                <div className={styles.structuredError}>
                  <div className={styles.structuredErrorTitle}>
                    {test.message?.includes("Unexpected") || test.message?.includes("toNotHaveTx")
                      ? "Unexpected transaction with the following parameters:"
                      : "Cannot find transaction with the following parameters:"}
                  </div>
                  <div className={styles.errorHeader}>
                    <div className={styles.errorRoute}>
                      <span className={styles.routeLabel}>From:</span>
                      {test.failed_transaction_context.from_address ? (
                        <ContractChip
                          address={normalizeAddress(test.failed_transaction_context.from_address)}
                          contracts={contracts}
                        />
                      ) : (
                        <span className={styles.anyAddress}>&lt;any&gt;</span>
                      )}
                    </div>
                    <div className={styles.errorRoute}>
                      <span className={styles.routeLabel}>To:</span>
                      {test.failed_transaction_context.to_address ? (
                        <ContractChip
                          address={normalizeAddress(test.failed_transaction_context.to_address)}
                          contracts={contracts}
                        />
                      ) : (
                        <span className={styles.anyAddress}>&lt;any&gt;</span>
                      )}
                    </div>
                  </div>
                  <div className={styles.errorParams}>
                    {test.failed_transaction_context.params.map(([key, value]) => (
                      <div key={key} className={styles.errorParam}>
                        <span className={styles.paramKey}>{key}:</span>
                        <span className={styles.paramValue}>{value}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}

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
                        {ides.map(ide => (
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

          {visibleMatcherEvents.length > 0 && (
            <div className={styles.matcherSection}>
              <div className={styles.matcherTitle}>
                Jest Matcher Events ({visibleMatcherEvents.length})
              </div>
              <div className={styles.matcherTableWrapper}>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Status</TableHead>
                      <TableHead>Matcher</TableHead>
                      <TableHead>Received</TableHead>
                      <TableHead>Expected</TableHead>
                      <TableHead>Message</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {visibleMatcherEvents.map((event, index) => (
                      <TableRow key={`${event.matcher}:${index}`}>
                        <TableCell>
                          <span
                            className={
                              event.status.toLowerCase() === "passed"
                                ? styles.matcherPassed
                                : event.status.toLowerCase() === "failed"
                                  ? styles.matcherFailed
                                  : styles.matcherOther
                            }
                          >
                            {event.status}
                          </span>
                        </TableCell>
                        <TableCell>{event.matcher}</TableCell>
                        <TableCell>
                          {event.received || <span className={styles.emptyCellValue}>-</span>}
                        </TableCell>
                        <TableCell>
                          {event.expected.length > 0 ? (
                            compactValue(event.expected.join(", "))
                          ) : (
                            <span className={styles.emptyCellValue}>-</span>
                          )}
                        </TableCell>
                        <TableCell>
                          {event.message || <span className={styles.emptyCellValue}>-</span>}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </div>
            </div>
          )}

          {traceFeeSummaries.length > 0 && (
            <div className={styles.traceFeesSection}>
              <div className={styles.traceFeesTitle}>Fee Summary</div>
              <div className={styles.traceFeesTableWrapper}>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Trace</TableHead>
                      <TableHead>Tx Count</TableHead>
                      <TableHead>Gas Used</TableHead>
                      <TableHead>Gas Fee</TableHead>
                      <TableHead>Forward Fee</TableHead>
                      <TableHead>Total Fee</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {traceFeeSummaries.map(summary => (
                      <TableRow
                        key={`${test.suite_name}:${test.name}:trace-fee:${summary.traceIndex}`}
                      >
                        <TableCell>
                          <button
                            type="button"
                            className={styles.traceLinkButton}
                            onClick={() => handleOpenTraceTransactions(summary.traceIndex)}
                            title={`Open Trace ${summary.traceIndex + 1} (${summary.firstMessageName}) in Transactions`}
                          >
                            <span>
                              Trace {summary.traceIndex + 1}
                              <span className={styles.traceMessageSeparator} aria-hidden="true">
                                {" · "}
                              </span>
                              <span className={styles.traceMessageName}>
                                {summary.firstMessageName}
                              </span>
                            </span>
                            <FiArrowUpRight className={styles.traceLinkIcon} aria-hidden="true" />
                          </button>
                        </TableCell>
                        <TableCell className={styles.numericCell}>
                          {summary.transactionCount.toString()}
                        </TableCell>
                        <TableCell className={styles.numericCell}>
                          {summary.totalGasUsed.toString()}
                        </TableCell>
                        <TableCell className={styles.numericCell}>
                          {fmt.formatCurrency(summary.totalGasFees)}
                        </TableCell>
                        <TableCell className={styles.numericCell}>
                          {fmt.formatCurrency(summary.totalForwardFees)}
                        </TableCell>
                        <TableCell className={styles.numericCell}>
                          {fmt.formatCurrency(summary.totalFees)}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </div>
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

        if (!hasVmLog && !hasExecutorLog) return

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
                {ides.map(ide => (
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
