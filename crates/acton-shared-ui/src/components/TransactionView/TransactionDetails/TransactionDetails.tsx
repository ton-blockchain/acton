import * as React from "react"
import {useEffect, useRef, useState} from "react"
import {FiChevronDown, FiChevronUp} from "react-icons/fi"
import type {Cell} from "@ton/core"

import type {BackendContractInfo, SourceLocation} from "@/types"
import type {ContractData, LoadedTransactionActions, TransactionInfo} from "@/types/transaction"
import {DataBlock, fmt} from "@/index"
import {decodeMessageBody, decodeStateInitData, getShardAccountBalance} from "@/utils/messageBody"
import {
  computeSendMode,
  getTransactionActionPhase,
  getTransactionComputePhase,
  getTransactionOpcode,
  getTransactionSourceLabel,
  getTransactionTriggerLabel,
  resolveTransactionOpcodeName,
} from "@/utils/transaction"

import {ParsedBodySection} from "../ParsedBodySection/ParsedBodySection"
import {ContractChip} from "../ContractChip/ContractChip"
import {DisasmSection} from "../DisasmSection/DisasmSection"
import {ExitCodeChip} from "../ExitCodeChip/ExitCodeChip"
import {OpcodeChip} from "../OpcodeChip/OpcodeChip"
import {ParsedValueView} from "../ParsedValueView/ParsedValueView"
import {SendModeViewer} from "../SendModeViewer/SendModeViewer"
import {StorageDiffView} from "../TransactionTree/StorageDiffView"
import {buildStorageDiff} from "../TransactionTree/storageDiff"

import {ActionsSummary} from "./ActionsSummary"
import styles from "./TransactionDetails.module.css"

export interface TransactionDetailsProps {
  readonly tx: TransactionInfo
  readonly contracts: Map<string, ContractData>
  readonly compilerAbisByCodeHash?: ReadonlyMap<string, ContractData["abi"]>
  readonly allContracts: readonly BackendContractInfo[]
  readonly onContractClick?: (address: string) => void
  readonly renderSourceLocation?: (location: SourceLocation) => React.ReactNode
  readonly loadActions?: (tx: TransactionInfo) => Promise<LoadedTransactionActions>
  readonly renderMessageRouteAction?: (tx: TransactionInfo) => React.ReactNode
}

export function TransactionDetails({
  tx,
  contracts,
  compilerAbisByCodeHash,
  allContracts,
  onContractClick,
  renderSourceLocation,
  loadActions,
  renderMessageRouteAction,
}: TransactionDetailsProps): React.JSX.Element {
  const [showActions, setShowActions] = useState(false)
  const [showStateInit, setShowStateInit] = useState(false)
  const [expandedStorageLt, setExpandedStorageLt] = useState<string | undefined>()
  const [loadedActions, setLoadedActions] = useState<LoadedTransactionActions | undefined>()
  const [isLoadingActions, setIsLoadingActions] = useState(false)
  const [loadActionsError, setLoadActionsError] = useState<string | undefined>()
  const currentTxIdRef = useRef(tx.id)

  useEffect(() => {
    currentTxIdRef.current = tx.id
    setShowActions(false)
    setLoadedActions(undefined)
    setIsLoadingActions(false)
    setLoadActionsError(undefined)
  }, [tx.id])

  const description = tx.transaction.description
  if (description.type !== "generic" && description.type !== "tick-tock") {
    return (
      <div className={styles.transactionDetailsContainer}>
        <div className={styles.detailRow}>
          <div className={styles.detailValue}>
            Non-generic transaction not supported (Type: {description.type})
          </div>
        </div>
      </div>
    )
  }

  const isTickTock = description.type === "tick-tock"
  const tickTockDescription = description.type === "tick-tock" ? description : undefined
  const computePhase = getTransactionComputePhase(tx.transaction)
  const actionPhase = getTransactionActionPhase(tx.transaction)
  const triggerLabel = getTransactionTriggerLabel(tx.transaction)
  const messageRouteAction = renderMessageRouteAction?.(tx)

  if (!computePhase) {
    return (
      <div className={styles.transactionDetailsContainer}>
        <div className={styles.detailRow}>
          <div className={styles.detailValue}>
            Transaction compute phase unavailable (Type: {description.type})
          </div>
        </div>
      </div>
    )
  }

  const formatBoolean = (v: boolean): React.JSX.Element => (
    <span className={v ? styles.booleanTrue : styles.booleanFalse}>{v ? "Yes" : "No"}</span>
  )
  const formatStatusChange = (value: "unchanged" | "frozen" | "deleted"): string => {
    switch (value) {
      case "unchanged": {
        return "Unchanged"
      }
      case "frozen": {
        return "Frozen"
      }
      case "deleted": {
        return "Deleted"
      }
    }
  }

  const inMessage = tx.transaction.inMessage ?? undefined
  const targetContract = tx.address ? contracts.get(tx.address.toString()) : undefined
  const targetAbi = tx.contractAbi ?? targetContract?.abi
  const targetContractWithAbi =
    targetContract && targetAbi && targetContract.abi !== targetAbi
      ? {...targetContract, abi: targetAbi}
      : targetContract
  const sourceLabel = getTransactionSourceLabel(tx.transaction)
  const hasMessageBody =
    inMessage != undefined &&
    (() => {
      const body = inMessage.body.asSlice()
      return body.remainingBits > 0 || body.remainingRefs > 0
    })()
  const stateInitCode = inMessage?.init?.code ?? undefined
  const stateInitData = inMessage?.init?.data ?? undefined
  const stateInitCodeBocHex = stateInitCode ? formatCellBocHex(stateInitCode) : undefined
  const stateInitCodeHash = stateInitCode?.hash().toString("hex")
  const stateInitAbiName = stateInitCodeHash
    ? compilerAbisByCodeHash?.get(stateInitCodeHash)?.contract_name?.trim()
    : undefined
  const parsedBody =
    tx.parsedBody ??
    (inMessage ? decodeMessageBody(inMessage, contracts, tx.address?.toString()) : undefined)
  const parsedStateInitData = decodeStateInitData(
    stateInitData,
    targetContractWithAbi,
    tx.contractName,
    allContracts,
  )
  const sendMode = computeSendMode(tx)

  const opcode = getTransactionOpcode(tx.transaction)
  const opcodeName = resolveTransactionOpcodeName(tx, contracts, allContracts)
  const resolvedOutActions = loadedActions?.outActions ?? tx.outActions
  const resolvedExecutorActions = loadedActions?.executorActions ?? tx.executorActions

  const sentTotal = [...tx.transaction.outMessages.values()].reduce(
    (accumulator: bigint, message) =>
      accumulator + (message.info.type === "internal" ? message.info.value.coins : 0n),
    0n,
  )
  const actionFee = actionPhase?.totalActionFees ?? undefined
  const endBalance = tx.accountBalanceAfter ?? getShardAccountBalance(tx.shardAccountAfter)
  const tickTockStorageFeesDue = tickTockDescription?.storagePhase.storageFeesDue
  const hasAccountStatusChange = tx.transaction.oldStatus !== tx.transaction.endStatus
  const storageDiff = buildStorageDiff(tx.parsedStorageBefore, tx.parsedStorageAfter)
  const showStorageDiff = expandedStorageLt === tx.lt
  const storageChangeLabel =
    storageDiff === undefined
      ? undefined
      : storageDiff.status === "unchanged"
        ? "Intact"
        : "Changed"

  const hasResolvedActions = resolvedOutActions.length > 0
  const canLoadActions =
    !hasResolvedActions && actionPhase != null && actionPhase.totalActions > 0 && loadActions !== undefined
  const canToggleActions = hasResolvedActions || canLoadActions
  const handleActionsToggle = async () => {
    if (hasResolvedActions) {
      setShowActions(!showActions)
      return
    }

    if (!canLoadActions || isLoadingActions) {
      return
    }

    const requestedTxId = tx.id
    setIsLoadingActions(true)
    setLoadActionsError(undefined)
    try {
      const nextActions = await loadActions(tx)
      if (currentTxIdRef.current !== requestedTxId) {
        return
      }

      if (nextActions.outActions.length === 0) {
        setLoadActionsError("No actions returned by retrace")
        return
      }

      setLoadedActions(nextActions)
      setShowActions(true)
    } catch (error) {
      if (currentTxIdRef.current !== requestedTxId) {
        return
      }

      setLoadActionsError(error instanceof Error ? error.message : "Failed to load actions")
    } finally {
      if (currentTxIdRef.current === requestedTxId) {
        setIsLoadingActions(false)
      }
    }
  }

  return (
    <div className={styles.transactionDetailsContainer}>
      <div className={styles.detailRow}>
        <div className={styles.detailLabel}>{isTickTock ? "Trigger" : "Message Route"}</div>
        <div className={styles.heightDetailValue}>
          <div className={styles.messageRouteValue}>
            {isTickTock ? (
              <span className={styles.triggerRoute}>
                <span className={styles.triggerKind}>{triggerLabel ?? "Tick-Tock"}</span>
                <span aria-hidden="true">→</span>
                <ContractChip
                  address={tx.address?.toString()}
                  contracts={contracts}
                  onContractClick={onContractClick}
                />
              </span>
            ) : (
              <span className={styles.triggerRoute}>
                {sourceLabel ? (
                  <span className={styles.messageEndpointBadge}>{sourceLabel}</span>
                ) : (
                  <ContractChip
                    address={tx.transaction.inMessage?.info.src?.toString()}
                    contracts={contracts}
                    onContractClick={onContractClick}
                  />
                )}
                {" → "}
                <ContractChip
                  address={tx.transaction.inMessage?.info.dest?.toString()}
                  contracts={contracts}
                  onContractClick={onContractClick}
                />
              </span>
            )}
            {messageRouteAction && (
              <span className={styles.messageRouteAction}>{messageRouteAction}</span>
            )}
          </div>
        </div>
      </div>

      {isTickTock && (
        <div className={styles.labeledSectionRow}>
          <div className={styles.labeledSectionTitle}>Tick-Tock</div>
          <div className={styles.labeledSectionContent}>
            <div className={styles.multiColumnRow}>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Kind</div>
                <div className={styles.multiColumnItemValue}>{triggerLabel ?? "Tick-Tock"}</div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Aborted</div>
                <div className={styles.multiColumnItemValue}>
                  {formatBoolean(tickTockDescription?.aborted ?? false)}
                </div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Destroyed</div>
                <div className={styles.multiColumnItemValue}>
                  {formatBoolean(tickTockDescription?.destroyed ?? false)}
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {!isTickTock && inMessage && inMessage.info.type === "internal" && (
        <div className={styles.labeledSectionRow}>
          <div className={styles.labeledSectionTitle}>In Message</div>

          <div className={styles.labeledSectionContent}>
            <div className={styles.multiColumnRow}>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Value</div>
                <div className={`${styles.multiColumnItemValue}`}>
                  {fmt.formatCurrency(inMessage.info.value.coins)}
                </div>
              </div>
              {sendMode !== undefined && (
                <div className={styles.multiColumnItem}>
                  <div className={styles.multiColumnItemTitle}>Send Mode</div>
                  <div className={`${styles.multiColumnItemValue} ${styles.numberValue}`}>
                    <SendModeViewer mode={sendMode} />
                  </div>
                </div>
              )}
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Bounced</div>
                <div className={styles.multiColumnItemValue}>
                  {formatBoolean(inMessage.info.bounced)}
                </div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Bounce</div>
                <div className={styles.multiColumnItemValue}>
                  {formatBoolean(inMessage.info.bounce)}
                </div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Created At</div>
                <div
                  className={`${styles.multiColumnItemValue} ${styles.timestampValue}`}
                  data-visual-dynamic="timestamp"
                  data-visual-placeholder="<timestamp>"
                >
                  {formatDetailedTimestamp(inMessage.info.createdAt, false)}
                </div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Created Lt</div>
                <div className={`${styles.multiColumnItemValue} ${styles.numberValue}`}>
                  {inMessage.info.createdLt.toString()}
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {!isTickTock && (
        <div className={styles.labeledSectionRow}>
          <div className={styles.labeledSectionTitle}>Message Data</div>
          <div className={styles.labeledSectionContent}>
            <div className={styles.multiColumnRow}>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Opcode</div>
                <div className={styles.multiColumnItemValue}>
                  <OpcodeChip opcode={opcode} abiName={opcodeName} showOpcode={true} />
                </div>
              </div>
            </div>
            {parsedBody && hasMessageBody && (
              <ParsedBodySection
                key={tx.lt}
                parsedBody={parsedBody}
                contracts={contracts}
                onContractClick={onContractClick}
              />
            )}
            {(stateInitCode || stateInitData) && (
              <div className={styles.parsedBodySection}>
                <div className={styles.parsedBodyTitle}>
                  State Init
                  <button
                    type="button"
                    onClick={() => {
                      setShowStateInit(!showStateInit)
                    }}
                    className={styles.actionsToggleButton}
                    aria-label={showStateInit ? "Hide state init" : "Show state init"}
                  >
                    {showStateInit ? <FiChevronUp size={14} /> : <FiChevronDown size={14} />}
                    <span className={styles.actionsToggleText}>
                      {showStateInit ? "Hide" : "Show"}
                    </span>
                  </button>
                </div>
                {showStateInit && (
                  <div className={styles.stateInitSection}>
                    {stateInitCode && (
                      <div className={styles.stateInitField}>
                        <div className={styles.multiColumnItemTitle}>Code</div>
                        <DataBlock data={stateInitCodeBocHex!} />
                        <DisasmSection bocHex={stateInitCodeBocHex!} title="Code Disassembly" />
                      </div>
                    )}
                    {stateInitAbiName && (
                      <div className={styles.stateInitField}>
                        <div className={styles.multiColumnItemTitle}>ABI</div>
                        <div className={styles.multiColumnItemValue}>{stateInitAbiName}</div>
                      </div>
                    )}
                    {stateInitData && (
                      <div className={styles.stateInitField}>
                        <div className={styles.multiColumnItemTitle}>Data</div>
                        {parsedStateInitData ? (
                          <div className={styles.parsedBodyTree}>
                            <div className={styles.parsedBodyContent}>
                              <ParsedValueView
                                value={parsedStateInitData.value}
                                contracts={contracts}
                                onContractClick={onContractClick}
                                fallbackTypeName={parsedStateInitData.name}
                              />
                            </div>
                          </div>
                        ) : (
                          <DataBlock data={formatCellBocHex(stateInitData)} />
                        )}
                      </div>
                    )}
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      )}

      <div className={styles.labeledSectionRow}>
        <div className={styles.labeledSectionTitle}>Storage</div>
        <div className={styles.labeledSectionContent}>
          <div className={styles.storageSummaryRow}>
            <div className={styles.storageSummaryMain}>
              {storageChangeLabel && (
                <span className={styles.storageChangeBadge}>{storageChangeLabel}</span>
              )}
              {!storageDiff && (
                <span className={styles.storageUnavailable}>Storage data unavailable</span>
              )}
              {hasAccountStatusChange && (
                <span className={styles.storageAccountStatus}>
                  {formatAccountStatus(tx.transaction.oldStatus)} →{" "}
                  {formatAccountStatus(tx.transaction.endStatus)}
                </span>
              )}
            </div>
            {storageDiff && (
              <button
                type="button"
                onClick={() => {
                  setExpandedStorageLt(showStorageDiff ? undefined : tx.lt)
                }}
                className={`${styles.actionsToggleButton} ${styles.storageToggleButton}`}
                aria-label={
                  showStorageDiff ? "Hide storage state change" : "Show storage state change"
                }
              >
                {showStorageDiff ? <FiChevronUp size={14} /> : <FiChevronDown size={14} />}
                <span className={styles.actionsToggleText}>
                  {showStorageDiff ? "Hide" : "Show"}
                </span>
              </button>
            )}
          </div>

          {showStorageDiff && storageDiff && (
            <div className={styles.storageDiffDetails} data-testid="storage-diff-details">
              <StorageDiffView
                diff={storageDiff}
                contracts={contracts}
                onContractClick={onContractClick}
              />
            </div>
          )}
        </div>
      </div>

      <div className={styles.labeledSectionRow}>
        <div className={styles.labeledSectionTitle}>Fees & Sent</div>
        <div className={styles.labeledSectionContent}>
          <div className={styles.multiColumnRow}>
            <div className={styles.multiColumnItem}>
              <div className={styles.multiColumnItemTitle}>Amount Sent (Total)</div>
              <div className={`${styles.multiColumnItemValue}`}>
                {fmt.formatCurrency(sentTotal)}
              </div>
            </div>
            <div className={styles.multiColumnItem}>
              <div className={styles.multiColumnItemTitle}>End Balance</div>
              <div className={`${styles.multiColumnItemValue}`}>
                {endBalance === undefined ? "—" : fmt.formatCurrency(endBalance)}
              </div>
            </div>
            <div className={styles.multiColumnItem}>
              <div className={styles.multiColumnItemTitle}>Total Fee</div>
              <div className={`${styles.multiColumnItemValue}`}>
                {fmt.formatCurrency(tx.transaction.totalFees.coins)}
              </div>
            </div>
            {actionPhase && (
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Action Fee</div>
                <div className={`${styles.multiColumnItemValue}`}>
                  {actionFee === undefined ? "—" : fmt.formatCurrency(actionFee)}
                </div>
              </div>
            )}
            {tx.transaction.inMessage?.info.type === "internal" && (
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Forward Fee</div>
                <div className={`${styles.multiColumnItemValue}`}>
                  {fmt.formatCurrency(tx.transaction.inMessage.info.forwardFee)}
                </div>
              </div>
            )}
          </div>
        </div>
      </div>

      {tickTockDescription && (
        <div className={styles.labeledSectionRow}>
          <div className={styles.labeledSectionTitle}>Storage Phase</div>
          <div className={styles.labeledSectionContent}>
            <div className={styles.multiColumnRow}>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Storage Fee</div>
                <div className={styles.multiColumnItemValue}>
                  {fmt.formatCurrency(tickTockDescription.storagePhase.storageFeesCollected)}
                </div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Storage Due</div>
                <div className={styles.multiColumnItemValue}>
                  {typeof tickTockStorageFeesDue === "bigint"
                    ? fmt.formatCurrency(tickTockStorageFeesDue)
                    : "—"}
                </div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Status Change</div>
                <div className={styles.multiColumnItemValue}>
                  {formatStatusChange(tickTockDescription.storagePhase.statusChange)}
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      <div className={styles.labeledSectionRow}>
        <div className={styles.labeledSectionTitle}>Compute Phase</div>
        <div className={styles.labeledSectionContent}>
          {computePhase.type === "skipped" ? (
            <div className={styles.multiColumnItemValue}>Skipped ({computePhase.reason})</div>
          ) : (
            <div className={styles.multiColumnRow}>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Success</div>
                <div className={styles.multiColumnItemValue}>
                  {formatBoolean(computePhase.success)}
                </div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Exit Code</div>
                <div className={styles.multiColumnItemValue}>
                  <ExitCodeChip exitCode={computePhase.exitCode} abi={targetAbi} />
                </div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>VM Steps</div>
                <div className={`${styles.multiColumnItemValue} ${styles.numberValue}`}>
                  {computePhase.vmSteps}
                </div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Gas Used</div>
                <div className={styles.multiColumnItemValue}>{computePhase.gasUsed.toString()}</div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Gas Fee</div>
                <div className={styles.multiColumnItemValue}>
                  {fmt.formatCurrency(computePhase.gasFees)}
                </div>
              </div>
            </div>
          )}
        </div>
      </div>

      <div className={styles.labeledSectionRow}>
        <div className={styles.labeledSectionTitle}>Action Phase</div>
        <div className={styles.labeledSectionContent}>
          {actionPhase ? (
            <div className={styles.multiColumnRow}>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Success</div>
                <div
                  className={`${styles.multiColumnItemValue} ${actionPhase.success ? styles.booleanTrue : styles.booleanFalse}`}
                >
                  {formatBoolean(actionPhase.success)}
                </div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Exit Code</div>
                <div className={styles.multiColumnItemValue}>
                  <ExitCodeChip exitCode={actionPhase.resultCode} abi={targetAbi} phase="action" />
                </div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Total Actions</div>
                <div className={`${styles.multiColumnItemValue} ${styles.numberValue}`}>
                  {fmt.formatNumber(actionPhase.totalActions)}
                  {canToggleActions && (
                    <button
                      type="button"
                      onClick={() => void handleActionsToggle()}
                      className={styles.actionsToggleButton}
                      aria-label={showActions ? "Hide actions" : "Show actions"}
                      disabled={isLoadingActions}
                    >
                      {showActions ? <FiChevronUp size={14} /> : <FiChevronDown size={14} />}
                      <span className={styles.actionsToggleText}>
                        {isLoadingActions ? "Loading" : showActions ? "Hide" : "Show"}
                      </span>
                    </button>
                  )}
                </div>
              </div>
            </div>
          ) : (
            <div className={styles.multiColumnItemValue}>No action phase</div>
          )}
          {loadActionsError && <div className={styles.actionsLoadError}>{loadActionsError}</div>}
        </div>
      </div>

      {showActions && hasResolvedActions && (
        <div className={styles.labeledSectionRow}>
          <div className={styles.labeledSectionTitle}>Actions Details</div>
          <div className={styles.labeledSectionContent}>
            <ActionsSummary
              actions={resolvedOutActions}
              executorActions={resolvedExecutorActions}
              contracts={contracts}
              contractAddress={tx.address?.toString() ?? ""}
              renderSourceLocation={renderSourceLocation}
            />
          </div>
        </div>
      )}

      <div className={styles.detailRow}>
        <div className={styles.detailLabel}>Time</div>
        <div
          className={`${styles.detailValue} ${styles.timestampValue}`}
          data-visual-dynamic="timestamp"
          data-visual-placeholder="<timestamp>"
        >
          {formatDetailedTimestamp(tx.transaction.now)}
        </div>
      </div>
    </div>
  )
}

function formatAccountStatus(status: string): string {
  switch (status) {
    case "non-existing": {
      return "Non-existing"
    }
    case "uninitialized": {
      return "Uninitialized"
    }
    case "active": {
      return "Active"
    }
    case "frozen": {
      return "Frozen"
    }
    default: {
      return status
    }
  }
}

function formatCellBocHex(cell: Cell): string {
  return cell.toBoc({idx: false, crc32: false}).toString("hex")
}

function formatDetailedTimestamp(
  timestampInput: number | string | undefined,
  showShort = true,
): React.JSX.Element | string {
  if (timestampInput === undefined) return "—"

  const date =
    typeof timestampInput === "string" ? new Date(timestampInput) : new Date(timestampInput * 1000)

  const pad = (number: number): string => number.toString().padStart(2, "0")
  const monthAbbrs = [
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "May",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec",
  ]

  const day = date.getDate()
  const monthIndex = date.getMonth()
  const monthNumber = monthIndex + 1
  const year = date.getFullYear()
  const hours = date.getHours()
  const minutes = date.getMinutes()
  const seconds = date.getSeconds()

  const fullPart = `${pad(day)}.${pad(monthNumber)}.${year}, ${pad(hours)}:${pad(minutes)}:${pad(seconds)}`
  const shortPart = `${pad(day)} ${monthAbbrs[monthIndex]}, ${pad(hours)}:${pad(minutes)}`

  return (
    <>
      {fullPart}
      {showShort && <span className={styles.timestampDetailSecondary}> — {shortPart}</span>}
    </>
  )
}
