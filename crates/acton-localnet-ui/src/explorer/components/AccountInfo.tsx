import {InfoPopover} from "@acton/shared-ui"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import {Check, Copy, Edit2, QrCode, Star, X} from "lucide-react"
import {QRCodeSVG} from "qrcode.react"
import type {FC} from "react"
import {useEffect, useId, useRef, useState} from "react"
import type {TonClient} from "../api/client"
import type {ContractAbiLink, ExtendedContractABI} from "../api/compilerAbi"
import type {AddressInformation, JettonMasterMetadata, JettonWallet} from "../api/types"
import {useAddressBook, useAddressName} from "../hooks/useAddressBook"
import {useFavoriteAccounts} from "../hooks/useFavoriteAccounts"
import {type ExplorerNetworkId, useNetworkInfo} from "../hooks/useNetworkInfo"

import styles from "./AccountInfo.module.css"
import {
  getImageSources,
  getPrimaryImageSource,
  replaceBrokenImageWithFallback,
  TOKEN_IMAGE_SOURCE_KEYS,
} from "./imageFallbacks"
import {formatAddress, formatNano, normalizeAddress, toRawAddress} from "./utils"

const TOKEN_PREVIEW_LIMIT = 5

interface AccountInfoProps {
  readonly address: string
  readonly state?: AddressInformation
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
  readonly imageSources?: readonly string[]
  readonly name?: string
}

export const AccountInfo: FC<AccountInfoProps> = ({
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
  const {isFavorite, toggleFavorite} = useFavoriteAccounts()
  const resolvedName = useAddressName(address)
  const {addressFormat, forkNetwork, network} = useNetworkInfo()
  const displayAddress = normalizeAddress(address, addressFormat)
  const rawAddress = toRawAddress(address)

  const [tokenMastersByAddress, setTokenMastersByAddress] = useState<
    Map<string, JettonMasterMetadata>
  >(() => new Map())
  const [tokenMastersLoading, setTokenMastersLoading] = useState(false)

  const [copied, setCopied] = useState(false)
  const favorite = isFavorite(address)

  useEffect(() => {
    let isActive = true

    const inlineMasters = new Map<string, JettonMasterMetadata>()
    const missingJettonAddresses = new Set<string>()

    for (const wallet of jettonWallets.slice(0, TOKEN_PREVIEW_LIMIT)) {
      const key = toRawAddress(wallet.jetton)
      if (wallet.master) {
        inlineMasters.set(key, wallet.master)
      } else {
        missingJettonAddresses.add(wallet.jetton)
      }
    }

    setTokenMastersByAddress(inlineMasters)
    if (missingJettonAddresses.size === 0) {
      setTokenMastersLoading(false)
      return
    }

    setTokenMastersLoading(true)
    void client
      .getJettonMasters([...missingJettonAddresses])
      .then(masters => {
        if (!isActive) return
        setTokenMastersByAddress(
          new Map([
            ...inlineMasters,
            ...masters.map(master => [toRawAddress(master.address), master] as const),
          ]),
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
  }, [rawAddress])

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

  const handleToggleFavorite = () => {
    toggleFavorite(address)
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
  const statusInfo = getStatusInfo(state)
  const shortAddress = formatAddress(displayAddress, true, addressFormat)
  const addressRowText = hasContextCard ? shortAddress : displayAddress
  const statusAddress = formatRawAddress(displayAddress)
  const tonscanUrl = getTonscanUrl(displayAddress, network.id, forkNetwork)
  const isNameUnchanged = editValue.trim() === (customName || "")
  const stateLoading = accountLoading
  const firstWallet = jettonWallets[0]
  const canOpenTokens = Boolean(onMoreAssetsClick)
  const canOpenCollectibles = Boolean(onCollectiblesClick)
  const showCollectiblesRow = collectiblesLoading || collectiblesCount > 0
  const visibleCollectibles = collectiblePreviews.slice(0, 8)
  const firstMaster = firstWallet
    ? (firstWallet.master ?? tokenMastersByAddress.get(toRawAddress(firstWallet.jetton)))
    : undefined
  const assetMetadataLoading = jettonWallets.length > 0 && tokenMastersLoading && !firstMaster
  const showAssetsSkeleton = assetsLoading || stateLoading || assetMetadataLoading
  const tokenPreviewWallets = jettonWallets.slice(1, TOKEN_PREVIEW_LIMIT)
  const tokenPreviewItems = tokenPreviewWallets.map(wallet => ({
    wallet,
    master: wallet.master ?? tokenMastersByAddress.get(toRawAddress(wallet.jetton)),
  }))
  const firstWalletDecimals = Number(firstMaster?.jetton_content?.decimals || 9)
  const firstWalletSymbol = firstMaster?.jetton_content?.symbol || "tokens"
  const firstWalletImageSources = getImageSources(
    firstMaster?.jetton_content,
    TOKEN_IMAGE_SOURCE_KEYS,
  )
  const firstWalletImage = getPrimaryImageSource(
    firstMaster?.jetton_content,
    TOKEN_IMAGE_SOURCE_KEYS,
  )
  const cardClassName = hasContextCard ? `${styles.card} ${styles.cardCompactQr}` : styles.card

  const qrCode = (
    <QRCodeSVG
      value={rawAddress}
      size={132}
      level="M"
      marginSize={3}
      bgColor="var(--tonscan-card-bg)"
      fgColor="var(--tonscan-text-primary)"
      title={`QR code for ${rawAddress}`}
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
                  <button
                    type="button"
                    className={`${styles.iconButton} ${favorite ? styles.favoriteButtonActive : ""}`}
                    onClick={handleToggleFavorite}
                    title={favorite ? "Remove from favorites" : "Add to favorites"}
                    aria-label={favorite ? "Remove from favorites" : "Add to favorites"}
                    aria-pressed={favorite}
                  >
                    <Star size={16} className={favorite ? styles.favoriteIconActive : undefined} />
                  </button>
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
                          src={firstWalletImage}
                          alt={firstMaster?.jetton_content?.symbol || "Jetton"}
                          className={styles.assetIconImage}
                          onError={event =>
                            replaceBrokenImageWithFallback(event, firstWalletImageSources)
                          }
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
                          {tokenPreviewItems.map(({wallet, master}, index) => {
                            const imageSources = getImageSources(
                              master?.jetton_content,
                              TOKEN_IMAGE_SOURCE_KEYS,
                            )
                            const image = imageSources[0]
                            return image ? (
                              <img
                                key={wallet.address}
                                src={image}
                                alt={master?.jetton_content.symbol || "Jetton"}
                                className={styles.assetPreviewIcon}
                                style={{
                                  zIndex: tokenPreviewItems.length - index,
                                }}
                                onError={event =>
                                  replaceBrokenImageWithFallback(event, imageSources)
                                }
                              />
                            ) : (
                              <span
                                key={wallet.address}
                                className={styles.assetPreviewPlaceholder}
                                style={{
                                  zIndex: tokenPreviewItems.length - index,
                                }}
                              />
                            )
                          })}
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
                                  onError={event =>
                                    replaceBrokenImageWithFallback(
                                      event,
                                      item.imageSources ?? [item.image ?? ""],
                                    )
                                  }
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
                          <InfoPopover
                            id={contractDescriptionId}
                            ariaLabel="Show contract description"
                          >
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
                          </InfoPopover>
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
              <div className={`${styles.skeleton} ${styles.statusSkeleton}`} />
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
      return "Jetton Master interface"
    }
    case "jetton_wallet": {
      return "Jetton Wallet interface"
    }
    case "nft_item": {
      return "NFT item interface"
    }
    case "nft_collection": {
      return "NFT collection interface"
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

function getStatusInfo(state?: AddressInformation): {
  readonly label: string
  readonly className: "statusActive" | "statusFrozen" | "statusUninit" | "statusNonexist"
} {
  if (state && isEmptyZeroBalanceAccount(state)) {
    return {label: "Nonexist", className: "statusNonexist"}
  }

  switch (state?.status) {
    case "active": {
      return {label: "Active", className: "statusActive"}
    }
    case "frozen": {
      return {label: "Frozen", className: "statusFrozen"}
    }
    case "nonexist": {
      return {label: "Nonexist", className: "statusNonexist"}
    }
    case "uninitialized":
    case "uninit": {
      return {label: "Uninit", className: "statusUninit"}
    }
    default: {
      return {label: "-", className: "statusUninit"}
    }
  }
}

function isEmptyZeroBalanceAccount(state: AddressInformation): boolean {
  if (hasCellData(state.code) || hasCellData(state.data)) {
    return false
  }
  try {
    return BigInt(state.balance) === 0n
  } catch {
    return false
  }
}

function hasCellData(value: string | null): boolean {
  return value !== null && value.trim().length > 0
}

function getTonscanUrl(
  address: string,
  networkId: ExplorerNetworkId,
  forkNetwork?: string,
): string | undefined {
  const normalizedNetwork =
    networkId === "mainnet" || networkId === "testnet"
      ? networkId
      : normalizeForkNetwork(forkNetwork)

  if (!normalizedNetwork) {
    return undefined
  }

  const encodedAddress = encodeURIComponent(address)
  if (normalizedNetwork === "testnet") {
    return `https://testnet.tonscan.org/address/${encodedAddress}`
  }

  return `https://tonscan.org/address/${encodedAddress}`
}

function normalizeForkNetwork(forkNetwork?: string): "mainnet" | "testnet" | undefined {
  const normalizedFork = forkNetwork?.trim().toLowerCase()
  if (normalizedFork === "mainnet" || normalizedFork === "testnet") {
    return normalizedFork
  }
  return undefined
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
