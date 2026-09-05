import {Button, Input} from "@acton/ui"
import {Address, Cell, toNano} from "@ton/core"
import {useEffect, useRef, useState} from "react"
import type {FC, FormEvent} from "react"
import {useSearchParams} from "react-router"
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

const phases: Record<string, string> = {
  preparing: "Preparing operation",
  stopping: "Stopping network",
  backingUp: "Saving recovery snapshots",
  suspending: "Suspending validators",
  building: "Building hardfork",
  installing: "Installing hardfork",
  verifying: "Verifying state on every node",
  configuring: "Updating blockchain configuration",
  resuming: "Checking block production",
  indexing: "Waiting for the indexer",
  restoring: "Restoring previous state",
  completed: "Changes applied",
  failed: "Operation failed",
}

function cellBoc(value: string): string {
  const boc = value.trim()
  Cell.fromBase64(boc)
  return boc
}

export const AdminPage: FC<{readonly environment: StudioEnvironment}> = ({environment}) => {
  const [params] = useSearchParams()
  const [address, setAddress] = useState(params.get("address") ?? "")
  const [action, setAction] = useState<AdminAccountChange["type"] | "config">("balance")
  const [value, setValue] = useState("")
  const [index, setIndex] = useState("")
  const [operation, setOperation] = useState<AdminOperation | null>(null)
  const [error, setError] = useState<string>()
  const [submitting, setSubmitting] = useState(false)
  const [loaded, setLoaded] = useState(false)
  // Retain the exact request after an ambiguous response. Retrying must not
  // create a second hardfork, even if the first HTTP response was lost.
  const pending = useRef<AdminRequest | null>(null)
  const [uncertain, setUncertain] = useState(false)
  const active = operation !== null && operation.finishedAt === null

  useEffect(() => {
    const controller = new AbortController()
    let polling = false
    async function poll() {
      if (polling) return
      polling = true
      try {
        const current = await fetchStudioAdminOperation(environment.id, controller.signal)
        if (controller.signal.aborted) return
        setOperation(current)
        setLoaded(true)
        if (current?.id === pending.current?.id) {
          pending.current = null
          setUncertain(false)
          setError(undefined)
        }
      } catch (cause) {
        if (!controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : String(cause))
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
  }, [environment.id])

  async function submit(event: FormEvent) {
    event.preventDefault()
    setError(undefined)
    setSubmitting(true)
    try {
      if (!pending.current) {
        const id = crypto.randomUUID()
        if (action === "config") {
          if (
            !/^-?\d+$/.test(index) ||
            Number(index) < -2_147_483_648 ||
            Number(index) > 2_147_483_647
          )
            throw new Error("Enter a signed 32-bit parameter number")
          pending.current = {id, kind: "config", index: Number(index), boc: cellBoc(value)}
        } else {
          const target = Address.parse(address.trim())
          if (target.workChain !== 0 && target.workChain !== -1)
            throw new Error("Only workchains 0 and -1 are supported")
          let change: AdminAccountChange
          if (action === "balance") {
            if (!/^\d+(\.\d{1,9})?$/.test(value.trim()))
              throw new Error("Enter a nonnegative TON amount with at most 9 decimal places")
            change = {type: action, balance: toNano(value.trim()).toString()}
          } else if (action === "code" || action === "data" || action === "replace") {
            change = {type: action, boc: cellBoc(value)}
          } else {
            change = {type: action}
          }
          pending.current = {
            id,
            kind: "accounts",
            edits: [{address: target.toRawString(), ...change}],
          }
        }
      }
      const result = await startStudioAdminOperation(environment.id, pending.current)
      setOperation(result)
      pending.current = null
      setUncertain(false)
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
      if (cause instanceof StudioRequestError && cause.status < 500) pending.current = null
      // Polling will reconcile accepted requests. Keep the form frozen until
      // this exact request is acknowledged or definitively rejected.
      setUncertain(pending.current !== null)
    } finally {
      setSubmitting(false)
    }
  }

  const needsValue = ["balance", "code", "data", "replace", "config"].includes(action)
  const disabled = active || submitting || uncertain
  return (
    <div className={styles.page}>
      <div className={styles.notice}>
        <strong>Edit network state</strong>
        <p>
          The network pauses while account changes are applied. Recovery snapshots are saved
          automatically. All nodes must be available.
        </p>
      </div>
      {operation && (
        <section className={styles.status} aria-live="polite" aria-busy={active}>
          <strong>{phases[operation.phase] ?? operation.phase}</strong>
          {active && <p>You can leave this page; the operation will continue.</p>}
          {operation.blockSeqno !== null && (
            <p>
              Verified at masterchain block #{operation.blockSeqno}. Block production and indexing
              have resumed.
            </p>
          )}
          {operation.error && <p className={styles.error}>{operation.error}</p>}
        </section>
      )}
      <form className={styles.form} onSubmit={submit}>
        <fieldset disabled={disabled}>
          <label htmlFor="admin-action">Action</label>
          <select
            id="admin-action"
            value={action}
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
            <option value="config">Set configuration parameter</option>
          </select>
          {action === "config" ? (
            <>
              <label htmlFor="admin-index">Parameter number</label>
              <Input
                id="admin-index"
                value={index}
                onChange={event => setIndex(event.target.value)}
                placeholder="21"
                required
              />
            </>
          ) : (
            <>
              <label htmlFor="admin-address">Account address</label>
              <Input
                id="admin-address"
                value={address}
                onChange={event => setAddress(event.target.value)}
                placeholder="0:… or EQ…"
                required
              />
            </>
          )}
          {needsValue && (
            <>
              <label htmlFor="admin-value">
                {action === "balance"
                  ? "New balance (TON)"
                  : action === "replace"
                    ? "ShardAccount (base64 BoC)"
                    : "Cell (base64 BoC)"}
              </label>
              {action === "balance" ? (
                <Input
                  id="admin-value"
                  value={value}
                  onChange={event => setValue(event.target.value)}
                  placeholder="10"
                  required
                />
              ) : (
                <textarea
                  id="admin-value"
                  value={value}
                  onChange={event => setValue(event.target.value)}
                  rows={7}
                  spellCheck={false}
                  required
                />
              )}
            </>
          )}
          {action === "uninit" && <p>Removes code and data, preserving the balance.</p>}
          {action === "freeze" && (
            <p>Replaces the active state with its StateInit hash, preserving the balance.</p>
          )}
          {action === "delete" && <p>Removes the account, including its balance, code and data.</p>}
          {action === "config" && (
            <p>
              The configuration contract applies the parameter. The operation waits for confirmation
              and checks that blocks continue to be produced.
            </p>
          )}
        </fieldset>
        {error && (
          <p role="alert" className={styles.error}>
            {error}
          </p>
        )}
        {uncertain && (
          <p>The response could not be confirmed. Retry sends the same operation safely.</p>
        )}
        <Button
          type="submit"
          disabled={
            !loaded || active || submitting || (!uncertain && environment.status !== "running")
          }
        >
          {submitting
            ? "Submitting…"
            : uncertain
              ? "Retry same operation"
              : active
                ? "Applying changes…"
                : "Apply changes"}
        </Button>
        {!active && environment.status !== "running" && (
          <p>Start the environment to apply changes.</p>
        )}
      </form>
    </div>
  )
}
