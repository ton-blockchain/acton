import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import {Check, Copy, Edit2, QrCode, X} from "lucide-react"
import type React from "react"
import {useEffect, useRef, useState} from "react"
import {QRCodeSVG} from "qrcode.react"

import type {FullAccountState, JettonMaster, JettonWallet} from "../api/types"
import {TonClient} from "../api/client"
import {useAddressBook, useAddressName} from "../hooks/useAddressBook"
import {useNetworkInfo} from "../hooks/useNetworkInfo"

import styles from "./AccountInfo.module.css"
import {formatAddress, formatNano, normalizeAddress, toRawAddress} from "./utils"

const TOKEN_PREVIEW_LIMIT = 5
const TOKEN_PLACEHOLDER_IMAGE = "/token-placeholder.svg"

interface AccountInfoProps {
  readonly address: string
  readonly state?: FullAccountState
  readonly compilerAbi?: ContractABI
  readonly contractInterfaces?: readonly string[]
  readonly jettonWallets: JettonWallet[]
  readonly accountLoading?: boolean
  readonly assetsLoading?: boolean
  readonly amount?: string
  readonly amountLoading?: boolean
  readonly client: TonClient
  readonly onMoreAssetsClick?: () => void
  readonly collectiblesCount?: number
  readonly collectiblePreviews?: readonly CollectiblePreview[]
  readonly collectiblesLoading?: boolean
  readonly onCollectiblesClick?: () => void
  readonly hasContextCard?: boolean
}

interface CollectiblePreview {
  readonly image?: string
  readonly name?: string
}

export const AccountInfo: React.FC<AccountInfoProps> = ({
  address,
  state,
  compilerAbi,
  contractInterfaces,
  jettonWallets,
  accountLoading = false,
  assetsLoading = false,
  amount,
  amountLoading = false,
  client,
  onMoreAssetsClick,
  collectiblesCount = 0,
  collectiblePreviews = [],
  collectiblesLoading = false,
  onCollectiblesClick,
  hasContextCard = false,
}) => {
  const [isEditing, setIsEditing] = useState(false)
  const [customName, setCustomName] = useState<string | undefined>()
  const [editValue, setEditValue] = useState("")
  const [renameSaving, setRenameSaving] = useState(false)
  const [qrOpen, setQrOpen] = useState(false)
  const editInputRef = useRef<HTMLInputElement>(null)
  const {setAddressName} = useAddressBook()
  const resolvedName = useAddressName(address)
  const {addressFormat, forkNetwork} = useNetworkInfo()
  const displayAddress = normalizeAddress(address, addressFormat)

  const [tokenMastersByAddress, setTokenMastersByAddress] = useState<Map<string, JettonMaster>>(
    () => new Map(),
  )
  const [tokenMastersLoading, setTokenMastersLoading] = useState(false)

  const [copied, setCopied] = useState(false)

  useEffect(() => {
    let isActive = true

    const previewJettonAddresses = [
      ...new Set(jettonWallets.slice(0, TOKEN_PREVIEW_LIMIT).map(wallet => wallet.jetton)),
    ]

    if (previewJettonAddresses.length > 0) {
      setTokenMastersByAddress(new Map())
      setTokenMastersLoading(true)
      void client
        .getJettonMasters(previewJettonAddresses)
        .then(masters => {
          if (!isActive) return
          setTokenMastersByAddress(
            new Map(masters.map(master => [toRawAddress(master.address), master])),
          )
        })
        .catch(error => {
          if (isActive) {
            console.error("Failed to fetch jetton master previews", error)
          }
        })
        .finally(() => {
          if (isActive) setTokenMastersLoading(false)
        })
    } else {
      setTokenMastersByAddress(new Map())
      setTokenMastersLoading(false)
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

  useEffect(() => {
    setQrOpen(false)
  }, [displayAddress])

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

  const gramBalance = state ? formatNano(state.balance) : undefined

  const copyToClipboard = () => {
    void navigator.clipboard.writeText(displayAddress)
    setCopied(true)
  }

  const contractTypeLabels = getContractTypeLabels(compilerAbi, contractInterfaces)
  const contractTypeText = contractTypeLabels.join(", ")
  const statusInfo = getStatusInfo(state?.state)
  const shortAddress = formatAddress(displayAddress, true, addressFormat)
  const addressRowText = hasContextCard ? shortAddress : displayAddress
  const statusAddress = formatRawAddress(displayAddress)
  const tonscanUrl = getTonscanUrl(displayAddress, forkNetwork)
  const isNameUnchanged = editValue.trim() === (customName || "")
  const stateLoading = accountLoading
  const assetMetadataLoading = jettonWallets.length > 0 && tokenMastersLoading
  const showAssetsSkeleton = assetsLoading || stateLoading || assetMetadataLoading
  const firstWallet = jettonWallets[0]
  const canOpenTokens = Boolean(onMoreAssetsClick)
  const canOpenCollectibles = Boolean(onCollectiblesClick)
  const showCollectiblesRow = collectiblesLoading || collectiblesCount > 0
  const visibleCollectibles = collectiblePreviews.slice(0, 8)
  const firstMaster = firstWallet
    ? tokenMastersByAddress.get(toRawAddress(firstWallet.jetton))
    : undefined
  const tokenPreviewWallets = jettonWallets.slice(1, TOKEN_PREVIEW_LIMIT)
  const tokenPreviewItems = tokenPreviewWallets.map(wallet => ({
    wallet,
    master: tokenMastersByAddress.get(toRawAddress(wallet.jetton)),
  }))
  const firstWalletDecimals = Number(firstMaster?.jetton_content?.decimals || 9)
  const firstWalletSymbol = firstMaster?.jetton_content?.symbol || "tokens"
  const cardClassName = hasContextCard ? `${styles.card} ${styles.cardCompactQr}` : styles.card

  const qrCode = (
    <QRCodeSVG
      value={displayAddress}
      size={132}
      level="M"
      marginSize={3}
      bgColor="var(--tonscan-card-bg)"
      fgColor="var(--tonscan-text-primary)"
      title={`QR code for ${displayAddress}`}
      className={styles.qrSvg}
    />
  )

  return (
    <div className={cardClassName}>
      <div className={styles.cardBody}>
        <div className={styles.infoColumn}>
          <div className={styles.rows}>
            {isEditing ? (
              <div className={styles.infoRow}>
                <div className={styles.label}>Name</div>
                <div className={styles.rowValue}>
                  <div className={styles.renamePanel}>
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
                </div>
              </div>
            ) : customName ? (
              <div className={styles.infoRow}>
                <div className={styles.label}>Name</div>
                <div className={styles.rowValue}>
                  <span className={styles.customName}>{customName}</span>
                  <button
                    type="button"
                    className={styles.iconButton}
                    onClick={handleStartEdit}
                    title="Rename address"
                    aria-label="Rename address"
                  >
                    <Edit2 size={16} />
                  </button>
                </div>
              </div>
            ) : undefined}

            <div className={`${styles.infoRow} ${styles.addressInfoRow}`}>
              <div className={styles.label}>Address</div>
              <div className={styles.rowValue}>
                <span className={styles.addressValue} title={displayAddress}>
                  {addressRowText}
                </span>
                <span className={styles.addressActions}>
                  {!customName && !isEditing && (
                    <button
                      type="button"
                      className={styles.iconButton}
                      onClick={handleStartEdit}
                      title="Rename address"
                      aria-label="Rename address"
                    >
                      <Edit2 size={16} />
                    </button>
                  )}
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
            </div>

            <div className={styles.infoRow}>
              <div className={styles.label}>Balance</div>
              <div className={styles.rowValue}>
                {stateLoading ? (
                  <div className={`${styles.skeleton} ${styles.skeletonValue}`} />
                ) : state ? (
                  <span className={styles.primaryValue}>{gramBalance} GRAM</span>
                ) : (
                  <span className={styles.mutedValue}>-</span>
                )}
              </div>
            </div>

            {(amountLoading || amount) && (
              <div className={styles.infoRow}>
                <div className={styles.label}>Amount</div>
                <div className={styles.rowValue}>
                  {amountLoading ? (
                    <div className={`${styles.skeleton} ${styles.skeletonValue}`} />
                  ) : (
                    <span className={styles.primaryValue}>{amount}</span>
                  )}
                </div>
              </div>
            )}

            {(showAssetsSkeleton || jettonWallets.length > 0) && (
              <div className={styles.infoRow}>
                <div className={styles.label}>Tokens</div>
                <div className={styles.rowValue}>
                  {showAssetsSkeleton ? (
                    <div className={styles.assetRow}>
                      <div className={`${styles.skeleton} ${styles.skeletonIcon}`} />
                      <div className={`${styles.skeleton} ${styles.skeletonValue}`} />
                    </div>
                  ) : firstWallet ? (
                    <div className={styles.assetRow}>
                      <button
                        type="button"
                        className={styles.assetLink}
                        onClick={onMoreAssetsClick}
                        disabled={!canOpenTokens}
                      >
                        {firstMaster?.jetton_content?.image ? (
                          <img
                            src={firstMaster.jetton_content.image}
                            alt={firstMaster.jetton_content.symbol || "Jetton"}
                            className={styles.assetIconImage}
                          />
                        ) : (
                          <div className={styles.assetIcon} />
                        )}
                        <span className={styles.primaryValue}>
                          {formatTokenAmount(firstWallet.balance, firstWalletDecimals)}{" "}
                          {firstWalletSymbol}
                        </span>
                      </button>
                      {tokenPreviewItems.length > 0 && (
                        <button
                          type="button"
                          className={styles.assetPreviewStack}
                          onClick={onMoreAssetsClick}
                          disabled={!canOpenTokens}
                          aria-label="Open all tokens"
                        >
                          {tokenPreviewItems.map(({wallet, master}, index) =>
                            master?.jetton_content?.image ? (
                              <img
                                key={wallet.address}
                                src={master.jetton_content.image}
                                alt={master.jetton_content.symbol || "Jetton"}
                                className={styles.assetPreviewIcon}
                                style={{zIndex: tokenPreviewItems.length - index}}
                                onError={event => {
                                  const image = event.currentTarget
                                  if (image.getAttribute("src") === TOKEN_PLACEHOLDER_IMAGE) {
                                    return
                                  }
                                  image.src = TOKEN_PLACEHOLDER_IMAGE
                                }}
                              />
                            ) : (
                              <span
                                key={wallet.address}
                                className={styles.assetPreviewPlaceholder}
                                style={{zIndex: tokenPreviewItems.length - index}}
                              />
                            ),
                          )}
                        </button>
                      )}
                      {canOpenTokens && jettonWallets.length > 0 && (
                        <button
                          type="button"
                          className={styles.moreLink}
                          onClick={onMoreAssetsClick}
                        >
                          View all
                        </button>
                      )}
                    </div>
                  ) : undefined}
                </div>
              </div>
            )}

            {showCollectiblesRow && (
              <div className={styles.infoRow}>
                <div className={styles.label}>Collectibles</div>
                <div className={styles.rowValue}>
                  {collectiblesLoading ? (
                    <div className={styles.collectiblesRow}>
                      <div className={`${styles.skeleton} ${styles.skeletonThumb}`} />
                      <div className={`${styles.skeleton} ${styles.skeletonThumb}`} />
                      <div className={`${styles.skeleton} ${styles.skeletonThumb}`} />
                    </div>
                  ) : (
                    <div className={styles.collectiblesRow}>
                      <button
                        type="button"
                        className={styles.collectiblesLink}
                        onClick={onCollectiblesClick}
                        disabled={!canOpenCollectibles}
                      >
                        {visibleCollectibles.length > 0 ? (
                          <span className={styles.collectibleThumbs}>
                            {visibleCollectibles.map((item, index) =>
                              item.image ? (
                                <img
                                  key={`${item.image}-${index}`}
                                  src={item.image}
                                  alt={item.name || "NFT"}
                                  className={styles.collectibleThumb}
                                />
                              ) : (
                                <span
                                  key={`collectible-placeholder-${index}`}
                                  className={styles.collectibleThumbPlaceholder}
                                />
                              ),
                            )}
                          </span>
                        ) : (
                          <span className={styles.primaryValue}>
                            {formatCollectibleCount(collectiblesCount)}
                          </span>
                        )}
                      </button>
                      {canOpenCollectibles && (
                        <button
                          type="button"
                          className={styles.moreLink}
                          onClick={onCollectiblesClick}
                        >
                          View all
                        </button>
                      )}
                    </div>
                  )}
                </div>
              </div>
            )}

            <div className={styles.infoRow}>
              <div className={styles.label}>Contract type</div>
              <div className={styles.rowValue}>
                {stateLoading ? (
                  <div className={`${styles.skeleton} ${styles.skeletonTagWide}`} />
                ) : (
                  <span className={styles.primaryValue}>{contractTypeText}</span>
                )}
              </div>
            </div>
          </div>

          <div className={styles.statusBar}>
            {stateLoading ? (
              <div className={`${styles.skeleton} ${styles.skeletonTag}`} />
            ) : (
              <span className={`${styles.status} ${styles[statusInfo.className]}`}>
                {statusInfo.label}
              </span>
            )}
            <span className={styles.statusAddress}>{statusAddress}</span>
            {tonscanUrl && (
              <>
                <a
                  className={styles.externalLink}
                  href={tonscanUrl}
                  target="_blank"
                  rel="noreferrer"
                >
                  tonscan.org
                </a>
              </>
            )}
          </div>
        </div>

        <button
          type="button"
          className={styles.qrToggle}
          onClick={() => setQrOpen(value => !value)}
          title="Show QR code"
          aria-label="Show QR code"
          aria-expanded={qrOpen}
        >
          <QrCode size={16} />
        </button>

        <div className={styles.qrPanel} aria-label="Address QR code">
          {qrCode}
        </div>
      </div>

      {qrOpen && (
        <div className={styles.qrPopover}>
          <div className={styles.qrPopoverHeader}>
            <span>QR</span>
            <button
              type="button"
              className={styles.iconButton}
              onClick={() => setQrOpen(false)}
              aria-label="Close QR code"
            >
              <X size={16} />
            </button>
          </div>
          <div className={styles.qrPopoverBody}>{qrCode}</div>
        </div>
      )}
    </div>
  )
}

function getContractTypeLabels(
  compilerAbi?: ContractABI,
  interfaces?: readonly string[],
): string[] {
  const abiContractName = compilerAbi?.contract_name?.trim()
  const interfaceLabels = (interfaces ?? [])
    .map(value => getInterfaceLabel(value))
    .filter((value): value is string => value !== undefined)

  const labels = abiContractName ? [abiContractName, ...interfaceLabels] : interfaceLabels
  return labels.length > 0 ? [...new Set(labels)] : ["Unknown"]
}

function getInterfaceLabel(value: string): string | undefined {
  const normalizedInterface = value.trim().toLowerCase()
  if (!normalizedInterface) {
    return undefined
  }

  switch (normalizedInterface) {
    case "jetton_master": {
      return "Jetton master"
    }
    case "jetton_wallet": {
      return "Jetton wallet"
    }
    case "nft_item": {
      return "NFT item"
    }
    case "nft_collection": {
      return "NFT collection"
    }
    default: {
      return normalizedInterface.replaceAll("_", " ")
    }
  }
}

function getStatusInfo(state?: FullAccountState["state"]): {
  readonly label: string
  readonly className: "statusActive" | "statusFrozen" | "statusUninit" | "statusNonexist"
} {
  switch (state) {
    case "active": {
      return {label: "Active", className: "statusActive"}
    }
    case "frozen": {
      return {label: "Frozen", className: "statusFrozen"}
    }
    case "nonexist": {
      return {label: "Nonexist", className: "statusNonexist"}
    }
    case "uninitialized": {
      return {label: "Uninit", className: "statusUninit"}
    }
    default: {
      return {label: "-", className: "statusUninit"}
    }
  }
}

function getTonscanUrl(address: string, forkNetwork?: string): string | undefined {
  if (!forkNetwork) {
    return undefined
  }

  const normalizedFork = forkNetwork.trim().toLowerCase()
  const baseUrl =
    normalizedFork === "testnet"
      ? "https://testnet.tonscan.org/address/"
      : "https://tonscan.org/address/"
  return `${baseUrl}${encodeURIComponent(address)}`
}

function formatTokenAmount(value: string, decimals: number): string {
  const decimalsNumber = Number.isFinite(decimals) ? decimals : 9
  return (Number(value) / 10 ** decimalsNumber).toLocaleString(undefined, {
    maximumFractionDigits: decimalsNumber,
  })
}

function formatCollectibleCount(count: number): string {
  return `${count.toLocaleString()} ${count === 1 ? "NFT" : "NFTs"}`
}

function formatRawAddress(address: string): string {
  const [workchain, hash] = toRawAddress(address).trim().split(":")
  if (!workchain || !hash) {
    return address
  }
  if (hash.length <= 11) {
    return `${workchain}:${hash}`
  }
  return `${workchain}:${hash.slice(0, 3)}…${hash.slice(-5)}`
}
