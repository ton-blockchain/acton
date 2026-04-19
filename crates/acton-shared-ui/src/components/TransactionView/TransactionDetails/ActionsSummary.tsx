import type {OutAction} from "@ton/core"
import React, {useState} from "react"

import type {BackendExecutorAction, BackendExecutorActionFailureReason} from "@/types"
import type {ContractData} from "@/types/transaction"
import {fmt, DataBlock} from "@/index"
import {parseSendMode} from "@/components/TransactionView/SendModeViewer/parser"
import {parseReserveMode} from "@/utils/transaction"

import {ContractChip} from "../ContractChip/ContractChip"
import {ExitCodeChip} from "../ExitCodeChip/ExitCodeChip"
import {ReserveModeViewer} from "../ReserveModeViewer/ReserveModeViewer"

import styles from "./ActionsSummary.module.css"

interface ActionsSummaryProps {
  readonly actions: readonly OutAction[]
  readonly executorActions?: readonly BackendExecutorAction[]
  readonly contracts: Map<string, ContractData>
  readonly contractAddress: string
  readonly onContractClick?: (address: string) => void
}

interface ActionExecutionMeta {
  readonly isFailed: boolean
  readonly failureCode: number | undefined
  readonly failureReasonText: string | undefined
}

const getActionIcon = (actionType: OutAction["type"]): React.JSX.Element => {
  switch (actionType) {
    case "sendMsg": {
      return (
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
          <title>Send Message</title>
          <path
            d="M22 2L11 13M22 2L15 22L11 13L2 9L22 2Z"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      )
    }
    case "setCode": {
      return (
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
          <title>Set Code</title>
          <path
            d="M16 18L22 12L16 6M8 6L2 12L8 18"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      )
    }
    case "reserve": {
      return (
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
          <title>Reserve</title>
          <path
            d="M12 1V23M17 5H9.5C8.57174 5 7.6815 5.36875 7.02513 6.02513C6.36875 6.6815 6 7.57174 6 8.5C6 9.42826 6.36875 10.3185 7.02513 10.9749C7.6815 11.6313 8.57174 12 9.5 12H14.5C15.4283 12 16.3185 12.3687 16.9749 13.0251C17.6313 13.6815 18 14.5717 18 15.5C18 16.4283 17.6313 17.3185 16.9749 17.9749C16.3185 18.6313 15.4283 19 14.5 19H6"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      )
    }
    default: {
      return (
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
          <title>Action</title>
          <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="2" />
          <path d="M9 9h6v6H9z" fill="currentColor" />
        </svg>
      )
    }
  }
}

const formatBoolean = (v: boolean): React.JSX.Element => (
  <span className={v ? styles.booleanTrue : styles.booleanFalse}>{v ? "Yes" : "No"}</span>
)

const formatModeNames = (names: readonly string[]): string =>
  names.length > 0 ? names.join(" + ") : "—"

const formatSendModeNames = (mode: number): string => {
  return formatModeNames(parseSendMode(mode).map(flag => flag.name))
}

const getReserveModeNames = (mode: number): readonly string[] => {
  return parseReserveMode(mode).map(flag => flag.name)
}

const formatReserveModeNames = (mode: number): string => {
  return formatModeNames(getReserveModeNames(mode))
}

const isActionFailed = (action: BackendExecutorAction): boolean => {
  return action.failure_code !== undefined || action.failure_reason !== undefined
}

const formatNanoTon = (value: string): string => {
  try {
    return fmt.formatCurrency(BigInt(value))
  } catch {
    return `${value} ng`
  }
}

const formatFailureReason = (
  reason: BackendExecutorActionFailureReason | undefined,
): string | undefined => {
  if (!reason) return

  switch (reason.type) {
    case "not_enough_toncoin_to_send": {
      return `Not enough Toncoin: balance ${formatNanoTon(reason.remaining_balance)}, required ${formatNanoTon(reason.required)}.`
    }
    case "cannot_reserve_toncoin": {
      return `Cannot reserve ${formatNanoTon(reason.requested)}: only ${formatNanoTon(reason.available)} available.`
    }
  }
}

const mapExecutorActionsByType = (
  actions: readonly OutAction[],
  executorActions: readonly BackendExecutorAction[],
): Array<BackendExecutorAction | undefined> => {
  const mapped: Array<BackendExecutorAction | undefined> = []
  let cursor = 0

  for (const action of actions) {
    if (action.type !== "sendMsg" && action.type !== "reserve") {
      mapped.push(undefined)
      continue
    }

    let matched: BackendExecutorAction | undefined
    while (cursor < executorActions.length) {
      const candidate = executorActions[cursor]
      cursor += 1
      const typeMatches =
        (action.type === "sendMsg" && candidate.type === "send_message") ||
        (action.type === "reserve" && candidate.type === "reserve_currency")

      if (typeMatches) {
        matched = candidate
        break
      }
    }

    mapped.push(matched)
  }

  return mapped
}

const getActionExecutionMeta = (
  executorAction: BackendExecutorAction | undefined,
): ActionExecutionMeta => ({
  isFailed: executorAction ? isActionFailed(executorAction) : false,
  failureCode: executorAction?.failure_code,
  failureReasonText: formatFailureReason(executorAction?.failure_reason),
})

const renderActionDetails = (
  action: OutAction,
  executorAction: BackendExecutorAction | undefined,
  contractAddress: string,
  contracts: Map<string, ContractData>,
  onContractClick?: (address: string) => void,
): React.JSX.Element | undefined => {
  const execution = getActionExecutionMeta(executorAction)
  const contract = contracts.get(contractAddress)
  const contractAbi = contract?.abi
  const contractCompilerAbi = contract?.compilerAbi

  switch (action.type) {
    case "sendMsg": {
      const message = action.outMsg
      const info = message.info

      return (
        <div className={styles.actionDetails}>
          <div className={styles.detailsHeader}>
            <h4>Details</h4>
          </div>
          <div className={styles.detailsContent}>
            <div className={styles.detailRow}>
              <span className={styles.detailLabel}>Mode:</span>
              <span className={styles.detailValue}>{formatSendModeNames(action.mode)}</span>
            </div>
            <div className={styles.detailRow}>
              <span className={styles.detailLabel}>Type:</span>
              <span className={styles.detailValue}>{info.type}</span>
            </div>
            {info.type === "internal" && (
              <>
                <div className={styles.detailRow}>
                  <span className={styles.detailLabel}>From:</span>
                  <div className={styles.detailValue}>
                    <ContractChip
                      address={contractAddress}
                      contracts={contracts}
                      onContractClick={onContractClick}
                    />
                  </div>
                </div>
                <div className={styles.detailRow}>
                  <span className={styles.detailLabel}>To:</span>
                  <div className={styles.detailValue}>
                    <ContractChip
                      address={info.dest.toString()}
                      contracts={contracts}
                      onContractClick={onContractClick}
                    />
                  </div>
                </div>
                <div className={styles.detailRow}>
                  <span className={styles.detailLabel}>Value:</span>
                  <span className={styles.detailValue}>{fmt.formatCurrency(info.value.coins)}</span>
                </div>
                <div className={styles.detailRow}>
                  <span className={styles.detailLabel}>Bounce:</span>
                  <span className={styles.detailValue}>{formatBoolean(info.bounce)}</span>
                </div>
                <div className={styles.detailRow}>
                  <span className={styles.detailLabel}>Bounced:</span>
                  <span className={styles.detailValue}>{formatBoolean(info.bounced)}</span>
                </div>
              </>
            )}
            {info.type === "external-out" && (
              <>
                <div className={styles.detailRow}>
                  <span className={styles.detailLabel}>From:</span>
                  <div className={styles.detailValue}>
                    <ContractChip
                      address={contractAddress}
                      contracts={contracts}
                      onContractClick={onContractClick}
                    />
                  </div>
                </div>
                <div className={styles.detailRow}>
                  <span className={styles.detailLabel}>To:</span>
                  <div className={styles.detailValue}>
                    {info.dest ? (
                      <ContractChip
                        address={info.dest.toString()}
                        contracts={contracts}
                        onContractClick={onContractClick}
                      />
                    ) : (
                      "External"
                    )}
                  </div>
                </div>
              </>
            )}
            <div className={styles.detailRow}>
              <span className={styles.detailLabel}>Body:</span>
              <div className={styles.detailValue}>
                <DataBlock data={message.body.toBoc().toString("hex")} />
              </div>
            </div>
            {execution.failureReasonText && (
              <div className={styles.detailRow}>
                <span className={styles.detailLabel}>Failure:</span>
                <span className={`${styles.detailValue} ${styles.detailFailure}`}>
                  {execution.failureReasonText}
                </span>
              </div>
            )}
            {execution.failureCode !== undefined && (
              <div className={styles.detailRow}>
                <span className={styles.detailLabel}>Exit Code:</span>
                <span className={styles.detailValue}>
                  <ExitCodeChip
                    exitCode={execution.failureCode}
                    abi={contractAbi}
                    compilerAbi={contractCompilerAbi}
                    phase="action"
                  />
                </span>
              </div>
            )}
          </div>
        </div>
      )
    }
    case "setCode": {
      return (
        <div className={styles.actionDetails}>
          <div className={styles.detailsHeader}>
            <h4>Details</h4>
          </div>
          <div className={styles.detailsContent}>
            <div className={styles.detailRow}>
              <span className={styles.detailLabel}>New Code Hash:</span>
              <span className={styles.detailValue}>
                <DataBlock data={action.newCode.toBoc().toString("hex")} />
              </span>
            </div>
          </div>
        </div>
      )
    }
    case "reserve": {
      return (
        <div className={styles.actionDetails}>
          <div className={styles.detailsHeader}>
            <h4>Details</h4>
          </div>
          <div className={styles.detailsContent}>
            <div className={styles.detailRow}>
              <span className={styles.detailLabel}>Mode:</span>
              <div className={styles.detailValue}>
                <ReserveModeViewer mode={action.mode} />
              </div>
            </div>
            <div className={styles.detailRow}>
              <span className={styles.detailLabel}>Amount:</span>
              <span className={styles.detailValue}>
                {fmt.formatCurrency(action.currency.coins)}
              </span>
            </div>
            {execution.failureReasonText && (
              <div className={styles.detailRow}>
                <span className={styles.detailLabel}>Failure:</span>
                <span className={`${styles.detailValue} ${styles.detailFailure}`}>
                  {execution.failureReasonText}
                </span>
              </div>
            )}
            {execution.failureCode !== undefined && (
              <div className={styles.detailRow}>
                <span className={styles.detailLabel}>Exit Code:</span>
                <span className={styles.detailValue}>
                  <ExitCodeChip
                    exitCode={execution.failureCode}
                    abi={contractAbi}
                    compilerAbi={contractCompilerAbi}
                    phase="action"
                  />
                </span>
              </div>
            )}
          </div>
        </div>
      )
    }
  }

  return undefined
}

export function ActionsSummary({
  actions,
  executorActions = [],
  contracts,
  contractAddress,
  onContractClick,
}: ActionsSummaryProps): React.JSX.Element {
  const [selectedActionIndex, setSelectedActionIndex] = useState<number | undefined>()
  const mappedExecutorActions = mapExecutorActionsByType(actions, executorActions)

  if (actions.length === 0) {
    return (
      <div className={styles.container}>
        <div className={styles.emptyState}>No actions</div>
      </div>
    )
  }

  const getActionSummary = (
    action: OutAction,
  ): {title: string; description: string; value: string} => {
    switch (action.type) {
      case "sendMsg": {
        const message = action.outMsg
        const messageType = message.info.type === "internal" ? "Internal" : "External"
        const destination =
          message.info.type === "internal"
            ? message.info.dest.toString()
            : (message.info.dest?.toString() ?? "External")
        const value =
          message.info.type === "internal" ? fmt.formatCurrency(message.info.value.coins) : ""
        return {
          title: "Send Message",
          description: `${messageType} → ${destination}`,
          value: value,
        }
      }
      case "setCode": {
        return {
          title: "Set Code",
          description: "Update contract code",
          value: "",
        }
      }
      case "reserve": {
        return {
          title: "Reserve",
          description: `Mode: ${formatReserveModeNames(action.mode)}`,
          value: fmt.formatCurrency(action.currency.coins),
        }
      }
      default: {
        return {
          title: "Unknown",
          description: `Type: ${action.type}`,
          value: "",
        }
      }
    }
  }

  return (
    <div className={styles.container}>
      <div className={styles.scrollWrapper}>
        <div className={styles.actionsList}>
          {actions.map((action, index) => {
            const summary = getActionSummary(action)
            const isSelected = selectedActionIndex === index
            const execution = getActionExecutionMeta(mappedExecutorActions[index])

            let enhancedDescription: React.ReactNode = summary.description
            if (action.type === "sendMsg") {
              const info = action.outMsg.info
              if (info.type === "internal") {
                enhancedDescription = (
                  <div className={styles.actionDescriptionWithChip}>
                    <span>Internal → </span>
                    <ContractChip
                      address={info.dest.toString()}
                      contracts={contracts}
                      onContractClick={onContractClick}
                    />
                  </div>
                )
              }
            } else if (action.type === "reserve") {
              const reserveModeNames = getReserveModeNames(action.mode)
              enhancedDescription = (
                <span>
                  Mode:{" "}
                  {reserveModeNames.map((modeName, modeIndex) => (
                    <React.Fragment key={`${modeName}-${modeIndex}`}>
                      {modeIndex > 0 && <span className={styles.modeSummarySeparator}> + </span>}
                      <span className={styles.modeSummaryName}>{modeName}</span>
                    </React.Fragment>
                  ))}
                </span>
              )
            }

            return (
              <div
                key={action.type === "sendMsg" ? action.outMsg.body.hash().toString("hex") : index}
                className={`${styles.actionCard} ${isSelected ? styles.actionCardSelected : ""} ${
                  execution.isFailed ? styles.actionCardFailed : ""
                }`}
                onClick={() => {
                  setSelectedActionIndex(isSelected ? undefined : index)
                }}
                onKeyDown={event => {
                  if (event.key === "Enter" || event.key === " ") {
                    setSelectedActionIndex(isSelected ? undefined : index)
                  }
                }}
                role="button"
                tabIndex={0}
              >
                <div className={styles.actionContent}>
                  <div className={styles.actionTitle}>
                    <div className={styles.actionIcon}>{getActionIcon(action.type)}</div>
                    <span className={styles.actionTitleText}>{summary.title}</span>
                    {execution.isFailed && <span className={styles.failureBadge}>Failed</span>}
                  </div>
                  <div className={styles.actionDescription}>{enhancedDescription}</div>
                  {summary.value && <div className={styles.actionValue}>{summary.value}</div>}
                </div>
              </div>
            )
          })}
        </div>
      </div>

      {selectedActionIndex !== undefined && selectedActionIndex < actions.length && (
        <div className={styles.detailsContainer}>
          {renderActionDetails(
            actions[selectedActionIndex],
            mappedExecutorActions[selectedActionIndex],
            contractAddress,
            contracts,
            onContractClick,
          )}
        </div>
      )}
    </div>
  )
}
