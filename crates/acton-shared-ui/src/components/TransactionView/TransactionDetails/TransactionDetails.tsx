import * as React from "react"
import {useEffect, useState} from "react"
import {FiChevronDown, FiChevronUp} from "react-icons/fi"

import type {BackendContractInfo} from "@/types"
import type {ContractData, ParsedValue, TransactionInfo} from "@/types/transaction"
import {fmt} from "@/index"
import {computeSendMode, getTransactionOpcode} from "@/utils/transaction"

import {ContractChip} from "../ContractChip/ContractChip"
import {ExitCodeChip} from "../ExitCodeChip/ExitCodeChip"
import {OpcodeChip} from "../OpcodeChip/OpcodeChip"
import {SendModeViewer} from "../SendModeViewer/SendModeViewer"

import {ActionsSummary} from "./ActionsSummary"
import styles from "./TransactionDetails.module.css"

export interface TransactionDetailsProps {
  readonly tx: TransactionInfo
  readonly contracts: Map<string, ContractData>
  readonly allContracts: readonly BackendContractInfo[]
  readonly onContractClick?: (address: string) => void
}

const DECIMAL_SCALAR_PATTERN = /^-?\d+(?:\.\d+)?$/

function ParsedTypeLabel({typeName}: {readonly typeName: string}): React.JSX.Element {
  return <span className={styles.parsedTypeLabel}>{typeName}</span>
}

function ParsedValueRow({
  label,
  value,
  contracts,
  onContractClick,
}: {
  readonly label: string
  readonly value: ParsedValue
  readonly contracts: Map<string, ContractData>
  readonly onContractClick?: (address: string) => void
}): React.JSX.Element {
  return (
    <>
      <div className={styles.parsedEntryKey}>{label}:</div>
      <div className={styles.parsedEntryValue}>
        <ParsedValueView value={value} contracts={contracts} onContractClick={onContractClick} />
      </div>
    </>
  )
}

function ParsedValueView({
  value,
  contracts,
  onContractClick,
  fallbackTypeName,
}: {
  readonly value: ParsedValue
  readonly contracts: Map<string, ContractData>
  readonly onContractClick?: (address: string) => void
  readonly fallbackTypeName?: string
}): React.JSX.Element {
  switch (value.kind) {
    case "null": {
      return <span className={styles.parsedNull}>null</span>
    }
    case "address": {
      return (
        <ContractChip
          address={value.value}
          contracts={contracts}
          onContractClick={onContractClick}
        />
      )
    }
    case "boolean": {
      return (
        <span className={value.value ? styles.booleanTrue : styles.booleanFalse}>
          {value.value ? "true" : "false"}
        </span>
      )
    }
    case "scalar": {
      return (
        <span
          className={
            DECIMAL_SCALAR_PATTERN.test(value.value)
              ? styles.parsedPlainScalar
              : styles.parsedScalar
          }
        >
          {value.value}
        </span>
      )
    }
    case "array": {
      if (value.items.length === 0) {
        return <span className={styles.parsedEmpty}>[]</span>
      }

      return (
        <div className={styles.parsedContainer}>
          <span className={styles.parsedBadge}>array</span>
          <div className={styles.parsedNested}>
            {value.items.map((item, index) => (
              <ParsedValueRow
                key={`array-item-${index}`}
                label={`[${index}]`}
                value={item}
                contracts={contracts}
                onContractClick={onContractClick}
              />
            ))}
          </div>
        </div>
      )
    }
    case "object": {
      const typeName = value.typeName ?? fallbackTypeName

      return (
        <div className={styles.parsedContainer}>
          {typeName && <ParsedTypeLabel typeName={typeName} />}
          {value.entries.length === 0 ? (
            <span className={styles.parsedEmpty}>{"{}"}</span>
          ) : (
            <div className={styles.parsedNested}>
              {value.entries.map(entry => (
                <ParsedValueRow
                  key={entry.key}
                  label={entry.key}
                  value={entry.value}
                  contracts={contracts}
                  onContractClick={onContractClick}
                />
              ))}
            </div>
          )}
        </div>
      )
    }
    case "map": {
      return (
        <div className={styles.parsedContainer}>
          <span className={styles.parsedBadge}>map</span>
          {value.entries.length === 0 ? (
            <span className={styles.parsedEmpty}>{"{}"}</span>
          ) : (
            <div className={styles.parsedNested}>
              {value.entries.map((entry, index) => (
                <div key={`map-entry-${index}`} className={styles.parsedMapEntry}>
                  <ParsedValueRow
                    label="key"
                    value={entry.key}
                    contracts={contracts}
                    onContractClick={onContractClick}
                  />
                  <ParsedValueRow
                    label="value"
                    value={entry.value}
                    contracts={contracts}
                    onContractClick={onContractClick}
                  />
                </div>
              ))}
            </div>
          )}
        </div>
      )
    }
  }
}

export function TransactionDetails({
  tx,
  contracts,
  allContracts,
  onContractClick,
}: TransactionDetailsProps): React.JSX.Element {
  const [showActions, setShowActions] = useState(false)
  const [showParsedBody, setShowParsedBody] = useState(false)

  useEffect(() => {
    setShowParsedBody(false)
  }, [tx.lt])

  const description = tx.transaction.description
  if (description.type !== "generic") {
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

  const computePhase = description.computePhase
  const actionPhase = description.actionPhase

  const formatBoolean = (v: boolean): React.JSX.Element => (
    <span className={v ? styles.booleanTrue : styles.booleanFalse}>{v ? "Yes" : "No"}</span>
  )

  const inMessage = tx.transaction.inMessage ?? undefined
  const hasMessageBody =
    inMessage != undefined &&
    (() => {
      const body = inMessage.body.asSlice()
      return body.remainingBits > 0 || body.remainingRefs > 0
    })()
  const sendMode = computeSendMode(tx)

  const opcode = getTransactionOpcode(tx.transaction)

  const thisAddress = tx.address
  const targetContract = thisAddress ? contracts.get(thisAddress.toString()) : undefined
  let typeAbi = targetContract?.abi?.messages.find(it => it.opcode === opcode)
  if (typeAbi === undefined) {
    for (const contract of allContracts) {
      typeAbi = contract.abi?.messages.find(it => it.opcode === opcode)
    }
  }
  const opcodeName = typeAbi?.name

  const sentTotal = [...tx.transaction.outMessages.values()].reduce(
    (accumulator: bigint, message) =>
      accumulator + (message.info.type === "internal" ? message.info.value.coins : 0n),
    0n,
  )

  return (
    <div className={styles.transactionDetailsContainer}>
      <div className={styles.detailRow}>
        <div className={styles.detailLabel}>Message Route</div>
        <div className={styles.detailValue}>
          <ContractChip
            address={tx.transaction.inMessage?.info.src?.toString()}
            contracts={contracts}
            onContractClick={onContractClick}
          />
          {" → "}
          <ContractChip
            address={tx.transaction.inMessage?.info.dest?.toString()}
            contracts={contracts}
            onContractClick={onContractClick}
          />
        </div>
      </div>

      {inMessage && inMessage.info.type === "internal" && (
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
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Send Mode</div>
                <div className={`${styles.multiColumnItemValue} ${styles.numberValue}`}>
                  <SendModeViewer mode={sendMode} />
                </div>
              </div>
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
                <div className={`${styles.multiColumnItemValue} ${styles.timestampValue}`}>
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
          {tx.parsedBody && hasMessageBody && (
            <div className={styles.parsedBodySection}>
              <div className={styles.parsedBodyTitle}>
                Parsed Body
                <button
                  type="button"
                  onClick={() => {
                    setShowParsedBody(!showParsedBody)
                  }}
                  className={styles.actionsToggleButton}
                  aria-label={showParsedBody ? "Hide parsed body" : "Show parsed body"}
                >
                  {showParsedBody ? <FiChevronUp size={14} /> : <FiChevronDown size={14} />}
                  <span className={styles.actionsToggleText}>
                    {showParsedBody ? "Hide" : "Show"}
                  </span>
                </button>
              </div>
              {showParsedBody && (
                <div className={styles.parsedBodyTree}>
                  <div className={styles.parsedBodyContent}>
                    <ParsedValueView
                      value={tx.parsedBody.value}
                      contracts={contracts}
                      onContractClick={onContractClick}
                      fallbackTypeName={tx.parsedBody.name}
                    />
                  </div>
                </div>
              )}
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
              <div className={styles.multiColumnItemTitle}>Total Fee</div>
              <div className={`${styles.multiColumnItemValue}`}>
                {fmt.formatCurrency(tx.transaction.totalFees.coins)}
              </div>
            </div>
            <div className={styles.multiColumnItem}>
              <div className={styles.multiColumnItemTitle}>Gas Fee</div>
              <div className={`${styles.multiColumnItemValue}`}>
                {computePhase.type === "skipped" ? "N/A" : fmt.formatCurrency(computePhase.gasFees)}
              </div>
            </div>
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
                  <ExitCodeChip exitCode={computePhase.exitCode} abi={targetContract?.abi} />
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
                  <ExitCodeChip
                    exitCode={actionPhase.resultCode}
                    abi={targetContract?.abi}
                    phase="action"
                  />
                </div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Total Actions</div>
                <div className={`${styles.multiColumnItemValue} ${styles.numberValue}`}>
                  {fmt.formatNumber(tx.outActions.length)}
                  {tx.outActions.length > 0 && (
                    <button
                      type="button"
                      onClick={() => {
                        setShowActions(!showActions)
                      }}
                      className={styles.actionsToggleButton}
                      aria-label={showActions ? "Hide actions" : "Show actions"}
                    >
                      {showActions ? <FiChevronUp size={14} /> : <FiChevronDown size={14} />}
                      <span className={styles.actionsToggleText}>
                        {showActions ? "Hide" : "Show"}
                      </span>
                    </button>
                  )}
                </div>
              </div>
            </div>
          ) : (
            <div className={styles.multiColumnItemValue}>No action phase</div>
          )}
        </div>
      </div>

      {showActions && tx.outActions.length > 0 && (
        <div className={styles.labeledSectionRow}>
          <div className={styles.labeledSectionTitle}>Actions Details</div>
          <div className={styles.labeledSectionContent}>
            <ActionsSummary
              actions={tx.outActions}
              executorActions={tx.executorActions}
              contracts={contracts}
              contractAddress={tx.address?.toString() ?? ""}
            />
          </div>
        </div>
      )}

      <div className={styles.detailRow}>
        <div className={styles.detailLabel}>Time</div>
        <div className={`${styles.detailValue} ${styles.timestampValue}`}>
          {formatDetailedTimestamp(tx.transaction.now)}
        </div>
      </div>
    </div>
  )
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
