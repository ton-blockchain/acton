import {Check, Copy, KeyRound, Shield, X} from "lucide-react"
import * as React from "react"
import {Button, useToast} from "@acton/shared-ui"
import {
  formatUnits,
  type ConnectionRequestEvent,
  type RequestErrorEvent,
  type SendTransactionRequestEvent,
  type SignDataRequestEvent,
  type TONConnectSession,
} from "@ton/walletkit"

import styles from "../dashboard/pages/WalletsPage.module.css"
import type {TonClient} from "../explorer/api/client"
import type {StartupWallet} from "../explorer/api/types"
import {formatAddress, normalizeAddress} from "../explorer/components/utils"
import {useAddressFormat} from "../explorer/hooks/useNetworkInfo"

import {addStartupWalletToKit, createWalletKit, getWalletNetworkLabel} from "./kit"
import type {RuntimeWallet, StartupWalletRecord} from "./types"
import {isSupportedWalletVersion} from "./types"
import {useTonConnectPasteHandler} from "./useTonConnectPasteHandler"
import {
  WalletRuntimeContext,
  type WalletBalanceState,
  type WalletRuntimeContextValue,
} from "./useWalletRuntime"

type WalletBalanceResult =
  | {readonly id: string; readonly balance: string}
  | {readonly id: string; readonly error: string}

interface WalletRuntimeProviderProps {
  readonly client: TonClient
  readonly host: string
  readonly children: React.ReactNode
}

export const WalletRuntimeProvider: React.FC<WalletRuntimeProviderProps> = ({
  client,
  host,
  children,
}) => {
  const {showToast} = useToast()
  const addressFormat = useAddressFormat()
  const [startupWallets, setStartupWallets] = React.useState<StartupWallet[]>([])
  const [walletKit, setWalletKit] = React.useState<ReturnType<typeof createWalletKit>>()
  const [runtimeWallets, setRuntimeWallets] = React.useState<RuntimeWallet[]>([])
  const [sessions, setSessions] = React.useState<TONConnectSession[]>([])
  const [isLoadingWallets, setIsLoadingWallets] = React.useState(true)
  const [isInitializing, setIsInitializing] = React.useState(true)
  const [isSyncingWallets, setIsSyncingWallets] = React.useState(false)
  const [isSubmitting, setIsSubmitting] = React.useState(false)
  const [isRefreshingBalances, setIsRefreshingBalances] = React.useState(false)
  const [walletBalances, setWalletBalances] = React.useState<Record<string, WalletBalanceState>>({})
  const [copiedAddress, setCopiedAddress] = React.useState<string>()
  const [tonConnectUrl, setTonConnectUrl] = React.useState("")
  const [selectedConnectWalletId, setSelectedConnectWalletId] = React.useState<string>()
  const [pendingConnectRequest, setPendingConnectRequest] = React.useState<ConnectionRequestEvent>()
  const [pendingTransactionRequest, setPendingTransactionRequest] =
    React.useState<SendTransactionRequestEvent>()
  const [pendingSignDataRequest, setPendingSignDataRequest] = React.useState<SignDataRequestEvent>()

  const supportedWallets = React.useMemo(
    () =>
      startupWallets.filter((wallet): wallet is StartupWalletRecord =>
        isSupportedWalletVersion(wallet.version),
      ),
    [startupWallets],
  )
  const unsupportedWallets = React.useMemo(
    () => startupWallets.filter(wallet => !isSupportedWalletVersion(wallet.version)),
    [startupWallets],
  )
  const selectedConnectWallet =
    runtimeWallets.find(wallet => wallet.id === selectedConnectWalletId) ?? runtimeWallets[0]

  const showErrorToast = React.useCallback(
    (title: string, error: unknown, fallback: string) => {
      showToast({
        variant: "error",
        title,
        description: getErrorMessage(error, fallback),
      })
    },
    [showToast],
  )

  const showStaleRequestToast = React.useCallback(
    (title: string) => {
      showToast({
        variant: "error",
        title,
        description: "TON Connect session is no longer active. Closed the request locally.",
      })
    },
    [showToast],
  )

  const refreshSessions = React.useCallback(
    async (kit = walletKit) => {
      if (!kit) {
        return
      }

      setSessions(await kit.listSessions())
    },
    [walletKit],
  )

  const refreshWalletBalances = React.useCallback(
    async (wallets: readonly RuntimeWallet[] = runtimeWallets) => {
      if (wallets.length === 0) {
        setWalletBalances({})
        return
      }

      setIsRefreshingBalances(true)
      setWalletBalances(current => {
        const nextBalances: Record<string, WalletBalanceState> = {}
        for (const runtimeWallet of wallets) {
          const previous = current[runtimeWallet.id]
          nextBalances[runtimeWallet.id] = {
            value: previous?.value,
            isLoading: true,
            error: previous?.error,
          }
        }
        return nextBalances
      })

      const results: WalletBalanceResult[] = await Promise.all(
        wallets.map(async runtimeWallet => {
          try {
            return {
              id: runtimeWallet.id,
              balance: await runtimeWallet.wallet.getBalance(),
            }
          } catch (error) {
            return {
              id: runtimeWallet.id,
              error: getErrorMessage(error, "Failed to refresh wallet balance."),
            }
          }
        }),
      )

      setWalletBalances(current => {
        const nextBalances: Record<string, WalletBalanceState> = {}
        for (const result of results) {
          const previous = current[result.id]
          nextBalances[result.id] =
            "balance" in result
              ? {
                  value: result.balance,
                  isLoading: false,
                }
              : {
                  value: previous?.value,
                  isLoading: false,
                  error: result.error,
                }
        }
        return nextBalances
      })

      const failedResults = results.filter(
        (result): result is {readonly id: string; readonly error: string} => "error" in result,
      )
      if (failedResults.length > 0) {
        showToast({
          variant: "error",
          title: "Balance refresh failed",
          description:
            failedResults.length === 1
              ? failedResults[0].error
              : `Failed to refresh ${failedResults.length} wallet balances.`,
        })
      }

      setIsRefreshingBalances(false)
    },
    [runtimeWallets, showToast],
  )

  React.useEffect(() => {
    void refreshWalletBalances(runtimeWallets)
  }, [refreshWalletBalances, runtimeWallets])

  React.useEffect(() => {
    let cancelled = false

    void (async () => {
      setIsLoadingWallets(true)
      try {
        const wallets = await client.getStartupWallets()
        if (!cancelled) {
          setStartupWallets(wallets)
        }
      } catch (error) {
        if (!cancelled) {
          showErrorToast("Failed to load wallets", error, "Failed to load startup wallets.")
          setStartupWallets([])
        }
      } finally {
        if (!cancelled) {
          setIsLoadingWallets(false)
        }
      }
    })()

    return () => {
      cancelled = true
    }
  }, [client, showErrorToast])

  React.useEffect(() => {
    let cancelled = false
    const nextWalletKit = createWalletKit(host)

    const handleRequestError = (event: RequestErrorEvent) => {
      const fallback = "WalletKit request failed."
      showToast({
        variant: "error",
        title: "Wallet request failed",
        description: event.error?.message || fallback,
      })
    }

    const initialize = async () => {
      try {
        await nextWalletKit.ensureInitialized()

        if (cancelled) {
          await nextWalletKit.close()
          return
        }

        nextWalletKit.onConnectRequest(event => setPendingConnectRequest(event))
        nextWalletKit.onTransactionRequest(event => setPendingTransactionRequest(event))
        nextWalletKit.onSignDataRequest(event => setPendingSignDataRequest(event))
        nextWalletKit.onDisconnect(() => {
          showToast({
            variant: "info",
            title: "Session disconnected",
            description: "Session disconnected.",
          })
          void nextWalletKit.listSessions().then(setSessions)
        })
        nextWalletKit.onRequestError(handleRequestError)

        setWalletKit(nextWalletKit)
        setSessions(await nextWalletKit.listSessions())
      } catch (error) {
        if (!cancelled) {
          showErrorToast("Wallet runtime failed", error, "Failed to initialize wallet runtime.")
        }
      } finally {
        if (!cancelled) {
          setIsInitializing(false)
        }
      }
    }

    void initialize()

    return () => {
      cancelled = true
      void nextWalletKit.close()
    }
  }, [host, showErrorToast, showToast])

  React.useEffect(() => {
    if (!walletKit) {
      return
    }

    let cancelled = false

    const syncWallets = async () => {
      setIsSyncingWallets(true)

      try {
        const nextRuntimeWallets: RuntimeWallet[] = []
        for (const walletRecord of supportedWallets) {
          const wallet = await addStartupWalletToKit(walletKit, walletRecord)
          if (wallet) {
            nextRuntimeWallets.push({
              id: wallet.getWalletId(),
              record: walletRecord,
              wallet,
            })
          }
        }

        if (!cancelled) {
          setRuntimeWallets(nextRuntimeWallets)
          await refreshSessions(walletKit)
        }
      } catch (error) {
        if (!cancelled) {
          showErrorToast(
            "Wallet sync failed",
            error,
            "Failed to load startup wallets into WalletKit.",
          )
        }
      } finally {
        if (!cancelled) {
          setIsSyncingWallets(false)
        }
      }
    }

    void syncWallets()

    return () => {
      cancelled = true
    }
  }, [refreshSessions, showErrorToast, supportedWallets, walletKit])

  React.useEffect(() => {
    if (!pendingConnectRequest) {
      setSelectedConnectWalletId(undefined)
      return
    }

    setSelectedConnectWalletId(previous =>
      previous && runtimeWallets.some(wallet => wallet.id === previous)
        ? previous
        : runtimeWallets[0]?.id,
    )
  }, [pendingConnectRequest, runtimeWallets])

  React.useEffect(() => {
    if (!copiedAddress) {
      return
    }

    const timeoutId = globalThis.setTimeout(() => setCopiedAddress(undefined), 2000)
    return () => globalThis.clearTimeout(timeoutId)
  }, [copiedAddress])

  const handleConnectUrl = React.useCallback(
    async (url: string) => {
      if (!walletKit) {
        showToast({
          variant: "error",
          title: "Wallet runtime unavailable",
          description: "Wallet runtime is still initializing.",
        })
        return
      }
      if (runtimeWallets.length === 0) {
        showToast({
          variant: "error",
          title: "No wallets available",
          description: "No startup wallets are available for TON Connect.",
        })
        return
      }

      setIsSubmitting(true)
      try {
        await walletKit.handleTonConnectUrl(url.trim())
        setTonConnectUrl("")
        showToast({
          variant: "success",
          title: "TON Connect request received",
          description: "Review and approve the request in Acton.",
        })
      } catch (error) {
        showErrorToast("TON Connect failed", error, "Failed to process TON Connect URL.")
      } finally {
        setIsSubmitting(false)
      }
    },
    [runtimeWallets.length, showErrorToast, showToast, walletKit],
  )

  useTonConnectPasteHandler(handleConnectUrl)

  const handleApproveConnect = React.useCallback(async () => {
    if (!walletKit || !pendingConnectRequest || !selectedConnectWallet) {
      return
    }

    setIsSubmitting(true)
    try {
      await walletKit.approveConnectRequest({
        ...pendingConnectRequest,
        walletAddress: selectedConnectWallet.record.address,
        walletId: selectedConnectWallet.id,
      })
      setPendingConnectRequest(undefined)
      showToast({
        variant: "success",
        title: "Connected",
        description: `Connected ${getDappName(pendingConnectRequest.preview.dAppInfo?.name)} to ${selectedConnectWallet.record.name}.`,
      })
      await refreshSessions()
    } catch (error) {
      if (isStaleTonConnectRequest(error)) {
        setPendingConnectRequest(undefined)
        showStaleRequestToast("Connection request expired")
        await refreshSessions()
      } else {
        showErrorToast("Connection failed", error, "Failed to approve connection request.")
      }
    } finally {
      setIsSubmitting(false)
    }
  }, [
    pendingConnectRequest,
    refreshSessions,
    selectedConnectWallet,
    showErrorToast,
    showStaleRequestToast,
    showToast,
    walletKit,
  ])

  const handleRejectConnect = React.useCallback(async () => {
    if (!walletKit || !pendingConnectRequest) {
      return
    }

    setIsSubmitting(true)
    try {
      await walletKit.rejectConnectRequest(pendingConnectRequest, "User rejected the connection")
      setPendingConnectRequest(undefined)
      showToast({
        variant: "info",
        title: "Connection rejected",
        description: "Connection request rejected.",
      })
    } catch (error) {
      if (isStaleTonConnectRequest(error)) {
        setPendingConnectRequest(undefined)
        showStaleRequestToast("Connection request expired")
        await refreshSessions()
      } else {
        showErrorToast("Reject failed", error, "Failed to reject connection request.")
      }
    } finally {
      setIsSubmitting(false)
    }
  }, [
    pendingConnectRequest,
    refreshSessions,
    showErrorToast,
    showStaleRequestToast,
    showToast,
    walletKit,
  ])

  const handleApproveTransaction = React.useCallback(async () => {
    if (!walletKit || !pendingTransactionRequest) {
      return
    }

    setIsSubmitting(true)
    try {
      await walletKit.approveTransactionRequest(pendingTransactionRequest)
      setPendingTransactionRequest(undefined)
      showToast({
        variant: "success",
        title: "Transaction approved",
        description: "Transaction request approved.",
      })
      await refreshWalletBalances()
    } catch (error) {
      if (isStaleTonConnectRequest(error)) {
        setPendingTransactionRequest(undefined)
        showStaleRequestToast("Transaction request expired")
        await refreshSessions()
      } else {
        showErrorToast("Approval failed", error, "Failed to approve transaction request.")
      }
    } finally {
      setIsSubmitting(false)
    }
  }, [
    pendingTransactionRequest,
    refreshSessions,
    refreshWalletBalances,
    showErrorToast,
    showStaleRequestToast,
    showToast,
    walletKit,
  ])

  const handleRejectTransaction = React.useCallback(async () => {
    if (!walletKit || !pendingTransactionRequest) {
      return
    }

    setIsSubmitting(true)
    try {
      await walletKit.rejectTransactionRequest(
        pendingTransactionRequest,
        "User rejected the transaction",
      )
      setPendingTransactionRequest(undefined)
      showToast({
        variant: "info",
        title: "Transaction rejected",
        description: "Transaction request rejected.",
      })
    } catch (error) {
      if (isStaleTonConnectRequest(error)) {
        setPendingTransactionRequest(undefined)
        showStaleRequestToast("Transaction request expired")
        await refreshSessions()
      } else {
        showErrorToast("Reject failed", error, "Failed to reject transaction request.")
      }
    } finally {
      setIsSubmitting(false)
    }
  }, [
    pendingTransactionRequest,
    refreshSessions,
    showErrorToast,
    showStaleRequestToast,
    showToast,
    walletKit,
  ])

  const handleApproveSignData = React.useCallback(async () => {
    if (!walletKit || !pendingSignDataRequest) {
      return
    }

    setIsSubmitting(true)
    try {
      await walletKit.approveSignDataRequest(pendingSignDataRequest)
      setPendingSignDataRequest(undefined)
      showToast({
        variant: "success",
        title: "Sign request approved",
        description: "Sign request approved.",
      })
    } catch (error) {
      if (isStaleTonConnectRequest(error)) {
        setPendingSignDataRequest(undefined)
        showStaleRequestToast("Sign request expired")
        await refreshSessions()
      } else {
        showErrorToast("Sign failed", error, "Failed to approve sign request.")
      }
    } finally {
      setIsSubmitting(false)
    }
  }, [
    pendingSignDataRequest,
    refreshSessions,
    showErrorToast,
    showStaleRequestToast,
    showToast,
    walletKit,
  ])

  const handleRejectSignData = React.useCallback(async () => {
    if (!walletKit || !pendingSignDataRequest) {
      return
    }

    setIsSubmitting(true)
    try {
      await walletKit.rejectSignDataRequest(
        pendingSignDataRequest,
        "User rejected the sign request",
      )
      setPendingSignDataRequest(undefined)
      showToast({
        variant: "info",
        title: "Sign request rejected",
        description: "Sign request rejected.",
      })
    } catch (error) {
      if (isStaleTonConnectRequest(error)) {
        setPendingSignDataRequest(undefined)
        showStaleRequestToast("Sign request expired")
        await refreshSessions()
      } else {
        showErrorToast("Reject failed", error, "Failed to reject sign request.")
      }
    } finally {
      setIsSubmitting(false)
    }
  }, [
    pendingSignDataRequest,
    refreshSessions,
    showErrorToast,
    showStaleRequestToast,
    showToast,
    walletKit,
  ])

  const handleDisconnectSession = React.useCallback(
    async (sessionId: string) => {
      if (!walletKit) {
        return
      }

      setIsSubmitting(true)
      try {
        await walletKit.disconnect(sessionId)
        await refreshSessions()
        showToast({
          variant: "info",
          title: "Session disconnected",
          description: "Session disconnected.",
        })
      } catch (error) {
        showErrorToast("Disconnect failed", error, "Failed to disconnect session.")
      } finally {
        setIsSubmitting(false)
      }
    },
    [refreshSessions, showErrorToast, showToast, walletKit],
  )

  const handleCopyAddress = React.useCallback(
    async (address: string) => {
      try {
        await navigator.clipboard.writeText(address)
        setCopiedAddress(address)
      } catch (error) {
        showErrorToast("Copy failed", error, "Failed to copy address.")
      }
    },
    [showErrorToast],
  )

  const pendingRequestCount =
    Number(Boolean(pendingConnectRequest)) +
    Number(Boolean(pendingTransactionRequest)) +
    Number(Boolean(pendingSignDataRequest))

  const value = React.useMemo<WalletRuntimeContextValue>(
    () => ({
      host,
      runtimeWallets,
      unsupportedWallets,
      sessions,
      walletBalances,
      copiedAddress,
      tonConnectUrl,
      isLoadingWallets,
      isInitializing,
      isSyncingWallets,
      isSubmitting,
      isRefreshingBalances,
      pendingRequestCount,
      setTonConnectUrl,
      handleConnectUrl,
      refreshWalletBalances,
      handleDisconnectSession,
      handleCopyAddress,
    }),
    [
      copiedAddress,
      handleConnectUrl,
      handleCopyAddress,
      handleDisconnectSession,
      host,
      isInitializing,
      isLoadingWallets,
      isRefreshingBalances,
      isSubmitting,
      isSyncingWallets,
      pendingRequestCount,
      refreshWalletBalances,
      runtimeWallets,
      sessions,
      tonConnectUrl,
      unsupportedWallets,
      walletBalances,
    ],
  )

  return (
    <WalletRuntimeContext.Provider value={value}>
      {children}
      {pendingConnectRequest && (
        <ModalShell
          title="Connection Request"
          subtitle={`${getDappName(pendingConnectRequest.preview.dAppInfo?.name)} wants to connect.`}
          onDismiss={() => setPendingConnectRequest(undefined)}
        >
          <div className={styles.permissionsList}>
            {pendingConnectRequest.preview.permissions.map((permission, index) => (
              <div
                key={`${permission.name ?? "permission"}-${index}`}
                className={styles.permissionItem}
              >
                <Shield size={15} />
                <div>
                  <div className={styles.permissionTitle}>
                    {permission.title ?? permission.name ?? "Permission"}
                  </div>
                  <div className={styles.permissionDescription}>
                    {permission.description ?? "Requested by the dApp during connect."}
                  </div>
                </div>
              </div>
            ))}
          </div>

          <div className={styles.walletPicker}>
            <span className={styles.label}>Connect with</span>
            {runtimeWallets.map(wallet => {
              const isSelected = wallet.id === selectedConnectWallet?.id
              const walletAddress = normalizeAddress(wallet.record.address, addressFormat)
              return (
                <button
                  key={wallet.id}
                  type="button"
                  className={`${styles.pickerOption} ${isSelected ? styles.pickerOptionActive : ""}`}
                  onClick={() => setSelectedConnectWalletId(wallet.id)}
                >
                  <span>
                    <span className={styles.pickerTitle}>{wallet.record.name}</span>
                    <span className={styles.pickerSubtitle}>
                      {formatAddress(walletAddress, true, addressFormat)} ·{" "}
                      {getWalletNetworkLabel()}
                    </span>
                  </span>
                  <span className={styles.radio}>{isSelected && <Check size={14} />}</span>
                </button>
              )
            })}
          </div>

          <div className={styles.modalActions}>
            <Button
              variant="outline"
              onClick={() => void handleRejectConnect()}
              disabled={isSubmitting}
            >
              Reject
            </Button>
            <Button
              onClick={() => void handleApproveConnect()}
              disabled={!selectedConnectWallet || isSubmitting}
            >
              Connect
            </Button>
          </div>
        </ModalShell>
      )}

      {pendingTransactionRequest && (
        <ModalShell
          title="Transaction Request"
          subtitle={`${getDappName(pendingTransactionRequest.dAppInfo?.name)} wants this wallet to send a transaction.`}
          onDismiss={() => setPendingTransactionRequest(undefined)}
        >
          <div className={styles.requestSummary}>
            <MetaRow label="Messages">
              {String(pendingTransactionRequest.request.messages.length)}
            </MetaRow>
            <MetaRow label="Network">
              {pendingTransactionRequest.request.network?.chainId === "-239"
                ? "Mainnet"
                : "Testnet"}
            </MetaRow>
            <MetaRow label="Amount">
              {formatGramBalance(
                pendingTransactionRequest.request.messages
                  .reduce((sum, message) => sum + BigInt(message.amount), 0n)
                  .toString(),
              )}{" "}
              GRAM
            </MetaRow>
          </div>

          <div className={styles.requestMessages}>
            {pendingTransactionRequest.request.messages.map((message, index) => (
              <div key={`${message.address}-${index}`} className={styles.messageItem}>
                <span className={styles.messageIndex}>#{index + 1}</span>
                <div>
                  <CopyableAddress
                    address={normalizeAddress(message.address, addressFormat)}
                    copiedAddress={copiedAddress}
                    onCopy={handleCopyAddress}
                  />
                  <div className={styles.messageValue}>
                    {formatGramBalance(message.amount)} GRAM
                  </div>
                </div>
              </div>
            ))}
          </div>

          <div className={styles.modalActions}>
            <Button
              variant="outline"
              onClick={() => void handleRejectTransaction()}
              disabled={isSubmitting}
            >
              Reject
            </Button>
            <Button onClick={() => void handleApproveTransaction()} disabled={isSubmitting}>
              Approve
            </Button>
          </div>
        </ModalShell>
      )}

      {pendingSignDataRequest && (
        <ModalShell
          title="Sign Request"
          subtitle={`${getDappName(pendingSignDataRequest.preview.dAppInfo?.name)} wants a signature.`}
          onDismiss={() => setPendingSignDataRequest(undefined)}
        >
          <div className={styles.messageItem}>
            <KeyRound size={16} />
            <div>
              <div className={styles.messageAddress}>
                {pendingSignDataRequest.preview.data.type.toUpperCase()}
              </div>
              <div className={styles.permissionDescription}>
                {describeSignPreview(pendingSignDataRequest.preview.data)}
              </div>
            </div>
          </div>

          <div className={styles.modalActions}>
            <Button
              variant="outline"
              onClick={() => void handleRejectSignData()}
              disabled={isSubmitting}
            >
              Reject
            </Button>
            <Button onClick={() => void handleApproveSignData()} disabled={isSubmitting}>
              Sign
            </Button>
          </div>
        </ModalShell>
      )}
    </WalletRuntimeContext.Provider>
  )
}

interface ModalShellProps {
  readonly title: string
  readonly subtitle: string
  readonly onDismiss: () => void
  readonly children: React.ReactNode
}

const ModalShell: React.FC<ModalShellProps> = ({title, subtitle, onDismiss, children}) => (
  <div className={styles.modalBackdrop}>
    <div className={styles.modalCard}>
      <div className={styles.modalHeader}>
        <div className={styles.modalTitleRow}>
          <h3 className={styles.modalTitle}>{title}</h3>
          <button
            type="button"
            className={styles.modalCloseButton}
            onClick={onDismiss}
            aria-label="Close request"
          >
            <X size={16} />
          </button>
        </div>
        <p className={styles.modalSubtitle}>{subtitle}</p>
      </div>
      <div className={styles.modalContent}>{children}</div>
    </div>
  </div>
)

interface MetaRowProps {
  readonly label: string
  readonly children: React.ReactNode
}

const MetaRow: React.FC<MetaRowProps> = ({label, children}) => (
  <div className={styles.metaRow}>
    <span className={styles.metaLabel}>{label}</span>
    <span className={styles.metaValue}>{children}</span>
  </div>
)

interface CopyableAddressProps {
  readonly address: string
  readonly copiedAddress?: string
  readonly onCopy: (address: string) => Promise<void>
}

const CopyableAddress: React.FC<CopyableAddressProps> = ({address, copiedAddress, onCopy}) => {
  const isCopied = copiedAddress === address

  return (
    <div className={styles.copyableAddress}>
      <span className={styles.copyableAddressText} title={address}>
        {shortenAddress(address, 14)}
      </span>
      <button
        type="button"
        className={`${styles.addressCopyButton} ${isCopied ? styles.addressCopyButtonCopied : ""}`}
        onClick={() => void onCopy(address)}
        aria-label={isCopied ? "Address copied" : "Copy address"}
      >
        {isCopied ? <Check size={14} /> : <Copy size={14} />}
      </button>
    </div>
  )
}

function shortenAddress(address: string, visibleChars: number): string {
  if (address.length <= visibleChars * 2) {
    return address
  }

  return `${address.slice(0, visibleChars)}...${address.slice(-visibleChars)}`
}

function formatGramBalance(balance: string): string {
  return formatUnits(balance, 9)
}

function getDappName(name: string | undefined): string {
  return name && name.trim().length > 0 ? name : "Unknown dApp"
}

function getErrorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message.length > 0 ? error.message : fallback
}

function isStaleTonConnectRequest(error: unknown): boolean {
  return /session not found/i.test(getErrorMessage(error, ""))
}

function describeSignPreview(preview: SignDataRequestEvent["preview"]["data"]): string {
  switch (preview.type) {
    case "text": {
      return preview.value.content
    }
    case "binary": {
      return `${preview.value.content.length} base64 chars`
    }
    case "cell": {
      return preview.value.schema || "TON Cell payload"
    }
    default: {
      return "Unknown sign payload"
    }
  }
}
