import {Card, CardContent, CardHeader} from "@acton/shared-ui"
import {Check, Copy, Edit2, X} from "lucide-react"
import type React from "react"
import {useEffect, useState} from "react"

import type {FullAccountState, JettonMaster, JettonWallet} from "../api/types"
import {TonClient} from "../api/client"
import {useAddressBook, useAddressName} from "../hooks/useAddressBook"

import styles from "./AccountInfo.module.css"
import {formatAddress, formatNano} from "./utils"

interface AccountInfoProps {
  readonly address: string
  readonly state: FullAccountState
  readonly contractInterfaces?: readonly string[]
  readonly jettonWallets: JettonWallet[]
  readonly client: TonClient
  readonly onMoreAssetsClick?: () => void
}

export const AccountInfo: React.FC<AccountInfoProps> = ({
  address,
  state,
  contractInterfaces,
  jettonWallets,
  client,
  onMoreAssetsClick,
}) => {
  const [isEditing, setIsEditing] = useState(false)
  const [customName, setCustomName] = useState<string | undefined>()
  const [editValue, setEditValue] = useState("")
  const [loading, setLoading] = useState(false)
  const {setAddressName} = useAddressBook()
  const resolvedName = useAddressName(address)

  const [firstMaster, setFirstMaster] = useState<JettonMaster | undefined>()

  const [copied, setCopied] = useState(false)

  useEffect(() => {
    if (jettonWallets.length > 0) {
      void client.getJettonMasters([jettonWallets[0].jetton]).then(masters => {
        setFirstMaster(masters[0])
      })
    } else {
      setFirstMaster(undefined)
    }
  }, [jettonWallets, client])

  useEffect(() => {
    if (copied) {
      const timer = setTimeout(() => setCopied(false), 2000)
      return () => clearTimeout(timer)
    }
  }, [copied])

  useEffect(() => {
    setCustomName(resolvedName || undefined)
  }, [resolvedName])

  const handleStartEdit = () => {
    setEditValue(customName || "")
    setIsEditing(true)
  }

  const handleSave = async () => {
    setLoading(true)
    try {
      await setAddressName(address, editValue || "")
      setCustomName(editValue || undefined)
      setIsEditing(false)
    } catch (error) {
      console.error("Failed to save name:", error)
    } finally {
      setLoading(false)
    }
  }

  const tonBalance = formatNano(state.balance)
  const usdRate = 1.33 // Mock rate for UI matching
  const usdBalance = ((Number(state.balance) / 1e9) * usdRate).toLocaleString(undefined, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })

  const copyToClipboard = () => {
    void navigator.clipboard.writeText(address)
    setCopied(true)
  }

  const contractTypeLabel = getContractTypeLabel(contractInterfaces)

  return (
    <Card className={styles.card}>
      <CardHeader>
        <div className={styles.addressTitle}>Address</div>
        <div className={styles.addressHeader}>
          {isEditing ? (
            <div className={styles.editContainer}>
              <input
                type="text"
                className={styles.editInput}
                value={editValue}
                onChange={e => setEditValue(e.target.value)}
                onKeyDown={e => {
                  if (e.key === "Enter") {
                    void handleSave()
                  } else if (e.key === "Escape") {
                    setIsEditing(false)
                  }
                }}
                placeholder="Enter custom name"
              />
              <button
                type="button"
                className={styles.iconButton}
                onClick={() => {
                  void handleSave()
                }}
                disabled={loading}
              >
                <Check size={18} className={styles.saveIcon} />
              </button>
              <button
                type="button"
                className={styles.iconButton}
                onClick={() => setIsEditing(false)}
              >
                <X size={18} className={styles.cancelIcon} />
              </button>
            </div>
          ) : (
            <div className={styles.addressRow}>
              <div className={styles.addressValue}>
                {customName ? (
                  <span className={styles.customName}>
                    {customName}{" "}
                    <span className={styles.realAddress}>({formatAddress(address, true)})</span>
                  </span>
                ) : (
                  formatAddress(address, false)
                )}
              </div>
              <button type="button" className={styles.iconButton} onClick={handleStartEdit}>
                <Edit2 size={16} />
              </button>
              <button type="button" className={styles.iconButton} onClick={copyToClipboard}>
                {copied ? <Check size={16} className={styles.saveIcon} /> : <Copy size={16} />}
              </button>
            </div>
          )}
        </div>
      </CardHeader>
      <CardContent className={styles.grid}>
        <div className={styles.section}>
          <div className={styles.label}>Balance</div>
          <div className={styles.value}>
            {tonBalance} TON <span className={styles.subValue}>≈ $ {usdBalance}</span>
          </div>
        </div>
        <div className={styles.section}>
          <div className={styles.label}>Assets</div>
          {jettonWallets.length > 0 ? (
            <div className={styles.assetRow}>
              {firstMaster?.jetton_content?.image ? (
                <img
                  src={firstMaster.jetton_content.image}
                  alt={firstMaster.jetton_content.symbol || "Jetton"}
                  className={styles.assetIconImage}
                />
              ) : (
                <div className={styles.assetIcon}></div>
              )}
              <div className={styles.value}>
                <>
                  {(
                    Number(jettonWallets[0].balance) /
                    10 ** Number(firstMaster?.jetton_content?.decimals || 9)
                  ).toLocaleString(undefined, {
                    maximumFractionDigits: Number(firstMaster?.jetton_content?.decimals || 9),
                  })}{" "}
                  {firstMaster?.jetton_content?.symbol || "tokens"}{" "}
                  {jettonWallets.length > 1 && (
                    <span
                      className={styles.moreLink}
                      onClick={onMoreAssetsClick}
                      onKeyDown={e => {
                        if (e.key === "Enter" || e.key === " ") {
                          onMoreAssetsClick?.()
                        }
                      }}
                      role="button"
                      tabIndex={0}
                    >
                      and {jettonWallets.length - 1} more
                    </span>
                  )}
                </>
              </div>
            </div>
          ) : (
            <div className={styles.noAssets}>No assets</div>
          )}
        </div>
        <div className={styles.section}>
          <div className={styles.label}>Details</div>
          <div className={styles.detailsGrid}>
            <span
              className={`${styles.status} ${state.state === "active" ? "" : styles.statusUninitialized}`}
            >
              {state.state}
            </span>
            <span className={styles.tag}>{contractTypeLabel}</span>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

function getContractTypeLabel(interfaces?: readonly string[]): string {
  const primaryInterface = interfaces?.find(iface => iface.length > 0)

  if (!primaryInterface) {
    return "unknown"
  }

  const normalizedInterface = primaryInterface.trim().toLowerCase()

  switch (normalizedInterface) {
    case "jetton_master": {
      return "jetton master"
    }
    case "jetton_wallet": {
      return "jetton wallet"
    }
    case "nft_item": {
      return "nft item"
    }
    case "nft_collection": {
      return "nft collection"
    }
    default: {
      return normalizedInterface.replaceAll("_", " ")
    }
  }
}
