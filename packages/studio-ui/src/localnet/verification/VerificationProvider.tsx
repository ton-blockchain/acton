import {
  AddressChip,
  Button,
  ByteSize,
  Disclosure,
  Dialog,
  DialogActions,
  GramAmount,
  Select,
  SourceLocationValue,
  TechnicalValue,
  useToast,
} from "@acton/ui"
import {getNormalizedExtMessageHash, SendModeFlag, type Base64String} from "@ton/walletkit"
import {Check, ExternalLink, ShieldCheck} from "lucide-react"
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react"

import {normalizeCodeHash} from "@acton/explorer-core/metadata/codeHash"
import {fetchStudioInfo, StudioRequestError} from "../../studioApi"
import {useLocalnetRuntime} from "../LocalnetRuntimeProvider"
import {TonConnectWalletSelector} from "../wallet/TonConnectRequestDialog"
import {useOptionalWalletRuntime, type WalletRuntimeContextValue} from "../wallet/useWalletRuntime"
import {WalletRuntimeProvider} from "../wallet/WalletRuntimeProvider"
import {
  verificationApi,
  type VerificationOperation,
  type VerificationPreview,
  type VerificationStatus,
} from "./api"

import styles from "./VerificationProvider.module.css"

type Phase =
  | "preview"
  | "review"
  | "preparing"
  | "ready"
  | "sending"
  | "confirming"
  | "pending"
  | "expired"
  | "verified"
  | "failed"

interface VerificationFlow {
  readonly codeHash: string
  readonly phase: Phase
  readonly preview?: VerificationPreview
  readonly operation?: VerificationOperation
  readonly walletId?: string
  readonly contractId?: string
  readonly paymentMessageHash?: string
}

interface VerificationContextValue {
  readonly available: boolean
  readonly statuses: Readonly<Record<string, VerificationStatus>>
  readonly check: (codeHash: string) => Promise<void>
  readonly open: (codeHash: string) => void
}

const VerificationContext = createContext<VerificationContextValue | undefined>(undefined)
const progressLabels = {
  uploadingSources: "Uploading sources",
  confirmingPayment: "Waiting for finalized Testnet payment",
  ready: "Ready for Testnet payment",
  verified: "Source verified",
  failed: "Verification stopped",
} as const

/** Retains paid attempts across navigation and reloads while the Studio server owns their sources */
export function VerificationProvider({children}: {readonly children: ReactNode}) {
  const {environment} = useLocalnetRuntime()
  const environmentWallets = useOptionalWalletRuntime()
  const [testnetWallets, setTestnetWallets] = useState<WalletRuntimeContextValue>()
  const wallets = environment?.network.id === "testnet" ? environmentWallets : testnetWallets
  const {showToast, updateToast, dismissToast} = useToast()
  const api = useMemo(() => verificationApi(environment?.id ?? ""), [environment?.id])
  const [hasProject, setHasProject] = useState(false)
  const [statuses, setStatuses] = useState<Record<string, VerificationStatus>>({})
  const storagePrefix = `acton:verification:${environment?.id}`
  const attempts = useMemo(() => {
    const saved = new Map<string, {key: string; flow: VerificationFlow}>()

    // Use the reviewed code hash to restore paid attempts, regardless of their original instance.
    for (const key of Object.keys(sessionStorage).filter(key => key.startsWith(storagePrefix))) {
      try {
        const attempt = JSON.parse(sessionStorage.getItem(key) ?? "null") as VerificationFlow | null
        if (!attempt?.paymentMessageHash || !attempt.operation || !attempt.preview) continue

        const phase = ["expired", "failed", "review"].includes(attempt.phase)
          ? attempt.phase
          : "pending"
        const codeHash = normalizeCodeHash(attempt.preview.status.codeHash)
        if (!codeHash) continue
        saved.set(codeHash, {key, flow: {...attempt, codeHash, phase}})
      } catch {
        // A malformed saved entry must not prevent opening another contract.
      }
    }

    return saved
  }, [storagePrefix])
  const [flow, updateFlow] = useState<VerificationFlow>()
  const [open, setOpen] = useState(false)
  const [contractId, setContractId] = useState("")
  const [walletId, setWalletId] = useState("")
  const activeRequest = useRef(false)
  const progressToastId = useRef<string | undefined>(undefined)
  const alive = useRef(true)
  const checked = useRef(new Set<string>())
  const available = hasProject && environment?.config.kind === "remoteTonNetwork"

  const beginProgress = (title: string) => {
    if (progressToastId.current) dismissToast(progressToastId.current)
    const id = showToast({variant: "loading", title, durationMs: 0})
    progressToastId.current = id
    return id
  }

  const setFlow = (next: VerificationFlow) => {
    const codeHash = next.codeHash
    const key = attempts.get(codeHash)?.key ?? `${storagePrefix}:${codeHash}`
    attempts.set(codeHash, {key, flow: next})

    // Save the reference before broadcast, so even a lost response cannot lead to a second charge.
    if (next.paymentMessageHash && next.phase !== "verified") {
      sessionStorage.setItem(key, JSON.stringify(next))
    } else {
      sessionStorage.removeItem(key)
    }
    if (alive.current) updateFlow(next)
  }

  useEffect(() => {
    alive.current = true
    const controller = new AbortController()

    void fetchStudioInfo(controller.signal)
      .then(info => setHasProject(Boolean(info.workspace)))
      .catch(() => undefined)

    return () => {
      alive.current = false
      controller.abort()
      if (progressToastId.current) dismissToast(progressToastId.current)
    }
  }, [dismissToast])

  const reportError = useCallback(
    (error: unknown, toastId?: string, retry?: {label: string; run: () => void}) => {
      const feedback = {
        variant: "error" as const,
        title: "Source verification failed",
        description: (
          <span className={styles.feedback}>
            <span>
              {error instanceof Error ? error.message : "Unable to verify the contract source"}
            </span>
            {retry && (
              <Button size="sm" variant="outline" onClick={retry.run}>
                {retry.label}
              </Button>
            )}
          </span>
        ),
        durationMs: retry ? 0 : 8000,
      }
      if (toastId) updateToast(toastId, feedback)
      else showToast(feedback)
    },
    [showToast, updateToast],
  )

  const saveStatus = useCallback((status: VerificationStatus) => {
    setStatuses(current => ({...current, [status.codeHash]: status}))
  }, [])

  const check = useCallback(
    async (codeHash: string) => {
      const key = codeHash
      if (!available || checked.current.has(key)) return
      checked.current.add(key)

      try {
        const status = await api.status(codeHash)
        if (alive.current) saveStatus(status)
      } catch (error) {
        checked.current.delete(key)
        if (alive.current) reportError(error)
      }
    },
    [api, available, reportError, saveStatus],
  )

  const preview = async (codeHash: string, previous?: VerificationFlow) => {
    if (activeRequest.current) {
      return
    }

    activeRequest.current = true
    setFlow({...previous, codeHash, phase: "preview"})
    setOpen(true)
    const toastId = beginProgress("Checking code hash and project sources")

    try {
      const result = await api.preview(codeHash)
      if (!alive.current) return

      // Bind renewed sources to the same reviewed code before reusing its payment.
      if (normalizeCodeHash(result.status.codeHash) !== normalizeCodeHash(codeHash)) {
        throw new Error(
          "The preview returned a different code hash; the saved payment has not been used",
        )
      }

      saveStatus(result.status)
      if (result.status.verified) {
        markVerified({codeHash, phase: "verified", preview: result}, result.status, toastId)
        return
      }
      setContractId(
        result.candidates.find(candidate => candidate.matches)?.contractId ??
          result.candidates[0]?.contractId ??
          "",
      )
      setWalletId(previous?.walletId ?? wallets?.runtimeWallets[0]?.id ?? "")
      setFlow({...previous, codeHash, contractId: undefined, phase: "review", preview: result})
      setOpen(true)
      updateToast(toastId, {variant: "info", title: "Project sources checked", durationMs: 4000})
    } catch (error) {
      if (!alive.current) return
      setFlow({...previous, codeHash, phase: "failed"})
      reportError(error, toastId, {
        label: "Check sources again",
        run: () => void preview(codeHash, previous),
      })
    } finally {
      activeRequest.current = false
    }
  }

  const openVerification = (codeHash: string) => {
    if (
      activeRequest.current &&
      flow &&
      normalizeCodeHash(flow.codeHash) !== normalizeCodeHash(codeHash)
    ) {
      showToast({
        variant: "info",
        title: "A verification is running",
        description: "Wait for it to finish before starting another contract",
      })
      return
    }

    const saved = attempts.get(codeHash)?.flow
    if (saved && saved.phase !== "verified") {
      setFlow(saved)
      setContractId(
        saved.contractId ??
          saved.preview?.candidates.find(candidate => candidate.matches)?.contractId ??
          "",
      )
      setWalletId(saved.walletId ?? "")
      setOpen(true)
    } else {
      void preview(codeHash)
    }
  }

  const markVerified = (current: VerificationFlow, status: VerificationStatus, toastId: string) => {
    saveStatus(status)
    setFlow({...current, phase: "verified"})
    setOpen(false)
    updateToast(toastId, {
      variant: "success",
      title: "Source verified",
      description: (
        <a href={status.verifierUrl} target="_blank" rel="noreferrer">
          View in Acton Verifier
        </a>
      ),
      durationMs: 6000,
    })
  }

  const prepare = async () => {
    const wallet = wallets?.runtimeWallets.find(wallet => wallet.id === selectedWalletId)
    if (!flow?.preview || !wallet || activeRequest.current) return

    activeRequest.current = true
    let current = {...flow, contractId, walletId: selectedWalletId}
    setFlow({...current, phase: "preparing"})
    const toastId = beginProgress(
      current.paymentMessageHash ? "Restoring verification" : "Preparing Testnet payment",
    )

    try {
      const operation = await api.start(flow.preview.id, contractId, wallet.record.address)
      if (!alive.current) return
      if (operation.phase !== "ready" || !operation.message) {
        throw new Error(operation.error ?? "The verifier did not return a payment transaction")
      }
      current = {...current, operation}
      if (current.paymentMessageHash) {
        await confirm(current, toastId)
      } else {
        setFlow({...current, phase: "ready"})
        updateToast(toastId, {
          variant: "info",
          title: "Ready for Testnet payment",
          durationMs: 4000,
        })
      }
    } catch (error) {
      if (!alive.current) return
      handleFailure(current, error, toastId)
    } finally {
      activeRequest.current = false
    }
  }

  const confirm = async (current: VerificationFlow, toastId: string) => {
    if (!current.operation || !current.paymentMessageHash) {
      throw new Error(
        "The wallet did not return a payment reference; check its transactions before sending another payment",
      )
    }

    // Resubmitting the same reference resumes the server operation without paying again.
    updateToast(toastId, {
      title: progressLabels.confirmingPayment,
      variant: "loading",
      description: undefined,
      durationMs: 0,
    })
    let operation = await api.completePayment(current.operation.id, current.paymentMessageHash)
    let lastPhase: VerificationOperation["phase"] = "confirmingPayment"
    while (alive.current) {
      setFlow({...current, phase: "confirming", operation})
      if (operation.phase === "failed") throw new Error(operation.error ?? "Verification failed")
      if (operation.phase === "verified") {
        updateToast(toastId, {title: "Checking published verification"})
        const status = await api.status(current.codeHash)
        if (
          !status.verified ||
          normalizeCodeHash(status.codeHash) !== normalizeCodeHash(current.codeHash)
        ) {
          throw new Error(
            "The verifier returned a different code hash; verification could not be confirmed",
          )
        }
        markVerified(current, status, toastId)
        return
      }

      if (operation.phase !== lastPhase) {
        updateToast(toastId, {title: progressLabels[operation.phase]})
        lastPhase = operation.phase
      }

      await new Promise(resolve => setTimeout(resolve, 1500))
      if (!alive.current) return
      operation = await api.operation(operation.id)
    }
  }

  const send = async () => {
    let current = flow
    const wallet = wallets?.runtimeWallets.find(wallet => wallet.id === current?.walletId)
    if (!current?.operation?.message || !wallet || activeRequest.current) return

    activeRequest.current = true
    setFlow({...current, phase: "sending"})
    const toastId = beginProgress("Checking verification status")
    let sendAttempted = false

    try {
      // Recheck after the review delay, before asking the project wallet to sign.
      const status = await api.status(current.codeHash)
      if (!alive.current) return
      if (normalizeCodeHash(status.codeHash) !== normalizeCodeHash(current.codeHash)) {
        throw new Error("The verifier returned a different code hash; payment has not been sent")
      }
      if (status.verified) {
        markVerified(current, status, toastId)
        return
      }

      updateToast(toastId, {title: "Signing Testnet payment"})
      const signed = await wallet.wallet.getSignedSendTransaction({
        fromAddress: wallet.record.address,
        validUntil: Math.floor(Date.now() / 1000) + 300,
        messages: [
          {
            ...current.operation.message,
            payload: current.operation.message.payload as Base64String,
            mode: {flags: [SendModeFlag.PAY_GAS_SEPARATELY, SendModeFlag.IGNORE_ERRORS]},
          },
        ],
      })
      current = {...current, paymentMessageHash: getNormalizedExtMessageHash(signed).hash}
      setFlow({...current, phase: "sending"})
      sendAttempted = true
      updateToast(toastId, {title: "Sending Testnet payment"})
      await wallet.wallet.getClient().sendBoc(signed)

      // Start the server-owned upload even if navigation unmounted this provider while sending.
      await confirm(current, toastId)
    } catch (error) {
      if (!alive.current) return
      // A lost send response can still mean the transaction was broadcast.
      // Keep payment disabled while its outcome is unknown.
      handleFailure(
        sendAttempted ? current : {...current, paymentMessageHash: undefined},
        error,
        toastId,
      )
    } finally {
      activeRequest.current = false
    }
  }

  const refreshConfirmation = async (current = flow) => {
    if (!current || activeRequest.current) return
    activeRequest.current = true
    const toastId = beginProgress("Resuming verification with the same payment")

    try {
      await confirm(current, toastId)
    } catch (error) {
      if (alive.current) {
        handleFailure(current, error, toastId)
      }
    } finally {
      activeRequest.current = false
    }
  }

  const handleFailure = (current: VerificationFlow, error: unknown, toastId: string) => {
    const expired =
      error instanceof StudioRequestError && error.code === "verification_preview_expired"
    const phase = expired ? "expired" : current.paymentMessageHash ? "pending" : "failed"
    setFlow({...current, phase})
    setOpen(true)

    reportError(
      error,
      toastId,
      phase === "pending"
        ? {label: "Retry verification", run: () => void refreshConfirmation(current)}
        : {label: "Check sources again", run: () => void preview(current.codeHash, current)},
    )
  }

  const selectedContractId = flow?.contractId ?? contractId
  const candidate = flow?.preview?.candidates.find(
    candidate => candidate.contractId === selectedContractId,
  )
  const selectedWalletId = flow?.walletId || walletId || wallets?.runtimeWallets[0]?.id || ""
  const busy = flow && ["preview", "preparing", "sending", "confirming"].includes(flow.phase)
  const walletOptions =
    wallets?.runtimeWallets.map(wallet => ({
      id: wallet.id,
      name: wallet.record.name,
      address: wallet.record.address,
      network: "Testnet",
      balance:
        wallets.walletBalances[wallet.id]?.value === undefined ? (
          "Balance unavailable"
        ) : (
          <GramAmount value={wallets.walletBalances[wallet.id].value ?? "0"} />
        ),
    })) ?? []

  return (
    <VerificationContext.Provider value={{available, statuses, check, open: openVerification}}>
      {children}
      {available && environment?.network.id === "mainnet" && (
        <WalletRuntimeProvider
          enabled={Boolean(flow)}
          apiBaseUrl={new URL(
            "/api/v1/environments/testnet/rpc",
            globalThis.location.origin,
          ).toString()}
          environmentId="testnet"
          environmentKind="remoteTonNetwork"
          networkLabel="Testnet"
          chainId={-3}
        >
          <TestnetWalletBridge onChange={setTestnetWallets} />
        </WalletRuntimeProvider>
      )}
      <Dialog
        open={open}
        onOpenChange={setOpen}
        title="Verify contract source"
        maxWidth="38rem"
        busy={Boolean(busy)}
        footer={
          <DialogActions className={styles.actions}>
            {flow?.phase === "review" && (
              <Button
                variant="primary"
                disabled={
                  !candidate?.matches ||
                  !flow.preview?.payment ||
                  !walletOptions.some(wallet => wallet.id === selectedWalletId)
                }
                onClick={() => void prepare()}
              >
                {flow.paymentMessageHash ? "Resume verification" : "Continue"}
              </Button>
            )}
            {flow?.phase === "ready" && (
              <Button variant="primary" onClick={() => void send()}>
                Pay and verify
              </Button>
            )}
            {flow?.phase === "pending" && (
              <Button variant="primary" onClick={() => void refreshConfirmation()}>
                Retry verification
              </Button>
            )}
            {(flow?.phase === "failed" || flow?.phase === "expired") && (
              <Button variant="primary" onClick={() => void preview(flow.codeHash, flow)}>
                Check sources again
              </Button>
            )}
            <Button variant="outline" onClick={() => setOpen(false)}>
              Close
            </Button>
          </DialogActions>
        }
      >
        <div className={styles.content}>
          {flow && (
            <div className={styles.target}>
              <span>Code hash</span>
              <TechnicalValue value={flow.codeHash} copyLabel="code hash" />
            </div>
          )}
          {flow?.preview && flow.phase !== "verified" && (
            <>
              <Select
                label="Project contract"
                value={selectedContractId}
                disabled={flow.phase !== "review"}
                onChange={event => setContractId(event.target.value)}
              >
                {flow.preview.candidates.length === 0 && (
                  <option value="">No Tolk contracts in this project</option>
                )}
                {flow.preview.candidates.map(candidate => (
                  <option key={candidate.contractId} value={candidate.contractId}>
                    {candidate.contractId}
                    {candidate.matches ? " · Code matches" : ""}
                  </option>
                ))}
              </Select>
              {candidate && (
                <div className={styles.match}>
                  {candidate.matches ? (
                    <>
                      <Check size={16} />
                      Compiled code matches the requested hash
                    </>
                  ) : (
                    <span>
                      Code does not match · Check the source revision and compiler version
                    </span>
                  )}
                </div>
              )}
              {candidate?.error && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => reportError(new Error(candidate.error ?? "Compilation failed"))}
                >
                  Show compilation error
                </Button>
              )}
              <dl className={styles.details}>
                <div>
                  <dt>Compiler</dt>
                  <dd>Tolk {flow.preview.compilerVersion}</dd>
                </div>
              </dl>
              {candidate && candidate.files.length > 0 && (
                <Disclosure label={`Source files (${candidate.files.length})`}>
                  <ul className={styles.files}>
                    {candidate.files.map(file => (
                      <li key={file.path}>
                        <SourceLocationValue value={{file: file.path}} />
                        <ByteSize value={file.sizeBytes} />
                      </li>
                    ))}
                  </ul>
                </Disclosure>
              )}
              {walletOptions.length > 0 ? (
                <TonConnectWalletSelector
                  options={
                    flow.phase === "review" && !flow.paymentMessageHash
                      ? walletOptions
                      : walletOptions.filter(wallet => wallet.id === selectedWalletId)
                  }
                  selectedId={selectedWalletId}
                  onSelect={setWalletId}
                />
              ) : (
                <p className={styles.note}>
                  Add a v4r2 or v5r1 wallet to your Acton project to pay for verification
                </p>
              )}
              {flow.phase === "review" && (
                <p className={styles.note}>
                  {flow.paymentMessageHash
                    ? "The reviewed source files will be published using your saved Testnet payment · No new transaction will be sent"
                    : "The listed source files will be published through Acton Verifier after payment"}
                  {!flow.paymentMessageHash && flow.preview.payment && (
                    <>
                      {" "}
                      · <GramAmount value={flow.preview.payment.amount} /> on Testnet plus network
                      fees
                    </>
                  )}
                </p>
              )}
            </>
          )}
          {flow?.phase === "ready" && flow.operation?.message && (
            <>
              <dl className={styles.details}>
                <div>
                  <dt>Payment recipient · Testnet</dt>
                  <dd>
                    <AddressChip address={flow.operation.message.address} />
                  </dd>
                </div>
                <div>
                  <dt>Transaction amount</dt>
                  <dd>
                    <GramAmount value={flow.operation.message.amount} /> plus network fees
                  </dd>
                </div>
              </dl>
              <p className={styles.note}>
                Payment goes through Testnet for contracts from either network · After it is
                finalized, Studio uploads the reviewed source files to Acton Verifier
              </p>
            </>
          )}
        </div>
      </Dialog>
    </VerificationContext.Provider>
  )
}

/** Keeps the Testnet wallet scope separate from the inspected Mainnet workspace */
function TestnetWalletBridge({
  onChange,
}: {
  readonly onChange: (wallets: WalletRuntimeContextValue | undefined) => void
}) {
  const wallets = useOptionalWalletRuntime()

  useEffect(() => {
    onChange(wallets)
  }, [wallets, onChange])

  return null
}

/** Shared entry point used by Contracts and the embedded Explorer */
export function VerificationAction({codeHash}: {readonly codeHash?: string}) {
  const context = useContext(VerificationContext)
  const hash = normalizeCodeHash(codeHash)

  useEffect(() => {
    if (context?.available && hash) void context.check(hash)
  }, [hash, context?.available, context?.check])

  if (!context?.available || !hash) return null
  const status = context.statuses[hash]

  return status?.verified && normalizeCodeHash(status.codeHash) === normalizeCodeHash(codeHash) ? (
    <a className={styles.verified} href={status.verifierUrl} target="_blank" rel="noreferrer">
      <ShieldCheck size={16} />
      Source verified
      <ExternalLink size={14} />
    </a>
  ) : (
    <Button
      variant="outline"
      size="sm"
      leadingIcon={<ShieldCheck />}
      onClick={() => context.open(hash)}
    >
      Verify source
    </Button>
  )
}
