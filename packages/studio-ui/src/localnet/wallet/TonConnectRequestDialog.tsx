import {AddressChip, Button, Dialog, DialogActions} from "@acton/ui"
import {Check, ChevronDown, Globe2, WalletCards} from "lucide-react"
import {useEffect, useState} from "react"
import type {FC, ReactNode} from "react"

import studioLogo from "../../assets/acton-studio-logo.svg"

import styles from "./TonConnectRequestDialog.module.css"

interface DappInfo {
  readonly iconUrl?: string
  readonly name?: string
  readonly url?: string
}

/** Stable display data shared by the selected-wallet plate and its optional picker */
export interface TonConnectWalletOption {
  readonly address: string
  readonly balance: ReactNode
  readonly id: string
  readonly name: string
  readonly network: string
}

interface TonConnectRequestDialogProps {
  readonly approveDisabled?: boolean
  readonly approveLabel: string
  readonly busy: boolean
  readonly children: ReactNode
  readonly className?: string
  readonly dappInfo?: DappInfo
  readonly description: ReactNode
  readonly disclaimer: ReactNode
  readonly domain?: string
  readonly kind: "connect" | "sign" | "transaction"
  readonly open: boolean
  readonly titlePrefix: string
  readonly wallet: ReactNode
  readonly onApprove: () => void
  readonly onReject: () => void
}

/**
 * Presents every TON Connect approval through the same trust-oriented shell
 * while request-specific content remains owned by the WalletKit integration
 */
export const TonConnectRequestDialog: FC<TonConnectRequestDialogProps> = ({
  approveDisabled = false,
  approveLabel,
  busy,
  children,
  className,
  dappInfo,
  description,
  disclaimer,
  domain,
  kind,
  open,
  titlePrefix,
  wallet,
  onApprove,
  onReject,
}) => {
  const dappLabel = getDappLabel(dappInfo, domain)

  return (
    <Dialog
      open={open}
      title={
        <span className={styles.requestTitle}>
          {titlePrefix}
          <br />
          <span className={styles.dappName}>{dappLabel}</span>?
        </span>
      }
      description={<span className={styles.requestDescription}>{description}</span>}
      leadingIcon={<RequestIdentity dappInfo={dappInfo} />}
      className={`${styles.dialog} ${className ?? ""}`}
      maxWidth={448}
      headerClassName={styles.requestHeader}
      contentPadding="none"
      dismissible={false}
      busy={busy}
      onOpenChange={() => undefined}
      footer={
        <DialogActions className={styles.actions}>
          <Button
            className={styles.actionButton}
            variant="primary"
            size="lg"
            loading={busy}
            disabled={approveDisabled}
            onClick={onApprove}
          >
            {approveLabel}
          </Button>
          <Button
            className={styles.actionButton}
            variant="ghost"
            size="lg"
            disabled={busy}
            onClick={onReject}
          >
            Reject
          </Button>
        </DialogActions>
      }
    >
      <div className={styles.body} data-testid={`ton-connect-${kind}-request`}>
        {wallet}
        {children}
        <p className={styles.disclaimer}>{disclaimer}</p>
      </div>
    </Dialog>
  )
}

interface TonConnectWalletSelectorProps {
  readonly options: readonly TonConnectWalletOption[]
  readonly selectedId?: string
  readonly onSelect: (walletId: string) => void
}

/** Lets a user confirm the signing identity without exposing every wallet by default */
export const TonConnectWalletSelector: FC<TonConnectWalletSelectorProps> = ({
  options,
  selectedId,
  onSelect,
}) => {
  const [expanded, setExpanded] = useState(false)
  const selected = options.find(option => option.id === selectedId) ?? options[0]

  if (!selected) {
    return null
  }

  const selectable = options.length > 1

  return (
    <div className={styles.walletSelector}>
      <TonConnectWalletPlate
        wallet={selected}
        expanded={expanded}
        selectable={selectable}
        onClick={selectable ? () => setExpanded(current => !current) : undefined}
      />

      {expanded && (
        <div className={styles.walletOptions} role="listbox" aria-label="Project wallets">
          {options.map(option => {
            const isSelected = option.id === selected.id

            return (
              <button
                key={option.id}
                type="button"
                role="option"
                aria-selected={isSelected}
                className={`${styles.walletOption} ${isSelected ? styles.walletOptionActive : ""}`}
                onClick={() => {
                  onSelect(option.id)
                  setExpanded(false)
                }}
              >
                <WalletIdentity wallet={option} />
                <span className={styles.optionCheck}>{isSelected && <Check size={14} />}</span>
              </button>
            )
          })}
        </div>
      )}
    </div>
  )
}

interface TonConnectWalletPlateProps {
  readonly expanded?: boolean
  readonly selectable?: boolean
  readonly wallet: TonConnectWalletOption
  readonly onClick?: () => void
}

/** Shows which project wallet owns the approval and any resulting signature */
export const TonConnectWalletPlate: FC<TonConnectWalletPlateProps> = ({
  expanded = false,
  selectable = false,
  wallet,
  onClick,
}) => {
  if (!selectable) {
    return (
      <div className={styles.walletPlate}>
        <WalletIdentity wallet={wallet} />
      </div>
    )
  }

  return (
    <button
      type="button"
      className={`${styles.walletPlate} ${styles.walletPlateButton}`}
      aria-expanded={expanded}
      aria-label={`Change wallet, currently ${wallet.name}`}
      onClick={onClick}
    >
      <WalletIdentity wallet={wallet} />
      <ChevronDown
        className={expanded ? styles.walletChevronExpanded : styles.walletChevron}
        size={18}
        aria-hidden="true"
      />
    </button>
  )
}

const WalletIdentity: FC<{readonly wallet: TonConnectWalletOption}> = ({wallet}) => (
  <>
    <span className={styles.walletIcon}>
      <WalletCards size={19} aria-hidden="true" />
    </span>
    <span className={styles.walletContent}>
      <span className={styles.walletPrimary}>
        <span className={styles.walletName}>{wallet.name}</span>
        <span className={styles.walletBalance}>{wallet.balance}</span>
      </span>
      <span className={styles.walletMeta}>
        <span className={styles.walletAddress}>
          <AddressChip address={wallet.address} copyable={false} variant="plain" />
        </span>
        <span className={styles.walletNetwork}>{wallet.network}</span>
      </span>
    </span>
  </>
)

const RequestIdentity: FC<{readonly dappInfo?: DappInfo}> = ({dappInfo}) => {
  const [iconFailed, setIconFailed] = useState(false)

  useEffect(() => setIconFailed(false), [dappInfo?.iconUrl])

  return (
    <div className={styles.requestIdentity} aria-hidden="true">
      <span className={styles.studioMark}>
        <img src={studioLogo} alt="" />
      </span>
      <span className={styles.dappMark}>
        {dappInfo?.iconUrl && !iconFailed ? (
          <img src={dappInfo.iconUrl} alt="" onError={() => setIconFailed(true)} />
        ) : (
          <Globe2 size={32} />
        )}
      </span>
    </div>
  )
}

/** Keep the requesting origin visible instead of relying on a self-reported app name */
function getDappLabel(dappInfo: DappInfo | undefined, domain: string | undefined): string {
  if (domain?.trim()) {
    return domain.trim()
  }

  if (dappInfo?.url) {
    try {
      return new URL(dappInfo.url).host
    } catch {
      // A malformed manifest URL must not prevent the user from rejecting the request
    }
  }

  return dappInfo?.name?.trim() || "this dApp"
}
