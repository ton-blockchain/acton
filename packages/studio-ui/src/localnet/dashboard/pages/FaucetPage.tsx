import {ArrowUpRight, Check, ChevronDown, Coins, Loader2} from "lucide-react"
import {Button, Dialog, Input, parseGramAmount, parseTokenAmount, useToast} from "@acton/ui"
import {TonAddressInput, type TonAddressSuggestion} from "@acton/transaction-ui"
import type {FC, FormEvent, ReactNode} from "react"
import {useCallback, useEffect, useMemo, useRef, useState} from "react"
import {useSearchParams} from "react-router"

import type {JettonMaster} from "@acton/explorer-core/api/types"
import type {TonClient} from "@acton/explorer-core/api/client"
import {waitForTraceTransactionHash} from "@acton/explorer-core/api/waitForTraceTransactionHash"
import {
  formatAddress,
  hashToHex,
  isSameAddress,
  parseAddress,
} from "@acton/explorer-core/components/utils"
import {useAddressFormat} from "@acton/explorer-core/hooks/useNetworkInfo"
import {useExplorerRoutePaths} from "@acton/explorer-core/hooks/useExplorerRoutePaths"
import {useOptionalWalletRuntime} from "../../wallet/useWalletRuntime"
import usdtLogo from "../../../assets/usdt-logo.png"

import styles from "../DashboardPage.module.css"
import {TOKEN_PLACEHOLDER_IMAGE} from "@acton/explorer-core/components/imageFallbacks"

interface FaucetPageProps {
  readonly client: TonClient
  readonly gramFaucetEnabled: boolean
  readonly jettonFaucetEnabled: boolean
  readonly projectWalletsEnabled: boolean
}

type FaucetMode = "ton" | "jetton"

const GRAM_LOGO_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80" fill="none" viewBox="0 0 80 80"><path fill="#30A1F5" d="M52.017 12.097H27.984c-3.201 0-4.802 0-6.25.448a10 10 0 0 0-3.496 1.909c-1.159.975-2.024 2.322-3.755 5.014l-7.64 11.884c-1.144 1.78-1.716 2.668-1.87 3.605a4.6 4.6 0 0 0 .263 2.45c.35.882 1.098 1.63 2.593 3.125L36.217 68.92c1.325 1.324 1.987 1.986 2.75 2.234a3.34 3.34 0 0 0 2.067 0c.763-.248 1.425-.91 2.75-2.234l28.388-28.388c1.495-1.495 2.243-2.243 2.593-3.125.31-.778.4-1.625.263-2.45-.155-.937-.727-1.826-1.87-3.605l-7.64-11.884c-1.73-2.692-2.596-4.039-3.756-5.014a10 10 0 0 0-3.496-1.91c-1.448-.447-3.048-.447-6.249-.447"/><path fill="#fff" d="M47.465 21.472c.39-1.055 1.883-1.055 2.274 0l2.698 7.292a1.6 1.6 0 0 0 .945.946l7.293 2.698c1.055.39 1.055 1.883 0 2.274l-7.293 2.698a1.6 1.6 0 0 0-.945.945l-2.698 7.293c-.39 1.055-1.883 1.055-2.274 0l-2.698-7.293a1.6 1.6 0 0 0-.946-.945l-7.292-2.698c-1.055-.39-1.055-1.883 0-2.274l7.292-2.698a1.6 1.6 0 0 0 .946-.946z"/></svg>'
const GRAM_LOGO_IMAGE = `data:image/svg+xml,${encodeURIComponent(GRAM_LOGO_SVG)}`
const PINNED_USDT_MINTER_ADDRESS = "EQCxE6mUtQJKFnGfaROTKOt1lZbDiiX1kCixRv7Nw2Id_sDs"
const TOKEN_MINTER_NOT_FOUND_MESSAGE = "This address is not a token minter."
const TOKEN_MINTER_NOT_MINTABLE_MESSAGE = "This token cannot be minted by the faucet."
const FAUCET_TRACE_WAIT_ATTEMPTS = 60

interface FaucetOption {
  readonly id: string
  readonly title: string
  readonly subtitle: string
  readonly value: string
  readonly badge?: string
  readonly image?: string
  readonly fallbackInitial?: string
}

export const FaucetPage: FC<FaucetPageProps> = ({
  client,
  gramFaucetEnabled,
  jettonFaucetEnabled,
  projectWalletsEnabled,
}) => {
  const {dismissToast, showToast, updateToast} = useToast()
  const walletRuntime = useOptionalWalletRuntime()
  const addressFormat = useAddressFormat()
  const routes = useExplorerRoutePaths()
  const [searchParams] = useSearchParams()
  const requestedJettonMinter = jettonFaucetEnabled
    ? (searchParams.get("jetton")?.trim() ?? "")
    : ""
  const [mode, setMode] = useState<FaucetMode>(() => (gramFaucetEnabled ? "ton" : "jetton"))
  const [address, setAddress] = useState("")
  const [jettonMinter, setJettonMinter] = useState("")
  const [amount, setAmount] = useState("1")
  const [jettonMasters, setJettonMasters] = useState<JettonMaster[]>([])
  const [jettonsLoading, setJettonsLoading] = useState(jettonFaucetEnabled)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [isAssetModalOpen, setIsAssetModalOpen] = useState(false)
  const [minterAddressDraft, setMinterAddressDraft] = useState("")
  const minterInputRef = useRef<HTMLInputElement>(null)
  const lastAutoMinterLookupAddressRef = useRef<string | undefined>(undefined)
  const minterLookupSequenceRef = useRef(0)
  const minterLookupToastRef = useRef<string | undefined>(undefined)
  const loadedJettonMinterQueryRef = useRef<string | undefined>(undefined)
  const projectWallets = walletRuntime?.projectWallets ?? []
  const isJettonMode = mode === "jetton"
  const canChooseAsset = jettonFaucetEnabled
  const isSubmitDisabled =
    isSubmitting ||
    address.trim().length === 0 ||
    amount.trim().length === 0 ||
    (isJettonMode && jettonMinter.trim().length === 0)

  const selectMode = useCallback((nextMode: FaucetMode) => {
    setMode(nextMode)
  }, [])

  const openAssetModal = useCallback(() => {
    if (!canChooseAsset) return
    setMinterAddressDraft("")
    lastAutoMinterLookupAddressRef.current = undefined
    setIsAssetModalOpen(true)
  }, [canChooseAsset])

  const selectGramAsset = useCallback(() => {
    if (!gramFaucetEnabled) return
    selectMode("ton")
    setIsAssetModalOpen(false)
  }, [gramFaucetEnabled, selectMode])

  const selectJettonAsset = useCallback(
    (option: FaucetOption) => {
      if (!jettonFaucetEnabled) return
      setJettonMinter(option.value)
      selectMode("jetton")
      setIsAssetModalOpen(false)
    },
    [jettonFaucetEnabled, selectMode],
  )

  useEffect(() => {
    if (!jettonFaucetEnabled) {
      setIsAssetModalOpen(false)
      setJettonMinter("")
      setMinterAddressDraft("")
    }

    if (mode === "jetton" && !jettonFaucetEnabled && gramFaucetEnabled) {
      setMode("ton")
    } else if (mode === "ton" && !gramFaucetEnabled && jettonFaucetEnabled) {
      setMode("jetton")
    }
  }, [gramFaucetEnabled, jettonFaucetEnabled, mode])

  useEffect(() => {
    if (!isAssetModalOpen || !jettonFaucetEnabled) {
      return
    }

    const frame = globalThis.requestAnimationFrame(() => {
      minterInputRef.current?.focus()
    })
    return () => {
      globalThis.cancelAnimationFrame(frame)
    }
  }, [isAssetModalOpen, jettonFaucetEnabled])

  useEffect(() => {
    if (!isAssetModalOpen || !jettonFaucetEnabled) {
      return
    }

    const parsedMinter = parseAddress(minterAddressDraft.trim())
    if (!parsedMinter) {
      return
    }

    const normalizedMinter = parsedMinter.toString(addressFormat)
    if (lastAutoMinterLookupAddressRef.current === normalizedMinter) {
      return
    }

    const timeoutId = globalThis.setTimeout(() => {
      if (lastAutoMinterLookupAddressRef.current === normalizedMinter) {
        return
      }

      lastAutoMinterLookupAddressRef.current = normalizedMinter
      void loadMinterAddress(normalizedMinter)
    }, 250)

    return () => {
      globalThis.clearTimeout(timeoutId)
    }
  }, [addressFormat, isAssetModalOpen, jettonFaucetEnabled, minterAddressDraft])

  useEffect(() => {
    if (!jettonFaucetEnabled) {
      setJettonMasters([])
      setJettonsLoading(false)
      return
    }

    let cancelled = false

    void (async () => {
      setJettonsLoading(true)

      try {
        const masters = await client.getJettonMasters(undefined, 100, 0)
        if (!cancelled) {
          setJettonMasters(masters)
        }
      } catch (error) {
        if (!cancelled) {
          setJettonMasters([])
          console.error(error instanceof Error ? error.message : "Failed to load jettons")
        }
      } finally {
        if (!cancelled) {
          setJettonsLoading(false)
        }
      }
    })()

    return () => {
      cancelled = true
    }
  }, [client, jettonFaucetEnabled])

  const walletSuggestions = useMemo<TonAddressSuggestion[]>(
    () =>
      projectWallets.map(wallet => {
        const value = parseAddress(wallet.address)?.toString(addressFormat) ?? wallet.address
        return {
          address: value,
          label: wallet.name,
          description: `${wallet.version} · ${formatAddress(value, true, addressFormat)}`,
        }
      }),
    [addressFormat, projectWallets],
  )
  const jettonOptions = useMemo<FaucetOption[]>(() => {
    if (!jettonFaucetEnabled) return []

    const usdtValue =
      parseAddress(PINNED_USDT_MINTER_ADDRESS)?.toString(addressFormat) ??
      PINNED_USDT_MINTER_ADDRESS
    const pinnedUsdtOption: FaucetOption = {
      id: PINNED_USDT_MINTER_ADDRESS,
      title: "Tether USD",
      subtitle: formatAddress(usdtValue, true, addressFormat),
      value: usdtValue,
      badge: "USD₮",
      image: usdtLogo,
      fallbackInitial: "U",
    }
    const apiOptions = jettonMasters
      .filter(master => master.mintable)
      .filter(master => !isSameAddress(master.address, PINNED_USDT_MINTER_ADDRESS))
      .map(master => {
        const symbol = jettonSymbol(master)
        const value = parseAddress(master.address)?.toString(addressFormat) ?? master.address
        return {
          id: master.address,
          title: master.jetton_content.name || symbol,
          subtitle: formatAddress(value, true, addressFormat),
          value,
          badge: symbol,
          image:
            typeof master.jetton_content.image === "string" &&
            master.jetton_content.image.length > 0
              ? master.jetton_content.image
              : TOKEN_PLACEHOLDER_IMAGE,
          fallbackInitial: symbol.slice(0, 1).toUpperCase(),
        }
      })

    return [pinnedUsdtOption, ...apiOptions]
  }, [addressFormat, jettonFaucetEnabled, jettonMasters])
  const selectedJettonOption = useMemo(
    () => jettonOptions.find(option => isSameAddress(option.value, jettonMinter)),
    [jettonMinter, jettonOptions],
  )
  const selectedAssetSymbol = isJettonMode ? (selectedJettonOption?.badge ?? "JETTON") : "GRAM"
  const selectedAssetTitle = isJettonMode ? (selectedJettonOption?.title ?? "Jetton") : "GRAM"

  useEffect(() => {
    if (
      !jettonFaucetEnabled ||
      jettonsLoading ||
      requestedJettonMinter.length === 0 ||
      loadedJettonMinterQueryRef.current === requestedJettonMinter
    ) {
      return
    }

    loadedJettonMinterQueryRef.current = requestedJettonMinter
    void loadMinterAddress(requestedJettonMinter)
  }, [jettonFaucetEnabled, jettonsLoading, requestedJettonMinter])

  async function handleSubmit(event?: FormEvent): Promise<void> {
    event?.preventDefault()
    if ((isJettonMode && !jettonFaucetEnabled) || (!isJettonMode && !gramFaucetEnabled)) {
      return
    }

    const trimmedAddress = address.trim()
    const parsedAddress = parseAddress(trimmedAddress)
    const tonAmountNano = parseGramAmount(amount)
    if (!parsedAddress) {
      showToast({
        variant: "error",
        title: "Invalid address",
        description: "Enter a valid TON address.",
      })
      return
    }
    if (!isJettonMode && (tonAmountNano === undefined || tonAmountNano <= 0n)) {
      showToast({
        variant: "error",
        title: "Invalid amount",
        description: "Enter a valid amount greater than zero",
      })
      return
    }

    const normalized = parsedAddress.toString(addressFormat)
    setIsSubmitting(true)

    try {
      if (isJettonMode) {
        await mintJettons(normalized)
      } else {
        if (tonAmountNano === undefined) {
          return
        }
        await sendTons(normalized, tonAmountNano)
      }
    } finally {
      setIsSubmitting(false)
    }
  }

  async function sendTons(normalized: string, nanoAmount: bigint) {
    const recipient = formatAddress(normalized, true, addressFormat)
    const toastId = showToast({
      variant: "loading",
      title: "Sending transfer",
      description: `Sending ${amount.trim()} GRAM to ${recipient}.`,
      durationMs: 60_000,
    })

    try {
      const msgHash = await client.fundAccount(normalized, nanoAmount)
      await updateFaucetResultToast({
        toastId,
        title: "Transfer sent",
        description: (
          <>
            Sent {amount.trim()} GRAM to {recipient}.
          </>
        ),
        msgHash,
      })
    } catch (error) {
      updateToast(toastId, {
        variant: "error",
        title: "Transfer failed",
        description: error instanceof Error ? error.message : "Failed to send GRAM.",
        durationMs: 8000,
      })
    }
  }

  async function mintJettons(normalized: string) {
    const parsedMinter = parseAddress(jettonMinter.trim())
    if (!parsedMinter) {
      showToast({
        variant: "error",
        title: "Invalid minter",
        description: "Enter a valid jetton minter address.",
      })
      return
    }

    const normalizedMinter = parsedMinter.toString(addressFormat)
    const recipient = formatAddress(normalized, true, addressFormat)
    const toastId = showToast({
      variant: "loading",
      title: "Sending mint",
      description: `Preparing ${amount.trim()} ${selectedAssetSymbol} for ${recipient}.`,
      durationMs: 60_000,
    })

    try {
      const master = jettonMasters.find(item => isSameAddress(item.address, normalizedMinter))
      const parsedAmount = parseTokenAmount(amount, master?.jetton_content.decimals)
      if (parsedAmount === undefined || parsedAmount <= 0n) {
        updateToast(toastId, {
          variant: "error",
          title: "Invalid amount",
          description: "Enter a valid positive amount for this token",
          durationMs: 8000,
        })
        return
      }

      const symbol = master ? jettonSymbol(master) : selectedAssetSymbol
      const msgHash = await client.fundJetton(normalized, normalizedMinter, amount.trim())
      await updateFaucetResultToast({
        toastId,
        title: "Mint sent",
        description: (
          <>
            Minted {amount.trim()} {symbol} to {recipient}.
          </>
        ),
        msgHash,
      })
    } catch (error) {
      updateToast(toastId, {
        variant: "error",
        title: "Mint failed",
        description: error instanceof Error ? error.message : "Failed to mint jettons.",
        durationMs: 8000,
      })
    }
  }

  async function updateFaucetResultToast({
    toastId,
    title,
    description,
    msgHash,
  }: {
    readonly toastId: string
    readonly title: string
    readonly description: ReactNode
    readonly msgHash: string
  }) {
    updateToast(toastId, {
      variant: "loading",
      title: "Waiting for transaction",
      description: "Message accepted. Resolving the trace link.",
      durationMs: 60_000,
    })

    const rawTxHash = await waitForTraceTransactionHash(client, msgHash, FAUCET_TRACE_WAIT_ATTEMPTS)
    const txHash = hashToHex(rawTxHash) ?? rawTxHash

    updateToast(toastId, {
      variant: "success",
      title,
      description: (
        <span>
          {description}
          {txHash ? (
            <>
              <br />
              <br />
              <a href={routes.transactionPath(txHash)}>View transaction</a>
            </>
          ) : undefined}
        </span>
      ),
      durationMs: 8000,
    })
  }

  async function loadMinterAddress(rawMinter: string): Promise<void> {
    if (!jettonFaucetEnabled) return

    const parsedMinter = parseAddress(rawMinter.trim())
    if (!parsedMinter) {
      showToast({
        variant: "error",
        title: "Token not loaded",
        description: "Enter a valid token minter address.",
      })
      return
    }

    const normalizedMinter = parsedMinter.toString(addressFormat)
    const lookupSequence = minterLookupSequenceRef.current + 1
    minterLookupSequenceRef.current = lookupSequence
    const cachedOption = jettonOptions.find(option => isSameAddress(option.value, normalizedMinter))
    if (cachedOption) {
      setJettonMinter(cachedOption.value)
      lastAutoMinterLookupAddressRef.current = undefined
      selectMode("jetton")
      return
    }

    lastAutoMinterLookupAddressRef.current = normalizedMinter
    if (minterLookupToastRef.current) {
      dismissToast(minterLookupToastRef.current)
    }
    minterLookupToastRef.current = showToast({
      variant: "info",
      title: "Loading token",
      description: `Checking ${formatAddress(normalizedMinter, true, addressFormat)}.`,
      durationMs: 60_000,
    })

    try {
      const [master] = await client.getJettonMasters([normalizedMinter])
      if (minterLookupSequenceRef.current !== lookupSequence) {
        return
      }
      if (!master) {
        throw new Error(TOKEN_MINTER_NOT_FOUND_MESSAGE)
      }
      if (!master.mintable) {
        throw new Error(TOKEN_MINTER_NOT_MINTABLE_MESSAGE)
      }

      setJettonMasters(current => {
        const exists = current.some(item => isSameAddress(item.address, master.address))
        if (!exists) {
          return [master, ...current]
        }
        return current.map(item => (isSameAddress(item.address, master.address) ? master : item))
      })
      setJettonMinter(normalizedMinter)
      lastAutoMinterLookupAddressRef.current = undefined
      selectMode("jetton")
      showToast({
        variant: "success",
        title: "Token loaded",
        description: `${jettonSymbol(master)} is ready in the faucet.`,
      })
    } catch (error) {
      if (minterLookupSequenceRef.current !== lookupSequence) {
        return
      }
      const description = error instanceof Error ? error.message : TOKEN_MINTER_NOT_FOUND_MESSAGE
      showToast({
        variant: "error",
        title: "Token not loaded",
        description,
      })
    } finally {
      if (minterLookupSequenceRef.current === lookupSequence && minterLookupToastRef.current) {
        dismissToast(minterLookupToastRef.current)
        minterLookupToastRef.current = undefined
      }
    }
  }

  const symbolHint = isJettonMode ? (selectedJettonOption?.badge ?? "jettons") : "GRAM"

  function jettonSymbol(master: JettonMaster): string {
    const symbol = master.jetton_content.symbol
    return typeof symbol === "string" && symbol.trim().length > 0 ? symbol.trim() : "jettons"
  }

  return (
    <>
      <section className={styles.faucetLayout}>
        <form className={styles.formCard} onSubmit={event => void handleSubmit(event)}>
          <div className={styles.fieldBlock}>
            <label className={styles.label} htmlFor="dashboard-amount">
              Amount
            </label>
            <div className={styles.amountAssetField}>
              <Input
                id="dashboard-amount"
                aria-label="Amount"
                className={`${styles.fieldInput} ${styles.amountAssetInput} ${
                  canChooseAsset ? "" : styles.amountAssetInputStatic
                }`}
                inputMode="decimal"
                placeholder={isJettonMode ? "0.0" : "0.0 GRAM"}
                value={amount}
                autoComplete="off"
                autoCorrect="off"
                spellCheck={false}
                onChange={event => setAmount(event.target.value)}
              />
              {canChooseAsset ? (
                <button
                  type="button"
                  className={styles.assetSelectorButton}
                  aria-label="Choose faucet asset"
                  aria-haspopup="dialog"
                  aria-expanded={isAssetModalOpen}
                  onClick={openAssetModal}
                >
                  <span className={styles.assetSelectorIcon}>
                    {isJettonMode && selectedJettonOption?.image ? (
                      <img
                        src={selectedJettonOption.image}
                        alt=""
                        onError={event => {
                          const imageElement = event.currentTarget
                          if (imageElement.getAttribute("src") !== TOKEN_PLACEHOLDER_IMAGE) {
                            imageElement.src = TOKEN_PLACEHOLDER_IMAGE
                          }
                        }}
                      />
                    ) : isJettonMode ? (
                      <Coins size={17} />
                    ) : (
                      <img src={GRAM_LOGO_IMAGE} alt="" />
                    )}
                  </span>
                  <span className={styles.assetSelectorText}>
                    <span className={styles.assetSelectorSymbol}>{selectedAssetSymbol}</span>
                    {isJettonMode && selectedAssetTitle !== selectedAssetSymbol && (
                      <span className={styles.assetSelectorName}>{selectedAssetTitle}</span>
                    )}
                  </span>
                  <ChevronDown size={16} aria-hidden="true" />
                </button>
              ) : (
                <div className={styles.assetSelectorStatic} aria-label="Available asset: GRAM">
                  <span className={styles.assetSelectorIcon}>
                    <img src={GRAM_LOGO_IMAGE} alt="" />
                  </span>
                  <span className={styles.assetSelectorText}>
                    <span className={styles.assetSelectorSymbol}>GRAM</span>
                  </span>
                </div>
              )}
            </div>
          </div>

          <div className={styles.fieldBlock}>
            <TonAddressInput
              ariaLabel="Recipient"
              className={styles.fieldInput}
              label="Recipient"
              placeholder="EQ..."
              suggestions={projectWalletsEnabled ? walletSuggestions : []}
              value={address}
              onValueChange={setAddress}
            />
          </div>

          <div className={styles.quickActions}>
            {["1", "5", "20", "100"].map(value => (
              <Button
                key={value}
                type="button"
                variant={amount === value ? "secondary" : "outline"}
                size="sm"
                className={styles.quickActionButton}
                onClick={() => setAmount(value)}
              >
                {value} {symbolHint}
              </Button>
            ))}
          </div>

          <div className={styles.formFooter}>
            <div />
            <Button
              type="submit"
              variant="primary"
              className={styles.sendButton}
              trailingIcon={<ArrowUpRight size={16} />}
              disabled={isSubmitDisabled}
            >
              {isSubmitting ? "Sending..." : isJettonMode ? "Mint Jetton" : "Send GRAM"}
            </Button>
          </div>
        </form>
      </section>

      {jettonFaucetEnabled ? (
        <Dialog
          open={isAssetModalOpen}
          title="Asset"
          className={styles.dashboardDialog}
          maxWidth={560}
          closeLabel="Close asset selector"
          onOpenChange={setIsAssetModalOpen}
        >
          <div className={styles.assetModalContent}>
            <div className={styles.assetChoiceList}>
              {gramFaucetEnabled ? (
                <button
                  type="button"
                  className={`${styles.assetChoiceButton} ${isJettonMode ? "" : styles.assetChoiceButtonSelected}`}
                  onClick={selectGramAsset}
                >
                  <img src={GRAM_LOGO_IMAGE} alt="" className={styles.assetChoiceImage} />
                  <span className={styles.assetChoiceText}>
                    <span className={styles.assetChoiceTitle}>GRAM</span>
                    <span className={styles.assetChoiceSubtitle}>Native network balance</span>
                  </span>
                  {!isJettonMode && <Check size={17} className={styles.assetChoiceCheck} />}
                </button>
              ) : undefined}

              {jettonOptions.map(option => {
                const isSelected = isJettonMode && isSameAddress(option.value, jettonMinter)
                return (
                  <button
                    key={option.id}
                    type="button"
                    className={`${styles.assetChoiceButton} ${isSelected ? styles.assetChoiceButtonSelected : ""}`}
                    onClick={() => selectJettonAsset(option)}
                  >
                    {option.image ? (
                      <img
                        src={option.image}
                        alt=""
                        className={styles.assetChoiceImage}
                        onError={event => {
                          const imageElement = event.currentTarget
                          if (imageElement.getAttribute("src") !== TOKEN_PLACEHOLDER_IMAGE) {
                            imageElement.src = TOKEN_PLACEHOLDER_IMAGE
                          }
                        }}
                      />
                    ) : (
                      <span className={styles.assetChoiceIcon}>
                        <Coins size={18} />
                      </span>
                    )}
                    <span className={styles.assetChoiceText}>
                      <span className={styles.assetChoiceTitle}>{option.title}</span>
                      <span className={styles.assetChoiceSubtitle}>{option.subtitle}</span>
                    </span>
                    {option.badge && (
                      <span className={styles.assetChoiceBadge}>{option.badge}</span>
                    )}
                    {isSelected && <Check size={17} className={styles.assetChoiceCheck} />}
                  </button>
                )
              })}
              {jettonsLoading && (
                <div className={styles.assetLookupStatus}>
                  <Loader2 size={14} className={styles.spinning} />
                  Loading jettons...
                </div>
              )}
            </div>

            <div className={styles.assetMinterLookup}>
              <label className={styles.label} htmlFor="dashboard-asset-minter">
                Paste token minter address
              </label>
              <Input
                ref={minterInputRef}
                id="dashboard-asset-minter"
                className={styles.fieldInput}
                placeholder="EQ..."
                value={minterAddressDraft}
                autoComplete="off"
                autoCorrect="off"
                spellCheck={false}
                onChange={event => {
                  setMinterAddressDraft(event.target.value)
                }}
                onPaste={event => {
                  const pastedText = event.clipboardData.getData("text")
                  const parsedMinter = parseAddress(pastedText.trim())
                  if (!parsedMinter) {
                    return
                  }

                  event.preventDefault()
                  const normalizedMinter = parsedMinter.toString(addressFormat)
                  lastAutoMinterLookupAddressRef.current = normalizedMinter
                  setMinterAddressDraft(normalizedMinter)
                  void loadMinterAddress(normalizedMinter)
                }}
              />
            </div>
          </div>
        </Dialog>
      ) : undefined}
    </>
  )
}
