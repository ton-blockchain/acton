/* eslint-disable unicorn/prefer-spread */

import type {Address} from "@ton/core"
import type React from "react"
import {useEffect, useLayoutEffect, useMemo, useRef, useState} from "react"
import {
  type CustomNodeElementProps,
  type RawNodeDatum,
  Tree,
  type TreeLinkDatum,
} from "react-d3-tree"

import type {BackendContractInfo, SourceLocation} from "@/types"
import type {ContractData, LoadedTransactionActions, TransactionInfo} from "@/types/transaction"
import {fmt} from "@/index"
import {
  getTransactionActionPhase,
  getTransactionComputePhase,
  getTransactionOpcode,
  getTransactionSourceLabel,
  resolveTransactionOpcodeName,
} from "@/utils/transaction"

import {TransactionDetails} from "../TransactionDetails/TransactionDetails"

import {SmartTooltip} from "./SmartTooltip"
import {StorageDiffView} from "./StorageDiffView"
import styles from "./TransactionTree.module.css"
import {buildStorageDiff, type StorageDiffNode} from "./storageDiff"
import {useTooltip} from "./useTooltip"

interface EdgeTransactionTooltipData {
  readonly fromAddress: string
  readonly computePhase: {
    readonly success: boolean
    readonly exitCode?: number
    readonly gasUsed?: bigint
    readonly vmSteps?: number
  }
  readonly fees: {
    readonly gasFees?: bigint
    readonly totalFees: bigint
  }
  readonly sentTotal: bigint
}

interface NodeTransactionTooltipData {
  readonly contract: {
    readonly typeName: string
    readonly address: string
  }
  readonly account: {
    readonly isCreated: boolean
    readonly isDestroyed: boolean
  }
  readonly storageDiff: StorageDiffNode | undefined
}

interface TransactionTreeProps {
  readonly transactions: TransactionInfo[]
  readonly contracts: Map<string, ContractData>
  readonly compilerAbisByCodeHash?: ReadonlyMap<string, ContractData["abi"]>
  readonly allContracts: readonly BackendContractInfo[]
  readonly selectedTransactionId?: string
  readonly onContractClick?: (address: string) => void
  readonly renderSourceLocation?: (location: SourceLocation) => React.ReactNode
  readonly renderSelectedTransactionExtra?: (tx: TransactionInfo) => React.ReactNode
  readonly renderSelectedTransactionMessageRouteAction?: (tx: TransactionInfo) => React.ReactNode
  readonly loadActions?: (tx: TransactionInfo) => Promise<LoadedTransactionActions>
}

interface TreeLayout {
  readonly height: number
  readonly width: number
  readonly translate: {
    readonly x: number
    readonly y: number
  }
}

const TREE_NODE_SIZE = {x: 200, y: 120} as const
const TREE_SEPARATION = {siblings: 0.7, nonSiblings: 1} as const
const TREE_MIN_SIZE = {height: 80, width: 800} as const
const TREE_PADDING = {top: 8, right: 32, bottom: 8, left: 50} as const
const TREE_DETAILS_GAP = 15
const TREE_EDGE_LABEL = {width: 150, height: 64, failedHeight: 84, x: -180, y: -40} as const

const INITIAL_TREE_LAYOUT: TreeLayout = {
  height: TREE_MIN_SIZE.height,
  width: TREE_MIN_SIZE.width,
  translate: {
    x: TREE_PADDING.left,
    y: TREE_MIN_SIZE.height / 2,
  },
}

function EdgeTransactionTooltipContent({
  data,
}: {
  data: EdgeTransactionTooltipData
}): React.JSX.Element {
  return (
    <div className={styles.tooltipContent}>
      <div className={styles.tooltipField}>
        <div className={styles.tooltipFieldLabel}>From Address</div>
        <div className={styles.tooltipFieldValue}>{data.fromAddress}</div>
      </div>

      <div className={styles.tooltipField}>
        <div className={styles.tooltipFieldLabel}>Compute Phase</div>
        <div className={styles.tooltipFieldValue}>
          {data.computePhase.success ? "Success" : "Failed"}
          {data.computePhase.exitCode !== undefined && data.computePhase.exitCode !== 0 && (
            <span>
              {" "}
              {"(Exit:"} {data.computePhase.exitCode})
            </span>
          )}
          {data.computePhase.gasUsed !== undefined && (
            <div className={styles.tooltipSubValue}>
              Gas Used: {data.computePhase.gasUsed.toString()}
            </div>
          )}
          {data.computePhase.vmSteps !== undefined && (
            <div className={styles.tooltipSubValue}>
              VM Steps: {data.computePhase.vmSteps.toString()}
            </div>
          )}
        </div>
      </div>

      <div className={styles.tooltipField}>
        <div className={styles.tooltipFieldLabel}>Money</div>
        <div className={styles.tooltipFieldValue}>
          <div>Sent Total: {fmt.formatCurrency(data.sentTotal)}</div>
          <div className={styles.tooltipSubValue}>
            Total Fees: {fmt.formatCurrency(data.fees.totalFees)}
          </div>
          {data.fees.gasFees !== undefined && (
            <div className={styles.tooltipSubValue}>
              Gas Fees: {fmt.formatCurrency(data.fees.gasFees)}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

function NodeTransactionTooltipContent({
  data,
  contracts,
  onContractClick,
}: {
  data: NodeTransactionTooltipData
  contracts: Map<string, ContractData>
  onContractClick?: (address: string) => void
}): React.JSX.Element {
  return (
    <div className={styles.tooltipContent}>
      <div className={styles.tooltipField}>
        <div className={styles.tooltipFieldLabel}>{data.contract.typeName}</div>
        <div className={styles.tooltipFieldValue}>{data.contract.address}</div>
      </div>

      <div className={styles.tooltipField}>
        <div className={styles.tooltipFieldLabel}>Storage</div>
        <div className={`${styles.tooltipFieldValue} ${styles.tooltipFieldValueStructured}`}>
          {(data.account.isCreated || data.account.isDestroyed) && (
            <div className={styles.storageMeta}>
              {data.account.isCreated && (
                <span className={`${styles.storageMetaBadge} ${styles.storageMetaBadgeCreated}`}>
                  Account created
                </span>
              )}
              {data.account.isDestroyed && (
                <span className={`${styles.storageMetaBadge} ${styles.storageMetaBadgeDestroyed}`}>
                  Account destroyed
                </span>
              )}
            </div>
          )}
          {data.storageDiff ? (
            <div className={styles.storageDiffScroll}>
              <StorageDiffView
                diff={data.storageDiff}
                contracts={contracts}
                onContractClick={onContractClick}
              />
            </div>
          ) : (
            <span className={styles.storageUnavailable}>Storage data unavailable</span>
          )}
        </div>
      </div>
    </div>
  )
}

export function TransactionTree({
  transactions,
  contracts,
  compilerAbisByCodeHash,
  allContracts,
  selectedTransactionId,
  onContractClick,
  renderSourceLocation,
  renderSelectedTransactionExtra,
  renderSelectedTransactionMessageRouteAction,
  loadActions,
}: TransactionTreeProps): React.JSX.Element {
  const {
    tooltip,
    showTooltip,
    hideTooltip,
    forceHideTooltip,
    setIsTooltipHovered,
    calculateOptimalPosition,
  } = useTooltip()

  const [selectedTransactionIdState, setSelectedTransactionIdState] = useState<string | undefined>(
    selectedTransactionId,
  )
  const triggerRectReference = useRef<DOMRect | undefined>(undefined)
  const treeContainerRef = useRef<HTMLDivElement | null>(null)
  const treeWrapperRef = useRef<HTMLDivElement | null>(null)
  const [treeLayout, setTreeLayout] = useState<TreeLayout>(INITIAL_TREE_LAYOUT)

  const rootTransactions = useMemo(() => {
    return transactions
      .filter(tx => !tx.parent)
      .sort((a, b) => Number(a.transaction.lt - b.transaction.lt))
  }, [transactions])

  const transactionMap = useMemo(() => {
    const map: Map<string, TransactionInfo> = new Map()
    for (const tx of transactions) {
      map.set(tx.id, tx)
    }
    return map
  }, [transactions])

  const handleNodeClick = (id: string): void => {
    const transaction = transactionMap.get(id)
    if (!transaction) return

    forceHideTooltip()

    if (selectedTransactionIdState === id) {
      setSelectedTransactionIdState(undefined)
    } else {
      setSelectedTransactionIdState(id)
    }
  }

  const showEdgeTransactionTooltip = (event: React.MouseEvent, tx: TransactionInfo): void => {
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect()
    triggerRectReference.current = rect

    const computePhase = getTransactionComputePhase(tx.transaction)
    const sourceLabel = getTransactionSourceLabel(tx.transaction)

    const tooltipData: EdgeTransactionTooltipData = {
      fromAddress:
        sourceLabel ??
        (tx.transaction.inMessage?.info.src
          ? formatAddress(tx.transaction.inMessage.info.src as Address, contracts)
          : "unknown"),
      computePhase: {
        success: computePhase?.type === "vm" ? computePhase.success : true,
        exitCode: computePhase?.type === "vm" ? computePhase.exitCode : undefined,
        gasUsed: computePhase?.type === "vm" ? computePhase.gasUsed : undefined,
        vmSteps: computePhase?.type === "vm" ? computePhase.vmSteps : undefined,
      },
      fees: {
        gasFees: computePhase?.type === "vm" ? computePhase.gasFees : undefined,
        totalFees: tx.transaction.totalFees.coins,
      },
      sentTotal: [...tx.transaction.outMessages.values()].reduce(
        (accumulator: bigint, message) =>
          accumulator + (message.info.type === "internal" ? message.info.value.coins : 0n),
        0n,
      ),
    }

    showTooltip({
      x: rect.left,
      y: rect.top,
      content: <EdgeTransactionTooltipContent data={tooltipData} />,
    })
  }

  const showNodeTransactionTooltip = (event: React.MouseEvent, tx: TransactionInfo): void => {
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect()
    triggerRectReference.current = rect

    const contractAddress = tx.address ? tx.address.toString({testOnly: true}) : "unknown"
    const isCreated =
      tx.transaction.oldStatus === "non-existing" && tx.transaction.endStatus === "active"
    const isDestroyed =
      tx.transaction.oldStatus === "active" && tx.transaction.endStatus === "non-existing"
    const contractTypeName = tx.contractName ?? tx.contractAbi?.contract_name?.trim() ?? "unknown"

    const tooltipData: NodeTransactionTooltipData = {
      contract: {
        typeName: contractTypeName,
        address: contractAddress,
      },
      account: {
        isCreated,
        isDestroyed,
      },
      storageDiff: buildStorageDiff(tx.parsedStorageBefore, tx.parsedStorageAfter),
    }

    showTooltip({
      x: rect.left,
      y: rect.top,
      content: (
        <NodeTransactionTooltipContent
          data={tooltipData}
          contracts={contracts}
          onContractClick={onContractClick}
        />
      ),
    })
  }

  const treeData: RawNodeDatum = useMemo<RawNodeDatum>(() => {
    const convertTransactionToNode = (tx: TransactionInfo): RawNodeDatum => {
      const thisAddress = tx.address
      const addressName = formatAddress(thisAddress, contracts)

      const computePhase = getTransactionComputePhase(tx.transaction)
      const actionPhase = getTransactionActionPhase(tx.transaction)

      const inMessage = tx.transaction.inMessage
      const withInitCode = inMessage?.init?.code !== undefined
      const isBounced = inMessage?.info.type === "internal" ? inMessage.info.bounced : false

      const isComputeSuccess = computePhase?.type === "vm" ? computePhase.success : true
      const isActionSuccess = actionPhase ? actionPhase.resultCode === 0 : true
      const isSuccess = isComputeSuccess && isActionSuccess
      const exitCode = computePhase?.type === "vm" ? computePhase.exitCode : undefined

      const value =
        inMessage?.info.type === "external-in"
          ? "—"
          : fmt.formatCurrency(
              inMessage?.info.type === "internal" ? inMessage.info.value.coins : undefined,
            )

      const opcode = getTransactionOpcode(tx.transaction)
      const targetContract = thisAddress ? contracts.get(thisAddress.toString()) : undefined
      const opcodeName = resolveTransactionOpcodeName(tx, contracts, allContracts)
      const opcodeHex = opcodeName ?? (opcode === undefined ? "empty" : `0x${opcode.toString(16)}`)

      const contractLetter = thisAddress ? (targetContract?.letter ?? "?") : "?"

      const lt = tx.lt
      const id = tx.id
      const isSelected = selectedTransactionIdState === id

      const hasExternalOut = [...tx.transaction.outMessages.values()].some(outMessage => {
        return outMessage.info.type === "external-out"
      })

      const externalOutChildren = hasExternalOut
        ? [
            {
              name: "",
              attributes: {
                isExternalOut: true,
                parentId: id,
              },
              children: [],
            },
          ]
        : []

      return {
        name: addressName,
        attributes: {
          from:
            getTransactionSourceLabel(tx.transaction) ??
            inMessage?.info.src?.toString() ??
            "unknown",
          to: inMessage?.info.dest?.toString() ?? "unknown",
          id,
          lt,
          success: isSuccess ? "✓" : "✗",
          exitCode: exitCode?.toString() ?? "0",
          value,
          opcode: opcodeHex,
          outMsgs: tx.transaction.outMessagesCount.toString(),
          withInitCode,
          isBounced,
          contractLetter,
          isSelected,
        },
        children: [...tx.children.map(it => convertTransactionToNode(it)), ...externalOutChildren],
      } satisfies RawNodeDatum
    }

    if (rootTransactions.length > 0) {
      const sharedInternalSource = getSharedInternalSource(rootTransactions)

      if (
        rootTransactions.length === 1 &&
        rootTransactions[0]?.transaction.inMessage?.info.type === "external-in"
      ) {
        return {
          name: "",
          attributes: {
            isRoot: "hidden",
            contractLetter: "",
          },
          children: [convertTransactionToNode(rootTransactions[0])],
        } satisfies RawNodeDatum
      }

      if (sharedInternalSource) {
        const sourceContract = contracts.get(sharedInternalSource.toString())

        return {
          name: formatAddress(sharedInternalSource, contracts),
          attributes: {
            isRoot: "source",
            contractLetter: sourceContract?.letter ?? "BL",
          },
          children: rootTransactions.map(it => convertTransactionToNode(it)),
        } satisfies RawNodeDatum
      }

      return {
        name: "",
        attributes: {
          isRoot: "true",
          contractLetter: "",
        },
        children: rootTransactions.map(it => convertTransactionToNode(it)),
      } satisfies RawNodeDatum
    }

    return {
      name: "No transactions",
      attributes: {
        isRoot: "false",
        contractLetter: "",
      },
      children: [],
    } satisfies RawNodeDatum
  }, [rootTransactions, contracts, selectedTransactionIdState, allContracts])

  const renderCustomNodeElement = ({nodeDatum}: CustomNodeElementProps): React.JSX.Element => {
    if (nodeDatum.attributes?.isRoot === "hidden") {
      return <g />
    }

    if (nodeDatum.attributes?.isRoot === "source") {
      return (
        <g className={styles.rootNode}>
          <circle
            r={15}
            fill={"var(--bg-color)"}
            stroke="var(--text-primary)"
            strokeWidth={1.5}
            className={styles.rootCircle}
          />
          <text
            fill="var(--text-primary)"
            strokeWidth="0"
            x="0"
            y="5"
            fontSize="14"
            fontWeight="bold"
            textAnchor="middle"
            className={styles.nodeText}
          >
            {(nodeDatum.attributes?.contractLetter as string) || "BL"}
          </text>
        </g>
      )
    }

    if (nodeDatum.attributes?.isRoot === "true") {
      return (
        <g className={styles.rootNode}>
          <circle
            r={15}
            fill={"var(--bg-color)"}
            stroke="var(--text-primary)"
            strokeWidth={1.5}
            className={styles.rootCircle}
          />
          <text
            fill="var(--text-primary)"
            strokeWidth="0"
            x="0"
            y="5"
            fontSize="14"
            fontWeight="bold"
            textAnchor="middle"
            className={styles.nodeText}
          >
            BL
          </text>
        </g>
      )
    }

    if (nodeDatum.attributes?.isExternalOut) {
      const parentId = nodeDatum.attributes.parentId as string
      const parentTx = transactionMap.get(parentId)

      const externalOutMessage = [...(parentTx?.transaction.outMessages.values() ?? [])].find(
        message => message.info.type === "external-out",
      )
      const externalOutDestination =
        externalOutMessage?.info.type === "external-out"
          ? (externalOutMessage.info.dest?.toString() ?? "External")
          : "External"
      const createdLt =
        externalOutMessage?.info.type === "external-out"
          ? externalOutMessage.info.createdLt.toString()
          : ""

      return (
        <g>
          <foreignObject
            width="4"
            height="6"
            x="-20"
            y="-3"
            className={styles.foreignObjectContainer}
          >
            <svg
              width="4"
              height="6"
              viewBox="0 0 4 5"
              xmlns="http://www.w3.org/2000/svg"
              className={styles.iconSvg}
            >
              <title>External Out</title>
              <path
                d="M0.400044 0.549983C0.648572 0.218612 1.11867 0.151455 1.45004 0.399983L3.45004 1.89998C3.6389 2.04162 3.75004 2.26392 3.75004 2.49998C3.75004 2.73605 3.6389 2.95834 3.45004 3.09998L1.45004 4.59998C1.11867 4.84851 0.648572 4.78135 0.400044 4.44998C0.151516 4.11861 0.218673 3.64851 0.550044 3.39998L1.75004 2.49998L0.550044 1.59998C0.218673 1.35145 0.151516 0.881354 0.400044 0.549983Z"
                fill="var(--text-muted)"
              ></path>
            </svg>
          </foreignObject>

          <circle
            r={15}
            fill="transparent"
            stroke="var(--border-color)"
            strokeWidth={1}
            className={styles.nodeCircleDefault}
          />

          <foreignObject
            width={TREE_EDGE_LABEL.width}
            height={TREE_EDGE_LABEL.height}
            x={TREE_EDGE_LABEL.x}
            y={TREE_EDGE_LABEL.y}
          >
            <div className={styles.edgeText}>
              <div className={styles.topText}>
                <p className={styles.edgeTextTitle}>{externalOutDestination}</p>
                <p className={styles.edgeTextContent}>Lt: {createdLt}</p>
              </div>
              <div className={styles.bottomText}>
                <p className={styles.edgeTextContent}>Type: external-out</p>
              </div>
            </div>
          </foreignObject>
        </g>
      )
    }

    const opcode = (nodeDatum.attributes?.opcode as string | undefined) ?? "empty opcode"
    const isSelected = nodeDatum.attributes?.isSelected as boolean
    const id = nodeDatum.attributes?.id as string
    const tx = transactionMap.get(id)
    const exitCode = (nodeDatum.attributes?.exitCode as string | undefined) ?? "0"
    const hasFailureDetails = exitCode !== "0"
    const successMark = nodeDatum.attributes?.success as string | undefined
    const isFailed = successMark !== "✓"
    const nodeCircleClassName = [
      styles.nodeCircle,
      isSelected ? styles.nodeCircleSelected : undefined,
    ]
      .filter(Boolean)
      .join(" ")

    return (
      <g>
        <foreignObject
          width="4"
          height="6"
          x="-20"
          y="-3"
          className={styles.foreignObjectContainer}
        >
          <svg
            width="4"
            height="6"
            viewBox="0 0 4 5"
            xmlns="http://www.w3.org/2000/svg"
            className={styles.iconSvg}
          >
            <title>Incoming Message</title>
            <path
              d="M0.400044 0.549983C0.648572 0.218612 1.11867 0.151455 1.45004 0.399983L3.45004 1.89998C3.6389 2.04162 3.75004 2.26392 3.75004 2.49998C3.75004 2.73605 3.6389 2.95834 3.45004 3.09998L1.45004 4.59998C1.11867 4.84851 0.648572 4.78135 0.400044 4.44998C0.151516 4.11861 0.218673 3.64851 0.550044 3.39998L1.75004 2.49998L0.550044 1.59998C0.218673 1.35145 0.151516 0.881354 0.400044 0.549983Z"
              fill="var(--text-muted)"
            ></path>
          </svg>
        </foreignObject>
        <circle
          r={15}
          role="button"
          tabIndex={0}
          aria-label={`Transaction ${id}`}
          fill={
            isFailed
              ? "var(--transaction-tree-failed-node-fill)"
              : isSelected
                ? "var(--text-primary)"
                : "var(--bg-color)"
          }
          stroke={isFailed ? "var(--transaction-tree-failed-node-stroke)" : "var(--text-primary)"}
          strokeWidth={isFailed ? 2 : 1.5}
          onClick={() => {
            handleNodeClick(id)
          }}
          onKeyDown={event => {
            if (event.key === "Enter" || event.key === " ") {
              handleNodeClick(id)
            }
          }}
          onMouseEnter={event => {
            if (!tx) return
            showNodeTransactionTooltip(event, tx)
          }}
          onMouseLeave={() => {
            hideTooltip()
          }}
          className={nodeCircleClassName}
        />

        <text
          fill={
            isFailed
              ? "var(--transaction-tree-failed-node-text)"
              : isSelected
                ? "var(--bg-color)"
                : "var(--text-primary)"
          }
          strokeWidth="0"
          x="0"
          y="5"
          fontSize="14"
          fontWeight="bold"
          textAnchor="middle"
          className={styles.nodeText}
        >
          {nodeDatum.attributes?.contractLetter as string}
        </text>
        {isFailed && (
          <g className={styles.failedBadge} aria-hidden="true">
            <circle
              cx={10}
              cy={10}
              r={5.4}
              fill="var(--transaction-tree-failed-badge-fill)"
              stroke="var(--transaction-tree-failed-badge-stroke)"
              strokeWidth={0.75}
            />
            <rect
              x={9.05}
              y={6.5}
              width={1.9}
              height={4.4}
              rx={0.95}
              strokeWidth={0}
              fill="var(--transaction-tree-failed-badge-text)"
              stroke="var(--transaction-tree-failed-badge-mark-stroke)"
            />
            <circle
              cx={10}
              cy={12.7}
              r={0.9}
              strokeWidth={0}
              fill="var(--transaction-tree-failed-badge-text)"
              stroke="var(--transaction-tree-failed-badge-mark-stroke)"
            />
          </g>
        )}
        <foreignObject
          width={TREE_EDGE_LABEL.width}
          height={hasFailureDetails ? TREE_EDGE_LABEL.failedHeight : TREE_EDGE_LABEL.height}
          x={TREE_EDGE_LABEL.x}
          y={TREE_EDGE_LABEL.y}
        >
          <div
            className={styles.edgeText}
            role="note"
            onMouseEnter={event => {
              if (!tx) return
              showEdgeTransactionTooltip(event, tx)
            }}
            onMouseLeave={() => {
              hideTooltip()
            }}
          >
            <div className={styles.topText}>
              <p className={styles.edgeTextTitle}>{nodeDatum.name}</p>
              {nodeDatum.attributes?.value && (
                <p className={styles.edgeTextContent}>{nodeDatum.attributes.value as string}</p>
              )}
            </div>
            <div className={styles.bottomText}>
              <p className={styles.edgeTextContent}>{opcode}</p>
              {hasFailureDetails && (
                <p className={styles.edgeTextContent}>
                  Exit: {exitCode} | Success: {successMark === "✓" ? "true" : "false"}
                </p>
              )}
            </div>
          </div>
        </foreignObject>
      </g>
    )
  }

  const getDynamicPathClass = ({target}: TreeLinkDatum): string => {
    const attributes = target.data.attributes
    if (attributes?.withInitCode) {
      return `${styles.edgeStyle} ${styles.edgeStyleWithInit}`
    }
    if (attributes?.isBounced) {
      return `${styles.edgeStyle} ${styles.edgeStyleBounced}`
    }

    return styles.edgeStyle
  }

  useEffect(() => {
    setSelectedTransactionIdState(selectedTransactionId)
  }, [selectedTransactionId])

  const selectedTransaction = useMemo(() => {
    return selectedTransactionIdState ? transactionMap.get(selectedTransactionIdState) : undefined
  }, [selectedTransactionIdState, transactionMap])

  useLayoutEffect(() => {
    const wrapper = treeWrapperRef.current
    const treeGroup = wrapper?.querySelector<SVGGElement>(".rd3t-g")

    if (!wrapper || !treeGroup) {
      return
    }

    const wrapperRect = wrapper.getBoundingClientRect()
    const groupRect = treeGroup.getBoundingClientRect()

    if (groupRect.width === 0 || groupRect.height === 0) {
      return
    }

    const groupLeft = groupRect.left - wrapperRect.left
    const groupTop = groupRect.top - wrapperRect.top
    const nextLayout: TreeLayout = {
      height: Math.max(
        TREE_MIN_SIZE.height,
        Math.ceil(groupRect.height + TREE_PADDING.top + TREE_PADDING.bottom + TREE_DETAILS_GAP),
      ),
      width: Math.max(
        TREE_MIN_SIZE.width,
        Math.ceil(groupRect.width + TREE_PADDING.left + TREE_PADDING.right),
      ),
      translate: {
        x: Math.round(treeLayout.translate.x + TREE_PADDING.left - groupLeft),
        y: Math.round(treeLayout.translate.y + TREE_PADDING.top - groupTop),
      },
    }

    const isSameLayout =
      Math.abs(treeLayout.height - nextLayout.height) <= 1 &&
      Math.abs(treeLayout.width - nextLayout.width) <= 1 &&
      Math.abs(treeLayout.translate.x - nextLayout.translate.x) <= 1 &&
      Math.abs(treeLayout.translate.y - nextLayout.translate.y) <= 1

    if (!isSameLayout) {
      setTreeLayout(nextLayout)
    }
  }, [treeData, treeLayout])

  useLayoutEffect(() => {
    const container = treeContainerRef.current
    const wrapper = treeWrapperRef.current
    const selectedId = selectedTransactionIdState
    if (!container || !wrapper || !selectedId) {
      return
    }

    const selectedNode = [
      ...wrapper.querySelectorAll<SVGCircleElement>('circle[aria-label^="Transaction "]'),
    ].find(node => node.getAttribute("aria-label") === `Transaction ${selectedId}`)

    if (!selectedNode) {
      return
    }

    const containerRect = container.getBoundingClientRect()
    const nodeRect = selectedNode.getBoundingClientRect()
    const inlineMargin = Math.min(96, container.clientWidth / 4)
    const nodeLeft = nodeRect.left - containerRect.left
    const nodeRight = nodeRect.right - containerRect.left

    if (nodeLeft >= inlineMargin && nodeRight <= container.clientWidth - inlineMargin) {
      return
    }

    const nodeCenter = container.scrollLeft + nodeLeft + nodeRect.width / 2
    const maxScrollLeft = container.scrollWidth - container.clientWidth
    const nextScrollLeft = Math.max(
      0,
      Math.min(maxScrollLeft, nodeCenter - container.clientWidth / 2),
    )

    if (Math.abs(container.scrollLeft - nextScrollLeft) > 1) {
      container.scrollTo({left: nextScrollLeft})
    }
  }, [selectedTransactionIdState, treeLayout])

  return (
    <div className={styles.container}>
      <div
        className={styles.treeContainer}
        ref={treeContainerRef}
        style={{height: `${treeLayout.height}px`}}
      >
        <div
          className={styles.treeWrapper}
          ref={treeWrapperRef}
          style={{width: `${treeLayout.width}px`}}
        >
          <Tree
            data={treeData}
            orientation="horizontal"
            pathFunc={event => {
              const t = event.target.data.attributes ?? {}
              return t.isFirst
                ? "M"
                    .concat(event.source.y.toString(), ",")
                    .concat(event.source.x.toString(), "V")
                    .concat((event.target.x + 10).toString(), "a10 10 0 0 1 10 -10H")
                    .concat((event.target.y - 18).toString())
                : t.isLast
                  ? "M"
                      .concat(event.source.y.toString(), ",")
                      .concat(event.source.x.toString(), "V")
                      .concat((event.target.x - 10).toString(), "a10 10 0 0 0 10 10H")
                      .concat((event.target.y - 18).toString())
                  : "M"
                      .concat(event.source.y.toString(), ",")
                      .concat(event.source.x.toString(), "V")
                      .concat(event.target.x.toString(), "H")
                      .concat((event.target.y - 18).toString())
            }}
            nodeSize={TREE_NODE_SIZE}
            separation={TREE_SEPARATION}
            renderCustomNodeElement={renderCustomNodeElement}
            pathClassFunc={getDynamicPathClass}
            translate={treeLayout.translate}
            zoom={1}
            enableLegacyTransitions={false}
            collapsible={false}
            zoomable={false}
            draggable={false}
            scaleExtent={{min: 1, max: 1}}
          />
          {tooltip && triggerRectReference.current && (
            <SmartTooltip
              content={tooltip.content}
              triggerRect={triggerRectReference.current}
              onMouseEnter={() => {
                setIsTooltipHovered(true)
              }}
              onMouseLeave={() => {
                setIsTooltipHovered(false)
              }}
              onForceHide={forceHideTooltip}
              calculateOptimalPosition={calculateOptimalPosition}
            />
          )}
        </div>
      </div>

      {selectedTransaction && (
        <div className={styles.transactionDetails}>
          <TransactionDetails
            tx={selectedTransaction}
            contracts={contracts}
            compilerAbisByCodeHash={compilerAbisByCodeHash}
            allContracts={allContracts}
            onContractClick={onContractClick}
            renderSourceLocation={renderSourceLocation}
            loadActions={loadActions}
            renderMessageRouteAction={renderSelectedTransactionMessageRouteAction}
          />
          {renderSelectedTransactionExtra?.(selectedTransaction)}
        </div>
      )}
    </div>
  )
}

function formatAddress(address: Address | undefined, contracts: Map<string, ContractData>): string {
  if (!address) {
    return "unknown"
  }

  const displayAddress = address.toString({testOnly: true})
  const addressString = address.toString()
  const meta = contracts.get(addressString)
  if (meta) {
    const name = meta.displayName
    if (name !== "Unknown Contract") {
      return name
    }
  }

  return `${displayAddress.slice(0, 5)}...${displayAddress.slice(-5)}`
}

function getSharedInternalSource(
  rootTransactions: readonly TransactionInfo[],
): Address | undefined {
  if (rootTransactions.length === 0) {
    return undefined
  }

  const firstInMessage = rootTransactions[0]?.transaction.inMessage
  if (firstInMessage?.info.type !== "internal") {
    return undefined
  }

  const source = firstInMessage.info.src
  const sourceAddress = source.toString()

  const allShareSameInternalSource = rootTransactions.every(tx => {
    const inMessage = tx.transaction.inMessage
    return inMessage?.info.type === "internal" && inMessage.info.src.toString() === sourceAddress
  })

  return allShareSameInternalSource ? source : undefined
}
