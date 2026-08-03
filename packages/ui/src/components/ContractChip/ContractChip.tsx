import type {MouseEvent} from "react"

import {cx} from "../../lib/cx"
import {CopyInlineAction, InlineActions} from "../InlineActions/InlineActions"
import {Tooltip} from "../Tooltip"

import styles from "./ContractChip.module.css"

export interface ContractChipData {
  readonly displayName: string
  readonly letter: string
}

export type ContractChipAddressFormatter = (address: string) => string

export interface ContractReferenceOptions {
  readonly contracts?: ReadonlyMap<string, ContractChipData>
  readonly formatAddress?: ContractChipAddressFormatter
  readonly onContractClick?: (address: string, event?: MouseEvent<HTMLElement>) => void
}

export interface ContractChipProps extends ContractReferenceOptions {
  readonly address: string | undefined
  readonly className?: string
}

function getDisplayAddress(
  address: string,
  formatAddress: ContractChipAddressFormatter | undefined,
): string {
  if (!formatAddress) return address

  try {
    return formatAddress(address)
  } catch {
    return address
  }
}

function shortenAddress(address: string): string {
  if (address.length <= 13) return address
  return `${address.slice(0, 6)}…${address.slice(-6)}`
}

export function ContractChip({
  address,
  contracts,
  formatAddress,
  className,
  onContractClick,
}: ContractChipProps) {
  if (!address) {
    return <span className={cx(styles.contractChip, styles.unavailable, className)}>Unknown</span>
  }

  const displayAddress = getDisplayAddress(address, formatAddress)
  const contractInfo = contracts?.get(address) ?? contracts?.get(displayAddress)
  const shortAddress = shortenAddress(displayAddress)
  const isClickable = onContractClick !== undefined

  const content = contractInfo ? (
    <>
      <span className={styles.contractLetter}>{contractInfo.letter}</span>
      <span className={styles.contractName}>{contractInfo.displayName}</span>
      <span className={styles.contractAddress}>({shortAddress})</span>
    </>
  ) : (
    <>
      <span className={styles.contractLetter}>?</span>
      <span className={styles.contractName}>{shortAddress}</span>
    </>
  )

  const chip = isClickable ? (
    <Tooltip content="Open contract details">
      <button
        type="button"
        className={cx(styles.contractChip, styles.clickable)}
        onClick={event => {
          event.stopPropagation()
          onContractClick?.(displayAddress, event)
        }}
        onAuxClick={event => {
          if (event.button !== 1) return
          event.stopPropagation()
          onContractClick?.(displayAddress, event)
        }}
      >
        {content}
      </button>
    </Tooltip>
  ) : (
    <span className={styles.contractChip}>{content}</span>
  )

  return (
    <InlineActions
      className={cx(styles.contractChipActions, className)}
      actions={
        <CopyInlineAction
          className={styles.copyAction}
          value={displayAddress}
          label="Copy address"
          copiedLabel="Address copied"
          resetDelay={1500}
          size="compact"
        />
      }
    >
      {chip}
    </InlineActions>
  )
}
