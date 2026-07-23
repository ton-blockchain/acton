import {Button, Dialog, Input} from "@acton/ui"
import {ArrowLeft, ArrowUpRight, CheckCircle2, ShieldCheck} from "lucide-react"
import {useEffect, useRef, useState, type FormEvent} from "react"

import {
  formatTonBalance,
  shortenAddress,
  WALLET_NETWORKS,
  type WalletRecord,
} from "../domain/wallet"
import {
  previewGramTransfer,
  sendGramTransfer,
  type GramTransferInput,
  type GramTransferPreview,
  type SentGramTransfer,
} from "../services/walletTransfer"
import {getVaultKind} from "../services/walletVault"
import styles from "./TransferDialog.module.css"

interface TransferDialogProps {
  readonly wallet?: WalletRecord
  readonly onClose: () => void
  readonly onSent: (result: SentGramTransfer) => void
}

const EMPTY_TRANSFER: GramTransferInput = {
  recipient: "",
  amount: "",
  comment: "",
}

export function TransferDialog({wallet, onClose, onSent}: TransferDialogProps) {
  const [input, setInput] = useState<GramTransferInput>(EMPTY_TRANSFER)
  const [preview, setPreview] = useState<GramTransferPreview>()
  const [isPreviewing, setIsPreviewing] = useState(false)
  const [isSending, setIsSending] = useState(false)
  const [error, setError] = useState<string>()
  const errorRef = useRef<HTMLParagraphElement>(null)

  useEffect(() => {
    if (!error) return
    const frame = requestAnimationFrame(() => {
      errorRef.current?.scrollIntoView({block: "nearest"})
    })
    return () => cancelAnimationFrame(frame)
  }, [error])

  if (!wallet) {
    return null
  }

  const commentBytes = new TextEncoder().encode(input.comment ?? "").length
  const canPreview = canPreviewTransfer(input, commentBytes, isPreviewing)
  const presentation = getDialogPresentation(Boolean(preview), WALLET_NETWORKS[wallet.network].name)

  const handlePreview = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    setIsPreviewing(true)
    setError(undefined)
    try {
      setPreview(await previewGramTransfer(wallet, input))
    } catch (previewError) {
      setError(getErrorMessage(previewError, "The transfer could not be previewed."))
    } finally {
      setIsPreviewing(false)
    }
  }

  const handleSend = async () => {
    setIsSending(true)
    setError(undefined)
    try {
      const result = await sendGramTransfer(wallet, input)
      onSent(result)
      onClose()
    } catch (sendError) {
      setError(getErrorMessage(sendError, "The transfer could not be submitted."))
    } finally {
      setIsSending(false)
    }
  }

  return (
    <Dialog
      open={true}
      title={presentation.title}
      description={presentation.description}
      leadingIcon={presentation.icon}
      maxWidth={540}
      contentClassName={styles.dialogContent}
      dismissible={!isSending}
      onOpenChange={() => onClose()}
    >
      {preview ? (
        <div className={styles.review}>
          <div className={styles.previewStatus}>
            <CheckCircle2 size={19} />
            <span>
              <strong>Simulation succeeded</strong>
              <small className={styles.previewHint}>
                The current network state accepted this message.
              </small>
            </span>
          </div>

          <dl className={styles.summary}>
            <div>
              <dt>From</dt>
              <dd title={wallet.address}>{wallet.name}</dd>
            </div>
            <div>
              <dt>To</dt>
              <dd title={input.recipient}>{shortenAddress(input.recipient, 9)}</dd>
            </div>
            <div>
              <dt>Amount</dt>
              <dd>{input.amount} GRAM</dd>
            </div>
            <div>
              <dt>Network</dt>
              <dd>{WALLET_NETWORKS[wallet.network].name}</dd>
            </div>
            {input.comment ? (
              <div className={styles.summaryWide}>
                <dt>Comment</dt>
                <dd>{input.comment}</dd>
              </div>
            ) : undefined}
            {preview.outputNano ? (
              <div className={styles.summaryWide}>
                <dt>Emulated output</dt>
                <dd>{formatTonBalance(preview.outputNano)} GRAM</dd>
              </div>
            ) : undefined}
          </dl>

          {getVaultKind() === "native" ? undefined : (
            <p className={styles.error}>Open the desktop app to sign and submit this transfer.</p>
          )}
          {error ? (
            <p ref={errorRef} className={styles.error}>
              {error}
            </p>
          ) : undefined}

          <div className={styles.actions}>
            <Button
              variant="ghost"
              leadingIcon={<ArrowLeft size={15} />}
              disabled={isSending}
              onClick={() => {
                setPreview(undefined)
                setError(undefined)
              }}
            >
              Edit
            </Button>
            <Button
              variant="primary"
              leadingIcon={<ArrowUpRight size={15} />}
              loading={isSending}
              disabled={getVaultKind() !== "native"}
              onClick={() => void handleSend()}
            >
              Sign and send
            </Button>
          </div>
        </div>
      ) : (
        <form className={styles.form} onSubmit={event => void handlePreview(event)}>
          <Input
            label="Recipient"
            value={input.recipient}
            placeholder="EQ… or UQ…"
            autoComplete="off"
            spellCheck={false}
            required={true}
            onChange={event => setInput(current => ({...current, recipient: event.target.value}))}
          />
          <Input
            label="Amount"
            value={input.amount}
            placeholder="0.0"
            inputMode="decimal"
            autoComplete="off"
            suffix={<span className={styles.inputTicker}>GRAM</span>}
            required={true}
            onChange={event => setInput(current => ({...current, amount: event.target.value}))}
          />
          <label className={styles.commentField}>
            <span>
              Comment <small className={styles.optional}>Optional</small>
            </span>
            <textarea
              rows={3}
              value={input.comment}
              placeholder="Deployment payment, test transfer…"
              maxLength={120}
              onChange={event => setInput(current => ({...current, comment: event.target.value}))}
            />
            <small className={styles.commentCount} data-invalid={commentBytes > 120}>
              {commentBytes}/120 UTF-8 bytes
            </small>
          </label>

          <div className={styles.deliveryNote}>
            <ShieldCheck size={17} />
            <span>
              Network fees are paid separately. Action-phase errors are ignored so one failed action
              does not reject the wallet message.
            </span>
          </div>
          {error ? (
            <p ref={errorRef} className={styles.error}>
              {error}
            </p>
          ) : undefined}

          <div className={styles.actions}>
            <Button variant="ghost" disabled={isPreviewing} onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" variant="primary" loading={isPreviewing} disabled={!canPreview}>
              Preview transfer
            </Button>
          </div>
        </form>
      )}
    </Dialog>
  )
}

function getErrorMessage(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message) return error.message
  if (typeof error === "string" && error.trim()) return error
  return fallback
}

function canPreviewTransfer(
  input: GramTransferInput,
  commentBytes: number,
  isPreviewing: boolean,
): boolean {
  return (
    input.recipient.trim().length > 0 &&
    input.amount.trim().length > 0 &&
    commentBytes <= 120 &&
    !isPreviewing
  )
}

function getDialogPresentation(isReview: boolean, networkName: string) {
  if (isReview) {
    return {
      title: "Review transfer",
      description: `Verify every field before signing on ${networkName}.`,
      icon: <ShieldCheck size={20} />,
    }
  }
  return {
    title: "Send GRAM",
    description: "Preview execution before your wallet signs and submits the transfer.",
    icon: <ArrowUpRight size={20} />,
  }
}
