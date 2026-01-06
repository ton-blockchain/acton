import type { OutAction } from "@ton/core"
import type React from "react"
import { useState } from "react"
import type { ContractData } from "../../../types/transaction"
import { formatCurrency } from "../../../utils/format"
import { ContractChip } from "../ContractChip/ContractChip"
import { ReserveModeViewer } from "../ReserveModeViewer/ReserveModeViewer"
import { SendModeViewer } from "../SendModeViewer/SendModeViewer"

import styles from "./ActionsSummary.module.css"

interface ActionsSummaryProps {
  readonly actions: readonly OutAction[]
  readonly contracts: Map<string, ContractData>
  readonly contractAddress: string
  readonly onContractClick?: (address: string) => void
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

const renderActionDetails = (
  action: OutAction,
  contractAddress: string,
  contracts: Map<string, ContractData>,
  onContractClick?: (address: string) => void,
): React.JSX.Element | undefined => {
  switch (action.type) {
    case "sendMsg": {
      const msg = action.outMsg
      const info = msg.info

      return (
        <div className={styles.actionDetails}>
          <div className={styles.detailsHeader}>
            <h4>Details</h4>
          </div>
          <div className={styles.detailsContent}>
            <div className={styles.detailRow}>
              <span className={styles.detailLabel}>Mode:</span>
              <div className={styles.detailValue}>
                <SendModeViewer mode={action.mode} />
              </div>
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
                  <span className={styles.detailValue}>{formatCurrency(info.value.coins)}</span>
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
                <pre className={styles.codeBlock}>{msg.body.toBoc().toString("hex")}</pre>
              </div>
            </div>
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
                <pre className={styles.codeBlock}>{action.newCode.toBoc().toString("hex")}</pre>
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
              <span className={styles.detailValue}>{formatCurrency(action.currency.coins)}</span>
            </div>
          </div>
        </div>
      )
    }
  }

  return undefined
}

export function ActionsSummary({
  actions,
  contracts,
  contractAddress,
  onContractClick,
}: ActionsSummaryProps): React.JSX.Element {
  const [selectedActionIndex, setSelectedActionIndex] = useState<number | undefined>(undefined)

  if (actions.length === 0) {
    return (
      <div className={styles.container}>
        <div className={styles.emptyState}>No actions</div>
      </div>
    )
  }

  const getActionSummary = (
    action: OutAction,
  ): { title: string; description: string; value: string } => {
    switch (action.type) {
      case "sendMsg": {
        const msg = action.outMsg
        const msgType = msg.info.type === "internal" ? "Internal" : "External"
        const dest =
          msg.info.type === "internal"
            ? msg.info.dest.toString()
            : (msg.info.dest?.toString() ?? "External")
        const value = msg.info.type === "internal" ? formatCurrency(msg.info.value.coins) : ""
        return {
          title: "Send Message",
          description: `${msgType} → ${dest}`,
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
          description: `Mode: ${action.mode}`,
          value: formatCurrency(action.currency.coins),
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
            }

            return (
              <div
                key={action.type === "sendMsg" ? action.outMsg.body.hash().toString("hex") : index}
                className={`${styles.actionCard} ${isSelected ? styles.actionCardSelected : ""}`}
                onClick={() => {
                  setSelectedActionIndex(isSelected ? undefined : index)
                }}
                onKeyDown={(event) => {
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
            contractAddress,
            contracts,
            onContractClick,
          )}
        </div>
      )}
    </div>
  )
}
