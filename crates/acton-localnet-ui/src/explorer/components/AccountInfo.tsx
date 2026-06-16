import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import {Check, Copy, Edit2, Info, QrCode, X} from "lucide-react"
import type React from "react"
import {useCallback, useEffect, useId, useLayoutEffect, useRef, useState} from "react"
import {createPortal} from "react-dom"
import {QRCodeSVG} from "qrcode.react"

import type {FullAccountState, JettonMaster, JettonWallet} from "../api/types"
import type {TonClient} from "../api/client"
import type {ContractAbiLink, ExtendedContractABI} from "../api/compilerAbi"
import {useAddressBook, useAddressName} from "../hooks/useAddressBook"
import {useNetworkInfo} from "../hooks/useNetworkInfo"

import styles from "./AccountInfo.module.css"
import {formatAddress, formatNano, normalizeAddress, toRawAddress} from "./utils"

const TOKEN_PREVIEW_LIMIT = 5
const TOKEN_PLACEHOLDER_IMAGE = "/token-placeholder.svg"

interface AccountInfoProps {
  readonly address: string
  readonly state?: FullAccountState
  readonly extendedContractAbi?: ExtendedContractABI
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
  extendedContractAbi,
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
  const contractDescriptionId = useId()
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

  const compilerAbi = extendedContractAbi?.compiler_abi
  const contractTypeLabels = getContractTypeLabels(compilerAbi, contractInterfaces)
  const contractDescription = compilerAbi?.description?.trim()
  const contractDescriptionTitle =
    extendedContractAbi?.display_name?.trim() ||
    compilerAbi?.contract_name?.trim() ||
    contractTypeLabels[0]
  const contractDescriptionUrl = contractDescription && getExternalUrl(contractDescription)
  const contractLinks = getContractAbiLinks(extendedContractAbi)
  const hasContractDescriptionPopover = Boolean(contractDescription || contractLinks.length > 0)
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
                        <img
                          src={firstMaster?.jetton_content?.image || TOKEN_PLACEHOLDER_IMAGE}
                          alt={firstMaster?.jetton_content?.symbol || "Jetton"}
                          className={styles.assetIconImage}
                          onError={event => {
                            const image = event.currentTarget
                            if (image.getAttribute("src") === TOKEN_PLACEHOLDER_IMAGE) {
                              return
                            }
                            image.src = TOKEN_PLACEHOLDER_IMAGE
                          }}
                        />
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
                  <span className={styles.contractTypeValue}>
                    {contractTypeLabels.map((label, index) => (
                      <span key={`${label}-${index}`} className={styles.contractTypeItem}>
                        <span className={styles.primaryValue}>{label}</span>
                        {index === 0 && hasContractDescriptionPopover && (
                          <ContractDescriptionTooltip id={contractDescriptionId}>
                            <>
                              <span className={styles.contractDescriptionTitle}>
                                {contractDescriptionTitle}
                              </span>
                              {contractDescription && (
                                <>
                                  {contractDescriptionUrl ? (
                                    <a
                                      className={styles.contractDescriptionLink}
                                      href={contractDescriptionUrl}
                                      target="_blank"
                                      rel="noreferrer"
                                    >
                                      {contractDescription}
                                    </a>
                                  ) : (
                                    <span className={styles.contractDescriptionText}>
                                      {contractDescription}
                                    </span>
                                  )}
                                </>
                              )}
                              {contractLinks.length > 0 && (
                                <span className={styles.contractDescriptionLinks}>
                                  {contractLinks.map(link => (
                                    <a
                                      key={`${link.kind ?? "link"}-${link.url}`}
                                      className={styles.contractDescriptionLinkItem}
                                      href={link.url}
                                      target="_blank"
                                      rel="noreferrer"
                                    >
                                      <span className={styles.contractDescriptionLinkKind}>
                                        {formatContractLinkKind(link.kind)}
                                      </span>
                                      <span className={styles.contractDescriptionLinkTitle}>
                                        {link.url}
                                      </span>
                                    </a>
                                  ))}
                                </span>
                              )}
                            </>
                          </ContractDescriptionTooltip>
                        )}
                        {index < contractTypeLabels.length - 1 && (
                          <span className={styles.contractTypeSeparator}>,</span>
                        )}
                      </span>
                    ))}
                  </span>
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

type ContractDescriptionPlacement = "right" | "left" | "bottom" | "top"

interface RectSnapshot {
  readonly left: number
  readonly top: number
  readonly right: number
  readonly bottom: number
  readonly width: number
  readonly height: number
}

interface ContractDescriptionPosition {
  readonly left: number
  readonly top: number
  readonly placement: ContractDescriptionPlacement
  readonly arrowX: number
  readonly arrowY: number
}

interface ContractDescriptionTooltipProps {
  readonly id: string
  readonly children: React.ReactNode
}

const CONTRACT_DESCRIPTION_MARGIN = 12
const CONTRACT_DESCRIPTION_GAP = 12
const CONTRACT_DESCRIPTION_ARROW_MIN = 16

function ContractDescriptionTooltip({
  id,
  children,
}: ContractDescriptionTooltipProps): React.JSX.Element {
  const triggerRef = useRef<HTMLButtonElement | null>(null)
  const popoverRef = useRef<HTMLSpanElement | null>(null)
  const closeTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const [isOpen, setIsOpen] = useState(false)
  const [triggerRect, setTriggerRect] = useState<RectSnapshot | undefined>()
  const [position, setPosition] = useState<ContractDescriptionPosition | undefined>()

  const clearCloseTimer = useCallback((): void => {
    if (closeTimerRef.current) {
      clearTimeout(closeTimerRef.current)
      closeTimerRef.current = undefined
    }
  }, [])

  const openPopover = useCallback((): void => {
    clearCloseTimer()

    if (isOpen) {
      return
    }

    const rect = triggerRef.current?.getBoundingClientRect()
    if (rect) {
      setTriggerRect(snapshotRect(rect))
      setPosition(undefined)
    }
    setIsOpen(true)
  }, [clearCloseTimer, isOpen])

  const closePopover = useCallback((): void => {
    clearCloseTimer()

    closeTimerRef.current = setTimeout(() => {
      setIsOpen(false)
      setPosition(undefined)
    }, 120)
  }, [clearCloseTimer])

  const forceClosePopover = useCallback((): void => {
    clearCloseTimer()
    setIsOpen(false)
    setPosition(undefined)
  }, [clearCloseTimer])

  useLayoutEffect(() => {
    if (!isOpen || !triggerRect || !popoverRef.current) return

    const popoverRect = popoverRef.current.getBoundingClientRect()
    setPosition(
      calculateContractDescriptionPosition(triggerRect, popoverRect.width, popoverRect.height),
    )
  }, [isOpen, triggerRect, children])

  useEffect(() => {
    if (!isOpen) return

    const updateTriggerRect = (): void => {
      const rect = triggerRef.current?.getBoundingClientRect()
      if (rect) {
        setTriggerRect(snapshotRect(rect))
      }
    }

    const handlePointerDown = (event: MouseEvent): void => {
      const target = event.target as Node
      if (triggerRef.current?.contains(target) || popoverRef.current?.contains(target)) {
        return
      }
      forceClosePopover()
    }

    const handleKeyDown = (event: KeyboardEvent): void => {
      if (event.key === "Escape") {
        forceClosePopover()
      }
    }

    window.addEventListener("resize", updateTriggerRect)
    window.addEventListener("scroll", updateTriggerRect, true)
    document.addEventListener("mousedown", handlePointerDown)
    document.addEventListener("keydown", handleKeyDown)

    return () => {
      window.removeEventListener("resize", updateTriggerRect)
      window.removeEventListener("scroll", updateTriggerRect, true)
      document.removeEventListener("mousedown", handlePointerDown)
      document.removeEventListener("keydown", handleKeyDown)
    }
  }, [forceClosePopover, isOpen])

  useEffect(() => {
    return () => {
      if (closeTimerRef.current) {
        clearTimeout(closeTimerRef.current)
      }
    }
  }, [])

  const popoverStyle = {
    left: position?.left ?? CONTRACT_DESCRIPTION_MARGIN,
    top: position?.top ?? CONTRACT_DESCRIPTION_MARGIN,
    "--contract-description-arrow-x": `${position?.arrowX ?? CONTRACT_DESCRIPTION_ARROW_MIN}px`,
    "--contract-description-arrow-y": `${position?.arrowY ?? CONTRACT_DESCRIPTION_ARROW_MIN}px`,
  } as React.CSSProperties

  return (
    <span className={styles.contractDescription}>
      <button
        ref={triggerRef}
        type="button"
        className={styles.contractDescriptionButton}
        aria-label="Show contract description"
        aria-describedby={isOpen ? id : undefined}
        aria-expanded={isOpen}
        onMouseEnter={openPopover}
        onMouseLeave={closePopover}
        onFocus={openPopover}
        onBlur={closePopover}
        onClick={() => {
          if (isOpen) {
            forceClosePopover()
          } else {
            openPopover()
          }
        }}
      >
        <Info size={12} />
      </button>
      {isOpen &&
        createPortal(
          <span
            ref={popoverRef}
            id={id}
            className={styles.contractDescriptionPopover}
            data-placement={position?.placement ?? "right"}
            data-positioned={position ? "true" : "false"}
            role="tooltip"
            style={popoverStyle}
            onMouseEnter={clearCloseTimer}
            onMouseLeave={closePopover}
          >
            <span className={styles.contractDescriptionPopoverContent}>{children}</span>
          </span>,
          document.body,
        )}
    </span>
  )
}

function snapshotRect(rect: DOMRect): RectSnapshot {
  return {
    left: rect.left,
    top: rect.top,
    right: rect.right,
    bottom: rect.bottom,
    width: rect.width,
    height: rect.height,
  }
}

function calculateContractDescriptionPosition(
  triggerRect: RectSnapshot,
  popoverWidth: number,
  popoverHeight: number,
): ContractDescriptionPosition {
  const viewportWidth = window.innerWidth
  const viewportHeight = window.innerHeight
  const triggerCenterX = triggerRect.left + triggerRect.width / 2
  const triggerCenterY = triggerRect.top + triggerRect.height / 2

  const candidates: Array<{
    readonly placement: ContractDescriptionPlacement
    readonly left: number
    readonly top: number
    readonly preference: number
  }> = [
    {
      placement: "right",
      left: triggerRect.right + CONTRACT_DESCRIPTION_GAP,
      top: triggerCenterY - popoverHeight / 2,
      preference: 0,
    },
    {
      placement: "left",
      left: triggerRect.left - popoverWidth - CONTRACT_DESCRIPTION_GAP,
      top: triggerCenterY - popoverHeight / 2,
      preference: 1,
    },
    {
      placement: "bottom",
      left: triggerCenterX - popoverWidth / 2,
      top: triggerRect.bottom + CONTRACT_DESCRIPTION_GAP,
      preference: 2,
    },
    {
      placement: "top",
      left: triggerCenterX - popoverWidth / 2,
      top: triggerRect.top - popoverHeight - CONTRACT_DESCRIPTION_GAP,
      preference: 3,
    },
  ]

  const best = candidates
    .map(candidate => {
      const horizontalOverflow =
        Math.max(CONTRACT_DESCRIPTION_MARGIN - candidate.left, 0) +
        Math.max(candidate.left + popoverWidth - (viewportWidth - CONTRACT_DESCRIPTION_MARGIN), 0)
      const verticalOverflow =
        Math.max(CONTRACT_DESCRIPTION_MARGIN - candidate.top, 0) +
        Math.max(candidate.top + popoverHeight - (viewportHeight - CONTRACT_DESCRIPTION_MARGIN), 0)

      return {
        ...candidate,
        score: horizontalOverflow * 2 + verticalOverflow * 2 + candidate.preference,
      }
    })
    .sort((a, b) => a.score - b.score)[0]

  const left = clamp(
    best.left,
    CONTRACT_DESCRIPTION_MARGIN,
    viewportWidth - popoverWidth - CONTRACT_DESCRIPTION_MARGIN,
  )
  const top = clamp(
    best.top,
    CONTRACT_DESCRIPTION_MARGIN,
    viewportHeight - popoverHeight - CONTRACT_DESCRIPTION_MARGIN,
  )

  return {
    left,
    top,
    placement: best.placement,
    arrowX: clamp(
      triggerCenterX - left,
      CONTRACT_DESCRIPTION_ARROW_MIN,
      popoverWidth - CONTRACT_DESCRIPTION_ARROW_MIN,
    ),
    arrowY: clamp(
      triggerCenterY - top,
      CONTRACT_DESCRIPTION_ARROW_MIN,
      popoverHeight - CONTRACT_DESCRIPTION_ARROW_MIN,
    ),
  }
}

function clamp(value: number, min: number, max: number): number {
  if (max < min) {
    return min
  }

  return Math.min(Math.max(value, min), max)
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

function getExternalUrl(value: string): string | undefined {
  try {
    const url = new URL(value)
    return url.protocol === "http:" || url.protocol === "https:" ? url.toString() : undefined
  } catch {
    return undefined
  }
}

function getContractAbiLinks(extendedContractAbi?: ExtendedContractABI): ContractAbiLink[] {
  return (extendedContractAbi?.links ?? [])
    .map(link => normalizeContractAbiLink(link))
    .filter((link): link is ContractAbiLink => link !== undefined)
}

function normalizeContractAbiLink(link: ContractAbiLink): ContractAbiLink | undefined {
  const rawUrl = link.url.trim()
  const url = getExternalUrl(rawUrl)
  if (!url) {
    return undefined
  }

  const title = link.title.trim()
  const kind = link.kind.trim()
  const scope = link.scope.trim()

  return {
    title: title || url,
    url,
    kind: kind || "link",
    scope,
  }
}

function formatContractLinkKind(kind: string): string {
  return kind.replaceAll("_", " ")
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
