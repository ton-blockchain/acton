import {ArrowUpRight, Check, ChevronDown, Coins, Loader2, X} from "lucide-react"
import {Button, Input, useToast} from "@acton/shared-ui"
import type {Address} from "@ton/core"
import {useCallback, useEffect, useId, useMemo, useRef, useState} from "react"
import type {FC, FormEvent, ReactNode} from "react"

import type {JettonMaster, StartupWallet} from "../../explorer/api/types"
import type {TonClient} from "../../explorer/api/client"
import {
  formatAddress,
  hashToHex,
  isSameAddress,
  parseAddress,
} from "../../explorer/components/utils"
import {useAddressFormat} from "../../explorer/hooks/useNetworkInfo"
import {QUICK_AMOUNTS, TOKEN_PLACEHOLDER_IMAGE} from "../constants"
import {parseGramAmount} from "../dashboardUtils"
import {
  buildJettonMintInternalMessageBoc,
  normalizeJettonDecimals,
  parseJettonAmount,
} from "../jettonFaucet"
import usdtLogo from "../assets/usdt-logo.png"

import styles from "../DashboardPage.module.css"

interface FaucetPageProps {
  readonly client: TonClient
}

type FaucetMode = "ton" | "jetton"

const GRAM_LOGO_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80" fill="none" viewBox="0 0 80 80"><path fill="#30A1F5" d="M52.017 12.097H27.984c-3.201 0-4.802 0-6.25.448a10 10 0 0 0-3.496 1.909c-1.159.975-2.024 2.322-3.755 5.014l-7.64 11.884c-1.144 1.78-1.716 2.668-1.87 3.605a4.6 4.6 0 0 0 .263 2.45c.35.882 1.098 1.63 2.593 3.125L36.217 68.92c1.325 1.324 1.987 1.986 2.75 2.234a3.34 3.34 0 0 0 2.067 0c.763-.248 1.425-.91 2.75-2.234l28.388-28.388c1.495-1.495 2.243-2.243 2.593-3.125.31-.778.4-1.625.263-2.45-.155-.937-.727-1.826-1.87-3.605l-7.64-11.884c-1.73-2.692-2.596-4.039-3.756-5.014a10 10 0 0 0-3.496-1.91c-1.448-.447-3.048-.447-6.249-.447"/><path fill="#fff" d="M47.465 21.472c.39-1.055 1.883-1.055 2.274 0l2.698 7.292a1.6 1.6 0 0 0 .945.946l7.293 2.698c1.055.39 1.055 1.883 0 2.274l-7.293 2.698a1.6 1.6 0 0 0-.945.945l-2.698 7.293c-.39 1.055-1.883 1.055-2.274 0l-2.698-7.293a1.6 1.6 0 0 0-.946-.945l-7.292-2.698c-1.055-.39-1.055-1.883 0-2.274l7.292-2.698a1.6 1.6 0 0 0 .946-.946z"/></svg>'
const GRAM_LOGO_IMAGE = `data:image/svg+xml,${encodeURIComponent(GRAM_LOGO_SVG)}`
const PINNED_USDT_MINTER_ADDRESS = "EQCxE6mUtQJKFnGfaROTKOt1lZbDiiX1kCixRv7Nw2Id_sDs"
const TOKEN_MINTER_NOT_FOUND_MESSAGE = "This address is not a token minter."
const TOKEN_MINTER_NOT_MINTABLE_MESSAGE = "This token cannot be minted by the faucet."

interface FaucetOption {
  readonly id: string
  readonly title: string
  readonly subtitle: string
  readonly value: string
  readonly badge?: string
  readonly image?: string
  readonly fallbackInitial?: string
}

export const FaucetPage: FC<FaucetPageProps> = ({client}) => {
  const {dismissToast, showToast} = useToast()
  const addressFormat = useAddressFormat()
  const [mode, setMode] = useState<FaucetMode>("ton")
  const [address, setAddress] = useState("")
  const [jettonMinter, setJettonMinter] = useState("")
  const [amount, setAmount] = useState("1")
  const [startupWallets, setStartupWallets] = useState<StartupWallet[]>([])
  const [jettonMasters, setJettonMasters] = useState<JettonMaster[]>([])
  const [walletsLoading, setWalletsLoading] = useState(true)
  const [jettonsLoading, setJettonsLoading] = useState(true)
  const [walletsError, setWalletsError] = useState<string>()
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [isAssetModalOpen, setIsAssetModalOpen] = useState(false)
  const [minterAddressDraft, setMinterAddressDraft] = useState("")
  const minterInputRef = useRef<HTMLInputElement>(null)
  const lastAutoMinterLookupAddressRef = useRef<string | undefined>(undefined)
  const minterLookupSequenceRef = useRef(0)
  const minterLookupToastRef = useRef<string | undefined>(undefined)
  const amountNano = useMemo(() => parseGramAmount(amount), [amount])
  const isJettonMode = mode === "jetton"
  const isSubmitDisabled =
    isSubmitting ||
    address.trim().length === 0 ||
    amount.trim().length === 0 ||
    (isJettonMode && jettonMinter.trim().length === 0)

  const selectMode = useCallback((nextMode: FaucetMode) => {
    setMode(nextMode)
  }, [])

  const openAssetModal = useCallback(() => {
    setMinterAddressDraft("")
    lastAutoMinterLookupAddressRef.current = undefined
    setIsAssetModalOpen(true)
  }, [])

  const selectGramAsset = useCallback(() => {
    selectMode("ton")
    setIsAssetModalOpen(false)
  }, [selectMode])

  const selectJettonAsset = useCallback(
    (option: FaucetOption) => {
      setJettonMinter(option.value)
      selectMode("jetton")
      setIsAssetModalOpen(false)
    },
    [selectMode],
  )

  useEffect(() => {
    if (!isAssetModalOpen) {
      return
    }

    const frame = globalThis.requestAnimationFrame(() => {
      minterInputRef.current?.focus()
    })
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsAssetModalOpen(false)
      }
    }

    globalThis.addEventListener("keydown", onKeyDown)
    return () => {
      globalThis.cancelAnimationFrame(frame)
      globalThis.removeEventListener("keydown", onKeyDown)
    }
  }, [isAssetModalOpen])

  useEffect(() => {
    if (!isAssetModalOpen) {
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
  }, [addressFormat, isAssetModalOpen, minterAddressDraft])

  useEffect(() => {
    let cancelled = false

    void (async () => {
      setWalletsLoading(true)
      setWalletsError(undefined)

      try {
        const wallets = await client.getStartupWallets()
        if (!cancelled) {
          setStartupWallets(wallets)
        }
      } catch (error) {
        if (!cancelled) {
          setStartupWallets([])
          setWalletsError(error instanceof Error ? error.message : "Failed to load wallets")
        }
      } finally {
        if (!cancelled) {
          setWalletsLoading(false)
        }
      }
    })()

    return () => {
      cancelled = true
    }
  }, [client])

  useEffect(() => {
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
  }, [client])

  const walletOptions = useMemo<FaucetOption[]>(
    () =>
      startupWallets.map(wallet => {
        const value = parseAddress(wallet.address)?.toString(addressFormat) ?? wallet.address
        return {
          id: wallet.address,
          title: wallet.name,
          subtitle: `${wallet.version} · ${formatAddress(value, true, addressFormat)}`,
          value,
          badge: wallet.name,
          fallbackInitial: wallet.name.slice(0, 1).toUpperCase(),
        }
      }),
    [addressFormat, startupWallets],
  )
  const selectedWalletOption = useMemo(
    () => walletOptions.find(option => isSameAddress(option.value, address)),
    [address, walletOptions],
  )
  const jettonOptions = useMemo<FaucetOption[]>(() => {
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
  }, [addressFormat, jettonMasters])
  const selectedJettonOption = useMemo(
    () => jettonOptions.find(option => isSameAddress(option.value, jettonMinter)),
    [jettonMinter, jettonOptions],
  )
  const selectedAssetSymbol = isJettonMode ? (selectedJettonOption?.badge ?? "JETTON") : "GRAM"
  const selectedAssetTitle = isJettonMode ? (selectedJettonOption?.title ?? "Jetton") : "GRAM"

  async function handleSubmit(event?: FormEvent): Promise<void> {
    event?.preventDefault()
    const trimmedAddress = address.trim()
    const parsedAddress = parseAddress(trimmedAddress)
    const tonAmountNano = amountNano
    if (!parsedAddress) {
      showToast({
        variant: "error",
        title: "Invalid address",
        description: "Enter a valid TON address.",
      })
      return
    }
    if (!isJettonMode && tonAmountNano === undefined) {
      showToast({
        variant: "error",
        title: "Invalid amount",
        description: "Enter a valid amount greater than zero.",
      })
      return
    }

    const normalized = parsedAddress.toString(addressFormat)
    setIsSubmitting(true)

    try {
      if (isJettonMode) {
        await mintJettons(parsedAddress, normalized)
      } else {
        if (tonAmountNano === undefined) {
          return
        }
        await sendTons(normalized, tonAmountNano)
      }
    } catch (submitError) {
      showToast({
        variant: "error",
        title: isJettonMode ? "Mint failed" : "Transfer failed",
        description:
          submitError instanceof Error
            ? submitError.message
            : isJettonMode
              ? "Failed to mint jettons."
              : "Failed to send GRAM.",
      })
    } finally {
      setIsSubmitting(false)
    }
  }

  async function sendTons(normalized: string, nanoAmount: number) {
    const msgHash = await client.fundAccount(normalized, nanoAmount)
    await showFaucetSuccessToast({
      title: "Transfer sent",
      description: (
        <>
          Sent {amount.trim()} GRAM to {formatAddress(normalized, true, addressFormat)}.
        </>
      ),
      msgHash,
    })
  }

  async function mintJettons(recipientAddress: Address, normalized: string) {
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
    const [master] = await client.getJettonMasters([normalizedMinter])
    if (!master) {
      throw new Error(TOKEN_MINTER_NOT_FOUND_MESSAGE)
    }
    if (!master.mintable) {
      throw new Error(TOKEN_MINTER_NOT_MINTABLE_MESSAGE)
    }
    if (!master.admin_address) {
      throw new Error("This jetton master has no admin address, so faucet cannot mint it.")
    }

    const adminAddress = parseAddress(master.admin_address)
    if (!adminAddress) {
      throw new Error("Jetton master admin address is invalid.")
    }

    const decimals = normalizeJettonDecimals(master.jetton_content.decimals)
    const jettonAmount = parseJettonAmount(amount, decimals)
    if (jettonAmount === undefined) {
      showToast({
        variant: "error",
        title: "Invalid amount",
        description: `Enter a valid amount with up to ${decimals} decimal places.`,
      })
      return
    }

    const boc = buildJettonMintInternalMessageBoc({
      minter: parsedMinter,
      admin: adminAddress,
      recipient: recipientAddress,
      jettonAmount,
    })
    const symbol = jettonSymbol(master)
    const msgHash = await client.sendInternalMessage(boc)
    await showFaucetSuccessToast({
      title: "Mint sent",
      description: (
        <>
          Minted {amount.trim()} {symbol} to {formatAddress(normalized, true, addressFormat)}.
        </>
      ),
      msgHash,
    })
  }

  async function showFaucetSuccessToast({
    title,
    description,
    msgHash,
  }: {
    readonly title: string
    readonly description: ReactNode
    readonly msgHash: string
  }) {
    const txHash = await waitForTraceTransactionHash(msgHash)
    showToast({
      variant: "success",
      title,
      description: (
        <span>
          {description}
          {txHash && (
            <>
              <br />
              <br />
              <a href={`/explorer/tx/${encodeURIComponent(txHash)}`}>View transaction</a>
            </>
          )}
        </span>
      ),
      durationMs: txHash ? 8000 : undefined,
    })
  }

  async function loadMinterAddress(rawMinter: string): Promise<void> {
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

  async function waitForTraceTransactionHash(msgHash: string): Promise<string | undefined> {
    for (let attempt = 0; attempt < 8; attempt += 1) {
      if (attempt > 0) {
        await delay(500)
      }

      try {
        const response = await client.getTracesByMessageHash(msgHash)
        const txHash =
          response.traces[0]?.trace.tx_hash ?? response.traces[0]?.transactions_order[0]
        if (txHash) {
          return hashToHex(txHash) ?? txHash
        }
      } catch {
        // The mint message can be accepted before the next scheduled block indexes its trace.
      }
    }

    return undefined
  }

  const symbolHint = isJettonMode ? (selectedJettonOption?.badge ?? "jettons") : "GRAM"

  function jettonSymbol(master: JettonMaster): string {
    const symbol = master.jetton_content.symbol
    return typeof symbol === "string" && symbol.trim().length > 0 ? symbol.trim() : "jettons"
  }

  return (
    <>
      <section className={styles.hero}>
        <div>
          <h1 className={styles.title}>Faucet</h1>
        </div>
      </section>

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
                className={`${styles.fieldInput} ${styles.amountAssetInput}`}
                inputMode="decimal"
                placeholder={isJettonMode ? "0.0" : "0.0 GRAM"}
                value={amount}
                autoComplete="off"
                autoCorrect="off"
                spellCheck={false}
                onChange={event => setAmount(event.target.value)}
              />
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
            </div>
          </div>

          <div className={styles.fieldBlock}>
            <label className={styles.label} htmlFor="dashboard-address">
              Recipient
            </label>
            <FaucetDropdownInput
              id="dashboard-address"
              menuLabel="Choose wallet"
              emptyLabel={walletsError ?? "No startup wallets found."}
              isLoading={walletsLoading}
              loadingLabel="Loading wallets..."
              options={walletOptions}
              placeholder="EQ..."
              selectedOption={selectedWalletOption}
              value={address}
              onChange={setAddress}
              onSelect={option => setAddress(option.value)}
            />
          </div>

          <div className={styles.quickActions}>
            {QUICK_AMOUNTS.map(value => (
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
            <Button type="submit" className={styles.sendButton} disabled={isSubmitDisabled}>
              <span>
                {isSubmitting ? "Sending..." : isJettonMode ? "Mint Jetton" : "Send GRAM"}
              </span>
              <ArrowUpRight size={16} />
            </Button>
          </div>
        </form>
      </section>

      {isAssetModalOpen && (
        <div
          className={styles.assetModalBackdrop}
          onMouseDown={event => {
            if (event.target === event.currentTarget) {
              setIsAssetModalOpen(false)
            }
          }}
        >
          <section
            className={styles.assetModal}
            role="dialog"
            aria-modal="true"
            aria-labelledby="faucet-asset-modal-title"
          >
            <div className={styles.assetModalHeader}>
              <div>
                <h2 id="faucet-asset-modal-title" className={styles.assetModalTitle}>
                  Asset
                </h2>
              </div>
              <button
                type="button"
                className={styles.assetModalCloseButton}
                aria-label="Close asset selector"
                onClick={() => setIsAssetModalOpen(false)}
              >
                <X size={18} />
              </button>
            </div>

            <div className={styles.assetModalContent}>
              <div className={styles.assetChoiceList}>
                <button
                  type="button"
                  className={`${styles.assetChoiceButton} ${isJettonMode ? "" : styles.assetChoiceButtonSelected}`}
                  onClick={selectGramAsset}
                >
                  <img src={GRAM_LOGO_IMAGE} alt="" className={styles.assetChoiceImage} />
                  <span className={styles.assetChoiceText}>
                    <span className={styles.assetChoiceTitle}>GRAM</span>
                    <span className={styles.assetChoiceSubtitle}>Native localnet balance</span>
                  </span>
                  {!isJettonMode && <Check size={17} className={styles.assetChoiceCheck} />}
                </button>

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
                    Loading local jettons...
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
          </section>
        </div>
      )}
    </>
  )
}

function delay(durationMs: number): Promise<void> {
  return new Promise(resolve => {
    globalThis.setTimeout(resolve, durationMs)
  })
}

interface FaucetDropdownInputProps {
  readonly id: string
  readonly menuLabel: string
  readonly emptyLabel: string
  readonly isLoading: boolean
  readonly loadingLabel: string
  readonly options: readonly FaucetOption[]
  readonly placeholder: string
  readonly selectedOption?: FaucetOption
  readonly showOptionBadge?: boolean
  readonly value: string
  readonly onChange: (value: string) => void
  readonly onSelect: (option: FaucetOption) => void
}

const FaucetDropdownInput: FC<FaucetDropdownInputProps> = ({
  id,
  menuLabel,
  emptyLabel,
  isLoading,
  loadingLabel,
  options,
  placeholder,
  selectedOption,
  showOptionBadge = false,
  value,
  onChange,
  onSelect,
}) => {
  const [isOpen, setIsOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)
  const listboxId = useId()

  useEffect(() => {
    if (!isOpen) {
      return
    }

    const onPointerDown = (event: PointerEvent) => {
      const target = event.target
      if (target instanceof Node && !containerRef.current?.contains(target)) {
        setIsOpen(false)
      }
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsOpen(false)
      }
    }

    globalThis.addEventListener("pointerdown", onPointerDown)
    globalThis.addEventListener("keydown", onKeyDown)
    return () => {
      globalThis.removeEventListener("pointerdown", onPointerDown)
      globalThis.removeEventListener("keydown", onKeyDown)
    }
  }, [isOpen])

  return (
    <div
      ref={containerRef}
      className={`${styles.faucetDropdownField} ${isOpen ? styles.faucetDropdownFieldOpen : ""}`}
    >
      <Input
        id={id}
        className={`${styles.fieldInput} ${styles.faucetDropdownInput}`}
        placeholder={placeholder}
        value={value}
        autoComplete="off"
        autoCorrect="off"
        spellCheck={false}
        onChange={event => onChange(event.target.value)}
      />
      <button
        type="button"
        className={`${styles.faucetDropdownTrigger} ${isOpen ? styles.faucetDropdownTriggerOpen : ""}`}
        aria-label={menuLabel}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        aria-controls={listboxId}
        onClick={() => setIsOpen(current => !current)}
      >
        <span className={styles.faucetDropdownTriggerLabel}>
          {selectedOption?.image && (
            <img
              src={selectedOption.image}
              alt=""
              className={styles.faucetDropdownTriggerImage}
              onError={event => {
                const imageElement = event.currentTarget
                if (imageElement.getAttribute("src") !== TOKEN_PLACEHOLDER_IMAGE) {
                  imageElement.src = TOKEN_PLACEHOLDER_IMAGE
                }
              }}
            />
          )}
          {selectedOption?.badge ?? "Select"}
        </span>
        <ChevronDown
          size={16}
          className={`${styles.faucetDropdownChevron} ${isOpen ? styles.faucetDropdownChevronOpen : ""}`}
          aria-hidden="true"
        />
      </button>

      {isOpen && (
        <div id={listboxId} className={styles.faucetDropdownMenu} role="listbox">
          {isLoading ? (
            <div className={styles.faucetDropdownState}>{loadingLabel}</div>
          ) : options.length === 0 ? (
            <div className={styles.faucetDropdownState}>{emptyLabel}</div>
          ) : (
            options.map(option => {
              const isSelected = selectedOption?.id === option.id
              return (
                <button
                  key={option.id}
                  type="button"
                  className={`${styles.faucetDropdownOption} ${isSelected ? styles.faucetDropdownOptionSelected : ""}`}
                  role="option"
                  aria-selected={isSelected}
                  onClick={() => {
                    onSelect(option)
                    setIsOpen(false)
                  }}
                >
                  {option.image ? (
                    <img
                      src={option.image}
                      alt=""
                      className={styles.faucetDropdownOptionImage}
                      onError={event => {
                        const imageElement = event.currentTarget
                        if (imageElement.getAttribute("src") !== TOKEN_PLACEHOLDER_IMAGE) {
                          imageElement.src = TOKEN_PLACEHOLDER_IMAGE
                        }
                      }}
                    />
                  ) : (
                    <span className={styles.faucetDropdownOptionAvatar}>
                      {option.fallbackInitial || "A"}
                    </span>
                  )}
                  <span className={styles.faucetDropdownOptionBody}>
                    <span className={styles.faucetDropdownOptionTitle}>{option.title}</span>
                    <span className={styles.faucetDropdownOptionSubtitle}>{option.subtitle}</span>
                  </span>
                  {showOptionBadge && option.badge && (
                    <span className={styles.faucetDropdownBadge}>{option.badge}</span>
                  )}
                  {isSelected && <Check size={16} className={styles.faucetDropdownCheck} />}
                </button>
              )
            })
          )}
        </div>
      )}
    </div>
  )
}
