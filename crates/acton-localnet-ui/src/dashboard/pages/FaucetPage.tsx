import {ArrowUpRight, Coins, Wallet} from "lucide-react"
import * as React from "react"
import {Button, Input, useToast} from "@acton/shared-ui"
import type {Address} from "@ton/core"

import type {JettonMaster} from "../../explorer/api/types"
import type {TonClient} from "../../explorer/api/client"
import {formatAddress, parseAddress} from "../../explorer/components/utils"
import {useAddressFormat} from "../../explorer/hooks/useNetworkInfo"
import {QUICK_AMOUNTS} from "../constants"
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

export const FaucetPage: React.FC<FaucetPageProps> = ({client}) => {
  const {showToast} = useToast()
  const addressFormat = useAddressFormat()
  const [mode, setMode] = React.useState<FaucetMode>("ton")
  const [address, setAddress] = React.useState("")
  const [jettonMinter, setJettonMinter] = React.useState("")
  const [amount, setAmount] = React.useState("1")
  const [isSubmitting, setIsSubmitting] = React.useState(false)
  const amountNano = React.useMemo(() => parseGramAmount(amount), [amount])
  const isJettonMode = mode === "jetton"
  const isSubmitDisabled =
    isSubmitting ||
    address.trim().length === 0 ||
    amount.trim().length === 0 ||
    (isJettonMode && jettonMinter.trim().length === 0)

  async function handleSubmit(event?: React.FormEvent): Promise<void> {
    event?.preventDefault()
    const trimmedAddress = address.trim()
    const parsedAddress = parseAddress(trimmedAddress)
    if (!parsedAddress) {
      showToast({
        variant: "error",
        title: "Invalid address",
        description: "Enter a valid TON address.",
      })
      return
    }
    if (!isJettonMode && amountNano === undefined) {
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
        await client.fundAccount(normalized, amountNano as number)
        showToast({
          variant: "success",
          title: "Transfer sent",
          description: `Sent ${amount.trim()} GRAM to ${formatAddress(normalized, true, addressFormat)}.`,
        })
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
    await client.sendInternalMessage(boc)

    const symbol = jettonSymbol(master)
    showToast({
      variant: "success",
      title: "Mint sent",
      description: `Minted ${amount.trim()} ${symbol} to ${formatAddress(normalized, true, addressFormat)}.`,
    })
  }

  const symbolHint = isJettonMode ? "jettons" : "GRAM"

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
              onClick={() => setMode("ton")}
            >
              <Wallet size={15} />
              GRAM
            </button>
            <button
              type="button"
              className={`${styles.modeToggleButton} ${mode === "jetton" ? styles.modeToggleButtonActive : ""}`}
              aria-pressed={mode === "jetton"}
              onClick={() => setMode("jetton")}
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
              <Input
                id="dashboard-jetton-minter"
                className={styles.fieldInput}
                placeholder="EQ..."
                value={jettonMinter}
                autoComplete="off"
                autoCorrect="off"
                spellCheck={false}
                onChange={event => setJettonMinter(event.target.value)}
              />
              <p className={styles.hint}>Paste the jetton master contract address.</p>
            </div>
          )}

          <div className={styles.fieldBlock}>
            <label className={styles.label} htmlFor="dashboard-address">
              Recipient address
            </label>
            <Input
              id="dashboard-address"
              className={styles.fieldInput}
              placeholder="EQ..."
              value={address}
              autoComplete="off"
              autoCorrect="off"
              spellCheck={false}
              onChange={event => setAddress(event.target.value)}
            />
            <p className={styles.hint}>Paste any raw or user-friendly TON address.</p>
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
            <div className={styles.formHint}>
              {isJettonMode
                ? "Minting uses the local jetton master metadata."
                : "Use this faucet to fund wallets for testing."}
            </div>
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
