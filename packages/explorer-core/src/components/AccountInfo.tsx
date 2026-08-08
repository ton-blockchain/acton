import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import {Check, Edit2, QrCode, Star} from "lucide-react"
import {QRCodeSVG} from "qrcode.react"
import {memo, useEffect, useId, useRef, useState} from "react"
import type {FC, ReactNode} from "react"
import {
  Button,
  CopyInlineAction,
  formatCountLabel,
  GramAmount,
  humanizeIdentifier,
  InfoPopover,
  InlineAction,
  InlineActions,
  Input,
  Popover,
  shortenMiddle,
  TokenAmount,
  Tooltip,
} from "@acton/ui"

import type {AddressInformation, JettonMasterMetadata, JettonWallet} from "../api/types"
import type {TonClient} from "../api/client"
import type {ContractAbiLink, ExtendedContractABI} from "../api/compilerAbi"
import {useAddressBook, useAddressName, useAddressNameSources} from "../hooks/useAddressBook"
import {useFavoriteAccounts} from "../hooks/useFavoriteAccounts"
import {useNetworkInfo, type ExplorerNetworkId} from "../hooks/useNetworkInfo"

import styles from "./AccountInfo.module.css"
import {NftImage} from "./NftImage"
import {
  TOKEN_IMAGE_SOURCE_KEYS,
  getImageSources,
  getPrimaryImageSource,
  replaceBrokenImageWithFallback,
} from "./imageFallbacks"
import {getAccountNameDetails} from "./accountNameDetails"
import {formatAddress, normalizeAddress, toAccountQrAddress, toRawAddress} from "./utils"

const TOKEN_PREVIEW_LIMIT = 5

const AddressQrCode = memo<{readonly value: string}>(({value}) => (
  <QRCodeSVG
    value={value}
    size={132}
    level="M"
    marginSize={3}
    bgColor="var(--acton-color-surface-raised)"
    fgColor="var(--acton-color-text)"
    title={`QR code for ${value}`}
    className={styles.qrSvg}
  />
))

interface AccountInfoDetail {
  readonly key: string
  readonly label: string
  readonly value: ReactNode
}

interface AccountInfoProps {
  readonly address: string
  readonly domain?: string
  readonly domains?: readonly string[]
  readonly state?: AddressInformation
  readonly extendedContractAbi?: ExtendedContractABI
  readonly contractInterfaces?: readonly string[]
  readonly jettonWallets: JettonWallet[]
  readonly accountLoading?: boolean
  readonly assetsLoading?: boolean
  readonly amount?: ReactNode
  readonly amountLoading?: boolean
  readonly details?: readonly AccountInfoDetail[]
  readonly client: TonClient
  readonly onMoreAssetsClick?: () => void
  readonly collectiblesCount?: number
  readonly collectiblePreviews?: readonly CollectiblePreview[]
  readonly collectiblesLoading?: boolean
  readonly onCollectiblesClick?: () => void
  readonly hasContextCard?: boolean
  readonly showActonscanLink?: boolean
}

interface CollectiblePreview {
  readonly address: string
  readonly image?: string
  readonly imageSources?: readonly string[]
  readonly blurred?: boolean
  readonly collectionName?: string
  readonly name?: string
}

export const AccountInfo: FC<AccountInfoProps> = ({
  address,
  domain,
  domains = [],
  state,
  extendedContractAbi,
  contractInterfaces,
  jettonWallets,
  accountLoading = false,
  assetsLoading = false,
  amount,
  amountLoading = false,
  details = [],
  client,
  onMoreAssetsClick,
  collectiblesCount = 0,
  collectiblePreviews = [],
  collectiblesLoading = false,
  onCollectiblesClick,
  hasContextCard = false,
  showActonscanLink = false,
}) => {
  const [isEditing, setIsEditing] = useState(false)
  const [customName, setCustomName] = useState<string | undefined>()
  const [editValue, setEditValue] = useState("")
  const [renameSaving, setRenameSaving] = useState(false)
  const editInputRef = useRef<HTMLInputElement>(null)
  const contractDescriptionId = useId()
  const {setAddressName} = useAddressBook()
  const {isFavorite, toggleFavorite} = useFavoriteAccounts()
  const resolvedName = useAddressName(address)
  const nameSources = useAddressNameSources(address)
  const {addressFormat, forkNetwork, network} = useNetworkInfo()
  const displayAddress = normalizeAddress(address, addressFormat)
  const bounceableAddress = normalizeAddress(address, {...addressFormat, bounceable: true})
  const nonBounceableAddress = normalizeAddress(address, {...addressFormat, bounceable: false})
  const rawAddress = toRawAddress(address)
  const qrAddress = toAccountQrAddress(address, state?.status, addressFormat)

  const [tokenMastersByAddress, setTokenMastersByAddress] = useState<
    Map<string, JettonMasterMetadata>
  >(() => new Map())
  const [tokenMastersLoading, setTokenMastersLoading] = useState(false)

  const [hiddenCollectibleAddresses, setHiddenCollectibleAddresses] = useState<ReadonlySet<string>>(
    () => new Set(),
  )
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
    setCustomName(resolvedName || undefined)
  }, [resolvedName])

  useEffect(() => {
    if (!isEditing) return
    editInputRef.current?.focus()
    editInputRef.current?.select()
  }, [isEditing])

  const displayName = customName || domain
  const {displayNameText, groups: nameDetailGroups} = getAccountNameDetails({
    displayName,
    domain,
    domains,
    customName: nameSources.customName,
    registryName: nameSources.registryName,
    tonDnsName: nameSources.tonDnsName,
  })
  const hasNameDetails = nameDetailGroups.length > 0

  const handleStartEdit = () => {
    setEditValue(displayName || "")
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
  const actonscanUrl = showActonscanLink ? getActonscanUrl(displayAddress, network.id) : undefined
  const unfreezerUrl =
    state?.status === "frozen" ? getUnfreezerUrl(bounceableAddress, network.id) : undefined
  const isNameUnchanged = editValue.trim() === (displayName || "")
  const stateLoading = accountLoading
  const showContractType = stateLoading || state?.status === "active"
  const firstWallet = jettonWallets[0]
  const canOpenTokens = Boolean(onMoreAssetsClick)
  const canOpenCollectibles = Boolean(onCollectiblesClick)
  const showCollectiblesRow = collectiblesLoading || collectiblesCount > 0
  const visibleCollectibles = collectiblePreviews
    .filter(item => !hiddenCollectibleAddresses.has(item.address))
    .slice(0, 8)
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

  const qrCode = stateLoading ? (
    <div className={`${styles.skeleton} ${styles.qrSkeleton}`} aria-hidden="true" />
  ) : (
    <AddressQrCode value={qrAddress} />
  )
  const addressFormats = (
    <div className={styles.addressFormats}>
      <div className={styles.addressFormatRow}>
        <span className={styles.addressFormatLabel}>Bounceable</span>
        <div className={styles.addressFormatValueRow}>
          <code className={styles.addressFormatValue}>{bounceableAddress}</code>
          <CopyInlineAction
            size="compact"
            value={bounceableAddress}
            label="Copy bounceable address"
            copiedLabel="Bounceable address copied"
          />
        </div>
      </div>
      <div className={styles.addressFormatRow}>
        <span className={styles.addressFormatLabel}>Non-bounceable</span>
        <div className={styles.addressFormatValueRow}>
          <code className={styles.addressFormatValue}>{nonBounceableAddress}</code>
          <CopyInlineAction
            size="compact"
            value={nonBounceableAddress}
            label="Copy non-bounceable address"
            copiedLabel="Non-bounceable address copied"
          />
        </div>
      </div>
      <div className={styles.addressFormatRow}>
        <span className={styles.addressFormatLabel}>Raw</span>
        <div className={styles.addressFormatValueRow}>
          <code className={styles.addressFormatValue}>{rawAddress}</code>
          <CopyInlineAction
            size="compact"
            value={rawAddress}
            label="Copy raw address"
            copiedLabel="Raw address copied"
          />
        </div>
      </div>
    </div>
  )
  const nameDetailsContent = (
    <div className={styles.addressFormats}>
      {nameDetailGroups.map(group => (
        <div key={group.key} className={styles.addressFormatRow}>
          <span className={styles.addressFormatLabel}>{group.label}</span>
          <div className={styles.nameDetailValues}>
            {group.values.map(value => (
              <div key={value.copyValue} className={styles.addressFormatValueRow}>
                <code className={styles.addressFormatValue}>{value.displayValue}</code>
                <CopyInlineAction
                  size="compact"
                  value={value.copyValue}
                  label={`Copy ${group.label} name`}
                  copiedLabel={`${group.label} name copied`}
                />
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
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
                    <Input
                      size="sm"
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
                    <Button
                      className={styles.renameAction}
                      disabled={isNameUnchanged}
                      loading={renameSaving}
                      onClick={() => {
                        void handleSave()
                      }}
                      size="sm"
                      variant="primary"
                    >
                      Save
                    </Button>
                    <Button
                      className={styles.renameAction}
                      onClick={() => setIsEditing(false)}
                      size="sm"
                      variant="outline"
                    >
                      Cancel
                    </Button>
                  </div>
                </div>
              </div>
            ) : displayName ? (
              <div className={styles.infoRow}>
                <div className={styles.label}>Name</div>
                <div className={styles.rowValue}>
                  {hasNameDetails ? (
                    <Popover
                      aria-label="Show name details"
                      ariaLabel="Name details"
                      className={styles.namePopover}
                      content={nameDetailsContent}
                      maxWidth="min(36rem, calc(100vw - 32px))"
                      openDelay={150}
                      placement="bottom"
                    >
                      <span className={`${styles.customName} ${styles.nameWithDetails}`}>
                        {displayNameText}
                      </span>
                    </Popover>
                  ) : (
                    <span className={styles.customName}>{displayNameText}</span>
                  )}
                  <InlineAction
                    className={styles.addressAction}
                    icon={<Edit2 />}
                    label="Rename address"
                    onClick={handleStartEdit}
                    size="compact"
                  />
                </div>
              </div>
            ) : undefined}

            <div className={`${styles.infoRow} ${styles.addressInfoRow}`}>
              <div className={styles.label}>Address</div>
              <div className={styles.rowValue}>
                <InlineActions
                  className={styles.addressActions}
                  visibility="always"
                  actions={
                    <>
                      <InlineAction
                        aria-pressed={favorite}
                        className={`${styles.addressAction} ${favorite ? styles.favoriteButtonActive : ""}`}
                        icon={<Star className={favorite ? styles.favoriteIconActive : undefined} />}
                        label={favorite ? "Remove from favorites" : "Add to favorites"}
                        onClick={handleToggleFavorite}
                        size="compact"
                      />
                      {!displayName && !isEditing && (
                        <InlineAction
                          className={styles.addressAction}
                          icon={<Edit2 />}
                          label="Rename address"
                          onClick={handleStartEdit}
                          size="compact"
                        />
                      )}
                      <CopyInlineAction
                        className={styles.addressAction}
                        copiedIcon={<Check className={styles.saveIcon} />}
                        copiedLabel="Address copied"
                        label="Copy address"
                        size="compact"
                        value={displayAddress}
                      />
                    </>
                  }
                >
                  <Popover
                    aria-label="Show address formats"
                    ariaLabel="Address formats"
                    className={styles.addressPopover}
                    content={addressFormats}
                    maxWidth="min(36rem, calc(100vw - 32px))"
                    openDelay={150}
                    placement="bottom"
                  >
                    <span className={styles.addressValue}>
                      <span className={styles.addressValueDesktop}>{addressRowText}</span>
                      <span className={styles.addressValueMobile}>{shortAddress}</span>
                    </span>
                  </Popover>
                </InlineActions>
              </div>
            </div>

            <div className={styles.infoRow}>
              <div className={styles.label}>Balance</div>
              <div className={styles.rowValue}>
                {stateLoading ? (
                  <div className={`${styles.skeleton} ${styles.skeletonValue}`} />
                ) : state ? (
                  <span className={styles.primaryValue}>
                    <GramAmount value={state.balance} useGrouping />
                  </span>
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

            {details.map(detail => (
              <div className={styles.infoRow} key={detail.key}>
                <div className={styles.label}>{detail.label}</div>
                <div className={styles.rowValue}>{detail.value}</div>
              </div>
            ))}

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
                          <TokenAmount
                            decimals={firstMaster?.jetton_content.decimals}
                            symbol={firstWalletSymbol}
                            tabIndex={-1}
                            useGrouping
                            value={firstWallet.balance}
                          />
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
                                <span key={item.address} className={styles.collectibleThumb}>
                                  <NftImage
                                    sources={item.imageSources ?? [item.image]}
                                    alt={item.name || "NFT"}
                                    className={styles.collectibleThumbImage}
                                    blurredClassName={styles.blurredImage}
                                    collectionName={item.collectionName}
                                    blurred={item.blurred}
                                    onNsfw={() => {
                                      setHiddenCollectibleAddresses(current =>
                                        new Set(current).add(item.address),
                                      )
                                    }}
                                  />
                                </span>
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

            {showContractType && (
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
                                          {humanizeIdentifier(link.kind)}
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
            )}
          </div>

          <div className={styles.statusBar}>
            {stateLoading ? (
              <div className={`${styles.skeleton} ${styles.statusSkeleton}`} />
            ) : (
              <span className={`${styles.status} ${styles[statusInfo.className]}`}>
                {statusInfo.label}
              </span>
            )}
            <Tooltip
              content={
                <span className={styles.addressFormatValueRow}>
                  <code className={styles.addressFormatValue}>{rawAddress}</code>
                  <CopyInlineAction
                    copiedLabel="Raw address copied"
                    label="Copy raw address"
                    size="compact"
                    value={rawAddress}
                  />
                </span>
              }
              width="extra-wide"
            >
              <span className={styles.statusAddress}>{statusAddress}</span>
            </Tooltip>
            {actonscanUrl && (
              <a
                className={styles.externalLink}
                href={actonscanUrl}
                target="_blank"
                rel="noreferrer"
              >
                actonscan.com
              </a>
            )}
            {tonscanUrl && (
              <a className={styles.externalLink} href={tonscanUrl} target="_blank" rel="noreferrer">
                tonscan.org
              </a>
            )}
            {unfreezerUrl && (
              <a
                className={styles.externalLink}
                href={unfreezerUrl}
                target="_blank"
                rel="noreferrer"
              >
                unfreezer.ton.org
              </a>
            )}
          </div>
        </div>

        {!stateLoading && (
          <Tooltip content="Show QR code">
            <span className={styles.tooltipTrigger}>
              <Popover
                key={rawAddress}
                ariaLabel="Address QR code"
                content={qrCode}
                interaction="click"
                placement="bottom"
                triggerAsChild
              >
                <button type="button" className={styles.qrToggle} aria-label="Show QR code">
                  <QrCode size={16} />
                </button>
              </Popover>
            </span>
          </Tooltip>
        )}

        <div className={styles.qrPanel} aria-label="Address QR code" aria-busy={stateLoading}>
          {qrCode}
        </div>
      </div>
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
  const seen = new Set<string>()
  const uniqueLabels = labels.filter(label => {
    const key = contractTypeLabelKey(label)
    if (seen.has(key)) {
      return false
    }
    seen.add(key)
    return true
  })
  return uniqueLabels.length > 0 ? uniqueLabels : ["Unknown"]
}

function contractTypeLabelKey(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]/g, "")
    .replace(/interface$/, "")
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
    case "nft_item":
    case "nft_item_simple": {
      return "NFT item interface"
    }
    case "nft_collection": {
      return "NFT collection interface"
    }
    case "multisig_v2": {
      return "Multisig wallet v2"
    }
    case "multisig_order_v2": {
      return "Multisig order v2"
    }
    default: {
      return humanizeIdentifier(normalizedInterface)
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

  return {
    title: title || url,
    url,
    kind: kind || "link",
  }
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

function getActonscanUrl(address: string, networkId: ExplorerNetworkId): string | undefined {
  if (networkId !== "mainnet" && networkId !== "testnet") {
    return undefined
  }

  const encodedAddress = encodeURIComponent(address)
  return `https://actonscan.com/address/${encodedAddress}?network=${networkId}`
}

function getUnfreezerUrl(address: string, networkId: ExplorerNetworkId): string | undefined {
  if (networkId !== "mainnet" && networkId !== "testnet") {
    return undefined
  }

  const parameters = new URLSearchParams({address})
  if (networkId === "testnet") {
    parameters.set("testnet", "true")
  }
  return `https://unfreezer.ton.org/?${parameters.toString()}`
}

function normalizeForkNetwork(forkNetwork?: string): "mainnet" | "testnet" | undefined {
  const normalizedFork = forkNetwork?.trim().toLowerCase()
  if (normalizedFork === "mainnet" || normalizedFork === "testnet") {
    return normalizedFork
  }
  return undefined
}

function formatCollectibleCount(count: number): string {
  return formatCountLabel(count, {singular: "NFT"})
}

function formatRawAddress(address: string): string {
  const [workchain, hash] = toRawAddress(address).trim().split(":")
  if (!workchain || !hash) {
    return address
  }
  return `${workchain}:${shortenMiddle(hash, {start: 3, end: 5})}`
}
