import {Button, CopyButton, Dialog, GramLogo, ThemeSwitch, ToastProvider, useToast} from "@acton/ui"
import {
  ArrowUpRight,
  Check,
  KeyRound,
  LockKeyhole,
  MoreHorizontal,
  Plus,
  Radio,
  RefreshCw,
  ScanLine,
  ShieldCheck,
  Trash2,
  WalletCards,
} from "lucide-react"
import {QRCodeSVG} from "qrcode.react"
import {useCallback, useEffect, useMemo, useState} from "react"

import {
  WALLET_NETWORKS,
  formatTonBalance,
  getAccountExplorerUrl,
  shortenAddress,
  type WalletRecord,
} from "./domain/wallet"
import {
  WalletSetupDialog,
  type WalletSetupMode,
  type WalletSetupSubmission,
} from "./components/WalletSetupDialog"
import {TransferDialog} from "./components/TransferDialog"
import {WalletActivity} from "./components/WalletActivity"
import {
  createRandomMnemonic,
  deriveWalletRecord,
  fetchWalletBalance,
  normalizeMnemonic,
  validateMnemonic,
} from "./services/walletFactory"
import {openExternalUrl} from "./services/externalLinks"
import {getVaultKind, listWallets, removeWallet, saveWallet} from "./services/walletVault"
import styles from "./App.module.css"

interface BalanceState {
  readonly value?: string
  readonly isLoading: boolean
  readonly error?: string
}

export function App() {
  return (
    <ToastProvider>
      <WalletApplication />
    </ToastProvider>
  )
}

function WalletApplication() {
  const {showToast} = useToast()
  const [wallets, setWallets] = useState<readonly WalletRecord[]>([])
  const [selectedWalletId, setSelectedWalletId] = useState<string>()
  const [balance, setBalance] = useState<BalanceState>({isLoading: false})
  const [isLoadingWallets, setIsLoadingWallets] = useState(true)
  const [setupMode, setSetupMode] = useState<WalletSetupMode>("create")
  const [isSetupOpen, setIsSetupOpen] = useState(false)
  const [isSubmittingWallet, setIsSubmittingWallet] = useState(false)
  const [recoveryWords, setRecoveryWords] = useState<readonly string[]>()
  const [receiveWallet, setReceiveWallet] = useState<WalletRecord>()
  const [sendWallet, setSendWallet] = useState<WalletRecord>()
  const [removeCandidate, setRemoveCandidate] = useState<WalletRecord>()
  const [activityRefreshToken, setActivityRefreshToken] = useState(0)

  const selectedWallet = useMemo(
    () => wallets.find(wallet => wallet.id === selectedWalletId) ?? wallets[0],
    [selectedWalletId, wallets],
  )

  const loadWallets = useCallback(async () => {
    setIsLoadingWallets(true)
    try {
      const records = await listWallets()
      setWallets(records)
      setSelectedWalletId(current =>
        current && records.some(wallet => wallet.id === current) ? current : records[0]?.id,
      )
    } catch (error) {
      showToast({
        variant: "error",
        title: "Device vault unavailable",
        description: getErrorMessage(error, "Failed to read wallets from the device vault."),
      })
    } finally {
      setIsLoadingWallets(false)
    }
  }, [showToast])

  useEffect(() => {
    void loadWallets()
  }, [loadWallets])

  const refreshBalance = useCallback(async () => {
    if (!selectedWallet) {
      setBalance({isLoading: false})
      return
    }

    setBalance(current => ({...current, isLoading: true, error: undefined}))
    try {
      const value = await fetchWalletBalance(selectedWallet)
      setBalance({value, isLoading: false})
    } catch (error) {
      setBalance(current => ({
        value: current.value,
        isLoading: false,
        error: getErrorMessage(error, "Balance is temporarily unavailable."),
      }))
    }
  }, [selectedWallet])

  useEffect(() => {
    void refreshBalance()
  }, [refreshBalance])

  const openSetup = (mode: WalletSetupMode) => {
    setSetupMode(mode)
    setIsSetupOpen(true)
  }

  const handleWalletSetup = async (submission: WalletSetupSubmission) => {
    setIsSubmittingWallet(true)
    try {
      const words =
        submission.mnemonic === undefined
          ? await createRandomMnemonic()
          : normalizeMnemonic(submission.mnemonic)

      if (!(await validateMnemonic(words))) {
        throw new Error("Enter a valid 24-word TON mnemonic.")
      }

      const record = await deriveWalletRecord({
        name: submission.name,
        mnemonic: words,
        network: submission.network,
        version: submission.version,
      })
      await saveWallet(record, words)
      await loadWallets()
      setSelectedWalletId(record.id)
      setIsSetupOpen(false)

      if (submission.mnemonic === undefined) {
        setRecoveryWords(words)
      } else {
        showToast({
          variant: "success",
          title: "Wallet imported",
          description: `${record.name} is ready on ${WALLET_NETWORKS[record.network].name}.`,
        })
      }
    } catch (error) {
      showToast({
        variant: "error",
        title: "Wallet setup failed",
        description: getErrorMessage(error, "Failed to create the wallet."),
      })
    } finally {
      setIsSubmittingWallet(false)
    }
  }

  const handleRemoveWallet = async () => {
    if (!removeCandidate) {
      return
    }
    try {
      await removeWallet(removeCandidate.id)
      setRemoveCandidate(undefined)
      await loadWallets()
      showToast({
        variant: "success",
        title: "Wallet removed",
        description: "This wallet and its recovery phrase are no longer available on this device.",
      })
    } catch (error) {
      showToast({
        variant: "error",
        title: "Removal failed",
        description: getErrorMessage(error, "Failed to remove the wallet."),
      })
    }
  }

  return (
    <div className={styles.application}>
      <div className={styles.ambient} aria-hidden="true" />
      <aside className={styles.sidebar}>
        <div className={styles.brand}>
          <span className={styles.brandMark} aria-hidden="true" />
          <span>
            <strong>ACTON</strong>
            <small>DEV WALLET</small>
          </span>
          <span className={styles.alphaBadge}>α.01</span>
        </div>

        <div className={styles.sidebarSectionHeader}>
          <span>Wallets</span>
          <button type="button" aria-label="Add wallet" onClick={() => openSetup("create")}>
            <Plus size={15} />
          </button>
        </div>

        <div className={styles.walletList}>
          {wallets.map(wallet => {
            const isActive = wallet.id === selectedWallet?.id
            return (
              <button
                type="button"
                key={wallet.id}
                aria-label={`${wallet.name}, ${WALLET_NETWORKS[wallet.network].name}`}
                className={`${styles.walletListItem} ${isActive ? styles.walletListItemActive : ""}`}
                onClick={() => setSelectedWalletId(wallet.id)}
              >
                <span className={styles.walletGlyph}>
                  <WalletCards size={17} />
                </span>
                <span className={styles.walletListBody}>
                  <strong>{wallet.name}</strong>
                  <small>{shortenAddress(wallet.address, 5)}</small>
                </span>
                <span
                  className={styles.networkDot}
                  data-network={wallet.network}
                  title={WALLET_NETWORKS[wallet.network].name}
                />
              </button>
            )
          })}
          {!isLoadingWallets && wallets.length === 0 ? (
            <p className={styles.sidebarEmpty}>Create a wallet to receive test funds.</p>
          ) : undefined}
        </div>

        <div className={styles.sidebarFooter}>
          <div className={styles.vaultStatus}>
            <ShieldCheck size={16} />
            <span>
              <strong>{getVaultKind() === "native" ? "Device vault" : "Temporary wallet"}</strong>
              <small>
                {getVaultKind() === "native" ? "Protected by this device" : "Use test funds only"}
              </small>
            </span>
          </div>
          <ThemeSwitch />
        </div>
      </aside>

      <main className={styles.main}>
        <header className={styles.topbar}>
          <div className={styles.breadcrumb}>
            <span>Wallet</span>
            <span>/</span>
            <strong>{selectedWallet?.name ?? "New device"}</strong>
          </div>
          <div className={styles.topbarActions}>
            {selectedWallet ? (
              <span className={styles.networkBadge} data-network={selectedWallet.network}>
                <Radio size={13} />
                {WALLET_NETWORKS[selectedWallet.network].name}
              </span>
            ) : undefined}
            <Button
              size="sm"
              variant="outline"
              leadingIcon={<Plus size={15} />}
              onClick={() => openSetup("create")}
            >
              Add wallet
            </Button>
          </div>
        </header>

        {selectedWallet ? (
          <WalletDashboard
            wallet={selectedWallet}
            balance={balance}
            activityRefreshToken={activityRefreshToken}
            onRefreshBalance={() => void refreshBalance()}
            onSend={() => setSendWallet(selectedWallet)}
            onReceive={() => setReceiveWallet(selectedWallet)}
            onRemove={() => setRemoveCandidate(selectedWallet)}
            onOpenExplorer={() => {
              void openExternalUrl(getAccountExplorerUrl(selectedWallet)).catch(error => {
                showToast({
                  variant: "error",
                  title: "Could not open explorer",
                  description: getErrorMessage(error, "Open actonscan.com in your browser."),
                })
              })
            }}
          />
        ) : (
          <EmptyWalletState
            isLoading={isLoadingWallets}
            onCreate={() => openSetup("create")}
            onImport={() => openSetup("import")}
          />
        )}
      </main>

      <WalletSetupDialog
        mode={setupMode}
        open={isSetupOpen}
        isSubmitting={isSubmittingWallet}
        onOpenChange={setIsSetupOpen}
        onSubmit={handleWalletSetup}
      />

      <RecoveryDialog words={recoveryWords} onClose={() => setRecoveryWords(undefined)} />
      <TransferDialog
        key={sendWallet?.id ?? "closed"}
        wallet={sendWallet}
        onClose={() => setSendWallet(undefined)}
        onSent={result => {
          showToast({
            variant: "success",
            title: "Transfer submitted",
            description: `Message ${shortenAddress(result.messageHash, 8)} is waiting for confirmation.`,
          })
          void refreshBalance()
          setActivityRefreshToken(current => current + 1)
        }}
      />
      <ReceiveDialog wallet={receiveWallet} onClose={() => setReceiveWallet(undefined)} />
      <RemoveWalletDialog
        wallet={removeCandidate}
        onClose={() => setRemoveCandidate(undefined)}
        onConfirm={() => void handleRemoveWallet()}
      />
    </div>
  )
}

interface WalletDashboardProps {
  readonly wallet: WalletRecord
  readonly balance: BalanceState
  readonly activityRefreshToken: number
  readonly onRefreshBalance: () => void
  readonly onSend: () => void
  readonly onReceive: () => void
  readonly onRemove: () => void
  readonly onOpenExplorer: () => void
}

function WalletDashboard({
  wallet,
  balance,
  activityRefreshToken,
  onRefreshBalance,
  onSend,
  onReceive,
  onRemove,
  onOpenExplorer,
}: WalletDashboardProps) {
  const network = WALLET_NETWORKS[wallet.network]
  const formattedBalance = formatTonBalance(balance.value)

  return (
    <div className={styles.dashboard}>
      <section className={styles.balanceHero}>
        <div className={styles.heroEyebrow}>
          <span className={styles.livePulse} />
          Active account · workchain 0
        </div>
        <div className={styles.balanceLine}>
          <span className={styles.balanceValue}>
            {balance.isLoading ? "···" : formattedBalance}
          </span>
          <span className={styles.balanceTicker}>GRAM</span>
        </div>
        <div className={styles.addressLine}>
          <code>{wallet.address}</code>
          <CopyButton size="sm" variant="ghost" value={wallet.address} label="Copy wallet address">
            Copy
          </CopyButton>
        </div>
        {balance.error ? <p className={styles.balanceError}>{balance.error}</p> : undefined}
        <div className={styles.heroActions}>
          <Button variant="primary" leadingIcon={<ArrowUpRight size={16} />} onClick={onSend}>
            Send
          </Button>
          <Button variant="primary" leadingIcon={<ScanLine size={16} />} onClick={onReceive}>
            Receive
          </Button>
          <CopyButton variant="outline" value={wallet.address}>
            Copy address
          </CopyButton>
          <Button
            variant="ghost"
            size="icon"
            aria-label="Refresh balance"
            title="Refresh balance"
            disabled={balance.isLoading}
            onClick={onRefreshBalance}
          >
            <RefreshCw size={17} className={balance.isLoading ? styles.spinning : undefined} />
          </Button>
        </div>
      </section>

      <div className={styles.dashboardGrid}>
        <section className={styles.ledger}>
          <header className={styles.sectionHeader}>
            <div>
              <span className={styles.sectionIndex}>01</span>
              <h2>Assets</h2>
            </div>
            <button type="button" aria-label="Asset options">
              <MoreHorizontal size={17} />
            </button>
          </header>
          <div className={styles.assetTableHeader}>
            <span>Asset</span>
            <span>Network</span>
            <span>Balance</span>
          </div>
          <div className={styles.assetRow}>
            <span className={styles.assetIdentity}>
              <GramLogo className={styles.gramLogo} />
              <span>
                <strong>Gram</strong>
                <small>GRAM</small>
              </span>
            </span>
            <span className={styles.assetNetwork}>
              <span data-network={wallet.network} />
              {network.name}
            </span>
            <strong className={styles.assetBalance}>{formattedBalance}</strong>
          </div>
          <div className={styles.ledgerEmpty}>
            Jettons and NFTs will appear here after they are detected on-chain.
          </div>
        </section>

        <aside className={styles.identityPanel}>
          <header className={styles.sectionHeader}>
            <div>
              <span className={styles.sectionIndex}>02</span>
              <h2>Contract identity</h2>
            </div>
            <LockKeyhole size={16} />
          </header>
          <dl className={styles.identityList}>
            <div>
              <dt>Contract</dt>
              <dd>{wallet.version === "v5r1" ? "Wallet V5R1" : "Wallet V4R2"}</dd>
            </div>
            <div>
              <dt>Network ID</dt>
              <dd>{network.chainId}</dd>
            </div>
            <div>
              <dt>Public key</dt>
              <dd title={wallet.publicKey}>{shortenAddress(wallet.publicKey, 9)}</dd>
            </div>
            <div>
              <dt>Custody</dt>
              <dd>{getVaultKind() === "native" ? "Device protected" : "Test funds only"}</dd>
            </div>
          </dl>
          <a
            className={styles.explorerLink}
            href={getAccountExplorerUrl(wallet)}
            target="_blank"
            rel="noreferrer"
            onClick={event => {
              event.preventDefault()
              onOpenExplorer()
            }}
          >
            Open account in explorer
            <ArrowUpRight size={15} />
          </a>
          <button type="button" className={styles.removeAction} onClick={onRemove}>
            <Trash2 size={14} />
            Remove from this device
          </button>
        </aside>
      </div>
      <WalletActivity wallet={wallet} refreshToken={activityRefreshToken} />
    </div>
  )
}

function EmptyWalletState({
  isLoading,
  onCreate,
  onImport,
}: {
  readonly isLoading: boolean
  readonly onCreate: () => void
  readonly onImport: () => void
}) {
  return (
    <div className={styles.emptyState}>
      <div className={styles.emptyGrid} aria-hidden="true">
        <span />
        <span />
        <span />
        <span />
      </div>
      <div className={styles.emptyKicker}>
        <KeyRound size={15} />
        TON developer wallet
      </div>
      <h1>
        A TON wallet built
        <br />
        for <em>contract work.</em>
      </h1>
      <p>
        Create an identity for test deployments, contract calls, and TON Connect sessions. Start on
        testnet, then add a mainnet wallet when you are ready.
      </p>
      <div className={styles.emptyActions}>
        <Button
          size="lg"
          variant="primary"
          loading={isLoading}
          leadingIcon={<Plus size={17} />}
          onClick={onCreate}
        >
          Create wallet
        </Button>
        <Button size="lg" variant="outline" leadingIcon={<KeyRound size={17} />} onClick={onImport}>
          Import mnemonic
        </Button>
      </div>
      <div className={styles.emptyMeta}>
        <span>
          <Check size={14} /> V5R1 + V4R2
        </span>
        <span>
          <Check size={14} /> Mainnet + Testnet
        </span>
        <span>
          <Check size={14} /> Native keychain
        </span>
      </div>
    </div>
  )
}

function RecoveryDialog({
  words,
  onClose,
}: {
  readonly words?: readonly string[]
  readonly onClose: () => void
}) {
  const [confirmed, setConfirmed] = useState(false)

  useEffect(() => {
    if (words) {
      setConfirmed(false)
    }
  }, [words])

  if (!words) {
    return null
  }

  return (
    <Dialog
      open={true}
      title="Back up the recovery phrase"
      description="Keep these words offline and in the exact order shown. You will need them to recover the wallet."
      leadingIcon={<ShieldCheck size={20} />}
      maxWidth={580}
      dismissible={false}
      onOpenChange={() => undefined}
    >
      <div className={styles.recoveryBody}>
        <div className={styles.wordGrid}>
          {words.map((word, index) => (
            <span key={`${index}-${word}`}>
              <small>{String(index + 1).padStart(2, "0")}</small>
              <strong>{word}</strong>
            </span>
          ))}
        </div>
        <CopyButton value={words.join(" ")} variant="outline">
          Copy recovery phrase
        </CopyButton>
        <label className={styles.confirmBackup}>
          <input
            type="checkbox"
            checked={confirmed}
            onChange={event => setConfirmed(event.target.checked)}
          />
          <span>I saved the phrase in a secure place.</span>
        </label>
        <Button variant="primary" disabled={!confirmed} onClick={onClose}>
          Finish setup
        </Button>
      </div>
    </Dialog>
  )
}

function ReceiveDialog({
  wallet,
  onClose,
}: {
  readonly wallet?: WalletRecord
  readonly onClose: () => void
}) {
  if (!wallet) {
    return null
  }

  return (
    <Dialog
      open={true}
      title={`Receive on ${WALLET_NETWORKS[wallet.network].name}`}
      description="Share this address with the sender. Verify the network before transferring funds."
      leadingIcon={<ScanLine size={20} />}
      maxWidth={440}
      onOpenChange={() => onClose()}
    >
      <div className={styles.receiveBody}>
        <div className={styles.qrFrame}>
          <QRCodeSVG
            value={`ton://transfer/${wallet.address}`}
            size={184}
            level="M"
            marginSize={1}
          />
        </div>
        <code>{wallet.address}</code>
        <CopyButton value={wallet.address} variant="primary">
          Copy address
        </CopyButton>
      </div>
    </Dialog>
  )
}

function RemoveWalletDialog({
  wallet,
  onClose,
  onConfirm,
}: {
  readonly wallet?: WalletRecord
  readonly onClose: () => void
  readonly onConfirm: () => void
}) {
  if (!wallet) {
    return null
  }

  return (
    <Dialog
      open={true}
      title={`Remove ${wallet.name}?`}
      description="The wallet metadata and its system keychain entry will be removed from this device."
      leadingIcon={<Trash2 size={20} />}
      maxWidth={440}
      onOpenChange={() => onClose()}
    >
      <div className={styles.removeDialogBody}>
        <p>
          This does not affect the on-chain account. Make sure you have the recovery phrase before
          continuing.
        </p>
        <div>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="danger" onClick={onConfirm}>
            Remove wallet
          </Button>
        </div>
      </div>
    </Dialog>
  )
}

function getErrorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback
}
