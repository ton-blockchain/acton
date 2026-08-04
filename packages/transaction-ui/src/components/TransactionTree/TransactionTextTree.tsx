import type {OutAction} from "@ton/core"
import type React from "react"
import {useEffect, useMemo, useState} from "react"
import {Button, Checkbox, InlineButton, Input, ParsedValueView, Select} from "@acton/ui"
import {
  Braces,
  ChevronDown,
  ChevronRight,
  ChevronsDownUp,
  ChevronsUpDown,
  Search,
} from "lucide-react"

import type {BackendContractInfo} from "../../model/backend"
import type {
  ContractData,
  ParsedTransactionBody,
  ParsedValue,
  TransactionInfo,
} from "../../model/transaction"
import * as fmt from "../../lib/format"
import {
  decodeMessageBody,
  decodeTransactionMessageBody,
  resolveMessageOpcodeName,
} from "../../lib/messageBody"
import {
  getTransactionActionPhase,
  getTransactionComputePhase,
  getTransactionOpcode,
  getTransactionSourceLabel,
  getTransactionTriggerLabel,
  isTransactionSuccessful,
  resolveTransactionOpcodeName,
} from "../../lib/transaction"

import styles from "./TransactionTextTree.module.css"

interface TransactionTextTreeProps {
  readonly transactions: readonly TransactionInfo[]
  readonly contracts: Map<string, ContractData>
  readonly compilerAbisByCodeHash?: ReadonlyMap<string, ContractData["abi"]>
  readonly allContracts: readonly BackendContractInfo[]
  readonly selectedTransactionId?: string
  readonly highlightedTransactionIds?: ReadonlySet<string>
  readonly onContractClick?: (address: string) => void
  readonly onTransactionSelect: (tx: TransactionInfo) => void
  readonly renderTransactionDetails: (tx: TransactionInfo) => React.ReactNode
}

type SearchCategory = "all" | "message" | "from" | "to" | "contract" | "status"

interface TraceParticipant {
  readonly address?: string
  readonly label: string
}

interface TraceNode {
  readonly tx: TransactionInfo
  readonly type: string
  readonly source: TraceParticipant
  readonly destination: TraceParticipant
  readonly messageName: string
  readonly parsedBody?: ParsedTransactionBody
  readonly params?: string
  readonly value?: string
  readonly gas?: bigint
  readonly success: boolean
  readonly status: string
  readonly children: readonly TraceNode[]
}

interface TraceMetaRow {
  readonly id: string
  readonly type: "PHASE" | "ACTION" | "EVENT" | "EXT-OUT" | "ERROR"
  readonly content: React.ReactNode
  readonly failed?: boolean
}

interface TraceNodeRowProps {
  readonly node: TraceNode
  readonly depth: number
  readonly ancestorHasNext: readonly boolean[]
  readonly isLast: boolean
  readonly expandedIds: ReadonlySet<string>
  readonly expandedBodyIds: ReadonlySet<string>
  readonly forceExpanded: boolean
  readonly showGas: boolean
  readonly showPhases: boolean
  readonly showActions: boolean
  readonly selectedTransactionId?: string
  readonly highlightedTransactionIds?: ReadonlySet<string>
  readonly contracts: Map<string, ContractData>
  readonly compilerAbisByCodeHash?: ReadonlyMap<string, ContractData["abi"]>
  readonly allContracts: readonly BackendContractInfo[]
  readonly onToggle: (id: string) => void
  readonly onBodyToggle: (id: string) => void
  readonly onContractClick?: (address: string) => void
  readonly onTransactionSelect: (tx: TransactionInfo) => void
  readonly renderTransactionDetails: (tx: TransactionInfo) => React.ReactNode
}

const INLINE_VALUE_LIMIT = 64
const INLINE_COLLECTION_LIMIT = 2

function shorten(value: string, limit = 22): string {
  if (value.length <= limit) return value
  const side = Math.max(4, Math.floor((limit - 1) / 2))
  return `${value.slice(0, side)}…${value.slice(-side)}`
}

function participantFromAddress(
  address: {toString(): string} | null | undefined,
  contracts: Map<string, ContractData>,
  fallback: string,
): TraceParticipant {
  if (!address) return {label: fallback}

  const value = address.toString()
  const contract = contracts.get(value)
  const displayName = contract?.displayName
  return {
    address: value,
    label: displayName && displayName !== "Unknown Contract" ? displayName : shorten(value),
  }
}

function formatParsedValue(value: ParsedValue, depth = 0): string {
  if (depth > 2) return "…"

  switch (value.kind) {
    case "null":
      return "null"
    case "void":
      return "void"
    case "address":
      return shorten(value.value, 18)
    case "boolean":
      return value.value ? "true" : "false"
    case "scalar":
      return shorten(value.value, 24)
    case "array": {
      const items = value.items
        .slice(0, INLINE_COLLECTION_LIMIT)
        .map(item => formatParsedValue(item, depth + 1))
      if (value.items.length > INLINE_COLLECTION_LIMIT)
        items.push(`+${value.items.length - items.length}`)
      return `[${items.join(", ")}]`
    }
    case "object": {
      const entries = value.entries
        .slice(0, INLINE_COLLECTION_LIMIT)
        .map(entry => `${entry.key} = ${formatParsedValue(entry.value, depth + 1)}`)
      if (value.entries.length > INLINE_COLLECTION_LIMIT) {
        entries.push(`+${value.entries.length - entries.length}`)
      }
      return entries.join(", ")
    }
    case "map": {
      const entries = value.entries
        .slice(0, INLINE_COLLECTION_LIMIT)
        .map(
          entry =>
            `${formatParsedValue(entry.key, depth + 1)} → ${formatParsedValue(entry.value, depth + 1)}`,
        )
      if (value.entries.length > INLINE_COLLECTION_LIMIT) {
        entries.push(`+${value.entries.length - entries.length}`)
      }
      return `{${entries.join(", ")}}`
    }
  }
}

function formatParams(parsedBody: ParsedTransactionBody | undefined): string | undefined {
  if (!parsedBody) return undefined
  const value = formatParsedValue(parsedBody.value)
  if (!value || value === "void") return undefined
  return shorten(value, INLINE_VALUE_LIMIT)
}

function getTraceType(tx: TransactionInfo): string {
  const inMessage = tx.transaction.inMessage
  if (inMessage?.info.type === "external-in") return "EXT-IN"
  if (inMessage?.info.type === "internal") return inMessage.info.bounced ? "BOUNCE" : "INT"
  return getTransactionTriggerLabel(tx.transaction)?.toUpperCase() ?? "TX"
}

function getMessageName(
  tx: TransactionInfo,
  parsedBody: ParsedTransactionBody | undefined,
  contracts: Map<string, ContractData>,
  allContracts: readonly BackendContractInfo[],
): string {
  const opcode = getTransactionOpcode(tx.transaction, parsedBody)
  return (
    resolveTransactionOpcodeName(tx, contracts, allContracts, parsedBody) ??
    parsedBody?.name ??
    (opcode === undefined ? "empty" : `0x${opcode.toString(16).padStart(8, "0")}`)
  )
}

function buildTraceNode(
  tx: TransactionInfo,
  contracts: Map<string, ContractData>,
  allContracts: readonly BackendContractInfo[],
  compilerAbisByCodeHash?: ReadonlyMap<string, ContractData["abi"]>,
): TraceNode {
  const inMessage = tx.transaction.inMessage
  const parsedBody = decodeTransactionMessageBody(
    tx,
    contracts,
    allContracts,
    compilerAbisByCodeHash,
  )
  const sourceLabel = getTransactionSourceLabel(tx.transaction)
  const source = sourceLabel
    ? {label: sourceLabel}
    : participantFromAddress(inMessage?.info.src, contracts, "N/A")
  const destination = participantFromAddress(
    tx.address ?? inMessage?.info.dest,
    contracts,
    "unknown",
  )
  const computePhase = getTransactionComputePhase(tx.transaction)
  const actionPhase = getTransactionActionPhase(tx.transaction)
  const success = isTransactionSuccessful(tx.transaction)
  const status = success
    ? "success"
    : computePhase?.type === "vm" && !computePhase.success
      ? `exit ${computePhase.exitCode}`
      : actionPhase && actionPhase.resultCode !== 0
        ? `action ${actionPhase.resultCode}`
        : "failed"

  return {
    tx,
    type: getTraceType(tx),
    source,
    destination,
    messageName: getMessageName(tx, parsedBody, contracts, allContracts),
    parsedBody,
    params: formatParams(parsedBody),
    value:
      inMessage?.info.type === "internal"
        ? fmt.formatCurrency(inMessage.info.value.coins)
        : undefined,
    gas: computePhase?.type === "vm" ? computePhase.gasUsed : undefined,
    success,
    status,
    children: [...tx.children]
      .sort((left, right) => Number(left.transaction.lt - right.transaction.lt))
      .map(child => buildTraceNode(child, contracts, allContracts, compilerAbisByCodeHash)),
  }
}

function matchesNode(node: TraceNode, query: string, category: SearchCategory): boolean {
  if (!query) return true
  const fields: Record<Exclude<SearchCategory, "all">, string> = {
    message: `${node.messageName} ${node.params ?? ""} ${node.type}`,
    from: `${node.source.label} ${node.source.address ?? ""}`,
    to: `${node.destination.label} ${node.destination.address ?? ""}`,
    contract: `${node.source.label} ${node.destination.label}`,
    status: node.status,
  }
  const target = category === "all" ? Object.values(fields).join(" ") : fields[category]
  return target.toLocaleLowerCase().includes(query)
}

function filterTraceNode(
  node: TraceNode,
  query: string,
  category: SearchCategory,
): TraceNode | undefined {
  if (!query) return node
  const children = node.children
    .map(child => filterTraceNode(child, query, category))
    .filter((child): child is TraceNode => child !== undefined)
  if (!matchesNode(node, query, category) && children.length === 0) return undefined
  return {...node, children}
}

function Participant({
  participant,
  onContractClick,
}: {
  readonly participant: TraceParticipant
  readonly onContractClick?: (address: string) => void
}): React.JSX.Element {
  if (!participant.address || !onContractClick) {
    return <span className={styles.participant}>{participant.label}</span>
  }

  return (
    <button
      type="button"
      className={styles.participantButton}
      title={participant.address}
      onClick={event => {
        event.stopPropagation()
        onContractClick(participant.address as string)
      }}
    >
      {participant.label}
    </button>
  )
}

function RouteArrow(): React.JSX.Element {
  return (
    <span className={styles.arrow} aria-hidden="true">
      →
    </span>
  )
}

function TreeGuides({
  ancestorHasNext,
  isLast,
}: {
  readonly ancestorHasNext: readonly boolean[]
  readonly isLast: boolean
}): React.JSX.Element {
  return (
    <span className={styles.guides} aria-hidden="true">
      {ancestorHasNext.slice(0, -1).map((hasNext, index) => (
        <span
          key={index}
          className={hasNext ? styles.guideContinue : styles.guideBlank}
          style={{left: `${index * 18}px`}}
        />
      ))}
      {ancestorHasNext.length > 0 && (
        <span className={isLast ? styles.guideLast : styles.guideBranch} />
      )}
    </span>
  )
}

function actionLabel(
  action: OutAction,
  tx: TransactionInfo,
  contracts: Map<string, ContractData>,
  additionalAbis: readonly NonNullable<ContractData["abi"]>[],
): React.ReactNode {
  switch (action.type) {
    case "sendMsg": {
      const message = action.outMsg
      const parsedBody = decodeMessageBody(
        message,
        contracts,
        tx.address?.toString(),
        additionalAbis,
      )
      const name =
        resolveMessageOpcodeName(message, contracts, tx.address?.toString(), parsedBody) ??
        parsedBody?.name
      const destination =
        message.info.type === "internal"
          ? participantFromAddress(message.info.dest, contracts, "unknown").label
          : message.info.type === "external-out"
            ? (message.info.dest?.toString() ?? "external")
            : "external"
      const value =
        message.info.type === "internal" ? ` · ${fmt.formatCurrency(message.info.value.coins)}` : ""
      return (
        <>
          Send {name ?? "message"}
          {value}
          <RouteArrow />
          <span className={styles.metaEmphasis}>{destination}</span>
        </>
      )
    }
    case "reserve":
      return `Reserve ${fmt.formatCurrency(action.currency.coins)} · mode ${action.mode}`
    case "setCode":
      return `Set code · ${shorten(action.newCode.hash().toString("hex"), 22)}`
    case "changeLibrary":
      return `Change library · mode ${action.mode}`
  }
}

function externalOutRows(
  tx: TransactionInfo,
  contracts: Map<string, ContractData>,
  additionalAbis: readonly NonNullable<ContractData["abi"]>[],
): TraceMetaRow[] {
  return [...tx.transaction.outMessages.values()].flatMap((message, index) => {
    if (message.info.type !== "external-out") return []
    const parsedBody = decodeMessageBody(message, contracts, tx.address?.toString(), additionalAbis)
    const name =
      resolveMessageOpcodeName(message, contracts, tx.address?.toString(), parsedBody) ??
      parsedBody?.name ??
      "empty"
    const destination = message.info.dest?.toString() ?? "external"
    return [
      {
        id: `${tx.id}-external-out-${index}`,
        type: "EXT-OUT" as const,
        content: (
          <>
            {name}
            <RouteArrow />
            <span className={styles.metaEmphasis}>{shorten(destination)}</span>
          </>
        ),
      },
    ]
  })
}

function buildMetaRows(
  node: TraceNode,
  showPhases: boolean,
  showActions: boolean,
  contracts: Map<string, ContractData>,
  compilerAbisByCodeHash: ReadonlyMap<string, ContractData["abi"]> | undefined,
  allContracts: readonly BackendContractInfo[],
): TraceMetaRow[] {
  const tx = node.tx
  const rows: TraceMetaRow[] = []
  const computePhase = getTransactionComputePhase(tx.transaction)
  const actionPhase = getTransactionActionPhase(tx.transaction)
  const additionalAbis = [
    ...allContracts.map(contract => contract.abi),
    ...(compilerAbisByCodeHash ? [...compilerAbisByCodeHash.values()] : []),
  ].filter((abi): abi is NonNullable<ContractData["abi"]> => abi !== undefined)

  if (tx.transaction.oldStatus === "non-existing" && tx.transaction.endStatus === "active") {
    rows.push({id: `${tx.id}-created`, type: "EVENT", content: "Account created"})
  } else if (tx.transaction.oldStatus === "active" && tx.transaction.endStatus === "non-existing") {
    rows.push({id: `${tx.id}-destroyed`, type: "EVENT", content: "Account destroyed"})
  }

  if (computePhase?.type === "vm" && !computePhase.success) {
    rows.push({
      id: `${tx.id}-compute-error`,
      type: "ERROR",
      failed: true,
      content: `Compute phase failed · exit code ${computePhase.exitCode}`,
    })
  } else if (computePhase?.type === "skipped") {
    rows.push({
      id: `${tx.id}-compute-skipped`,
      type: "ERROR",
      failed: true,
      content: `Compute phase skipped · ${computePhase.reason}`,
    })
  }

  if (actionPhase && actionPhase.resultCode !== 0) {
    rows.push({
      id: `${tx.id}-action-error`,
      type: "ERROR",
      failed: true,
      content: `Action phase failed · result code ${actionPhase.resultCode}`,
    })
  }

  rows.push(...externalOutRows(tx, contracts, additionalAbis))

  if (showPhases) {
    if (computePhase?.type === "vm") {
      rows.push({
        id: `${tx.id}-compute`,
        type: "PHASE",
        content: `Compute · ${computePhase.gasUsed.toLocaleString("en-US")} gas · exit ${computePhase.exitCode}`,
      })
    }
    if (actionPhase) {
      rows.push({
        id: `${tx.id}-action-phase`,
        type: "PHASE",
        content: `Action · ${actionPhase.totalActions} actions · result ${actionPhase.resultCode}`,
      })
    }
  }

  if (showActions) {
    tx.outActions.forEach((action, index) => {
      rows.push({
        id: `${tx.id}-action-${index}`,
        type: "ACTION",
        content: actionLabel(action, tx, contracts, additionalAbis),
      })
    })
  }

  return rows
}

function MetaRow({
  row,
  depth,
  ancestorHasNext,
  isLast,
}: {
  readonly row: TraceMetaRow
  readonly depth: number
  readonly ancestorHasNext: readonly boolean[]
  readonly isLast: boolean
}): React.JSX.Element {
  return (
    <div className={`${styles.row} ${styles.metaRow} ${row.failed ? styles.failedRow : ""}`}>
      <span className={`${styles.typeBadge} ${styles[`type${row.type.replace("-", "")}`]}`}>
        {row.type}
      </span>
      <span className={styles.gasCell} />
      <div className={styles.content} style={{"--trace-depth": depth} as React.CSSProperties}>
        <TreeGuides ancestorHasNext={ancestorHasNext} isLast={isLast} />
        <span className={styles.metaContent}>{row.content}</span>
      </div>
    </div>
  )
}

function TraceNodeRow({
  node,
  depth,
  ancestorHasNext,
  isLast,
  expandedIds,
  expandedBodyIds,
  forceExpanded,
  showGas,
  showPhases,
  showActions,
  selectedTransactionId,
  highlightedTransactionIds,
  contracts,
  compilerAbisByCodeHash,
  allContracts,
  onToggle,
  onBodyToggle,
  onContractClick,
  onTransactionSelect,
  renderTransactionDetails,
}: TraceNodeRowProps): React.JSX.Element {
  const metaRows = buildMetaRows(
    node,
    showPhases,
    showActions,
    contracts,
    compilerAbisByCodeHash,
    allContracts,
  )
  const hasChildren = node.children.length > 0 || metaRows.length > 0
  const expanded = forceExpanded || expandedIds.has(node.tx.id)
  const isBodyExpanded = expandedBodyIds.has(node.tx.id)
  const isSelected = selectedTransactionId === node.tx.id
  const isHighlighted = highlightedTransactionIds?.has(node.tx.id) ?? false
  const bodyRowCount = isBodyExpanded && node.parsedBody ? 1 : 0
  const childCount = bodyRowCount + (expanded ? metaRows.length + node.children.length : 0)

  return (
    <>
      <div
        className={`${styles.row} ${styles.transactionRow} ${isSelected ? styles.selectedRow : ""} ${
          isHighlighted ? styles.highlightedRow : ""
        } ${node.success ? "" : styles.failedRow}`}
        data-transaction-id={node.tx.id}
      >
        <span className={`${styles.typeBadge} ${styles[`type${node.type.replace("-", "")}`]}`}>
          {node.type}
        </span>
        <span className={`${styles.gasCell} ${showGas ? "" : styles.gasHidden}`}>
          {showGas && node.gas !== undefined ? node.gas.toLocaleString("en-US") : ""}
        </span>
        <div className={styles.content} style={{"--trace-depth": depth} as React.CSSProperties}>
          <TreeGuides ancestorHasNext={ancestorHasNext} isLast={isLast} />
          <button
            type="button"
            className={styles.disclosure}
            disabled={!hasChildren}
            aria-label={`${expanded ? "Collapse" : "Expand"} ${node.messageName}`}
            aria-expanded={hasChildren ? expanded : undefined}
            onClick={() => onToggle(node.tx.id)}
          >
            {hasChildren ? (
              expanded ? (
                <ChevronDown size={14} />
              ) : (
                <ChevronRight size={14} />
              )
            ) : (
              <span className={styles.disclosurePlaceholder} />
            )}
          </button>
          <div
            className={styles.rowButton}
            role="button"
            tabIndex={0}
            aria-pressed={isSelected}
            onClick={() => onTransactionSelect(node.tx)}
            onKeyDown={event => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault()
                onTransactionSelect(node.tx)
              }
            }}
          >
            <span className={styles.route}>
              <Participant participant={node.source} onContractClick={onContractClick} />
              {node.value && <span className={styles.value}>{node.value}</span>}
              <RouteArrow />
              <Participant participant={node.destination} onContractClick={onContractClick} />
            </span>
            <span className={styles.message}>
              <span className={styles.messageName}>{node.messageName}</span>
              {node.parsedBody && (
                <InlineButton
                  variant="utility"
                  className={styles.bodyToggle}
                  leadingIcon={<Braces size={11} />}
                  aria-expanded={isBodyExpanded}
                  title={isBodyExpanded ? "Hide parsed body" : "Show parsed body"}
                  onClick={event => {
                    event.stopPropagation()
                    onBodyToggle(node.tx.id)
                  }}
                >
                  Body
                </InlineButton>
              )}
            </span>
            {!node.success && <span className={styles.status}>{node.status}</span>}
          </div>
        </div>
      </div>

      {isBodyExpanded && node.parsedBody && (
        <div className={styles.bodyRow}>
          <span className={`${styles.typeBadge} ${styles.typeBODY}`}>BODY</span>
          <span className={styles.gasCell} />
          <div
            className={`${styles.content} ${styles.bodyContent}`}
            style={{"--trace-depth": depth + 1} as React.CSSProperties}
          >
            <TreeGuides
              ancestorHasNext={[...ancestorHasNext, childCount > 1]}
              isLast={childCount === 1}
            />
            <div className={styles.bodyPanel}>
              <ParsedValueView
                value={node.parsedBody.value}
                fallbackTypeName={node.parsedBody.name}
                contracts={contracts}
                onContractClick={onContractClick}
              />
            </div>
          </div>
        </div>
      )}

      {isSelected && (
        <div className={styles.inlineDetailsRow}>
          <div
            className={styles.inlineDetailsContent}
            style={{"--trace-depth": depth} as React.CSSProperties}
          >
            {renderTransactionDetails(node.tx)}
          </div>
        </div>
      )}

      {expanded &&
        metaRows.map((row, index) => {
          const metaIndex = bodyRowCount + index
          const metaIsLast = metaIndex === childCount - 1
          return (
            <MetaRow
              key={row.id}
              row={row}
              depth={depth + 1}
              ancestorHasNext={[...ancestorHasNext, !metaIsLast]}
              isLast={metaIsLast}
            />
          )
        })}

      {expanded &&
        node.children.map((child, index) => {
          const childIndex = bodyRowCount + metaRows.length + index
          const childIsLast = childIndex === childCount - 1
          return (
            <TraceNodeRow
              key={child.tx.id}
              node={child}
              depth={depth + 1}
              ancestorHasNext={[...ancestorHasNext, !childIsLast]}
              isLast={childIsLast}
              expandedIds={expandedIds}
              expandedBodyIds={expandedBodyIds}
              forceExpanded={forceExpanded}
              showGas={showGas}
              showPhases={showPhases}
              showActions={showActions}
              selectedTransactionId={selectedTransactionId}
              highlightedTransactionIds={highlightedTransactionIds}
              contracts={contracts}
              compilerAbisByCodeHash={compilerAbisByCodeHash}
              allContracts={allContracts}
              onToggle={onToggle}
              onBodyToggle={onBodyToggle}
              onContractClick={onContractClick}
              onTransactionSelect={onTransactionSelect}
              renderTransactionDetails={renderTransactionDetails}
            />
          )
        })}
    </>
  )
}

export function TransactionTextTree({
  transactions,
  contracts,
  compilerAbisByCodeHash,
  allContracts,
  selectedTransactionId,
  highlightedTransactionIds,
  onContractClick,
  onTransactionSelect,
  renderTransactionDetails,
}: TransactionTextTreeProps): React.JSX.Element {
  const [query, setQuery] = useState("")
  const [category, setCategory] = useState<SearchCategory>("all")
  const [showGas, setShowGas] = useState(true)
  const [showPhases, setShowPhases] = useState(false)
  const [showActions, setShowActions] = useState(false)
  const [expandedIds, setExpandedIds] = useState<ReadonlySet<string>>(
    () => new Set(transactions.map(tx => tx.id)),
  )
  const [expandedBodyIds, setExpandedBodyIds] = useState<ReadonlySet<string>>(() => new Set())

  useEffect(() => {
    setExpandedIds(current => {
      const next = new Set(current)
      for (const tx of transactions) next.add(tx.id)
      return next
    })
  }, [transactions])

  const trace = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase()
    return transactions
      .filter(tx => !tx.parent)
      .sort((left, right) => Number(left.transaction.lt - right.transaction.lt))
      .map(tx => buildTraceNode(tx, contracts, allContracts, compilerAbisByCodeHash))
      .map(node => filterTraceNode(node, normalizedQuery, category))
      .filter((node): node is TraceNode => node !== undefined)
  }, [allContracts, category, compilerAbisByCodeHash, contracts, query, transactions])

  const forceExpanded = query.trim().length > 0
  const allExpanded = transactions.length > 0 && transactions.every(tx => expandedIds.has(tx.id))

  const toggleExpanded = (id: string): void => {
    setExpandedIds(current => {
      const next = new Set(current)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const toggleBody = (id: string): void => {
    setExpandedBodyIds(current => {
      const next = new Set(current)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  return (
    <section className={styles.container} aria-label="Transaction trace">
      <div className={styles.header}>
        <div>
          <h3 className={styles.title}>Execution trace</h3>
          <p className={styles.description}>Messages, phases and actions in execution order</p>
        </div>
        <span className={styles.transactionCount}>{transactions.length} transactions</span>
      </div>

      <div className={styles.toolbar}>
        <div className={styles.searchGroup}>
          <Input
            size="sm"
            className={styles.searchInput}
            value={query}
            placeholder="Search trace"
            aria-label="Search transaction trace"
            leadingIcon={<Search size={14} />}
            onChange={event => setQuery(event.target.value)}
          />
          <Select
            size="sm"
            className={styles.categorySelect}
            value={category}
            aria-label="Trace search category"
            onChange={event => setCategory(event.target.value as SearchCategory)}
          >
            <option value="all">All</option>
            <option value="message">Message</option>
            <option value="from">From</option>
            <option value="to">To</option>
            <option value="contract">Contract</option>
            <option value="status">Status</option>
          </Select>
        </div>

        <div className={styles.filters}>
          <Checkbox
            label="Gas"
            checked={showGas}
            onChange={event => setShowGas(event.target.checked)}
          />
          <Checkbox
            label="Phases"
            checked={showPhases}
            onChange={event => setShowPhases(event.target.checked)}
          />
          <Checkbox
            label="Actions"
            checked={showActions}
            onChange={event => setShowActions(event.target.checked)}
          />
          <Button
            size="icon"
            variant="ghost"
            aria-label={allExpanded ? "Collapse all trace rows" : "Expand all trace rows"}
            title={allExpanded ? "Collapse all" : "Expand all"}
            onClick={() =>
              setExpandedIds(allExpanded ? new Set() : new Set(transactions.map(tx => tx.id)))
            }
          >
            {allExpanded ? <ChevronsDownUp size={15} /> : <ChevronsUpDown size={15} />}
          </Button>
        </div>
      </div>

      <div className={styles.columnHeader} aria-hidden="true">
        <span>Type</span>
        <span className={showGas ? "" : styles.gasHidden}>Gas</span>
        <span>Trace</span>
      </div>

      <div className={styles.rows}>
        {trace.length > 0 ? (
          trace.map((node, index) => (
            <TraceNodeRow
              key={node.tx.id}
              node={node}
              depth={0}
              ancestorHasNext={[]}
              isLast={index === trace.length - 1}
              expandedIds={expandedIds}
              expandedBodyIds={expandedBodyIds}
              forceExpanded={forceExpanded}
              showGas={showGas}
              showPhases={showPhases}
              showActions={showActions}
              selectedTransactionId={selectedTransactionId}
              highlightedTransactionIds={highlightedTransactionIds}
              contracts={contracts}
              compilerAbisByCodeHash={compilerAbisByCodeHash}
              allContracts={allContracts}
              onToggle={toggleExpanded}
              onBodyToggle={toggleBody}
              onContractClick={onContractClick}
              onTransactionSelect={onTransactionSelect}
              renderTransactionDetails={renderTransactionDetails}
            />
          ))
        ) : (
          <div className={styles.empty}>No trace entries match this search</div>
        )}
      </div>
    </section>
  )
}
