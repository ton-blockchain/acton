import type {OutAction} from "@ton/core"
import React, {useState} from "react"
import {FiBookOpen, FiCode, FiCornerUpRight, FiLock, FiPackage} from "react-icons/fi"
import {DataBlock, fmt} from "@/index"
import type {
  BackendExecutorAction,
  BackendExecutorActionFailureReason,
  SourceLocation,
} from "@/types"
import type {ContractData} from "@/types/transaction"
import {decodeMessageBody, getMessageOpcode, resolveMessageOpcodeName} from "@/utils/messageBody"
import {parseReserveMode} from "@/utils/transaction"
import {ChangeLibraryModeViewer} from "../ChangeLibraryModeViewer/ChangeLibraryModeViewer"
import {ContractChip} from "../ContractChip/ContractChip"
import {CopyValueButton} from "../CopyValueButton"
import {DisasmSection} from "../DisasmSection/DisasmSection"
import {ExitCodeChip} from "../ExitCodeChip/ExitCodeChip"
import {OpcodeChip} from "../OpcodeChip/OpcodeChip"
import {ParsedBodySection} from "../ParsedBodySection/ParsedBodySection"
import {ReserveModeViewer} from "../ReserveModeViewer/ReserveModeViewer"
import {
  formatCellBocHex,
  formatMessageRelaxedBocHex,
  formatOutActionBocHex,
  formatStateInitBocHex,
} from "../rawBoc"
import {SendModeViewer} from "../SendModeViewer/SendModeViewer"

import styles from "./ActionsSummary.module.css"

interface ActionsSummaryProps {
  readonly actions: readonly OutAction[]
  readonly executorActions?: readonly BackendExecutorAction[]
  readonly contracts: Map<string, ContractData>
  readonly contractAddress: string
  readonly additionalMessageBodyAbis?: readonly NonNullable<ContractData["abi"]>[]
  readonly onContractClick?: (address: string) => void
  readonly renderSourceLocation?: (location: SourceLocation) => React.ReactNode
}

interface ActionExecutionMeta {
  readonly isFailed: boolean
  readonly failureCode: number | undefined
  readonly failureReasonText: string | undefined
}

interface ActionIconMeta {
  readonly badgeClassName: string
  readonly element: React.JSX.Element
  readonly label: string
}

const getActionIcon = (actionType: OutAction["type"]): ActionIconMeta => {
  switch (actionType) {
    case "sendMsg": {
      return {
        badgeClassName: styles.actionIconSendMsg,
        element: <FiCornerUpRight size={14} />,
        label: "Send Message",
      }
    }
    case "setCode": {
      return {
        badgeClassName: styles.actionIconSetCode,
        element: <FiCode size={14} />,
        label: "Set Code",
      }
    }
    case "reserve": {
      return {
        badgeClassName: styles.actionIconReserve,
        element: <FiLock size={14} />,
        label: "Reserve",
      }
    }
    case "changeLibrary": {
      return {
        badgeClassName: styles.actionIconChangeLibrary,
        element: <FiBookOpen size={14} />,
        label: "Change Library",
      }
    }
    default: {
      return {
        badgeClassName: styles.actionIconUnknown,
        element: <FiPackage size={14} />,
        label: "Action",
      }
    }
  }
}

const formatBoolean = (v: boolean): React.JSX.Element => (
  <span className={v ? styles.booleanTrue : styles.booleanFalse}>{v ? "Yes" : "No"}</span>
)

const formatModeNames = (names: readonly string[]): string =>
  names.length > 0 ? names.join(" + ") : "—"

const getReserveModeNames = (mode: number): readonly string[] => {
  return parseReserveMode(mode).map(flag => flag.name)
}

const formatReserveModeNames = (mode: number): string => {
  return formatModeNames(getReserveModeNames(mode))
}

const renderExternalDestination = (
  destination: string | undefined,
  contracts: Map<string, ContractData>,
  onContractClick?: (address: string) => void,
): React.JSX.Element | string => {
  if (!destination) {
    return "External"
  }

  if (contracts.has(destination)) {
    return (
      <ContractChip address={destination} contracts={contracts} onContractClick={onContractClick} />
    )
  }

  return <span className={styles.externalDestination}>{destination}</span>
}

const isActionFailed = (action: BackendExecutorAction): boolean => {
  const failureReason = "failure_reason" in action ? action.failure_reason : undefined
  return action.failure_code !== undefined || failureReason !== undefined
}

const formatNanograms = (value: string): string => {
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
    case "not_enough_grams_to_send": {
      return `Not enough GRAM: balance ${formatNanograms(reason.remaining_balance)}, required ${formatNanograms(reason.required)}.`
    }
    case "cannot_reserve_grams": {
      return `Cannot reserve ${formatNanograms(reason.requested)}: only ${formatNanograms(reason.available)} available.`
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
    let matched: BackendExecutorAction | undefined
    if (cursor < executorActions.length) {
      const candidate = executorActions[cursor]
      cursor += 1
      const typeMatches =
        (action.type === "sendMsg" && candidate.type === "send_message") ||
        (action.type === "reserve" && candidate.type === "reserve_currency") ||
        (action.type === "setCode" && candidate.type === "set_code") ||
        (action.type === "changeLibrary" && candidate.type === "change_library")

      if (typeMatches) {
        matched = candidate
      } else {
        cursor -= 1
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
  failureReasonText: formatFailureReason(
    executorAction && "failure_reason" in executorAction
      ? executorAction.failure_reason
      : undefined,
  ),
})

const formatSourceLocation = (location: SourceLocation): string => {
  const parts = location.file.split("/")
  const file = parts.length > 3 ? `.../${parts.slice(-3).join("/")}` : location.file
  return `${file}:${location.line}:${location.column}`
}

const renderActionSourceLocation = (
  executorAction: BackendExecutorAction | undefined,
  renderSourceLocation: ((location: SourceLocation) => React.ReactNode) | undefined,
): React.JSX.Element | undefined => {
  const location = executorAction?.location
  if (!location) {
    return undefined
  }

  return (
    <div className={styles.detailRow}>
      <span className={styles.detailLabel}>Source:</span>
      <span className={`${styles.detailValue} ${styles.sourceLocationValue}`}>
        {renderSourceLocation ? renderSourceLocation(location) : formatSourceLocation(location)}
      </span>
    </div>
  )
}

const renderActionDetails = (
  action: OutAction,
  executorAction: BackendExecutorAction | undefined,
  contractAddress: string,
  contracts: Map<string, ContractData>,
  additionalMessageBodyAbis: readonly NonNullable<ContractData["abi"]>[],
  onContractClick?: (address: string) => void,
  renderSourceLocation?: (location: SourceLocation) => React.ReactNode,
): React.JSX.Element | undefined => {
  const execution = getActionExecutionMeta(executorAction)
  const contract = contracts.get(contractAddress)
  const contractAbi = contract?.abi
  const rawActionBocHex = formatOutActionBocHex(action)
  const copyRawActionButton = (
    <CopyValueButton
      className={styles.detailsCopyButton}
      value={rawActionBocHex}
      label="raw action"
      caption="Copy raw action"
    />
  )

  switch (action.type) {
    case "sendMsg": {
      const message = action.outMsg
      const info = message.info
      const messageBodyHash = message.body.hash().toString("hex")
      const messageBodyBocHex = formatCellBocHex(message.body)
      const messageBocHex = formatMessageRelaxedBocHex(message)
      const stateInitBocHex = message.init ? formatStateInitBocHex(message.init) : undefined
      const parsedBody = decodeMessageBody(
        message,
        contracts,
        contractAddress,
        additionalMessageBodyAbis,
      )
      const opcode = getMessageOpcode(message)
      const opcodeName = resolveMessageOpcodeName(message, contracts, contractAddress)
      const showMessageDataSection =
        info.type === "internal" ||
        (info.type === "external-out" &&
          (parsedBody !== undefined || opcode !== undefined || opcodeName !== undefined))
      const showRawBody =
        parsedBody === undefined && (info.type === "internal" || info.type === "external-out")

      return (
        <div className={styles.actionDetails}>
          <div className={styles.detailsHeader}>
            <h4>Details</h4>
            {copyRawActionButton}
          </div>
          <div className={styles.detailsContent}>
            <div className={styles.detailRow}>
              <span className={styles.detailLabel}>Mode:</span>
              <div className={`${styles.detailValue} ${styles.modeDetailValue}`}>
                <SendModeViewer mode={action.mode} />
              </div>
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
                    {renderExternalDestination(info.dest?.toString(), contracts, onContractClick)}
                  </div>
                </div>
              </>
            )}
            {showRawBody && (
              <div className={styles.detailRow}>
                <span className={styles.detailLabel}>Body:</span>
                <div className={styles.detailValue}>
                  <DataBlock data={messageBodyBocHex} copyLabel="body BOC" />
                </div>
              </div>
            )}
            {showMessageDataSection && (
              <div
                className={`${styles.messageDataSection} ${
                  parsedBody ? styles.copyableMessageDataSection : ""
                }`}
              >
                {parsedBody && (
                  <div className={styles.messageDataCopyActions}>
                    <CopyValueButton
                      className={styles.messageDataCopyButton}
                      value={messageBocHex}
                      label="raw message"
                      caption="Copy raw message"
                    />
                    <CopyValueButton
                      className={styles.messageDataCopyButton}
                      value={messageBodyBocHex}
                      label="raw data"
                      caption="Copy raw body"
                    />
                    {stateInitBocHex && (
                      <CopyValueButton
                        className={styles.messageDataCopyButton}
                        value={stateInitBocHex}
                        label="raw state init"
                        caption="Copy raw state init"
                      />
                    )}
                  </div>
                )}
                <div className={styles.messageDataTitle}>Message Data</div>
                <div className={styles.detailRow}>
                  <span className={styles.detailLabel}>Opcode:</span>
                  <div className={styles.detailValue}>
                    <OpcodeChip opcode={opcode} abiName={opcodeName} showOpcode={true} />
                  </div>
                </div>
                {parsedBody && (
                  <ParsedBodySection
                    key={messageBodyHash}
                    parsedBody={parsedBody}
                    contracts={contracts}
                    onContractClick={onContractClick}
                    defaultExpanded={true}
                  />
                )}
              </div>
            )}
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
                  <ExitCodeChip exitCode={execution.failureCode} abi={contractAbi} phase="action" />
                </span>
              </div>
            )}
            {renderActionSourceLocation(executorAction, renderSourceLocation)}
          </div>
        </div>
      )
    }
    case "setCode": {
      const newCodeBocHex = formatCellBocHex(action.newCode)

      return (
        <div className={styles.actionDetails}>
          <div className={styles.detailsHeader}>
            <h4>Details</h4>
            {copyRawActionButton}
          </div>
          <div className={styles.detailsContent}>
            <div className={styles.detailRow}>
              <span className={styles.detailLabel}>Code Hash:</span>
              <span className={styles.detailValue}>
                <DataBlock data={action.newCode.hash().toString("hex")} />
              </span>
            </div>
            <div className={styles.detailRow}>
              <span className={styles.detailLabel}>Code BoC:</span>
              <span className={styles.detailValue}>
                <DataBlock data={newCodeBocHex} copyLabel="code BOC" />
              </span>
            </div>
            <DisasmSection bocHex={newCodeBocHex} />
            {execution.failureCode !== undefined && (
              <div className={styles.detailRow}>
                <span className={styles.detailLabel}>Exit Code:</span>
                <span className={styles.detailValue}>
                  <ExitCodeChip exitCode={execution.failureCode} abi={contractAbi} phase="action" />
                </span>
              </div>
            )}
            {renderActionSourceLocation(executorAction, renderSourceLocation)}
          </div>
        </div>
      )
    }
    case "reserve": {
      return (
        <div className={styles.actionDetails}>
          <div className={styles.detailsHeader}>
            <h4>Details</h4>
            {copyRawActionButton}
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
                  <ExitCodeChip exitCode={execution.failureCode} abi={contractAbi} phase="action" />
                </span>
              </div>
            )}
            {renderActionSourceLocation(executorAction, renderSourceLocation)}
          </div>
        </div>
      )
    }
    case "changeLibrary": {
      const isEmbeddedLibrary = action.libRef.type === "ref"
      const embeddedLibraryBocHex =
        action.libRef.type === "ref" ? formatCellBocHex(action.libRef.library) : undefined

      return (
        <div className={styles.actionDetails}>
          <div className={styles.detailsHeader}>
            <h4>Details</h4>
            {copyRawActionButton}
          </div>
          <div className={styles.detailsContent}>
            <div className={styles.detailRow}>
              <span className={styles.detailLabel}>Mode:</span>
              <div className={`${styles.detailValue} ${styles.modeDetailValue}`}>
                <ChangeLibraryModeViewer mode={action.mode} />
              </div>
            </div>
            <div className={styles.detailRow}>
              <span className={styles.detailLabel}>Reference:</span>
              <span className={styles.detailValue}>
                {action.libRef.type === "hash" ? "Library Hash" : "Embedded Library"}
              </span>
            </div>
            <div className={styles.detailRow}>
              <span className={styles.detailLabel}>
                {action.libRef.type === "hash" ? "Hash:" : "Library:"}
              </span>
              <span className={styles.detailValue}>
                <DataBlock
                  copyLabel={action.libRef.type === "hash" ? "library hash" : "library BOC"}
                  data={
                    action.libRef.type === "hash"
                      ? action.libRef.libHash.toString("hex")
                      : embeddedLibraryBocHex!
                  }
                />
              </span>
            </div>
            {isEmbeddedLibrary && embeddedLibraryBocHex && (
              <DisasmSection bocHex={embeddedLibraryBocHex} title="Library Disassembly" />
            )}
            {execution.failureCode !== undefined && (
              <div className={styles.detailRow}>
                <span className={styles.detailLabel}>Exit Code:</span>
                <span className={styles.detailValue}>
                  <ExitCodeChip exitCode={execution.failureCode} abi={contractAbi} phase="action" />
                </span>
              </div>
            )}
            {renderActionSourceLocation(executorAction, renderSourceLocation)}
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
  additionalMessageBodyAbis = [],
  onContractClick,
  renderSourceLocation,
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
        const value =
          message.info.type === "internal"
            ? fmt.formatCurrency(message.info.value.coins)
            : "external-out"
        return {
          title: "Send Message",
          description:
            message.info.type === "internal"
              ? `Internal → ${message.info.dest.toString()}`
              : "External → External",
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
      case "changeLibrary": {
        return {
          title: "Change Library",
          description:
            action.libRef.type === "hash" ? "Attach library by hash" : "Attach embedded library",
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
            const icon = getActionIcon(action.type)
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
                    <div
                      className={`${styles.actionIcon} ${icon.badgeClassName}`}
                      aria-label={icon.label}
                      title={icon.label}
                    >
                      {icon.element}
                    </div>
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
            additionalMessageBodyAbis,
            onContractClick,
            renderSourceLocation,
          )}
        </div>
      )}
    </div>
  )
}
