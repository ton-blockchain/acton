import {
  Button,
  formatDateTime,
  formatNumberValue,
  formatRecurringPeriod,
  SearchInput,
  useToast,
} from "@acton/ui"
import type {SearchInputItem} from "@acton/ui"
import {Address} from "@ton/core"
import {
  ArrowRight,
  Check,
  ChevronDown,
  Circle,
  ExternalLink,
  Github,
  History,
  ShieldCheck,
  X,
} from "lucide-react"
import {useCallback, useEffect, useRef, useState} from "react"
import type {FC} from "react"
import {Link, useSearchParams} from "react-router"

import type {TonClient} from "@acton/explorer-core/api/client"
import {
  clearFaucetSession,
  disconnectFaucetSession,
  exchangeGitHubGrant,
  FaucetRequestError,
  githubAuthorizationUrl,
  requestFaucetAuthStatus,
  requestFaucetChallenge,
  requestFaucetSession,
  submitFaucetClaim,
  type FaucetAuthStatus,
  type FaucetChallenge,
  type FaucetSession,
} from "./faucetClient"
import type {
  FaucetPowProgress,
  FaucetPowRequest,
  FaucetPowSolution,
  FaucetPowWorkerResponse,
} from "./faucetPow"
import {
  FAUCET_REQUEST_HISTORY_STORAGE_KEY,
  FAUCET_REQUEST_LIMIT,
  FAUCET_REQUEST_WINDOW_MS,
  readFaucetUsage,
  recordFaucetRequest,
  type FaucetUsage,
} from "./faucetUsage"
import styles from "./FaucetPage.module.css"

type FaucetPhase =
  | "idle"
  | "challenge"
  | "solving"
  | "claiming"
  | "waiting"
  | "received"
  | "queued"
  | "cancelled"
  | "error"

interface FaucetPageProps {
  readonly isTestnetSelected: boolean
  readonly selectedNetworkLabel: string
  readonly testnetClient: TonClient
  readonly onSwitchToTestnet: () => void
}

interface FaucetRun {
  readonly controller: AbortController
  readonly worker?: Worker
}

const BALANCE_WAIT_ATTEMPTS = 10
const BALANCE_WAIT_INTERVAL_MS = 2000
const FAUCET_ADDRESS_HISTORY_STORAGE_KEY = "actonscanFaucetAddressHistory"
const GITHUB_RETURN_STATE_STORAGE_KEY = "actonscanFaucetGitHubReturnState"
const GITHUB_RETURN_STATE_TTL_MS = 30 * 60 * 1000
const MAX_GITHUB_RETURN_ADDRESS_LENGTH = 512
const MAX_GITHUB_RETURN_NETWORK_LENGTH = 128
const MAX_ADDRESS_HISTORY_ITEMS = 5

const STEPS: readonly {
  readonly phase: FaucetPhase
  readonly label: string
  readonly detail: string
}[] = [
  {
    phase: "challenge",
    label: "Request challenge",
    detail: "The faucet issues a short-lived proof-of-work task",
  },
  {
    phase: "solving",
    label: "Solve proof of work",
    detail: "Your browser computes SHA-256 in a background worker",
  },
  {
    phase: "claiming",
    label: "Queue transfer",
    detail: "The verified solution is exchanged for testnet GRAM",
  },
  {
    phase: "waiting",
    label: "Confirm balance",
    detail: "Actonscan watches Testnet for the incoming transfer",
  },
]

const PHASE_ORDER: Partial<Record<FaucetPhase, number>> = {
  challenge: 0,
  solving: 1,
  claiming: 2,
  waiting: 3,
  received: 4,
  queued: 4,
}

class MainnetAddressError extends Error {}

function normalizeTestnetAddress(value: string): string {
  if (!Address.isFriendly(value)) {
    return Address.parse(value).toString({bounceable: false, testOnly: true})
  }

  const parsedAddress = Address.parseFriendly(value)
  if (!parsedAddress.isTestOnly) {
    throw new MainnetAddressError()
  }

  return parsedAddress.address.toString({bounceable: false, testOnly: true})
}

function addressErrorToast(error: unknown): {readonly title: string; readonly description: string} {
  return error instanceof MainnetAddressError
    ? {
        title: "Mainnet address",
        description: "Enter a Testnet address (kQ… or 0Q…)",
      }
    : {
        title: "Invalid address",
        description: "Enter a valid TON address",
      }
}

export const FaucetPage: FC<FaucetPageProps> = props => {
  const {isTestnetSelected, selectedNetworkLabel, testnetClient} = props
  const {dismissToast, showToast, updateToast} = useToast()
  const [searchParams, setSearchParams] = useSearchParams()
  const [initialAuthParams] = useState(() => readGitHubRedirectParams(searchParams))
  const [address, setAddress] = useState(
    () => initialAuthParams.returnState?.address ?? searchParams.get("address") ?? "",
  )
  const [addressHistory, setAddressHistory] = useState<readonly string[]>(readFaucetAddressHistory)
  const [addressInvalid, setAddressInvalid] = useState(false)
  const [phase, setPhase] = useState<FaucetPhase>("idle")
  const [usage, setUsage] = useState<FaucetUsage>(() => readFaucetUsage())
  const [authStatus, setAuthStatus] = useState<FaucetAuthStatus | undefined>(undefined)
  const [githubSession, setGitHubSession] = useState<FaucetSession | undefined>(undefined)
  const [authBusy, setAuthBusy] = useState(true)
  const activeRunRef = useRef<FaucetRun | undefined>(undefined)
  const activeToastRef = useRef<string | undefined>(undefined)
  const authInitializationRef = useRef<Promise<FaucetAuthInitializationResult> | undefined>(
    undefined,
  )
  const requestLimit =
    githubSession?.maxRequests ?? authStatus?.guestMaxRequests ?? FAUCET_REQUEST_LIMIT
  const requestWindowMs = (authStatus?.windowSeconds ?? FAUCET_REQUEST_WINDOW_MS / 1000) * 1000
  const running = isRunningPhase(phase)
  const requestBlocked = !isTestnetSelected || running || authBusy || usage.limitReached
  const primaryButtonLabel =
    !running && usage.limitReached ? rateLimitButtonLabel(usage) : requestButtonLabel(phase)
  const addressHistoryItems: readonly SearchInputItem[] =
    address.trim().length > 0
      ? []
      : addressHistory.map(historyAddress => ({
          id: historyAddress,
          label: historyAddress,
          icon: <History size={16} />,
          onSelect: () => {
            setAddress(historyAddress)
            setAddressInvalid(false)
          },
          onRemove: () => {
            setAddressHistory(current => {
              const nextHistory = current.filter(item => item !== historyAddress)
              writeFaucetAddressHistory(nextHistory)
              return nextHistory
            })
          },
          removeLabel: `Remove ${historyAddress} from history`,
        }))

  const addAddressToHistory = useCallback((historyAddress: string) => {
    setAddressHistory(current => {
      const nextHistory = [
        historyAddress,
        ...current.filter(item => item !== historyAddress),
      ].slice(0, MAX_ADDRESS_HISTORY_ITEMS)
      writeFaucetAddressHistory(nextHistory)
      return nextHistory
    })
  }, [])

  const cancelActiveRun = useCallback(() => {
    const activeRun = activeRunRef.current
    if (!activeRun) return

    activeRun.controller.abort()
    activeRun.worker?.terminate()
    activeRunRef.current = undefined
  }, [])

  useEffect(
    () => () => {
      cancelActiveRun()
      if (activeToastRef.current) {
        dismissToast(activeToastRef.current)
      }
    },
    [cancelActiveRun, dismissToast],
  )

  useEffect(() => {
    let cancelled = false
    authInitializationRef.current ??= initializeFaucetAuth(initialAuthParams.grant)

    void authInitializationRef.current
      .then(({status, session, sessionError}) => {
        if (cancelled) return
        setAuthStatus(status)
        setGitHubSession(session)
        const toast = faucetAuthToast(initialAuthParams, session, sessionError)
        if (toast) showToast(toast)
        clearGitHubRedirectParams(initialAuthParams, setSearchParams)
      })
      .catch(error => {
        if (cancelled) return
        if (initialAuthParams.grant) {
          showToast({
            variant: "error",
            title: "GitHub connection failed",
            description: error instanceof Error ? error.message : "Unable to connect GitHub",
            durationMs: 8000,
          })
        } else {
          clearGitHubRedirectParams(initialAuthParams, setSearchParams)
        }
      })
      .finally(() => {
        if (!cancelled) setAuthBusy(false)
      })

    return () => {
      cancelled = true
    }
  }, [initialAuthParams, setSearchParams, showToast])

  useEffect(() => {
    setUsage(readFaucetUsage(Date.now(), requestLimit, requestWindowMs))
  }, [requestLimit, requestWindowMs])

  useEffect(() => {
    const refreshUsage = () => setUsage(readFaucetUsage(Date.now(), requestLimit, requestWindowMs))
    const handleStorage = (event: StorageEvent) => {
      if (event.key === FAUCET_REQUEST_HISTORY_STORAGE_KEY || event.key === null) {
        refreshUsage()
      }
    }
    const refreshDelay =
      usage.refreshAt === undefined ? undefined : Math.max(0, usage.refreshAt - Date.now()) + 100
    const refreshTimer =
      refreshDelay === undefined ? undefined : globalThis.setTimeout(refreshUsage, refreshDelay)

    globalThis.addEventListener("storage", handleStorage)
    return () => {
      globalThis.removeEventListener("storage", handleStorage)
      if (refreshTimer !== undefined) globalThis.clearTimeout(refreshTimer)
    }
  }, [requestLimit, requestWindowMs, usage.refreshAt])

  const handleCancel = () => {
    cancelActiveRun()
  }

  const handleConnectGitHub = () => {
    if (running || authBusy) return

    writeGitHubReturnState(
      address,
      searchParams.get("network") ?? (isTestnetSelected ? "testnet" : "mainnet"),
    )
    setAuthBusy(true)
    globalThis.location.assign(githubAuthorizationUrl())
  }

  const handleDisconnectGitHub = async () => {
    if (running || authBusy) return

    setAuthBusy(true)
    try {
      await disconnectFaucetSession()
      setGitHubSession(undefined)
      showToast({
        variant: "info",
        title: "GitHub disconnected",
        description: "Guest limits now apply",
        durationMs: 4000,
      })
    } catch (error) {
      showToast({
        variant: "error",
        title: "Could not disconnect GitHub",
        description: error instanceof Error ? error.message : "Try again",
        durationMs: 8000,
      })
    } finally {
      setAuthBusy(false)
    }
  }

  const handleSubmit = async () => {
    if (requestBlocked) return

    let testnetAddress: string
    try {
      testnetAddress = normalizeTestnetAddress(address.trim())
    } catch (error) {
      const errorToast = addressErrorToast(error)
      setPhase("error")
      setAddressInvalid(true)
      showToast({
        variant: "error",
        ...errorToast,
        durationMs: 8000,
      })
      return
    }

    cancelActiveRun()
    const controller = new AbortController()
    activeRunRef.current = {controller}
    setAddress(testnetAddress)
    addAddressToHistory(testnetAddress)
    setAddressInvalid(false)
    setPhase("challenge")
    const nextSearchParams = new URLSearchParams(searchParams)
    nextSearchParams.set("address", testnetAddress)
    setSearchParams(nextSearchParams, {replace: true})
    const toastId = showToast({
      variant: "loading",
      title: statusTitle("challenge"),
      description: statusDescription("challenge"),
      durationMs: 60_000,
    })
    activeToastRef.current = toastId

    try {
      const balanceBefore = await getTestnetBalance(testnetClient, testnetAddress)
      const challenge = await requestFaucetChallenge(testnetAddress, controller.signal)
      setPhase("solving")
      updateToast(toastId, {
        variant: "loading",
        title: statusTitle("solving"),
        description: statusDescription("solving"),
        durationMs: 60_000,
      })
      const solution = await solveChallengeInWorker(
        challenge,
        controller.signal,
        progress => {
          const progressDescription =
            progress.attempts === 0
              ? statusDescription("solving")
              : `${formatHashRate(progress)} · ${formatNumberValue(progress.attempts)} attempts`
          updateToast(toastId, {
            variant: "loading",
            title: statusTitle("solving"),
            description: progressDescription,
            durationMs: 60_000,
          })
        },
        worker => {
          activeRunRef.current = {controller, worker}
        },
      )
      setPhase("claiming")
      updateToast(toastId, {
        variant: "loading",
        title: statusTitle("claiming"),
        description: statusDescription("claiming"),
        durationMs: 60_000,
      })
      const claim = await submitFaucetClaim(
        testnetAddress,
        challenge,
        solution.nonce,
        controller.signal,
      )
      setUsage(recordFaucetRequest(Date.now(), requestLimit, requestWindowMs))
      setPhase("waiting")
      updateToast(toastId, {
        variant: "loading",
        title: statusTitle("waiting"),
        description: statusDescription("waiting", claim.message),
        durationMs: 60_000,
      })

      const received = await waitForBalanceIncrease(
        testnetClient,
        testnetAddress,
        balanceBefore,
        controller.signal,
      )
      const finalPhase = received ? "received" : "queued"
      setPhase(finalPhase)
      updateToast(toastId, {
        variant: received ? "success" : "info",
        title: statusTitle(finalPhase),
        description: (
          <span>
            {statusDescription(finalPhase)}
            <br />
            <br />
            <Link to={`/address/${encodeURIComponent(testnetAddress)}?network=testnet`}>
              View on Testnet
            </Link>
          </span>
        ),
        durationMs: 10_000,
      })
    } catch (requestError) {
      setGitHubSession(currentSession => sessionAfterRequestError(currentSession, requestError))
      if (controller.signal.aborted) {
        setPhase("cancelled")
        updateToast(toastId, {
          variant: "info",
          title: statusTitle("cancelled"),
          description: statusDescription("cancelled"),
          durationMs: 4000,
        })
      } else {
        setPhase("error")
        updateToast(toastId, {
          variant: "error",
          title: statusTitle("error"),
          description: faucetErrorCopy(requestError),
          durationMs: 8000,
        })
      }
    } finally {
      activeRunRef.current?.worker?.terminate()
      activeRunRef.current = undefined
      activeToastRef.current = undefined
    }
  }

  return (
    <div className={styles.page}>
      {!isTestnetSelected && (
        <section className={styles.networkNotice}>
          <div className={styles.networkNoticeCopy}>
            <strong>{selectedNetworkLabel} selected</strong>
            <span>Faucet payouts are always sent on Testnet</span>
          </div>
          <Button variant="secondary" size="sm" onClick={props.onSwitchToTestnet}>
            Switch to Testnet
          </Button>
        </section>
      )}

      <div className={styles.contentGrid}>
        <section className={styles.requestCard}>
          <div className={styles.cardHeader}>
            <div>
              <h1>Request testnet GRAM</h1>
              <p className={styles.limitCopy}>
                {faucetUsageCopy(usage, requestLimit, requestWindowMs)}
              </p>
            </div>
          </div>

          <form
            className={styles.form}
            onSubmit={event => {
              event.preventDefault()
              void handleSubmit()
            }}
          >
            <SearchInput
              ariaLabel="TON address"
              autoFocus
              inputClassName={styles.addressInput}
              items={addressHistoryItems}
              value={address}
              onSubmit={() => {
                void handleSubmit()
                return false
              }}
              onValueChange={nextAddress => {
                setAddress(nextAddress)
                setAddressInvalid(false)
              }}
              placeholder="Enter a friendly (kQ…) or raw (0:…) address"
              size="lg"
              invalid={addressInvalid}
              disabled={running || authBusy}
              variant="field"
            />
            <div className={styles.formActions}>
              <Button
                type="submit"
                variant="primary"
                size="lg"
                loading={running}
                disabled={requestBlocked}
                trailingIcon={requestBlocked ? undefined : <ArrowRight size={17} />}
              >
                {primaryButtonLabel}
              </Button>
              {running && (
                <Button
                  type="button"
                  variant="ghost"
                  size="lg"
                  leadingIcon={<X size={17} />}
                  onClick={handleCancel}
                >
                  Cancel
                </Button>
              )}
            </div>
          </form>
        </section>

        <GitHubLimitsCard
          status={authStatus}
          session={githubSession}
          requestWindowMs={requestWindowMs}
          busy={authBusy}
          disabled={running}
          onConnect={handleConnectGitHub}
          onDisconnect={() => void handleDisconnectGitHub()}
        />

        <section className={styles.processCard}>
          <div className={styles.cardHeader}>
            <div>
              <h2>What happens next</h2>
            </div>
          </div>
          <ol className={styles.steps}>
            {STEPS.map((step, index) => (
              <FaucetStep key={step.phase} step={step} index={index} phase={phase} />
            ))}
          </ol>
          <div className={styles.privacyNote}>
            <ShieldCheck size={17} aria-hidden="true" />
            <span>No wallet connection or private key is required</span>
          </div>
        </section>

        <details className={styles.cliGuide}>
          <summary>
            <span>Use Acton CLI</span>
            <ChevronDown size={18} aria-hidden="true" />
          </summary>
          <div className={styles.cliGuideBody}>
            <p>Request Testnet GRAM for an Acton wallet with the same proof-of-work flow.</p>
            <code>
              <span className={styles.cliCommand}>acton wallet airdrop</span>{" "}
              <span className={styles.cliArgument}>&lt;WALLET_NAME&gt;</span>
            </code>
            <p>
              Learn more in the{" "}
              <a
                className={styles.cliDocsLink}
                href="https://ton-blockchain.github.io/acton/docs/wallets#fund-a-wallet-on-testnet"
                target="_blank"
                rel="noreferrer"
              >
                documentation
                <ExternalLink size={14} aria-hidden="true" />
              </a>
              .
            </p>
          </div>
        </details>
      </div>
    </div>
  )
}

interface FaucetStepProps {
  readonly step: (typeof STEPS)[number]
  readonly index: number
  readonly phase: FaucetPhase
}

interface GitHubLimitsCardProps {
  readonly status?: FaucetAuthStatus
  readonly session?: FaucetSession
  readonly requestWindowMs: number
  readonly busy: boolean
  readonly disabled: boolean
  readonly onConnect: () => void
  readonly onDisconnect: () => void
}

const GitHubLimitsCard: FC<GitHubLimitsCardProps> = props => {
  const {status, session} = props
  if (!status?.enabled) return null

  return (
    <section className={styles.githubCard} aria-label="GitHub faucet limits">
      <div className={styles.githubCopy}>
        <span className={styles.githubIcon}>
          <Github size={18} aria-hidden="true" />
        </span>
        <div>
          <h2>{session ? `Connected as @${session.login}` : "Higher limits"}</h2>
          <p>
            {session
              ? `${tierLabel(session)} tier · ${session.maxRequests} requests ${formatRecurringPeriod(props.requestWindowMs)}`
              : `Connect GitHub to unlock up to ${status.establishedMaxRequests} requests ${formatRecurringPeriod(props.requestWindowMs)}`}
          </p>
        </div>
      </div>
      <Button
        type="button"
        variant={session ? "ghost" : "secondary"}
        size="sm"
        loading={props.busy}
        disabled={props.disabled}
        onClick={session ? props.onDisconnect : props.onConnect}
      >
        {session ? "Disconnect" : "Connect GitHub"}
      </Button>
    </section>
  )
}

const FaucetStep: FC<FaucetStepProps> = ({step, index, phase}) => {
  const currentIndex = PHASE_ORDER[phase]
  const complete = currentIndex !== undefined && currentIndex > index
  const active = currentIndex === index

  return (
    <li className={`${styles.step} ${active ? styles.stepActive : ""}`}>
      <span className={`${styles.stepIcon} ${complete ? styles.stepComplete : ""}`}>
        {complete ? <Check size={14} /> : <Circle size={12} />}
      </span>
      <span className={styles.stepCopy}>
        <strong>{step.label}</strong>
        <span>{step.detail}</span>
      </span>
    </li>
  )
}

function solveChallengeInWorker(
  challenge: FaucetChallenge,
  signal: AbortSignal,
  onProgress: (progress: FaucetPowProgress) => void,
  onWorker: (worker: Worker) => void,
): Promise<FaucetPowSolution> {
  const worker = new Worker(new URL("./faucetPow.worker.ts", import.meta.url), {type: "module"})
  onWorker(worker)

  return new Promise((resolve, reject) => {
    const cleanup = () => {
      signal.removeEventListener("abort", handleAbort)
      worker.terminate()
    }
    const handleAbort = () => {
      cleanup()
      reject(abortError())
    }

    signal.addEventListener("abort", handleAbort, {once: true})
    worker.onerror = event => {
      cleanup()
      reject(new Error(event.message || "PoW worker failed"))
    }
    worker.onmessage = (event: MessageEvent<FaucetPowWorkerResponse>) => {
      const message = event.data
      if (message.type === "progress") {
        onProgress(message.progress)
        return
      }

      cleanup()
      if (message.type === "solved") {
        resolve(message.solution)
      } else {
        reject(new Error(message.message))
      }
    }

    const request: FaucetPowRequest = {
      challenge: challenge.challenge,
      difficulty: challenge.difficulty,
      maxSolveTtlSeconds: challenge.maxSolveTtlSeconds,
      maxNonceAttempts: challenge.maxNonceAttempts,
    }
    worker.postMessage(request)
  })
}

async function getTestnetBalance(client: TonClient, address: string): Promise<bigint | undefined> {
  try {
    return BigInt((await client.getAddressInformation(address)).balance)
  } catch {
    return undefined
  }
}

async function waitForBalanceIncrease(
  client: TonClient,
  address: string,
  balanceBefore: bigint | undefined,
  signal: AbortSignal,
): Promise<boolean> {
  for (let attempt = 0; attempt < BALANCE_WAIT_ATTEMPTS; attempt += 1) {
    // biome-ignore lint/performance/noAwaitInLoops: balance polling is intentionally sequential.
    await abortableDelay(BALANCE_WAIT_INTERVAL_MS, signal)
    const balance = await getTestnetBalance(client, address)
    if (
      balance !== undefined &&
      (balanceBefore === undefined ? balance > 0n : balance > balanceBefore)
    ) {
      return true
    }
  }
  return false
}

function abortableDelay(durationMs: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const timer = globalThis.setTimeout(() => {
      signal.removeEventListener("abort", handleAbort)
      resolve()
    }, durationMs)
    const handleAbort = () => {
      globalThis.clearTimeout(timer)
      reject(abortError())
    }
    signal.addEventListener("abort", handleAbort, {once: true})
  })
}

function abortError(): Error {
  const error = new Error("Faucet request cancelled")
  error.name = "AbortError"
  return error
}

function isRunningPhase(phase: FaucetPhase): boolean {
  return phase === "challenge" || phase === "solving" || phase === "claiming" || phase === "waiting"
}

function requestButtonLabel(phase: FaucetPhase): string {
  if (phase === "challenge") return "Fetching challenge"
  if (phase === "solving") return "Solving proof of work"
  if (phase === "claiming") return "Submitting claim"
  if (phase === "waiting") return "Waiting for Testnet"
  if (phase === "error") return "Try again"
  if (phase === "received" || phase === "queued") return "Request again"
  return "Get testnet GRAM"
}

function faucetUsageCopy(
  usage: FaucetUsage,
  requestLimit: number,
  requestWindowMs: number,
): string {
  if (usage.limitReached && usage.availableAgainAt !== undefined) {
    return `${usage.used} of ${requestLimit} requests used · available again at ${formatDateTime(usage.availableAgainAt, {display: "time"})}`
  }
  if (usage.used > 0 && usage.lastRequestAt !== undefined) {
    return `${usage.used} of ${requestLimit} requests used · last request at ${formatDateTime(usage.lastRequestAt, {display: "time"})}`
  }
  return `Maximum of ${requestLimit} requests ${formatRecurringPeriod(requestWindowMs)}`
}

function rateLimitButtonLabel(usage: FaucetUsage): string {
  return usage.availableAgainAt === undefined
    ? "Hourly limit reached"
    : `Available again at ${formatDateTime(usage.availableAgainAt, {display: "time"})}`
}

interface GitHubRedirectParams {
  readonly grant: string | null
  readonly error: string | null
  readonly returnState?: GitHubReturnState
}

interface GitHubReturnState {
  readonly address: string
  readonly network?: string
  readonly createdAt: number
}

interface FaucetAuthInitializationResult {
  readonly status: FaucetAuthStatus
  readonly session?: FaucetSession
  readonly sessionError?: unknown
}

interface FaucetAuthToast {
  readonly variant: "error" | "success"
  readonly title: string
  readonly description: string
  readonly durationMs: number
}

function faucetAuthToast(
  params: GitHubRedirectParams,
  session: FaucetSession | undefined,
  sessionError: unknown,
): FaucetAuthToast | undefined {
  if (params.error) {
    return {
      variant: "error",
      title: "GitHub connection failed",
      description: "Authorization was cancelled or expired",
      durationMs: 8000,
    }
  }
  if (!params.grant) return undefined
  if (sessionError) {
    return {
      variant: "error",
      title: "GitHub connection failed",
      description:
        sessionError instanceof Error ? sessionError.message : "Unable to connect GitHub",
      durationMs: 8000,
    }
  }
  if (!session) return undefined
  return {
    variant: "success",
    title: "GitHub connected",
    description: `${tierLabel(session)} tier unlocks ${session.maxRequests} requests`,
    durationMs: 6000,
  }
}

function readGitHubRedirectParams(searchParams: URLSearchParams): GitHubRedirectParams {
  const fragmentParams = new URLSearchParams(globalThis.location.hash.slice(1))
  const fragmentGrant = fragmentParams.get("github_grant")
  const fragmentError = fragmentParams.get("github_error")
  const grant = fragmentGrant ?? searchParams.get("github_grant")
  const error = fragmentError ?? searchParams.get("github_error")
  return {
    grant,
    error,
    returnState: grant || error ? readGitHubReturnState() : undefined,
  }
}

function clearGitHubRedirectParams(
  params: GitHubRedirectParams,
  setSearchParams: ReturnType<typeof useSearchParams>[1],
): void {
  if (!params.grant && !params.error) return

  clearGitHubReturnState()
  setSearchParams(
    currentSearchParams => {
      const nextSearchParams = new URLSearchParams(currentSearchParams)
      nextSearchParams.delete("github_grant")
      nextSearchParams.delete("github_error")
      if (params.returnState?.network) {
        nextSearchParams.set("network", params.returnState.network)
      }
      if (params.returnState?.address) {
        nextSearchParams.set("address", params.returnState.address)
      }
      return nextSearchParams
    },
    {replace: true},
  )
}

function writeGitHubReturnState(address: string, network: string | null): void {
  const returnState: GitHubReturnState = {
    address: address.slice(0, MAX_GITHUB_RETURN_ADDRESS_LENGTH),
    network: network && network.length <= MAX_GITHUB_RETURN_NETWORK_LENGTH ? network : undefined,
    createdAt: Date.now(),
  }

  try {
    sessionStorage.setItem(GITHUB_RETURN_STATE_STORAGE_KEY, JSON.stringify(returnState))
  } catch {
    // OAuth still works when browser storage is unavailable, without restoring form state.
  }
}

function readGitHubReturnState(now = Date.now()): GitHubReturnState | undefined {
  let serialized: string | null
  try {
    serialized = sessionStorage.getItem(GITHUB_RETURN_STATE_STORAGE_KEY)
  } catch {
    return undefined
  }
  if (!serialized) return undefined

  try {
    const parsed = JSON.parse(serialized) as Partial<GitHubReturnState>
    if (
      typeof parsed.createdAt !== "number" ||
      !Number.isSafeInteger(parsed.createdAt) ||
      parsed.createdAt > now ||
      now - parsed.createdAt > GITHUB_RETURN_STATE_TTL_MS ||
      typeof parsed.address !== "string" ||
      parsed.address.length > MAX_GITHUB_RETURN_ADDRESS_LENGTH ||
      (parsed.network !== undefined &&
        (typeof parsed.network !== "string" ||
          parsed.network.length === 0 ||
          parsed.network.length > MAX_GITHUB_RETURN_NETWORK_LENGTH))
    ) {
      return undefined
    }

    return {
      address: parsed.address,
      network: parsed.network,
      createdAt: parsed.createdAt,
    }
  } catch {
    return undefined
  }
}

function clearGitHubReturnState(): void {
  try {
    sessionStorage.removeItem(GITHUB_RETURN_STATE_STORAGE_KEY)
  } catch {
    // Nothing else needs cleanup when browser storage is unavailable.
  }
}

async function initializeFaucetAuth(grant: string | null): Promise<FaucetAuthInitializationResult> {
  const status = await requestFaucetAuthStatus()
  if (!status.enabled) {
    clearFaucetSession()
    return {status}
  }

  try {
    const session = grant ? await exchangeGitHubGrant(grant) : await requestFaucetSession()
    return {status, session}
  } catch (sessionError) {
    return {status, sessionError}
  }
}

function tierLabel(session: FaucetSession): string {
  if (session.tier === "established") return "Established"
  if (session.tier === "verified") return "Verified"
  return "Guest"
}

function sessionAfterRequestError(
  session: FaucetSession | undefined,
  error: unknown,
): FaucetSession | undefined {
  return error instanceof FaucetRequestError && error.status === 401 ? undefined : session
}

function statusTitle(phase: FaucetPhase): string {
  if (phase === "challenge") return "Requesting a challenge"
  if (phase === "solving") return "Solving proof of work"
  if (phase === "claiming") return "Submitting the solution"
  if (phase === "waiting") return "Claim queued"
  if (phase === "received") return "Testnet GRAM received"
  if (phase === "queued") return "Transfer is still processing"
  if (phase === "cancelled") return "Request cancelled"
  if (phase === "error") return "Faucet request failed"
  return "Ready"
}

function statusDescription(phase: FaucetPhase, claimMessage?: string): string {
  if (phase === "challenge") return "Contacting the faucet and checking the current Testnet balance"
  if (phase === "solving") return "The calculation runs locally in a background worker"
  if (phase === "claiming") return "The faucet is verifying the nonce and queueing the transfer"
  if (phase === "waiting")
    return claimMessage ?? "The transfer is queued — watching the destination balance"
  if (phase === "received") return "Balance increased on TON Testnet"
  if (phase === "queued") {
    return "The faucet accepted the claim, but the balance has not appeared yet — check again shortly"
  }
  if (phase === "cancelled") {
    return "Balance tracking stopped — a submitted transfer may still arrive"
  }
  return ""
}

function faucetErrorCopy(error: unknown): string {
  if (error instanceof FaucetRequestError) {
    if (error.status === 403) {
      return "This address already has more Testnet GRAM than the faucet allows"
    }
    if (error.status === 429) {
      return "The faucet limit has been reached — please try again later"
    }
  }
  return error instanceof Error ? error.message : "The faucet request failed"
}

function formatHashRate(progress: FaucetPowProgress): string {
  if (progress.elapsedMs <= 0) return "Starting"
  const hashesPerSecond = progress.attempts / (progress.elapsedMs / 1000)
  if (hashesPerSecond >= 1_000_000) return `${(hashesPerSecond / 1_000_000).toFixed(1)} MH/s`
  if (hashesPerSecond >= 1000) return `${Math.round(hashesPerSecond / 1000)} kH/s`
  return `${Math.round(hashesPerSecond)} H/s`
}

function readFaucetAddressHistory(): readonly string[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(FAUCET_ADDRESS_HISTORY_STORAGE_KEY) ?? "[]")
    if (!Array.isArray(parsed)) return []

    const addresses = parsed.filter(
      (item): item is string => typeof item === "string" && item.length > 0,
    )
    return [...new Set(addresses)].slice(0, MAX_ADDRESS_HISTORY_ITEMS)
  } catch {
    return []
  }
}

function writeFaucetAddressHistory(addresses: readonly string[]): void {
  try {
    localStorage.setItem(FAUCET_ADDRESS_HISTORY_STORAGE_KEY, JSON.stringify(addresses))
  } catch {
    // The faucet still works when browser storage is unavailable
  }
}
