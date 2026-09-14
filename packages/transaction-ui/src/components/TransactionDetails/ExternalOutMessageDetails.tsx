import type {CommonMessageInfoExternalOut, Message} from "@ton/core"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import type React from "react"
import {ContractChip, InlineButton, OpcodeChip, ParsedBodySection, RawDataBlock} from "@acton/ui"
import {ScanSearch} from "lucide-react"

import type {ContractData} from "../../model/transaction"
import {decodeMessageBody, getMessageOpcode, resolveMessageOpcodeName} from "../../lib/messageBody"
import {formatCellBocHex, formatMessageBocHex} from "../../lib/rawBoc"
import {renderSectionCopyActions} from "./sectionCopyActions"

import styles from "./TransactionDetails.module.css"

interface ExternalOutMessageDetailsProps {
  readonly message: Message & {readonly info: CommonMessageInfoExternalOut}
  readonly contracts: Map<string, ContractData>
  readonly additionalAbis: readonly ContractABI[]
  readonly onCellInspect?: (boc: string) => void
  readonly onContractClick?: (address: string) => void
  readonly renderAddressChip?: (
    address: string,
    options: {readonly shorten: boolean},
  ) => React.ReactNode
}

/** Displays a delivered external-out message independently of its parent's inbound message. */
export function ExternalOutMessageDetails({
  message,
  contracts,
  additionalAbis,
  onCellInspect,
  onContractClick,
  renderAddressChip,
}: ExternalOutMessageDetailsProps): React.JSX.Element {
  const source = message.info.src.toString()
  const parsedBody = decodeMessageBody(message, contracts, source, additionalAbis)
  const opcode = getMessageOpcode(message, parsedBody)
  // A raw cell/slice type describes the payload, not an operation name.
  const opcodeName =
    resolveMessageOpcodeName(message, contracts, source, parsedBody) ??
    (parsedBody?.value.kind === "object" ? parsedBody.name : undefined)
  const bodyBoc = formatCellBocHex(message.body)

  return (
    <section aria-label="External-out message" className={styles.transactionDetailsContainer}>
      <div className={styles.detailRow}>
        <div className={styles.detailLabel}>Message Route</div>
        <div className={styles.heightDetailValue}>
          <div className={styles.triggerRoute}>
            {renderAddressChip ? (
              renderAddressChip(source, {shorten: true})
            ) : (
              <ContractChip
                address={source}
                contracts={contracts}
                onContractClick={onContractClick}
              />
            )}
            <span aria-hidden="true">→</span>
            <span className={styles.messageEndpointBadge}>
              {message.info.dest?.toString() ?? "External"}
            </span>
          </div>
        </div>
      </div>
      <div className={styles.labeledSectionRow}>
        <div className={styles.labeledSectionTitle}>Out Message</div>
        <div className={styles.labeledSectionContent}>
          <div className={styles.multiColumnRow}>
            <div className={styles.multiColumnItem}>
              <div className={styles.multiColumnItemTitle}>Created Lt</div>
              <div className={styles.multiColumnItemValue}>{message.info.createdLt.toString()}</div>
            </div>
          </div>
        </div>
      </div>
      <div className={styles.labeledSectionRow}>
        <div className={styles.labeledSectionTitle}>Message Data</div>
        <div className={`${styles.labeledSectionContent} ${styles.copyableSectionContent}`}>
          {renderSectionCopyActions([
            {
              value: formatMessageBocHex(message),
              label: "raw message",
              caption: "Copy raw message",
            },
            {value: bodyBoc, label: "raw body", caption: "Copy raw body"},
          ])}
          <div className={styles.multiColumnRow}>
            <div className={`${styles.multiColumnItem} ${styles.messageOpcodeItem}`}>
              <div className={styles.multiColumnItemTitle}>Opcode</div>
              <div className={styles.multiColumnItemValue}>
                <OpcodeChip
                  className={styles.messageOpcode}
                  opcode={opcode}
                  abiName={opcodeName ?? "unknown"}
                  showOpcode={true}
                />
              </div>
            </div>
            {onCellInspect && (
              <InlineButton
                variant="accent"
                onClick={() => onCellInspect(message.body.toBoc().toString("base64"))}
              >
                <ScanSearch size={14} aria-hidden="true" />
                Inspect body
              </InlineButton>
            )}
          </div>
          {parsedBody ? (
            <ParsedBodySection
              parsedBody={parsedBody}
              contracts={contracts}
              onCellInspect={onCellInspect}
              onContractClick={onContractClick}
              defaultExpanded={true}
            />
          ) : (
            <RawDataBlock
              value={bodyBoc}
              showCopy={false}
              empty={message.body.bits.length === 0 && message.body.refs.length === 0}
              emptyContent="Empty body"
            />
          )}
        </div>
      </div>
    </section>
  )
}
