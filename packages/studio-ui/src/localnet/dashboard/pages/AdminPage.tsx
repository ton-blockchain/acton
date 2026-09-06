import {BocInput, Button, InfoPopover, Input, Select, parseGramAmount, useToast} from "@acton/ui"
import type {ToastOptions} from "@acton/ui"
import {TonAddressInput, type TonAddressSuggestion} from "@acton/transaction-ui"
import type {LocalnetContract} from "@acton/explorer-core/api/types"
import {decodeCellInput} from "@acton/explorer-core/cell-inspector/inputNormalization"
import {normalizeAddress, parseAddress} from "@acton/explorer-core/components/utils"
import {useAddressFormat} from "@acton/explorer-core/hooks/useNetworkInfo"
import {useExplorerRoutePaths} from "@acton/explorer-core/hooks/useExplorerRoutePaths"
import {SlidersHorizontal} from "lucide-react"
import {useEffect, useMemo, useRef, useState} from "react"
import type {FC, FormEvent} from "react"
import {Link, useSearchParams} from "react-router"

import {
  fetchStudioAdminOperation,
  startStudioAdminOperation,
  StudioRequestError,
} from "../../../studioApi"
import type {
  AdminAccountChange,
  AdminOperation,
  AdminRequest,
  StudioEnvironment,
} from "../../../studioApi"
import styles from "./AdminPage.module.css"
import pageStyles from "../DashboardPage.module.css"
import {useLocalnetRuntime} from "../../LocalnetRuntimeProvider"
import {useOptionalWalletRuntime} from "../../wallet/useWalletRuntime"
import {getContractIdentity} from "../contracts/contractPresentation"

const phases: Record<string, string> = {
  preparing: "Preparing operation",
  stopping: "Stopping network",
  backingUp: "Saving recovery snapshots",
  suspending: "Suspending validators",
  building: "Building hardfork",
  installing: "Installing hardfork",
  verifying: "Verifying state on every node",
  resuming: "Checking block production",
  indexing: "Waiting for the indexer",
  restoring: "Restoring previous state",
  completed: "Changes applied",
  failed: "Operation failed",
}

const actionHelp: Record<AdminAccountChange["type"], string> = {
  balance: "Sets the account balance to the specified GRAM amount",
  code: "Replaces the account code, preserving its data and balance",
  data: "Replaces the account data, preserving its code and balance",
  replace:
    "Replaces the complete ShardAccount, including its balance, state and transaction reference",
  uninit: "Removes code and data, preserving the balance",
  freeze: "Replaces the active state with its StateInit hash, preserving the balance",
  delete: "Removes the account, including its balance, code and data",
}

/** Validates the UI's input formats before sending one ordinary root to the API */
function cellBoc(value: string): string {
  if (!value.trim()) {
    throw new Error("Enter a BoC in base64, base64url or hex")
  }

  const result = decodeCellInput(value, {
    maxEncodedChars: 16 * 1024 * 1024,
    maxInputBytes: 12 * 1024 * 1024,
    maxRoots: 1,
  })
  if (!result.ok) {
    throw new Error(result.error.message)
  }

  if (result.decoded.selectedRoot.isExotic) {
    throw new Error("Use an ordinary root cell")
  }

  // The API owns account validation and accepts one base64-encoded root.
  // Normalize only the transport encoding, preserving the supplied cell tree.
  return result.decoded.selectedRoot.toBoc().toString("base64")
}

/** Owns the edit form and feedback while the service runs the operation independently */
export const AdminPage: FC<{readonly environment: StudioEnvironment}> = ({environment}) => {
  const {showToast, updateToast, dismissToast} = useToast()
  const {client} = useLocalnetRuntime()
  const addressFormat = useAddressFormat()
  const {blockPath} = useExplorerRoutePaths()
  const walletRuntime = useOptionalWalletRuntime()
  const projectWallets = walletRuntime?.projectWallets
  const [contracts, setContracts] = useState<readonly LocalnetContract[]>([])
  const [params] = useSearchParams()
  const [address, setAddress] = useState(() =>
    normalizeAddress(params.get("address") ?? "", addressFormat),
  )
  const [action, setAction] = useState<AdminAccountChange["type"]>("balance")
  const [value, setValue] = useState("")
  const [operation, setOperation] = useState<AdminOperation | null>(null)
  const [submitting, setSubmitting] = useState(false)
  const [loaded, setLoaded] = useState(false)
  const [readingBoc, setReadingBoc] = useState(false)

  // Retain the exact request after an ambiguous response. Retrying must not
  // create a second hardfork, even if the first HTTP response was lost.
  const pending = useRef<AdminRequest | null>(null)
  const [uncertain, setUncertain] = useState(false)
  const active = operation !== null && operation.finishedAt === null
  const watchedOperationId = useRef<string | null>(null)
  const progressToastId = useRef<string | null>(null)
  const mounted = useRef(false)
  const requestRevision = useRef(0)
  const acknowledgedRequestId = useRef<string | null>(null)

  useEffect(() => {
    // Administrative edits restart the node and its APIs. Keep the last
    // suggestions until the network is ready instead of reporting expected outages.
    if (!loaded || environment.status !== "running" || active || submitting || uncertain) {
      return
    }

    let cancelled = false

    async function loadContracts() {
      try {
        const current = await client.listContracts()

        if (!cancelled) {
          setContracts(current)
        }
      } catch (cause) {
        if (!cancelled) {
          showToast({
            title: "Contract suggestions unavailable",
            description: cause instanceof Error ? cause.message : String(cause),
            variant: "error",
          })
        }
      }
    }

    void loadContracts()
    window.addEventListener("focus", loadContracts)

    return () => {
      cancelled = true
      window.removeEventListener("focus", loadContracts)
    }
  }, [active, client, environment.status, loaded, showToast, submitting, uncertain])

  const addressSuggestions = useMemo(() => {
    const suggestions = new Map<string, TonAddressSuggestion>()
    const entries = [
      ...(projectWallets ?? []).map(wallet => ({
        address: wallet.address,
        label: wallet.name,
        kind: `Wallet · ${wallet.version}`,
      })),
      ...contracts.map(contract => ({
        address: contract.address,
        label: getContractIdentity(contract).title,
        kind: "Contract",
      })),
    ]

    for (const entry of entries) {
      const target = parseAddress(entry.address)
      if (!target) continue

      const address = target.toRawString()
      const existing = suggestions.get(address)
      suggestions.set(address, {
        address: normalizeAddress(address, addressFormat),
        label:
          existing?.label && existing.label !== entry.label
            ? `${existing.label} · ${entry.label}`
            : entry.label,
        description: `${existing ? "Wallet · Contract" : entry.kind} · ${address}`,
      })
    }

    return [...suggestions.values()]
  }, [projectWallets, contracts, addressFormat])

  useEffect(() => {
    if (!operation) return
    if (pending.current && operation.id !== pending.current.id) return

    // Resume feedback for active work, but never replay a historical result
    // when the user opens the page or after they dismiss its notification.
    if (!active && watchedOperationId.current !== operation.id) return

    watchedOperationId.current = operation.id
    const message =
      operation.error ?? (operation.phase === "failed" ? "The operation failed" : null)
    const feedback = {
      title: message ? "Changes not applied" : (phases[operation.phase] ?? operation.phase),
      description:
        message ??
        (!active && operation.blockSeqno !== null ? (
          <>
            Verified at masterchain block{" "}
            <Link to={blockPath(-1, "8000000000000000", operation.blockSeqno)}>
              #{operation.blockSeqno}
            </Link>
          </>
        ) : undefined),
      variant: active ? "loading" : message ? "error" : "success",
      durationMs: active ? 0 : 6000,
    } satisfies ToastOptions

    if (progressToastId.current) {
      updateToast(progressToastId.current, feedback)
    } else {
      progressToastId.current = showToast(feedback)
    }

    if (!active) {
      watchedOperationId.current = null
      progressToastId.current = null
    }
  }, [active, blockPath, operation, showToast, updateToast])

  useEffect(() => {
    mounted.current = true

    return () => {
      // Leaving the page detaches feedback, not the operation on the service.
      // Its late HTTP response must not create a toast on another page.
      mounted.current = false

      if (progressToastId.current) {
        dismissToast(progressToastId.current)
      }
    }
  }, [dismissToast])

  useEffect(() => {
    const controller = new AbortController()
    let polling = false
    let lastError: string | undefined

    async function poll() {
      if (polling) return

      polling = true
      const revision = requestRevision.current

      try {
        const current = await fetchStudioAdminOperation(environment.id, controller.signal)
        if (controller.signal.aborted || revision !== requestRevision.current) return

        // A poll that predates a submission must not undo its acknowledged
        // result, even after the POST has cleared the pending request.
        if (!pending.current || current?.id === pending.current.id) {
          setOperation(current)
        }

        setLoaded(true)
        lastError = undefined

        if (current && current.id === pending.current?.id) {
          acknowledgedRequestId.current = current.id
          pending.current = null
          setUncertain(false)
        }
      } catch (cause) {
        if (controller.signal.aborted || revision !== requestRevision.current) return

        const message = cause instanceof Error ? cause.message : String(cause)
        if (message !== lastError) {
          showToast({
            title: "Operation status unavailable",
            description: message,
            variant: "error",
          })
          lastError = message
        }
      } finally {
        polling = false
      }
    }

    void poll()
    const timer = setInterval(() => void poll(), 1500)

    return () => {
      controller.abort()
      clearInterval(timer)
    }
  }, [environment.id, showToast])

  async function submit(event: FormEvent) {
    event.preventDefault()

    if (active || submitting || readingBoc || !loaded) return

    setSubmitting(true)
    let submittedRequest: typeof pending.current = null

    try {
      if (!pending.current) {
        const id = crypto.randomUUID()
        const target = parseAddress(address.trim())
        if (!target) {
          throw new Error("Enter a valid raw or friendly account address")
        }

        if (target.workChain !== 0 && target.workChain !== -1) {
          throw new Error("Only workchains 0 and -1 are supported")
        }

        let change: AdminAccountChange

        if (action === "balance") {
          const balance = parseGramAmount(value)

          if (balance === undefined) {
            throw new Error("Enter a nonnegative GRAM amount with at most 9 decimal places")
          }

          change = {type: action, balance: balance.toString()}
        } else if (action === "code" || action === "data" || action === "replace") {
          change = {type: action, boc: cellBoc(value)}
        } else {
          change = {type: action}
        }

        pending.current = {
          kind: "accounts",
          id,
          edits: [{address: target.toRawString(), ...change}],
        }
      }

      submittedRequest = pending.current
      requestRevision.current += 1
      watchedOperationId.current = submittedRequest.id
      progressToastId.current = showToast({
        title: phases.preparing,
        variant: "loading",
        durationMs: 0,
      })

      const result = await startStudioAdminOperation(environment.id, submittedRequest)
      if (!mounted.current) return

      // Polling can acknowledge the operation before POST returns. Its phase
      // is newer than the initial POST result, and a lost POST is then harmless.
      if (acknowledgedRequestId.current !== submittedRequest.id) {
        setOperation(result)
      }

      pending.current = null
      setUncertain(false)
    } catch (cause) {
      if (!mounted.current) return
      if (submittedRequest && acknowledgedRequestId.current === submittedRequest.id) return

      if (cause instanceof StudioRequestError && cause.status < 500) {
        pending.current = null
      }

      // Polling will reconcile accepted requests. Keep the form frozen until
      // this exact request is acknowledged or definitively rejected.
      setUncertain(pending.current !== null)

      const message = cause instanceof Error ? cause.message : String(cause)
      const feedback = {
        title: "Changes not submitted",
        description: pending.current
          ? `${message}\nRetry sends the same operation safely`
          : message,
        variant: "error",
        durationMs: 6000,
      } satisfies ToastOptions

      if (progressToastId.current) {
        updateToast(progressToastId.current, feedback)
      } else {
        showToast(feedback)
      }

      progressToastId.current = null
    } finally {
      requestRevision.current += 1

      if (mounted.current) {
        setSubmitting(false)
      }
    }
  }

  const disabled = active || submitting || uncertain

  return (
    <div className={styles.page}>
      <div className={pageStyles.settingsNotice}>
        <SlidersHorizontal size={17} aria-hidden="true" />
        <div>
          <strong>Account changes create a real hardfork</strong>
          <span>
            Studio pauses the network, saves recovery snapshots and applies the new state on every
            node
            <br />
            All nodes must be available ·{" "}
            <a
              href="https://github.com/ton-blockchain/acton/blob/master/apps/localton/docs/administrative-hardforks.md"
              target="_blank"
              rel="noreferrer"
            >
              About administrative hardforks
            </a>
          </span>
        </div>
      </div>
      <form className={styles.form} onSubmit={submit} noValidate>
        <fieldset disabled={disabled}>
          <Select
            id="admin-action"
            label="Action"
            labelAction={
              <InfoPopover ariaLabel="About this action" placement="bottom">
                {actionHelp[action]}
              </InfoPopover>
            }
            value={action}
            disabled={disabled}
            onChange={event => {
              setAction(event.target.value as typeof action)
              setValue("")
            }}
          >
            <option value="balance">Set balance</option>
            <option value="code">Replace code</option>
            <option value="data">Replace data</option>
            <option value="freeze">Freeze account</option>
            <option value="uninit">Make account uninitialized</option>
            <option value="delete">Delete account</option>
            <option value="replace">Replace complete ShardAccount</option>
          </Select>
          <TonAddressInput
            label="Account address"
            value={address}
            onValueChange={value => setAddress(normalizeAddress(value, addressFormat))}
            suggestions={addressSuggestions}
            disabled={disabled}
          />
          {action === "balance" && (
            <Input
              id="admin-value"
              label="New balance"
              suffix="GRAM"
              inputMode="decimal"
              value={value}
              onChange={event => setValue(event.target.value)}
              placeholder="10"
              disabled={disabled}
            />
          )}
          {(action === "code" || action === "data" || action === "replace") && (
            <BocInput
              key={action}
              id="admin-value"
              label={action === "replace" ? "ShardAccount" : "Cell"}
              value={value}
              onValueChange={setValue}
              onReadingChange={setReadingBoc}
              disabled={disabled}
              onError={error =>
                showToast({
                  title: "BoC file not loaded",
                  description: error.message,
                  variant: "error",
                })
              }
            />
          )}
        </fieldset>
        <div className={styles.actions}>
          <Button
            type="submit"
            variant="primary"
            loading={submitting || active}
            disabled={readingBoc || !loaded || (!uncertain && environment.status !== "running")}
          >
            {uncertain ? "Retry same operation" : "Apply changes"}
          </Button>
        </div>
      </form>
    </div>
  )
}
