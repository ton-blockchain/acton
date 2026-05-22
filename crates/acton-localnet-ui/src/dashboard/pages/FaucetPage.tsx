import {ArrowUpRight, Wallet} from "lucide-react"
import * as React from "react"
import {Button, Input, useToast} from "@acton/shared-ui"

import type {TonClient} from "../../explorer/api/client"
import {formatAddress, parseAddress} from "../../explorer/components/utils"
import {useAddressFormat} from "../../explorer/hooks/useNetworkInfo"
import {QUICK_AMOUNTS} from "../constants"
import {parseTonAmount} from "../dashboardUtils"

import styles from "../DashboardPage.module.css"

interface FaucetPageProps {
  readonly client: TonClient
}

export const FaucetPage: React.FC<FaucetPageProps> = ({client}) => {
  const {showToast} = useToast()
  const addressFormat = useAddressFormat()
  const [address, setAddress] = React.useState("")
  const [amount, setAmount] = React.useState("1")
  const [isSubmitting, setIsSubmitting] = React.useState(false)
  const amountNano = React.useMemo(() => parseTonAmount(amount), [amount])

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
    if (amountNano === undefined) {
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
      await client.fundAccount(normalized, amountNano)
      showToast({
        variant: "success",
        title: "Transfer sent",
        description: `Sent ${amount.trim()} TON to ${formatAddress(normalized, true, addressFormat)}.`,
      })
    } catch (submitError) {
      showToast({
        variant: "error",
        title: "Transfer failed",
        description: submitError instanceof Error ? submitError.message : "Failed to send TON.",
      })
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <>
      <section className={styles.hero}>
        <div>
          <h1 className={styles.title}>Send test TON</h1>
          <p className={styles.subtitle}>
            Top up any wallet address with test TON in a few seconds.
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
                <h2 className={styles.cardTitle}>Wallet top up</h2>
                <p className={styles.cardDescription}>
                  Enter an address, choose an amount, and send funds.
                </p>
              </div>
            </div>
          </div>

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
              placeholder="0.0 TON"
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
                {value} TON
              </Button>
            ))}
          </div>

          <div className={styles.formFooter}>
            <div className={styles.formHint}>Use this faucet to fund wallets for testing.</div>
            <Button
              type="submit"
              className={styles.sendButton}
              disabled={isSubmitting || address.trim().length === 0 || amount.trim().length === 0}
            >
              <span>{isSubmitting ? "Sending..." : "Send TON"}</span>
              <ArrowUpRight size={16} />
            </Button>
          </div>
        </form>
      </section>
    </>
  )
}
