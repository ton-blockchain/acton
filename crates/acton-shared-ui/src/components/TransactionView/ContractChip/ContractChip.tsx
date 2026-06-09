import type React from "react"
import {useCallback, useEffect, useState} from "react"

import type {ContractData} from "@/types/transaction"

import styles from "./ContractChip.module.css"

interface ContractChipProps {
  readonly address: string | undefined
  readonly contracts: Map<string, ContractData>
  readonly trimSoloAddress?: boolean
  readonly onContractClick?: (address: string) => void
}

export function ContractChip({
  address,
  contracts,
  trimSoloAddress = true,
  onContractClick,
}: ContractChipProps): React.JSX.Element {
  const [isCopied, setIsCopied] = useState(false)

  const handleCopy = useCallback(
    (event: React.MouseEvent) => {
      event.stopPropagation()
      if (address) {
        navigator.clipboard
          .writeText(address)
          .then(() => {
            setIsCopied(true)
          })
          .catch((error: unknown) => {
            console.error("Failed to copy:", error)
          })
      }
    },
    [address],
  )

  const handleChipClick = useCallback(
    (event: React.MouseEvent) => {
      event.stopPropagation()
      if (address && onContractClick) {
        onContractClick(address)
      }
    },
    [address, onContractClick],
  )

  useEffect((): (() => void) | undefined => {
    if (isCopied) {
      const timer = setTimeout(() => {
        setIsCopied(false)
      }, 1500)
      return () => {
        clearTimeout(timer)
      }
    }
    return undefined
  }, [isCopied])

  const copyIconSvg = (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <title>Copy address</title>
      <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
      <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
    </svg>
  )

  const checkIconSvg = (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <title>Copied!</title>
      <polyline points="20 6 9 17 4 12" />
    </svg>
  )

  if (!address) {
    return <span className={styles.contractChip}>Unknown</span>
  }

  const contractInfo = contracts.get(address)
  const isClickable = onContractClick !== undefined
  const shortAddress = `${address.slice(0, 6)}…${address.slice(-6)}`
  const fallbackAddress = trimSoloAddress ? shortAddress : address

  const chipContent = (
    <>
      {contractInfo ? (
        <>
          <span className={styles.contractLetter}>{contractInfo.letter}</span>
          <span className={styles.contractName}>{contractInfo.displayName}</span>
          <span className={styles.contractAddress}>({shortAddress})</span>
        </>
      ) : (
        <>
          <span className={styles.contractLetter}>?</span>
          <span className={styles.contractName}>{fallbackAddress}</span>
        </>
      )}
      <div
        onClick={handleCopy}
        onKeyDown={evemt => {
          if (evemt.key === "Enter" || evemt.key === " ") {
            evemt.preventDefault()
            handleCopy(evemt as unknown as React.MouseEvent)
          }
        }}
        className={styles.contractChipCopyButton}
        title={isCopied ? "Copied!" : "Copy address"}
        aria-label={isCopied ? "Copied to clipboard" : "Copy address"}
        role="button"
        tabIndex={0}
      >
        {isCopied ? checkIconSvg : copyIconSvg}
      </div>
    </>
  )

  if (isClickable) {
    return (
      <button
        className={`${styles.contractChip} ${styles.clickable}`}
        onClick={handleChipClick}
        title="Click to view contract details"
        type="button"
      >
        {chipContent}
      </button>
    )
  }

  return <span className={styles.contractChip}>{chipContent}</span>
}
