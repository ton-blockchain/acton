import {Button, Dialog, DialogActions, InlineButton, useToast} from "@acton/ui"
import {Check, Download, LoaderCircle} from "lucide-react"
import {createContext, useContext, useEffect, useRef, useState} from "react"
import {Link} from "react-router"

import {adminOperationPhases} from "../localnet/adminOperation"
import {
  type AdminOperation,
  type ImportAccountsRequest,
  type StudioEnvironment,
  StudioRequestError,
  fetchStudioAdminOperation,
  importStudioAccounts,
} from "../studioApi"
import {
  AccountImportEditor,
  type ImportedAccountForm,
  availableImportSources,
  preferredImportSource,
} from "./AccountImportEditor"
import styles from "./ImportAccountsAction.module.css"

interface ImportAccountsActionProps {
  readonly environment: StudioEnvironment
  readonly environments: readonly StudioEnvironment[]
  readonly basePath: string
  readonly onCompleted: () => Promise<void>
  readonly open: boolean
  readonly onOpenChange: (open: boolean) => void
}

/** Connects the dashboard action to a dialog that survives workspace restarts. */
export const ImportAccountsActionContext = createContext<{
  readonly open: boolean
  readonly onOpenChange: (open: boolean) => void
} | null>(null)

/** Uses the same inline action as the other controls in the environment toolbar. */
export function ImportAccountsButton() {
  const action = useContext(ImportAccountsActionContext)
  if (!action) return null

  return (
    <InlineButton
      leadingIcon={<Download size={15} aria-hidden="true" />}
      aria-haspopup="dialog"
      aria-expanded={action.open}
      onClick={() => action.onOpenChange(true)}
    >
      Import accounts
    </InlineButton>
  )
}

/** Keeps import progress outside the workspace, which unmounts during a hardfork. */
export function ImportAccountsAction({
  environment,
  environments,
  basePath,
  onCompleted,
  open,
  onOpenChange,
}: ImportAccountsActionProps) {
  const {showToast} = useToast()
  const storageKey = `actonStudioEnvironment:${environment.id}:accountImport`
  const [request, setRequest] = useState<ImportAccountsRequest | null>(() => {
    try {
      const saved = JSON.parse(
        localStorage.getItem(storageKey) ?? "null",
      ) as ImportAccountsRequest | null
      return saved &&
        typeof saved.id === "string" &&
        Array.isArray(saved.accounts) &&
        saved.accounts.every(
          account =>
            account &&
            typeof account.address === "string" &&
            typeof account.sourceEnvironmentId === "string" &&
            (account.name === undefined || typeof account.name === "string"),
        )
        ? saved
        : null
    } catch {
      return null
    }
  })
  const restoreProgress = useRef(Boolean(request))
  const [accounts, setAccounts] = useState<readonly ImportedAccountForm[]>(
    () =>
      request?.accounts.map((account, id) => ({...account, id, name: account.name ?? ""})) ?? [],
  )
  const [operation, setOperation] = useState<AdminOperation | null>(null)
  const [submitting, setSubmitting] = useState(false)
  const [uncertain, setUncertain] = useState(Boolean(request))
  const acknowledged = useRef<string | null>(null)
  const mounted = useRef(true)
  const sources = availableImportSources(environments).filter(
    source => source.id !== environment.id,
  )
  const active = operation !== null && operation.finishedAt === null
  const completed = operation?.phase === "completed"
  const frozen = Boolean(request) || submitting
  const submitLock = useRef(false)

  useEffect(() => {
    // The toolbar can be absent while the network starts. Restore the pending
    // dialog on reload so its progress remains accessible during the hardfork.
    if (restoreProgress.current) {
      restoreProgress.current = false
      onOpenChange(true)
    }
  }, [onOpenChange])

  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])

  // Only public selections are stored here. The server persists the pinned cells
  // and finishes registration even when the browser is closed or storage is disabled.
  useEffect(() => {
    try {
      if (request) localStorage.setItem(storageKey, JSON.stringify(request))
      else localStorage.removeItem(storageKey)
    } catch {
      /* Browser storage is optional; the service still owns the operation */
    }
  }, [request, storageKey])

  useEffect(() => {
    if (!request) return
    const controller = new AbortController()
    let timer: ReturnType<typeof setTimeout>
    let lastError: string | undefined

    async function poll() {
      try {
        const current = await fetchStudioAdminOperation(
          environment.id,
          controller.signal,
          request?.id,
        )
        if (controller.signal.aborted) return
        if (current && current.id === request?.id) {
          acknowledged.current = current.id
          setOperation(current)
          setUncertain(false)
          if (current.finishedAt) {
            setRequest(null)
            showToast({
              title: current.phase === "completed" ? "Accounts imported" : "Accounts not imported",
              description: current.error ?? undefined,
              variant: current.phase === "completed" ? "success" : "error",
            })
            if (current.phase === "completed") await onCompleted()
            return
          }
        }
        lastError = undefined
      } catch (cause) {
        if (controller.signal.aborted) return
        const message = cause instanceof Error ? cause.message : String(cause)
        if (message !== lastError)
          showToast({title: "Import status unavailable", description: message, variant: "error"})
        lastError = message
      }
      if (!controller.signal.aborted) timer = setTimeout(() => void poll(), 1500)
    }
    void poll()
    return () => {
      controller.abort()
      clearTimeout(timer)
    }
  }, [environment.id, onCompleted, request, showToast])

  async function submit() {
    if (submitLock.current || active) return
    let submitted: ImportAccountsRequest
    try {
      if (accounts.length === 0) throw new Error("Add at least one account to import")
      if (accounts.some(account => !account.address.trim()))
        throw new Error("Enter an account address or remove the empty import row")
      submitted = request ?? {
        id: crypto.randomUUID(),
        accounts: accounts.map(account => ({
          sourceEnvironmentId: account.sourceEnvironmentId,
          address: account.address.trim(),
          name: account.name.trim() || undefined,
        })),
      }
    } catch (cause) {
      showToast({
        title: "Check the selected accounts",
        description: cause instanceof Error ? cause.message : String(cause),
        variant: "error",
      })
      return
    }

    submitLock.current = true
    setSubmitting(true)
    setRequest(submitted)
    setOperation(null)
    try {
      const result = await importStudioAccounts(environment.id, submitted)
      if (!mounted.current || acknowledged.current === submitted.id) return
      setOperation(result)
      setUncertain(false)
    } catch (cause) {
      if (!mounted.current || acknowledged.current === submitted.id) return
      const rejected = cause instanceof StudioRequestError && cause.status < 500
      if (rejected) setRequest(null)
      setUncertain(!rejected)
      showToast({
        title: "Import not submitted",
        description: `${cause instanceof Error ? cause.message : String(cause)}${rejected ? "" : "\nRetry sends the same import safely"}`,
        variant: "error",
      })
    } finally {
      submitLock.current = false
      if (mounted.current) setSubmitting(false)
    }
  }

  return (
    <>
      <Dialog
        open={open}
        onOpenChange={onOpenChange}
        title="Import accounts"
        maxWidth="60rem"
        footer={
          <DialogActions className={styles.actions}>
            <Button variant="secondary" onClick={() => onOpenChange(false)}>
              {frozen || completed ? "Close" : "Cancel"}
            </Button>
            {completed ? (
              <Link
                className={styles.contractsLink}
                to={`${basePath}/contracts`}
                onClick={() => onOpenChange(false)}
              >
                View contracts
              </Link>
            ) : (
              <Button
                variant="primary"
                loading={submitting || active}
                disabled={
                  (!uncertain && frozen) ||
                  accounts.length === 0 ||
                  (!uncertain && environment.status !== "running")
                }
                onClick={() => void submit()}
              >
                {uncertain ? "Retry same import" : "Import accounts"}
              </Button>
            )}
          </DialogActions>
        }
      >
        <div className={styles.content}>
          <p className={styles.notice}>
            The network pauses for a hardfork, imports the selected accounts, then resumes. Existing
            accounts at those addresses are replaced
          </p>
          <fieldset className={styles.fields} disabled={frozen}>
            <AccountImportEditor
              accounts={accounts}
              sources={sources}
              description="Copy account balance, code, and data from another environment"
              onAdd={() => {
                const source = preferredImportSource(sources)
                if (source) {
                  setOperation(null)
                  setAccounts(current => [
                    ...current,
                    {
                      id: Math.max(-1, ...current.map(account => account.id)) + 1,
                      sourceEnvironmentId: source.id,
                      name: "",
                      address: "",
                    },
                  ])
                }
              }}
              onChange={(id, update) => {
                setOperation(null)
                setAccounts(current =>
                  current.map(account => (account.id === id ? {...account, ...update} : account)),
                )
              }}
              onRemove={id => {
                setOperation(null)
                setAccounts(current => current.filter(account => account.id !== id))
              }}
            />
          </fieldset>
          {(submitting || active || completed) && (
            <div className={styles.progress} role="status" aria-live="polite">
              {completed ? (
                <Check size={17} aria-hidden="true" />
              ) : (
                <LoaderCircle className={styles.spinner} size={17} aria-hidden="true" />
              )}
              <span>
                {completed
                  ? "Accounts imported"
                  : operation
                    ? (adminOperationPhases[operation.phase] ?? operation.phase)
                    : "Loading source accounts"}
              </span>
            </div>
          )}
          {request && (
            <p className={styles.notice}>
              The operation continues in the background after you close this dialog
            </p>
          )}
        </div>
      </Dialog>
    </>
  )
}
