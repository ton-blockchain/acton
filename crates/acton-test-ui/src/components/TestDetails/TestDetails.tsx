import path from "node:path"

import {Address} from "@ton/core"
import type React from "react"
import {useEffect, useMemo, useRef, useState} from "react"
import {FiArrowUpRight, FiCheck, FiChevronDown, FiCircle, FiMinus, FiX} from "react-icons/fi"
import {SiIntellijidea, SiRust, SiWebstorm} from "react-icons/si"
import {VscCode} from "react-icons/vsc"

import {
  type TestReport,
  type TestExecutionLogs,
  TestStatus,
  type Trace,
  ContractData,
  type FailedMessage,
  type TransactionInfo,
} from "@acton/shared-ui"
import {
  applyParsedBodies,
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
  readonly traceName: string
  readonly firstMessageName: string
  readonly transactionCount: number
  readonly transactionFees: readonly bigint[]
  readonly totalGasUsed: bigint
  readonly totalGasFees: bigint
  readonly totalForwardFees: bigint
  readonly totalFees: bigint
}

const formatTraceName = (name: string | undefined, index: number): string => {
  const trimmed = name?.trim()
  if (trimmed && trimmed.length > 0) {
    return trimmed
  }
  return `Trace #${index + 1}`
}

const isExternalMessageNotAcceptedError = (error: string): boolean => {
  const normalized = error.toLowerCase()
  const mentionsExternal = normalized.includes("external")
  const mentionsRejectedExternal =
    normalized.includes("not accepted") ||
    normalized.includes("cannot apply external") ||
    normalized.includes("did not accept")
  return mentionsExternal && mentionsRejectedExternal
}

const MISSING_VM_LOG_HINT = [
  "No VM logs were collected for this trace.",
  "Re-run with --verbose flag",
].join("\n")

const hasNonEmptyLog = (value: string | undefined): boolean => (value ?? "").trim().length > 0

const getStatusDescription = (test: TestReport): string | undefined => {
  if (test.status === TestStatus.Todo) {
    return test.details ?? "TODO"
  }

  if (test.status === TestStatus.Skipped) {
    return test.details
  }

  return undefined
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
  const [executionLogs, setExecutionLogs] = useState<TestExecutionLogs | undefined>()
  const [isLoadingExecutionLogs, setIsLoadingExecutionLogs] = useState(false)
  const [isHeaderIDESelectorOpen, setIsHeaderIDESelectorOpen] = useState(false)
  const [isGridIDESelectorOpen, setIsGridIDESelectorOpen] = useState(false)
  const headerDropdownRef = useRef<HTMLDivElement | null>(null)
  const gridDropdownRef = useRef<HTMLDivElement | null>(null)

  const contractNames = useMemo(() => {
    const names = new Set<string>(trace?.contracts ?? [])

    for (const traceItem of trace?.traces ?? []) {
      for (const transaction of traceItem.transactions) {
        if (transaction.dest_contract_info) {
          names.add(transaction.dest_contract_info)
        }
      }
    }

    for (const transaction of test.failed_transactions ?? []) {
      if (transaction.dest_contract_info) {
        names.add(transaction.dest_contract_info)
      }
    }

    return [...names]
  }, [trace, test.failed_transactions])
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

  useEffect(() => {
    const controller = new AbortController()
    const params = new URLSearchParams({
      file_path: test.file_path,
      name: test.name,
      row: test.row.toString(),
      column: test.column.toString(),
    })

    setIsLoadingExecutionLogs(true)
    setExecutionLogs(undefined)

    void fetch(`/api/test-logs?${params.toString()}`, {signal: controller.signal})
      .then(async res => {
        if (!res.ok) {
          throw new Error(`Failed to fetch test logs: ${res.status}`)
        }
        return (await res.json()) as TestExecutionLogs
      })
      .then(data => {
        setExecutionLogs(data)
        setIsLoadingExecutionLogs(false)
      })
      .catch(error => {
        if (error instanceof Error && error.name === "AbortError") {
          return
        }

        console.error("Failed to fetch test logs", error)
        setExecutionLogs({})
        setIsLoadingExecutionLogs(false)
      })

    return () => controller.abort()
  }, [test.file_path, test.name, test.row, test.column])

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

  const parsedTraceTransactionsWithBodies = useMemo((): TransactionInfo[][] => {
    return parsedTraceTransactions.map(transactions =>
      applyParsedBodies(transactions, backendContracts),
    )
  }, [backendContracts, parsedTraceTransactions])

  const parsedTransactions = useMemo(() => {
    return parsedTraceTransactionsWithBodies[selectedTraceIndex] ?? []
  }, [parsedTraceTransactionsWithBodies, selectedTraceIndex])

  const allContracts = useMemo(() => Object.values(backendContracts), [backendContracts])
  const statusDescription = getStatusDescription(test)

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

    return parsedTraceTransactionsWithBodies.map((transactions, traceIndex) => {
      const traceName = formatTraceName(trace?.traces[traceIndex]?.name, traceIndex)
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
        traceName,
        firstMessageName,
        transactionCount: transactions.length,
        transactionFees,
        totalGasUsed,
        totalGasFees,
        totalForwardFees,
        totalFees,
      }
    })
  }, [allContracts, backendContracts, parsedTraceTransactionsWithBodies, trace])

  const failedTransactions = useMemo(() => {
    if (!test.failed_transactions) return []
    try {
      return applyParsedBodies(processTransactions(test.failed_transactions), backendContracts)
    } catch (error) {
      console.error("Failed to process failed transactions", error)
      return []
    }
  }, [backendContracts, test.failed_transactions])

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
        compilerAbi: backendContract?.compiler_abi,
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

  const renderFailedMessages = (failedMessages: readonly FailedMessage[]) => {
    const isSingleFailedMessage = failedMessages.length === 1

    return failedMessages.map((failedMessage, index) => {
      const hasVmLog = hasNonEmptyLog(failedMessage.vm_log_diff)
      const hasExecutorLog = hasNonEmptyLog(failedMessage.executor_logs)
      const showExternalNotAcceptedTitle =
        isSingleFailedMessage && isExternalMessageNotAcceptedError(failedMessage.error)

      return (
        <div key={`failed-message-${index}`} className={styles.txLogs}>
          {showExternalNotAcceptedTitle && (
            <div className={styles.errorTitle}>External message was not accepted</div>
          )}
          {!isSingleFailedMessage && (
            <div className={styles.txHeader}>
              <span>Failed Message #{index + 1}</span>
            </div>
          )}
          <div className={styles.logSection}>
            <div className={styles.logSectionTitle}>Error</div>
            <DataBlock data={failedMessage.error} />
          </div>
          {failedMessage.vm_exit_code !== undefined && (
            <div className={styles.logSection}>
              <div className={styles.logSectionTitle}>VM Exit Code</div>
              <DataBlock data={failedMessage.vm_exit_code.toString()} />
            </div>
          )}
          {hasExecutorLog && (
            <div className={styles.logSection}>
              <div className={styles.logSectionTitle}>Executor Log</div>
              <DataBlock data={failedMessage.executor_logs ?? ""} />
            </div>
          )}
          <div className={styles.logSection}>
            <div className={styles.logSectionTitle}>VM Log</div>
            <DataBlock data={hasVmLog ? (failedMessage.vm_log_diff ?? "") : MISSING_VM_LOG_HINT} />
          </div>
        </div>
      )
    })
  }

  const renderTestExecutionLogs = () => {
    const hasStdout = hasNonEmptyLog(executionLogs?.stdout)
    const hasStderr = hasNonEmptyLog(executionLogs?.stderr)
    const hasVmLog = hasNonEmptyLog(executionLogs?.vm_log_diff)

    const summaryKinds = [
      hasStdout ? "stdout" : undefined,
      hasStderr ? "stderr" : undefined,
    ].filter(Boolean)
    const hasAnyLogs = hasStdout || hasStderr || hasVmLog

    if (!hasAnyLogs && !isLoadingExecutionLogs) {
      return
    }

    return (
      <details className={styles.infoLogsSection}>
        <summary className={styles.infoLogsSummary}>
          <span className={styles.infoLogsTitle}>Test Logs</span>
          {(isLoadingExecutionLogs || summaryKinds.length > 0) && (
            <span className={styles.infoLogsMeta}>
              {isLoadingExecutionLogs ? "loading..." : summaryKinds.join(" · ")}
            </span>
          )}
        </summary>
        {!isLoadingExecutionLogs && (
          <div className={styles.infoLogsContent}>
            {hasStdout && (
              <div className={styles.logSection}>
                <div className={styles.logSectionTitle}>Stdout</div>
                <DataBlock data={executionLogs?.stdout ?? ""} />
              </div>
            )}
            {hasStderr && (
              <div className={styles.logSection}>
                <div className={styles.logSectionTitle}>Stderr</div>
                <DataBlock data={executionLogs?.stderr ?? ""} />
              </div>
            )}
            {hasVmLog && (
              <div className={styles.logSection}>
                <DataBlock data={executionLogs?.vm_log_diff ?? ""} />
              </div>
            )}
          </div>
        )}
      </details>
    )
  }

  const renderTabContent = () => {
    if (activeTab === "info") {
      return (
        <div className={styles.infoTab}>
          <div className={styles.infoGrid}>
            <div className={styles.infoItem}>
              <div className={styles.infoLabel}>Status</div>
              <div className={styles.infoValueGroup}>
                <div className={`${styles.infoValue} ${styles[test.status.toLowerCase()]}`}>
                  {test.status}
                </div>
                {statusDescription && <div className={styles.statusDescription}>{statusDescription}</div>}
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
                            title={`Open ${summary.traceName} (${summary.firstMessageName}) in Transactions`}
                          >
                            <span>
                              {summary.traceName}
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

          {renderTestExecutionLogs()}
        </div>
      )
    }

    if (activeTab === "logs") {
      const currentTraceList = trace?.traces[selectedTraceIndex]
      const transactionLogs =
        currentTraceList?.transactions
          .map((tx, idx) => {
            const hasVmLog = hasNonEmptyLog(tx.vm_log_diff)
            const hasExecutorLog = hasNonEmptyLog(tx.executor_logs)

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
                <div className={styles.logSection}>
                  <div className={styles.logSectionTitle}>VM Log</div>
                  <DataBlock data={hasVmLog ? tx.vm_log_diff : MISSING_VM_LOG_HINT} />
                </div>
              </div>
            )
          })
          .filter(Boolean) ?? []
      const failedMessageLogs = currentTraceList
        ? renderFailedMessages(currentTraceList.failed_messages ?? [])
        : []
      const logs = [...transactionLogs, ...failedMessageLogs]

      if (logs.length === 0) {
        return (
          <div className={styles.txLogs}>
            <div className={styles.logSection}>
              <div className={styles.logSectionTitle}>VM Log</div>
              <DataBlock data={MISSING_VM_LOG_HINT} />
            </div>
          </div>
        )
      }

      return logs
    }

    if (!trace) return <div className={styles.empty}>No trace data available</div>
    const currentTraceList = trace.traces[selectedTraceIndex]
    if (!currentTraceList) return <div className={styles.empty}>Trace not found</div>

    if (activeTab === "transactions") {
      const failedMessages = currentTraceList.failed_messages ?? []
      if (parsedTransactions.length === 0) {
        if (failedMessages.length === 0) {
          return <div className={styles.empty}>No transaction data available for this trace</div>
        }
        return <div>{renderFailedMessages(failedMessages)}</div>
      }
      return (
        <>
          <div className={styles.treeWrapper}>
            <TransactionTree
              transactions={parsedTransactions}
              contracts={contracts}
              allContracts={allContracts}
            />
          </div>
          {failedMessages.length > 0 && <div>{renderFailedMessages(failedMessages)}</div>}
        </>
      )
    }
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
          {trace.traces.map((traceItem, index) => (
            <button
              key={`${trace.name}-${index}`}
              type="button"
              className={`${styles.traceTab} ${selectedTraceIndex === index ? styles.activeTraceTab : ""}`}
              onClick={() => handleSelectTraceIndex(index)}
            >
              {formatTraceName(traceItem.name, index)}
            </button>
          ))}
        </div>
      )}

      <div className={styles.tabContent}>{renderTabContent()}</div>
    </div>
  )
}
