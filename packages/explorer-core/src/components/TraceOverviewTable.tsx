import {BlockChip, CopyInlineAction, InlineActions} from "@acton/ui"
import {
  getTransactionComputePhase,
  type TransactionBlockRef,
  type TransactionInfo,
} from "@acton/transaction-ui"
import {useLayoutEffect, useRef, useState} from "react"
import type {FC, MouseEvent, ReactNode} from "react"

import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {formatNano, hashToHex} from "./utils"
import {buildTraceShardFlow} from "./traceShardFlow"

import styles from "./TraceOverviewTable.module.css"

export interface TraceOverviewData {
  readonly traceId: string
  readonly externalHash?: string
  readonly masterchainSeqnoStart: string
  readonly masterchainSeqnoEnd: string
  readonly startLt: string
  readonly endLt: string
  readonly startUtime: number
  readonly endUtime: number
  readonly isIncomplete: boolean
  readonly transactionCount: number
  readonly messageCount: number
  readonly pendingMessageCount: number
  readonly traceState: string
}

interface TraceOverviewTableProps {
  readonly data: TraceOverviewData
  readonly transactions: readonly TransactionInfo[]
  readonly currentTransaction?: TransactionInfo
  readonly actionCount?: number
  readonly onBlockClick?: (block: TransactionBlockRef, event: MouseEvent<HTMLElement>) => void
  readonly onShardHoverChange?: (transactionIds: readonly string[] | undefined) => void
}

const MASTERCHAIN_BLOCK_SHARD = "8000000000000000"

const formatState = (state: string): string => {
  const normalized = state.trim().replaceAll("_", " ")
  return normalized.length > 0
    ? normalized.charAt(0).toUpperCase() + normalized.slice(1)
    : "Unknown"
}

const formatDuration = (seconds: number): string => {
  if (seconds <= 0) {
    return "Less than 1 second"
  }
  if (seconds === 1) {
    return "1 second"
  }
  if (seconds < 60) {
    return `${seconds} seconds`
  }

  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = seconds % 60
  return remainingSeconds === 0 ? `${minutes} min` : `${minutes} min ${remainingSeconds} sec`
}

const isSameBlock = (left: TransactionBlockRef | undefined, right: TransactionBlockRef): boolean =>
  left?.workchain === right.workchain && left.shard === right.shard && left.seqno === right.seqno

const isTransactionAborted = (transactionInfo: TransactionInfo): boolean => {
  const {description} = transactionInfo.transaction
  return "aborted" in description && description.aborted
}

export const TraceOverviewTable: FC<TraceOverviewTableProps> = ({
  data,
  transactions,
  currentTransaction,
  actionCount,
  onBlockClick,
  onShardHoverChange,
}) => {
  const routes = useExplorerRoutePaths()
  const masterchainBlockPath = (seqno: string) =>
    routes.blockPath(-1, MASTERCHAIN_BLOCK_SHARD, Number(seqno))
  const accountCount = new Set(
    transactions
      .map(transaction => transaction.address?.toString())
      .filter((address): address is string => address !== undefined),
  ).size
  const totalFees = transactions.reduce(
    (total, transaction) => total + transaction.transaction.totalFees.coins,
    0n,
  )
  const abortedTransactionCount = transactions.filter(isTransactionAborted).length
  const skippedComputeCount = transactions.filter(
    transaction => getTransactionComputePhase(transaction.transaction)?.type === "skipped",
  ).length
  const shardFlow = buildTraceShardFlow(transactions)
  const currentShardFlowSegmentIndex = currentTransaction
    ? shardFlow.segments.findIndex(segment =>
        segment.transactionIds.includes(currentTransaction.id),
      )
    : -1
  const shardFlowRef = useRef<HTMLDivElement>(null)
  const currentShardFlowStepRef = useRef<HTMLDivElement>(null)
  const [hoveredShardKey, setHoveredShardKey] = useState<string>()

  useLayoutEffect(() => {
    const flow = shardFlowRef.current
    const currentStep = currentShardFlowStepRef.current
    if (!flow || !currentStep || currentShardFlowSegmentIndex < 0) {
      return
    }

    const maxScrollLeft = Math.max(0, flow.scrollWidth - flow.clientWidth)
    if (currentShardFlowSegmentIndex === shardFlow.segments.length - 1) {
      flow.scrollTo({left: maxScrollLeft, behavior: "smooth"})
      return
    }

    const flowRect = flow.getBoundingClientRect()
    const stepRect = currentStep.getBoundingClientRect()
    const stepLeft = flow.scrollLeft + stepRect.left - flowRect.left
    const stepRight = stepLeft + stepRect.width
    const visibleLeft = flow.scrollLeft
    const visibleRight = visibleLeft + flow.clientWidth
    const inlinePadding = 8

    if (stepLeft < visibleLeft) {
      flow.scrollTo({left: Math.max(0, stepLeft - inlinePadding), behavior: "smooth"})
    } else if (stepRight > visibleRight) {
      flow.scrollTo({
        left: Math.min(maxScrollLeft, stepRight - flow.clientWidth + inlinePadding),
        behavior: "smooth",
      })
    }
  }, [currentShardFlowSegmentIndex, currentTransaction?.id, shardFlow.segments.length])
  const duration = Math.max(0, data.endUtime - data.startUtime)
  const traceId = hashToHex(data.traceId) ?? data.traceId
  const externalHash = data.externalHash
    ? (hashToHex(data.externalHash) ?? data.externalHash)
    : undefined
  const status = data.isIncomplete ? "Incomplete" : "Complete"
  const traceState = formatState(data.traceState)
  const traceItems: readonly {readonly label: string; readonly value: ReactNode}[] = [
    {
      label: "Status",
      value: (
        <span className={data.isIncomplete ? styles.statusIncomplete : styles.statusComplete}>
          {status}
        </span>
      ),
    },
    ...(traceState.toLowerCase() === status.toLowerCase()
      ? []
      : [{label: "Trace State", value: traceState}]),
  ]
  const activityItems: readonly {readonly label: string; readonly value: ReactNode}[] = [
    {label: "Transactions", value: data.transactionCount},
    {label: "Messages", value: data.messageCount},
    {label: "Accounts", value: accountCount},
    ...(actionCount === undefined ? [] : [{label: "Actions", value: actionCount}]),
    {label: "Pending Messages", value: data.pendingMessageCount},
  ]
  const executionItems: readonly {readonly label: string; readonly value: ReactNode}[] = [
    {label: "Total Fees", value: `${formatNano(totalFees.toString())} GRAM`},
    {label: "Aborted", value: abortedTransactionCount},
    {label: "Skipped Compute", value: skippedComputeCount},
  ]
  const timeItems: readonly {readonly label: string; readonly value: ReactNode}[] = [
    {label: "Started", value: new Date(data.startUtime * 1000).toLocaleString()},
    {label: "Finished", value: new Date(data.endUtime * 1000).toLocaleString()},
    {label: "Duration", value: formatDuration(duration)},
    {label: "Logical Time", value: `${data.startLt} — ${data.endLt}`},
  ]

  return (
    <div className={styles.container} aria-label="Trace overview">
      {[
        {title: "Trace", items: traceItems},
        {title: "Activity", items: activityItems},
        {title: "Execution", items: executionItems},
        {title: "Time", items: timeItems},
      ].map(section => (
        <div key={section.title} className={styles.sectionRow}>
          <div className={styles.sectionTitle}>{section.title}</div>
          <div className={styles.sectionContent}>
            <div className={styles.multiColumnRow}>
              {section.items.map(item => (
                <div key={item.label} className={styles.multiColumnItem}>
                  <div className={styles.itemTitle}>{item.label}</div>
                  <div className={styles.itemValue}>{item.value}</div>
                </div>
              ))}
            </div>
          </div>
        </div>
      ))}

      <div className={styles.sectionRow}>
        <div className={styles.sectionTitle}>Blocks</div>
        <div className={styles.sectionContent}>
          <div className={styles.multiColumnRow}>
            <div className={styles.multiColumnItem}>
              <div className={styles.itemTitle}>Start Masterchain Block</div>
              <div className={styles.itemValue}>
                <BlockChip
                  workchain={-1}
                  shard={MASTERCHAIN_BLOCK_SHARD}
                  seqno={data.masterchainSeqnoStart}
                  href={masterchainBlockPath(data.masterchainSeqnoStart)}
                  onClick={event => {
                    if (!onBlockClick) return
                    event.preventDefault()
                    onBlockClick(
                      {
                        workchain: -1,
                        shard: MASTERCHAIN_BLOCK_SHARD,
                        seqno: Number(data.masterchainSeqnoStart),
                      },
                      event,
                    )
                  }}
                />
              </div>
            </div>
            <div className={styles.multiColumnItem}>
              <div className={styles.itemTitle}>End Masterchain Block</div>
              <div className={styles.itemValue}>
                <BlockChip
                  workchain={-1}
                  shard={MASTERCHAIN_BLOCK_SHARD}
                  seqno={data.masterchainSeqnoEnd}
                  href={masterchainBlockPath(data.masterchainSeqnoEnd)}
                  onClick={event => {
                    if (!onBlockClick) return
                    event.preventDefault()
                    onBlockClick(
                      {
                        workchain: -1,
                        shard: MASTERCHAIN_BLOCK_SHARD,
                        seqno: Number(data.masterchainSeqnoEnd),
                      },
                      event,
                    )
                  }}
                />
              </div>
            </div>
          </div>
        </div>
      </div>

      {shardFlow.segments.length > 0 ? (
        <div className={styles.sectionRow}>
          <div className={styles.sectionTitle}>Shard Flow</div>
          <div className={`${styles.sectionContent} ${styles.shardFlowContent}`}>
            <div className={styles.shardFlowSummary}>
              Logical-time order · {shardFlow.shardCount}{" "}
              {shardFlow.shardCount === 1 ? "shard" : "shards"} · {shardFlow.transactionCount}{" "}
              {shardFlow.transactionCount === 1 ? "transaction" : "transactions"}
            </div>
            <div
              ref={shardFlowRef}
              className={styles.shardFlow}
              aria-label="Transactions grouped by shard"
            >
              {shardFlow.segments.map((segment, index) => (
                <div
                  className={styles.shardFlowStepGroup}
                  key={`${segment.workchain}:${segment.shard}:${index}`}
                >
                  <div
                    ref={
                      index === currentShardFlowSegmentIndex ? currentShardFlowStepRef : undefined
                    }
                    role="group"
                    aria-label={`${segment.workchain}:${segment.shard}, ${segment.transactionCount} ${
                      segment.transactionCount === 1 ? "transaction" : "transactions"
                    }`}
                    className={`${styles.shardFlowStep} ${
                      index === currentShardFlowSegmentIndex ? styles.shardFlowStepCurrent : ""
                    } ${
                      hoveredShardKey === `${segment.workchain}:${segment.shard}`
                        ? styles.shardFlowStepRelated
                        : ""
                    } ${
                      hoveredShardKey !== undefined &&
                      hoveredShardKey !== `${segment.workchain}:${segment.shard}`
                        ? styles.shardFlowStepMuted
                        : ""
                    } ${styles.shardFlowStepInteractive}`}
                    onMouseEnter={() => {
                      setHoveredShardKey(`${segment.workchain}:${segment.shard}`)
                      onShardHoverChange?.(segment.transactionIds)
                    }}
                    onMouseLeave={() => {
                      setHoveredShardKey(undefined)
                      onShardHoverChange?.(undefined)
                    }}
                  >
                    <div className={styles.shardFlowShard}>
                      <span title={`Workchain ${segment.workchain}, shard ${segment.shard}`}>
                        {segment.workchain}:{segment.shard}
                      </span>
                    </div>
                    <div className={styles.shardFlowMeta}>
                      <span>
                        {segment.transactionCount}{" "}
                        {segment.transactionCount === 1 ? "transaction" : "transactions"}
                      </span>
                    </div>
                    <div className={styles.shardFlowBlocks}>
                      <span className={styles.shardFlowBlocksLabel}>
                        {segment.blocks.length === 1 ? "Block:" : "Blocks:"}
                      </span>
                      {segment.blocks.map(block => {
                        const isCurrentBlock =
                          index === currentShardFlowSegmentIndex &&
                          isSameBlock(currentTransaction?.blockRef, block)
                        return (
                          <BlockChip
                            key={`${block.workchain}:${block.shard}:${block.seqno}`}
                            className={styles.shardFlowBlockChip}
                            copyable={segment.blocks.length === 1}
                            workchain={block.workchain}
                            shard={block.shard}
                            seqno={block.seqno}
                            highlighted={isCurrentBlock}
                            href={routes.blockPath(block.workchain, block.shard, block.seqno)}
                            title={
                              isCurrentBlock
                                ? `Current transaction block · ${block.workchain}:${block.shard}:${block.seqno}`
                                : undefined
                            }
                            onClick={event => {
                              if (!onBlockClick) return
                              event.preventDefault()
                              onBlockClick(block, event)
                            }}
                          />
                        )
                      })}
                    </div>
                  </div>
                  {index < shardFlow.segments.length - 1 ? (
                    <span className={styles.shardFlowArrow} aria-hidden="true">
                      →
                    </span>
                  ) : undefined}
                </div>
              ))}
            </div>
          </div>
        </div>
      ) : undefined}

      <div className={styles.sectionRow}>
        <div className={styles.sectionTitle}>Identifiers</div>
        <div className={styles.sectionContent}>
          <div className={styles.identifierList}>
            <div className={styles.identifierItem}>
              <div className={styles.itemTitle}>Trace ID</div>
              <InlineActions
                className={styles.copyableValue}
                visibility="hover"
                actions={
                  <CopyInlineAction
                    value={traceId}
                    label="Copy trace ID"
                    copiedLabel="Trace ID copied"
                  />
                }
              >
                <span className={`${styles.itemValue} ${styles.hash}`}>{traceId}</span>
              </InlineActions>
            </div>
            {externalHash ? (
              <div className={styles.identifierItem}>
                <div className={styles.itemTitle}>External Hash</div>
                <InlineActions
                  className={styles.copyableValue}
                  visibility="hover"
                  actions={
                    <CopyInlineAction
                      value={externalHash}
                      label="Copy external hash"
                      copiedLabel="External hash copied"
                    />
                  }
                >
                  <span className={`${styles.itemValue} ${styles.hash}`}>{externalHash}</span>
                </InlineActions>
              </div>
            ) : undefined}
          </div>
        </div>
      </div>
    </div>
  )
}
