import {ArrowUpRight, Check, ChevronDown, Coins, Wallet} from "lucide-react"
import * as React from "react"
import {Button, Input, useToast} from "@acton/shared-ui"
import type {Address} from "@ton/core"
import {useSearchParams} from "react-router-dom"

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

import styles from "../DashboardPage.module.css"

interface FaucetPageProps {
  readonly client: TonClient
}

type FaucetMode = "ton" | "jetton"

const FAUCET_MODE_QUERY_PARAM = "mode"

interface FaucetOption {
  readonly id: string
  readonly title: string
  readonly subtitle: string
  readonly value: string
  readonly badge?: string
  readonly image?: string
  readonly fallbackInitial?: string
}

export const FaucetPage: React.FC<FaucetPageProps> = ({client}) => {
  const {showToast} = useToast()
  const addressFormat = useAddressFormat()
  const [searchParams, setSearchParams] = useSearchParams()
  const [mode, setMode] = React.useState<FaucetMode>(() =>
    parseFaucetMode(searchParams.get(FAUCET_MODE_QUERY_PARAM)),
  )
  const [address, setAddress] = React.useState("")
  const [jettonMinter, setJettonMinter] = React.useState("")
  const [amount, setAmount] = React.useState("1")
  const [startupWallets, setStartupWallets] = React.useState<StartupWallet[]>([])
  const [jettonMasters, setJettonMasters] = React.useState<JettonMaster[]>([])
  const [walletsLoading, setWalletsLoading] = React.useState(true)
  const [jettonsLoading, setJettonsLoading] = React.useState(true)
  const [walletsError, setWalletsError] = React.useState<string>()
  const [jettonsError, setJettonsError] = React.useState<string>()
  const [isSubmitting, setIsSubmitting] = React.useState(false)
  const amountNano = React.useMemo(() => parseGramAmount(amount), [amount])
  const isJettonMode = mode === "jetton"
  const isSubmitDisabled =
    isSubmitting ||
    address.trim().length === 0 ||
    amount.trim().length === 0 ||
    (isJettonMode && jettonMinter.trim().length === 0)

  React.useEffect(() => {
    setMode(parseFaucetMode(searchParams.get(FAUCET_MODE_QUERY_PARAM)))
  }, [searchParams])

  const selectMode = React.useCallback(
    (nextMode: FaucetMode) => {
      setMode(nextMode)
      setSearchParams(
        currentSearchParams => {
          const nextSearchParams = new URLSearchParams(currentSearchParams)
          nextSearchParams.set(FAUCET_MODE_QUERY_PARAM, nextMode)
          return nextSearchParams
        },
        {replace: true},
      )
    },
    [setSearchParams],
  )

  React.useEffect(() => {
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

  React.useEffect(() => {
    let cancelled = false

    void (async () => {
      setJettonsLoading(true)
      setJettonsError(undefined)

      try {
        const masters = await client.getJettonMasters(undefined, 100, 0)
        if (!cancelled) {
          setJettonMasters(masters)
        }
      } catch (error) {
        if (!cancelled) {
          setJettonMasters([])
          setJettonsError(error instanceof Error ? error.message : "Failed to load jettons")
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

  const walletOptions = React.useMemo<FaucetOption[]>(
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
  const selectedWalletOption = React.useMemo(
    () => walletOptions.find(option => isSameAddress(option.value, address)),
    [address, walletOptions],
  )
  const jettonOptions = React.useMemo<FaucetOption[]>(
    () =>
      jettonMasters
        .filter(master => master.mintable)
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
        }),
    [addressFormat, jettonMasters],
  )
  const selectedJettonOption = React.useMemo(
    () => jettonOptions.find(option => isSameAddress(option.value, jettonMinter)),
    [jettonMinter, jettonOptions],
  )

  async function handleSubmit(event?: React.FormEvent): Promise<void> {
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
      throw new Error("Jetton master was not found in localnet metadata.")
    }
    if (!master.mintable) {
      throw new Error("This jetton master is not mintable.")
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
    readonly description: React.ReactNode
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
          <h1 className={styles.title}>Local faucet</h1>
          <p className={styles.subtitle}>
            Top up any wallet address with test GRAM or mint local jettons from a minter.
          </p>
        </div>
      </section>

      <section className={styles.faucetLayout}>
        <form className={styles.formCard} onSubmit={event => void handleSubmit(event)}>
          <div className={styles.cardHeader}>
            <div className={styles.cardTitleRow}>
              <div className={styles.cardIcon}>
                <Wallet size={16} />
              </div>
              <div>
                <h2 className={styles.cardTitle}>
                  {isJettonMode ? "Jetton mint" : "Wallet top up"}
                </h2>
                <p className={styles.cardDescription}>
                  {isJettonMode
                    ? "Enter a minter, recipient, and amount."
                    : "Enter an address, choose an amount, and send funds."}
                </p>
              </div>
            </div>
          </div>

          <div className={styles.modeToggle} aria-label="Faucet asset type">
            <button
              type="button"
              className={`${styles.modeToggleButton} ${mode === "ton" ? styles.modeToggleButtonActive : ""}`}
              aria-pressed={mode === "ton"}
              onClick={() => selectMode("ton")}
            >
              <Wallet size={15} />
              GRAM
            </button>
            <button
              type="button"
              className={`${styles.modeToggleButton} ${mode === "jetton" ? styles.modeToggleButtonActive : ""}`}
              aria-pressed={mode === "jetton"}
              onClick={() => selectMode("jetton")}
            >
              <Coins size={15} />
              Jetton
            </button>
          </div>

          {isJettonMode && (
            <div className={styles.fieldBlock}>
              <label className={styles.label} htmlFor="dashboard-jetton-minter">
                Jetton minter
              </label>
              <FaucetDropdownInput
                id="dashboard-jetton-minter"
                menuLabel="Choose jetton"
                emptyLabel={jettonsError ?? "No mintable jettons found."}
                isLoading={jettonsLoading}
                loadingLabel="Loading jettons..."
                options={jettonOptions}
                placeholder="EQ..."
                selectedOption={selectedJettonOption}
                showOptionBadge
                value={jettonMinter}
                onChange={setJettonMinter}
                onSelect={option => setJettonMinter(option.value)}
              />
            </div>
          )}

          <div className={styles.fieldBlock}>
            <label className={styles.label} htmlFor="dashboard-address">
              Recipient address
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

          <div className={styles.fieldBlock}>
            <label className={styles.label} htmlFor="dashboard-amount">
              Amount
            </label>
            <Input
              id="dashboard-amount"
              className={styles.fieldInput}
              inputMode="decimal"
              placeholder={isJettonMode ? "0.0" : "0.0 GRAM"}
              value={amount}
              autoComplete="off"
              autoCorrect="off"
              spellCheck={false}
              onChange={event => setAmount(event.target.value)}
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
    </>
  )
}

function parseFaucetMode(value: string | null): FaucetMode {
  return value === "jetton" ? "jetton" : "ton"
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

const FaucetDropdownInput: React.FC<FaucetDropdownInputProps> = ({
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
  const [isOpen, setIsOpen] = React.useState(false)
  const containerRef = React.useRef<HTMLDivElement>(null)
  const listboxId = React.useId()

  React.useEffect(() => {
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
