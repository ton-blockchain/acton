import {
  AddressChip,
  Button,
  ByteSize,
  Disclosure,
  Dialog,
  DialogActions,
  GramAmount,
  InlineLoader,
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

import {formatAddress, toRawAddress} from "@acton/explorer-core/components/utils"
import {normalizeCodeHash} from "@acton/explorer-core/metadata/codeHash"
import {fetchStudioInfo} from "../../studioApi"
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
  | "verified"
  | "failed"

interface VerificationFlow {
  readonly address: string
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
  readonly check: (address: string, codeHash: string) => Promise<void>
  readonly open: (address: string) => void
}

const VerificationContext = createContext<VerificationContextValue | undefined>(undefined)
const preparingLabels = {
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
  const {showToast} = useToast()
  const api = useMemo(() => verificationApi(environment?.id ?? ""), [environment?.id])
  const [hasProject, setHasProject] = useState(false)
  const [statuses, setStatuses] = useState<Record<string, VerificationStatus>>({})
  const storageKey = `acton:verification:${environment?.id}`
  const [flow, updateFlow] = useState<VerificationFlow | undefined>(() => {
    try {
      const saved = JSON.parse(
        sessionStorage.getItem(storageKey) ?? "null",
      ) as VerificationFlow | null
      return saved?.paymentMessageHash && saved.operation && saved.preview
        ? {...saved, phase: "pending"}
        : undefined
    } catch {
      return undefined
    }
  })
  const [open, setOpen] = useState(false)
  const [contractId, setContractId] = useState("")
  const [walletId, setWalletId] = useState("")
  const activeRequest = useRef(false)
  const alive = useRef(true)
  const checked = useRef(new Set<string>())
  const available = hasProject && environment?.config.kind === "remoteTonNetwork"

  const setFlow = (next: VerificationFlow) => {
    // Save the reference before broadcast, so even a lost response cannot lead to a second charge.
    if (next.paymentMessageHash && next.phase !== "verified") {
      sessionStorage.setItem(storageKey, JSON.stringify(next))
    } else {
      sessionStorage.removeItem(storageKey)
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
    }
  }, [])

  const reportError = useCallback(
    (error: unknown) => {
      showToast({
        variant: "error",
        title: "Source verification failed",
        description:
          error instanceof Error ? error.message : "Unable to verify the contract source",
      })
    },
    [showToast],
  )

  const saveStatus = useCallback((status: VerificationStatus) => {
    setStatuses(current => ({...current, [toRawAddress(status.address)]: status}))
  }, [])

  const check = useCallback(
    async (address: string, codeHash: string) => {
      const key = `${toRawAddress(address)}:${normalizeCodeHash(codeHash)}`
      if (!available || checked.current.has(key)) return
      checked.current.add(key)

      try {
        const status = await api.status(address)
        if (alive.current) saveStatus(status)
      } catch (error) {
        checked.current.delete(key)
        if (alive.current) reportError(error)
      }
    },
    [api, available, reportError, saveStatus],
  )

  const preview = async (address: string) => {
    if (activeRequest.current) {
      setOpen(true)
      return
    }

    activeRequest.current = true
    setFlow({address, phase: "preview"})
    setOpen(true)

    try {
      const result = await api.preview(address)
      if (!alive.current) return

      saveStatus(result.status)
      setContractId(
        result.candidates.find(candidate => candidate.matches)?.contractId ??
          result.candidates[0]?.contractId ??
          "",
      )
      setWalletId(wallets?.runtimeWallets[0]?.id ?? "")
      setFlow({address, phase: result.status.verified ? "verified" : "review", preview: result})
    } catch (error) {
      if (!alive.current) return
      setFlow({address, phase: "failed"})
      reportError(error)
    } finally {
      activeRequest.current = false
    }
  }

  const openVerification = (address: string) => {
    if (flow && ["preparing", "ready", "sending", "confirming", "pending"].includes(flow.phase)) {
      setOpen(true)
      return
    }

    if (flow && toRawAddress(flow.address) === toRawAddress(address) && flow.phase !== "failed") {
      setOpen(true)
    } else {
      void preview(address)
    }
  }

  const markVerified = (current: VerificationFlow, status: VerificationStatus) => {
    saveStatus(status)
    setFlow({...current, phase: "verified"})
    showToast({
      variant: "success",
      title: "Source verified",
      description: (
        <a href={status.verifierUrl} target="_blank" rel="noreferrer">
          View in Acton Verifier
        </a>
      ),
    })
  }

  const prepare = async () => {
    const wallet = wallets?.runtimeWallets.find(wallet => wallet.id === selectedWalletId)
    if (!flow?.preview || !wallet || activeRequest.current) return

    activeRequest.current = true
    const current = {...flow, contractId, walletId: selectedWalletId}
    setFlow({...current, phase: "preparing"})

    try {
      const operation = await api.start(flow.preview.id, contractId, wallet.record.address)
      if (!alive.current) return
      if (operation.phase !== "ready" || !operation.message) {
        throw new Error(operation.error ?? "The verifier did not return a payment transaction")
      }
      setFlow({...current, phase: "ready", operation})
    } catch (error) {
      if (!alive.current) return
      setFlow({...current, phase: "failed"})
      reportError(error)
    } finally {
      activeRequest.current = false
    }
  }

  const confirm = async (current: VerificationFlow) => {
    if (!current.operation || !current.paymentMessageHash) {
      throw new Error(
        "The wallet did not return a payment reference; check its transactions before sending another payment",
      )
    }

    // Resubmitting the same reference resumes the server operation without paying again.
    let operation = await api.completePayment(current.operation.id, current.paymentMessageHash)
    while (alive.current) {
      setFlow({...current, phase: "confirming", operation})
      if (operation.phase === "failed") throw new Error(operation.error ?? "Verification failed")
      if (operation.phase === "verified") {
        const status = await api.status(current.address)
        if (!status.verified || status.codeHash !== current.preview?.status.codeHash) {
          throw new Error(
            "Verification finished for the reviewed code, but the contract now has different code",
          )
        }
        markVerified(current, status)
        return
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
    let sendAttempted = false

    try {
      // Recheck after the review delay, before asking the project wallet to sign.
      const status = await api.status(current.address)
      if (!alive.current) return
      if (status.codeHash !== current.preview?.status.codeHash) {
        throw new Error("The deployed code changed; check the project sources again")
      }
      if (status.verified) {
        markVerified(current, status)
        return
      }

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
      await wallet.wallet.getClient().sendBoc(signed)

      // Start the server-owned upload even if navigation unmounted this provider while sending.
      await confirm(current)
    } catch (error) {
      if (!alive.current) return
      // A lost send response can still mean the transaction was broadcast.
      // Keep payment disabled while its outcome is unknown.
      setFlow({...current, phase: sendAttempted ? "pending" : "ready"})
      reportError(error)
    } finally {
      activeRequest.current = false
    }
  }

  const refreshConfirmation = async () => {
    if (!flow || activeRequest.current) return
    activeRequest.current = true

    try {
      await confirm(flow)
    } catch (error) {
      if (alive.current) {
        setFlow({...flow, phase: "pending"})
        reportError(error)
      }
    } finally {
      activeRequest.current = false
    }
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
                Continue
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
            {flow?.phase === "failed" && (
              <Button variant="primary" onClick={() => void preview(flow.address)}>
                Check again
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
              <span>{environment?.network.label}</span>
              <AddressChip
                address={formatAddress(flow.address, false, {
                  testOnly: environment?.network.testOnly,
                })}
              />
            </div>
          )}
          {flow?.phase === "preview" && (
            <div className={styles.loading}>
              <InlineLoader message="Checking deployed code and project sources" />
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
                      Compiled code matches the deployed contract
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
                <div>
                  <dt>Code hash</dt>
                  <dd>
                    <TechnicalValue value={flow.preview.status.codeHash} copyLabel="code hash" />
                  </dd>
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
                    flow.phase === "review"
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
                  The listed source files will be published through Acton Verifier after payment
                  {flow.preview.payment && (
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
          {flow?.phase === "preparing" && <InlineLoader message="Preparing Testnet payment" />}
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
          {flow?.phase === "sending" && <InlineLoader message="Sending verification transaction" />}
          {flow?.phase === "confirming" && (
            <InlineLoader message={preparingLabels[flow.operation?.phase ?? "confirmingPayment"]} />
          )}
          {flow?.phase === "pending" && (
            <p className={styles.note}>
              Verification has not finished yet · Retry with the same payment without sending
              another transaction
            </p>
          )}
          {flow?.phase === "verified" && (
            <div className={styles.success}>
              <ShieldCheck />
              <span>Source verified</span>
              <a href={flow.preview?.status.verifierUrl} target="_blank" rel="noreferrer">
                View in Acton Verifier <ExternalLink size={14} />
              </a>
            </div>
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
export function VerificationAction({
  address,
  codeHash,
}: {
  readonly address: string
  readonly codeHash?: string
}) {
  const context = useContext(VerificationContext)

  useEffect(() => {
    if (context?.available && codeHash) void context.check(address, codeHash)
  }, [address, codeHash, context?.available, context?.check])

  if (!context?.available || !codeHash) return null
  const status = context.statuses[toRawAddress(address)]

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
      onClick={() => context.open(address)}
    >
      Verify source
    </Button>
  )
}
