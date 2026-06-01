import {Card, CardContent, CardHeader} from "@acton/shared-ui"
import {Check, Copy, Edit2} from "lucide-react"
import type React from "react"
import {useEffect, useRef, useState} from "react"

import type {FullAccountState, JettonMaster, JettonWallet} from "../api/types"
import {TonClient} from "../api/client"
import {useAddressBook, useAddressName} from "../hooks/useAddressBook"
import {useAddressFormat} from "../hooks/useNetworkInfo"

import styles from "./AccountInfo.module.css"
import {formatAddress, formatNano, normalizeAddress} from "./utils"

interface AccountInfoProps {
  readonly address: string
  readonly state?: FullAccountState
  readonly contractInterfaces?: readonly string[]
  readonly jettonWallets: JettonWallet[]
  readonly accountLoading?: boolean
  readonly assetsLoading?: boolean
  readonly client: TonClient
  readonly onMoreAssetsClick?: () => void
}

export const AccountInfo: React.FC<AccountInfoProps> = ({
  address,
  state,
  contractInterfaces,
  jettonWallets,
  accountLoading = false,
  assetsLoading = false,
  client,
  onMoreAssetsClick,
}) => {
  const [isEditing, setIsEditing] = useState(false)
  const [customName, setCustomName] = useState<string | undefined>()
  const [editValue, setEditValue] = useState("")
  const [renameSaving, setRenameSaving] = useState(false)
  const editInputRef = useRef<HTMLInputElement>(null)
  const {setAddressName} = useAddressBook()
  const resolvedName = useAddressName(address)
  const addressFormat = useAddressFormat()
  const displayAddress = normalizeAddress(address, addressFormat)

  const [firstMaster, setFirstMaster] = useState<JettonMaster | undefined>()
  const [firstMasterLoading, setFirstMasterLoading] = useState(false)

  const [copied, setCopied] = useState(false)

  useEffect(() => {
    let isActive = true

    if (jettonWallets.length > 0) {
      setFirstMaster(undefined)
      setFirstMasterLoading(true)
      void client
        .getJettonMasters([jettonWallets[0].jetton])
        .then(masters => {
          if (!isActive) return
          setFirstMaster(masters[0])
        })
        .catch(error => {
          if (isActive) {
            console.error("Failed to fetch first jetton master", error)
          }
        })
        .finally(() => {
          if (isActive) setFirstMasterLoading(false)
        })
    } else {
      setFirstMaster(undefined)
      setFirstMasterLoading(false)
    }

    return () => {
      isActive = false
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

  useEffect(() => {
    if (!isEditing) return
    editInputRef.current?.focus()
    editInputRef.current?.select()
  }, [isEditing])

  const handleStartEdit = () => {
    setEditValue(customName || "")
    setIsEditing(true)
  }

  const handleSave = async () => {
    const nextName = editValue.trim()
    setRenameSaving(true)
    try {
      await setAddressName(address, nextName)
      setCustomName(nextName || undefined)
      setIsEditing(false)
    } catch (error) {
      console.error("Failed to save name:", error)
    } finally {
      setRenameSaving(false)
    }
  }

  const tonBalance = state ? formatNano(state.balance) : undefined

  const copyToClipboard = () => {
    void navigator.clipboard.writeText(displayAddress)
    setCopied(true)
  }

  const contractTypeLabel = getContractTypeLabel(contractInterfaces)
  const isNameUnchanged = editValue.trim() === (customName || "")
  const statePending = !state
  const stateLoading = accountLoading || statePending
  const assetMetadataLoading = jettonWallets.length > 0 && firstMasterLoading
  const showAssetsSkeleton = assetsLoading || stateLoading || assetMetadataLoading

  return (
    <Card className={styles.card}>
      <CardHeader className={styles.header}>
        <div className={styles.addressTitle}>Address</div>
        <div className={styles.addressHeader}>
          {isEditing ? (
            <div className={styles.renamePanel}>
              <div className={styles.renameRow}>
                <input
                  ref={editInputRef}
                  type="text"
                  className={styles.editInput}
                  value={editValue}
                  autoComplete="off"
                  spellCheck="false"
                  aria-label="Custom address name"
                  onChange={e => setEditValue(e.target.value)}
                  onKeyDown={e => {
                    if (e.key === "Enter" && !isNameUnchanged) {
                      void handleSave()
                    } else if (e.key === "Escape") {
                      setIsEditing(false)
                    }
                  }}
                  placeholder="Name this address"
                />
                <button
                  type="button"
                  className={styles.renameSaveButton}
                  onClick={() => {
                    void handleSave()
                  }}
                  disabled={renameSaving || isNameUnchanged}
                >
                  {renameSaving ? "Saving..." : "Save"}
                </button>
                <button
                  type="button"
                  className={styles.renameCancelButton}
                  onClick={() => setIsEditing(false)}
                >
                  Cancel
                </button>
              </div>
              <div className={styles.renameMeta}>
                {formatAddress(displayAddress, false, addressFormat)}
              </div>
            </div>
          ) : (
            <div className={styles.addressRow}>
              <span className={styles.addressValue}>
                {customName ? (
                  <span className={styles.customName}>
                    {customName}{" "}
                    <span className={styles.realAddress}>
                      ({formatAddress(displayAddress, true, addressFormat)})
                    </span>
                  </span>
                ) : (
                  formatAddress(displayAddress, false, addressFormat)
                )}
              </span>
              <span className={styles.addressActions}>
                <button
                  type="button"
                  className={styles.iconButton}
                  onClick={handleStartEdit}
                  title="Rename address"
                  aria-label="Rename address"
                >
                  <Edit2 size={16} />
                </button>
                <button
                  type="button"
                  className={styles.iconButton}
                  onClick={copyToClipboard}
                  title={copied ? "Copied" : "Copy address"}
                  aria-label={copied ? "Copied" : "Copy address"}
                >
                  {copied ? <Check size={16} className={styles.saveIcon} /> : <Copy size={16} />}
                </button>
              </span>
            </div>
          )}
        </div>
      </CardHeader>
      <CardContent className={styles.grid}>
        <div className={styles.section}>
          <div className={styles.label}>Balance</div>
          {stateLoading ? (
            <div className={`${styles.skeleton} ${styles.skeletonValue}`} />
          ) : (
            <div className={styles.value}>{tonBalance} TON</div>
          )}
        </div>
        <div className={styles.section}>
          <div className={styles.label}>Assets</div>
          {showAssetsSkeleton ? (
            <div className={styles.assetRow}>
              <div className={`${styles.skeleton} ${styles.skeletonIcon}`} />
              <div className={`${styles.skeleton} ${styles.skeletonValue}`} />
            </div>
          ) : jettonWallets.length > 0 ? (
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
          {stateLoading ? (
            <div className={styles.detailsGrid}>
              <div className={`${styles.skeleton} ${styles.skeletonTag}`} />
              <div className={`${styles.skeleton} ${styles.skeletonTagWide}`} />
            </div>
          ) : state ? (
            <div className={styles.detailsGrid}>
              <span
                className={`${styles.status} ${state.state === "active" ? "" : styles.statusUninitialized}`}
              >
                {state.state}
              </span>
              <span className={styles.tag}>{contractTypeLabel}</span>
            </div>
          ) : undefined}
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
