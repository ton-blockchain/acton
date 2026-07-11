//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core
import type React from "react"
import {useState} from "react"
import {VscCopy, VscCheck, VscQuestion} from "react-icons/vsc"

import type {WalletInfo} from "../wallet-types"

import styles from "./WalletCard.module.css"

interface WalletCardProps {
  readonly info: WalletInfo
  readonly airdropStatus: "idle" | "requesting"
  readonly onAirdrop: () => void
  readonly onCopy: () => void
  readonly onOpenExplorer: () => void
}

export const WalletCard: React.FC<WalletCardProps> = ({
  info,
  airdropStatus,
  onAirdrop,
  onCopy,
  onOpenExplorer,
}) => {
  const [isCopied, setIsCopied] = useState(false)

  const balanceInTon = info.balance
    ? (
        BigInt(info.balance).toString().padStart(10, "0").slice(0, -9) +
        "." +
        BigInt(info.balance).toString().padStart(10, "0").slice(-9).replace(/0+$/, "")
      ).replace(/\.$/, "")
    : "0"

  const truncatedAddr =
    info.address.length > 20
      ? `${info.address.slice(0, 8)}…${info.address.slice(-8)}`
      : info.address

  const handleCopy = (): void => {
    onCopy()
    setIsCopied(true)
    setTimeout(() => {
      setIsCopied(false)
    }, 1500)
  }

  return (
    <div className={styles.card}>
      <div className={styles.header}>
        <span className={styles.name}>{info.name}</span>
        <span className={styles.tag}>{info.is_global ? "Global" : "Local"}</span>
      </div>
      <div className={styles.addressRow}>
        <button
          type="button"
          className={styles.linkButton}
          onClick={onOpenExplorer}
          title={`Open ${info.address} in Tonscan`}
        >
          {truncatedAddr}
        </button>
        <button
          type="button"
          className={`${styles.iconButton} ${isCopied ? styles.copied : ""}`}
          onClick={handleCopy}
          title="Copy address"
        >
          {isCopied ? <VscCheck size={12} /> : <VscCopy size={12} />}
        </button>
      </div>
      <div className={styles.balanceRow}>
        <span className={styles.balance}>{balanceInTon} TON</span>
        <span className={styles.kind}>Type: {info.kind}</span>
        <div style={{flexGrow: 1}}></div>
        <div className={styles.airdropGroup}>
          <button
            type="button"
            className={styles.airdropButton}
            onClick={onAirdrop}
            disabled={airdropStatus === "requesting"}
          >
            {airdropStatus === "requesting" ? "Requesting..." : "Request testnet TON"}
          </button>
          <span
            className={styles.helpIcon}
            title="Request testnet TONs from faucet by solving a small Proof-of-Work challenge"
          >
            <VscQuestion size={14} />
          </span>
        </div>
      </div>
      {airdropStatus === "requesting" && (
        <div className={styles.progressBar}>
          <div className={styles.progressInner}></div>
        </div>
      )}
    </div>
  )
}
