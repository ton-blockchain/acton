import React, { type JSX, useState } from "react"
import { FiChevronDown, FiChevronUp } from "react-icons/fi"
import type { ContractData, TransactionInfo } from "../../../types/transaction"
import { formatCurrency, formatNumber } from "../../../utils/format"
import { computeSendMode } from "../../../utils/transaction"
import { ContractChip } from "../ContractChip/ContractChip"
import { ExitCodeChip } from "../ExitCodeChip/ExitCodeChip"
import { OpcodeChip } from "../OpcodeChip/OpcodeChip"
import { SendModeViewer } from "../SendModeViewer/SendModeViewer"

import { ActionsSummary } from "./ActionsSummary"

import styles from "./TransactionDetails.module.css"

const formatDetailedTimestamp = (
  timestampInput: number | string | undefined,
  showShort: boolean = true,
): JSX.Element | string => {
  if (timestampInput === undefined) return "—"

  const date =
    typeof timestampInput === "string" ? new Date(timestampInput) : new Date(timestampInput * 1000)

  const pad = (num: number): string => num.toString().padStart(2, "0")
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
  const monthNum = monthIndex + 1
  const year = date.getFullYear()
  const hours = date.getHours()
  const minutes = date.getMinutes()
  const seconds = date.getSeconds()

  const fullPart = `${pad(day)}.${pad(monthNum)}.${year}, ${pad(hours)}:${pad(minutes)}:${pad(seconds)}`
  const shortPart = `${pad(day)} ${monthAbbrs[monthIndex]}, ${pad(hours)}:${pad(minutes)}`

  return (
    <>
      {fullPart}
      {showShort && <span className={styles.timestampDetailSecondary}> — {shortPart}</span>}
    </>
  )
}

export interface TransactionDetailsProps {
  readonly tx: TransactionInfo
  readonly transactions: TransactionInfo[]
  readonly contracts: Map<string, ContractData>
  readonly onContractClick?: (address: string) => void
}

export function TransactionDetails({
  tx,
  contracts,
  onContractClick,
  transactions,
}: TransactionDetailsProps): React.JSX.Element {
  const [showActions, setShowActions] = useState(false)

  if (tx.transaction.description.type !== "generic") {
    return (
      <div className={styles.transactionDetailsContainer}>
        <div className={styles.detailRow}>
          <div className={styles.detailValue}>
            Non-generic transaction not supported (Type: {tx.transaction.description.type})
          </div>
        </div>
      </div>
    )
  }

  const computeInfo = tx.computeInfo
  const formatBoolean = (v: boolean): React.JSX.Element => (
    <span className={v ? styles.booleanTrue : styles.booleanFalse}>{v ? "Yes" : "No"}</span>
  )

  const isSuccess = tx.computeInfo !== "skipped" && tx.computeInfo.success
  const inMessage = tx.transaction.inMessage
  const money = tx.money
  const sendMode = computeSendMode(tx, transactions)

  const thisAddress = tx.address
  const targetContract = thisAddress ? contracts.get(thisAddress.toString()) : undefined
  let typeAbi = targetContract?.abi?.messages.find((it: any) => it.opcode === tx.opcode)
  if (typeAbi === undefined) {
    ;[...contracts.values()].forEach((c) => {
      typeAbi = c.abi?.messages.find((it: any) => it.opcode === tx.opcode)
    })
  }
  const opcodeName = typeAbi?.name
  const knownExitCodes = contracts.get(tx.address?.toString() ?? "")?.abi?.exitCodes

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
                  {formatCurrency(inMessage.info.value.coins)}
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
                <OpcodeChip opcode={tx.opcode} abiName={opcodeName} />
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className={styles.labeledSectionRow}>
        <div className={styles.labeledSectionTitle}>Fees & Sent</div>
        <div className={styles.labeledSectionContent}>
          <div className={styles.multiColumnRow}>
            <div className={styles.multiColumnItem}>
              <div className={styles.multiColumnItemTitle}>Amount Sent (Total)</div>
              <div className={`${styles.multiColumnItemValue}`}>
                {formatCurrency(money.sentTotal)}
              </div>
            </div>
            <div className={styles.multiColumnItem}>
              <div className={styles.multiColumnItemTitle}>Total Fee</div>
              <div className={`${styles.multiColumnItemValue}`}>
                {formatCurrency(money.totalFees)}
              </div>
            </div>
            <div className={styles.multiColumnItem}>
              <div className={styles.multiColumnItemTitle}>Gas Fee</div>
              <div className={`${styles.multiColumnItemValue}`}>
                {tx.computeInfo === "skipped" ? "N/A" : formatCurrency(tx.computeInfo.gasFees)}
              </div>
            </div>
            {tx.transaction.inMessage?.info.type === "internal" && (
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Forward Fee</div>
                <div className={`${styles.multiColumnItemValue}`}>
                  {formatCurrency(money.forwardFee)}
                </div>
              </div>
            )}
          </div>
        </div>
      </div>

      <div className={styles.labeledSectionRow}>
        <div className={styles.labeledSectionTitle}>Compute Phase</div>
        <div className={styles.labeledSectionContent}>
          {computeInfo === "skipped" ? (
            <div className={styles.multiColumnItemValue}>Skipped</div>
          ) : (
            <div className={styles.multiColumnRow}>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Success</div>
                <div className={styles.multiColumnItemValue}>
                  {formatBoolean(computeInfo.success)}
                </div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Exit Code</div>
                <div className={styles.multiColumnItemValue}>
                  <ExitCodeChip exitCode={computeInfo.exitCode} exitCodes={knownExitCodes} />
                </div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>VM Steps</div>
                <div className={`${styles.multiColumnItemValue} ${styles.numberValue}`}>
                  {computeInfo.vmSteps}
                </div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Gas Used</div>
                <div className={styles.multiColumnItemValue}>{computeInfo.gasUsed.toString()}</div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Gas Fee</div>
                <div className={styles.multiColumnItemValue}>
                  {formatCurrency(computeInfo.gasFees)}
                </div>
              </div>
            </div>
          )}
        </div>
      </div>

      <div className={styles.labeledSectionRow}>
        <div className={styles.labeledSectionTitle}>Action Phase</div>
        <div className={styles.labeledSectionContent}>
          <div className={styles.multiColumnRow}>
            <div className={styles.multiColumnItem}>
              <div className={styles.multiColumnItemTitle}>Success</div>
              <div
                className={`${styles.multiColumnItemValue} ${isSuccess ? styles.booleanTrue : styles.booleanFalse}`}
              >
                {formatBoolean(isSuccess)}
              </div>
            </div>
            <div className={styles.multiColumnItem}>
              <div className={styles.multiColumnItemTitle}>Total Actions</div>
              <div className={`${styles.multiColumnItemValue} ${styles.numberValue}`}>
                {formatNumber(tx.outActions.length)}
                {tx.outActions.length > 0 && (
                  <button
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
        </div>
      </div>

      {showActions && tx.outActions.length > 0 && (
        <div className={styles.labeledSectionRow}>
          <div className={styles.labeledSectionTitle}>Actions Details</div>
          <div className={styles.labeledSectionContent}>
            <ActionsSummary
              actions={tx.outActions}
              contracts={contracts}
              contractAddress={tx.address?.toString() ?? ""}
              onContractClick={onContractClick}
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
