import {
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  Disclosure,
  Input,
  Select,
  Tooltip,
  useToast,
} from "@acton/ui"
import {Info, Plus, X} from "lucide-react"
import {type FormEvent, useEffect, useRef, useState} from "react"

import {
  type CreateEnvironmentRequest,
  type StudioEnvironment,
  createStudioEnvironment,
} from "../studioApi"
import {
  AccountImportEditor,
  availableImportSources,
  preferredImportSource,
  type ImportedAccountForm,
} from "../components/AccountImportEditor"
import {WalletNamesInput} from "./WalletNamesInput"

import styles from "./CreateEnvironmentDialog.module.css"

const FULL_LOCALNET_DOCKER_NOTICE_DISMISSED_STORAGE_KEY =
  "acton-studio:full-localnet-docker-notice-dismissed"

interface CreateEnvironmentDialogProps {
  readonly environments: readonly StudioEnvironment[]
  readonly importSourceEnvironments: readonly StudioEnvironment[]
  readonly open: boolean
  readonly walletNames: readonly string[]
  readonly defaultStartupAccounts?: readonly string[]
  readonly onCreated: (environment: StudioEnvironment) => void
  readonly onOpenChange: (open: boolean) => void
}

interface EnvironmentFormState {
  readonly kind: "actonSimulatedLocalnet" | "fullTonNetwork"
  readonly name: string
  readonly forkNetwork: string
  readonly forkBlockNumber: string
  readonly accounts: readonly string[]
  readonly rateLimit: string
  readonly responseDelayMs: string
  readonly blockTimeMs: string
  readonly fullTonBlockTimeMs: string
  readonly fullTonElectionTimeSeconds: string
  readonly noMining: boolean
  readonly mineEmptyBlocks: boolean
  readonly importedAccounts: readonly ImportedAccountForm[]
}

export function CreateEnvironmentDialog({
  environments,
  importSourceEnvironments,
  open,
  walletNames,
  defaultStartupAccounts,
  onCreated,
  onOpenChange,
}: CreateEnvironmentDialogProps) {
  const {showToast, updateToast} = useToast()
  const simulatedDefaultName = defaultEnvironmentName("actonSimulatedLocalnet", environments)
  const fullDefaultName = defaultEnvironmentName("fullTonNetwork", environments)
  const [form, setForm] = useState<EnvironmentFormState>(() =>
    createInitialForm(simulatedDefaultName, defaultStartupAccounts),
  )
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [isDockerNoticeDismissed, setIsDockerNoticeDismissed] = useState(
    () =>
      globalThis.localStorage.getItem(FULL_LOCALNET_DOCKER_NOTICE_DISMISSED_STORAGE_KEY) === "true",
  )
  const nextImportedAccountId = useRef(1)
  const wasOpen = useRef(false)
  const hasInitializedAccounts = useRef(false)

  useEffect(() => {
    if (open && !wasOpen.current) {
      setForm(createInitialForm(simulatedDefaultName, defaultStartupAccounts))
      nextImportedAccountId.current = 1
      hasInitializedAccounts.current = defaultStartupAccounts !== undefined
    } else if (open && !hasInitializedAccounts.current && defaultStartupAccounts !== undefined) {
      // Workspace info can arrive after opening; only prefill accounts that are still untouched.
      setForm(current => ({...current, accounts: [...new Set(defaultStartupAccounts)]}))
      hasInitializedAccounts.current = true
    }
    wasOpen.current = open
  }, [open, simulatedDefaultName, defaultStartupAccounts])

  const updateForm = <Key extends keyof EnvironmentFormState>(
    key: Key,
    value: EnvironmentFormState[Key],
  ) => {
    if (key === "accounts") hasInitializedAccounts.current = true
    setForm(current => ({...current, [key]: value}))
  }

  const updateKind = (kind: EnvironmentFormState["kind"]) => {
    setForm(current => {
      const currentDefaultName =
        current.kind === "actonSimulatedLocalnet" ? simulatedDefaultName : fullDefaultName
      return {
        ...current,
        kind,
        name:
          current.name === currentDefaultName
            ? kind === "actonSimulatedLocalnet"
              ? simulatedDefaultName
              : fullDefaultName
            : current.name,
      }
    })
  }

  const updateForkNetwork = (forkNetwork: string) => {
    setForm(current => ({
      ...current,
      forkNetwork,
      forkBlockNumber: forkNetwork ? current.forkBlockNumber : "",
    }))
  }

  const updateNoMining = (noMining: boolean) => {
    setForm(current => ({
      ...current,
      noMining,
      mineEmptyBlocks: noMining ? false : current.mineEmptyBlocks,
    }))
  }

  const addImportedAccount = () => {
    const sourceEnvironmentId = preferredImportSource(importSourceEnvironments)?.id
    if (!sourceEnvironmentId) return
    const account: ImportedAccountForm = {
      id: nextImportedAccountId.current,
      sourceEnvironmentId,
      name: "",
      address: "",
    }
    nextImportedAccountId.current += 1
    setForm(current => ({
      ...current,
      importedAccounts: [...current.importedAccounts, account],
    }))
  }

  const updateImportedAccount = (
    id: number,
    update: Partial<Pick<ImportedAccountForm, "sourceEnvironmentId" | "name" | "address">>,
  ) => {
    setForm(current => ({
      ...current,
      importedAccounts: current.importedAccounts.map(account =>
        account.id === id ? {...account, ...update} : account,
      ),
    }))
  }

  const removeImportedAccount = (id: number) => {
    setForm(current => ({
      ...current,
      importedAccounts: current.importedAccounts.filter(account => account.id !== id),
    }))
  }

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (isSubmitting) return

    let request: CreateEnvironmentRequest
    try {
      const importedAccounts = form.importedAccounts.map(account => ({
        sourceEnvironmentId: account.sourceEnvironmentId,
        name: account.name.trim() || undefined,
        address: account.address.trim(),
      }))
      if (importedAccounts.some(account => !account.address)) {
        throw new Error("Enter an account address or remove the empty import row")
      }
      request = {
        name: form.name.trim(),
        config:
          form.kind === "actonSimulatedLocalnet"
            ? {
                kind: "actonSimulatedLocalnet",
                forkNetwork: form.forkNetwork || undefined,
                forkBlockNumber: form.forkNetwork
                  ? optionalPositiveInteger(form.forkBlockNumber, "Fork block")
                  : undefined,
                accounts: form.accounts,
                rateLimit: optionalPositiveInteger(form.rateLimit, "Rate limit"),
                responseDelayMs: optionalPositiveInteger(form.responseDelayMs, "Response delay"),
                blockTimeMs: optionalPositiveInteger(form.blockTimeMs, "Block time"),
                noMining: form.noMining,
                mineEmptyBlocks: form.noMining ? false : form.mineEmptyBlocks,
              }
            : {
                kind: "fullTonNetwork",
                accounts: form.accounts,
                blockTimeMs: optionalPositiveInteger(
                  form.fullTonBlockTimeMs,
                  "Block time",
                  4_294_967_295,
                ),
                electionTimeSeconds: optionalPositiveInteger(
                  form.fullTonElectionTimeSeconds,
                  "Election time",
                  4_294_967_295,
                ),
                importedAccounts,
              },
      }
    } catch (error) {
      showToast({
        title: "Check environment settings",
        description: getErrorMessage(error),
        variant: "error",
      })
      return
    }

    const toastId = showToast({
      title: "Creating environment",
      description: request.name,
      variant: "loading",
      durationMs: 0,
    })
    setIsSubmitting(true)
    try {
      const environment = await createStudioEnvironment(request)
      onCreated(environment)
      onOpenChange(false)
      updateToast(toastId, {
        title: `${environment.name} is starting`,
        description: "Studio is starting the network in the background",
        variant: "success",
        durationMs: 4000,
      })
    } catch (error) {
      updateToast(toastId, {
        title: "Failed to create environment",
        description: getErrorMessage(error),
        variant: "error",
        durationMs: 8000,
      })
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Create environment"
      maxWidth="60rem"
      busy={isSubmitting}
      contentPadding="none"
      contentClassName={styles.dialogContent}
    >
      <form className={styles.form} onSubmit={event => void handleSubmit(event)}>
        <div className={styles.formBody}>
          <Select
            label="Environment type"
            description={
              form.kind === "actonSimulatedLocalnet" ? (
                <>
                  Starts instantly, uses very few system resources, and provides TON-compatible
                  blocks, TON Center-compatible v2/v3 APIs, forks, mining controls, and time travel
                  without validators or consensus. Use{" "}
                  <button
                    type="button"
                    className={styles.environmentTypeLink}
                    onClick={() => updateKind("fullTonNetwork")}
                  >
                    Full localnet
                  </button>{" "}
                  for real TON validators and full-node behavior
                </>
              ) : (
                <>
                  Runs real TON validators, the TON Center v2 API, and a v3 indexer for validator,
                  full-node, and indexed chain workflows, but starts more slowly and uses more
                  resources. Use{" "}
                  <button
                    type="button"
                    className={styles.environmentTypeLink}
                    onClick={() => updateKind("actonSimulatedLocalnet")}
                  >
                    Simulated localnet
                  </button>{" "}
                  for instant, lightweight development, forks, and deterministic network control
                </>
              )
            }
            value={form.kind}
            autoFocus
            onChange={event => updateKind(event.target.value as EnvironmentFormState["kind"])}
          >
            <option value="actonSimulatedLocalnet">Simulated localnet</option>
            <option value="fullTonNetwork">Full localnet</option>
          </Select>

          {form.kind === "fullTonNetwork" && !isDockerNoticeDismissed ? (
            <div className={styles.dockerRequirement}>
              <Info size={14} aria-hidden="true" />
              <span>
                Docker must be installed and running; the first launch downloads the 200 MB{" "}
                <a
                  href="https://github.com/ton-blockchain/acton/pkgs/container/localton"
                  target="_blank"
                  rel="noreferrer"
                >
                  localton
                </a>{" "}
                image
              </span>
              <Tooltip content="Don't show again">
                <button
                  type="button"
                  className={styles.dockerRequirementDismiss}
                  aria-label="Don't show again"
                  onClick={() => {
                    setIsDockerNoticeDismissed(true)
                    globalThis.localStorage.setItem(
                      FULL_LOCALNET_DOCKER_NOTICE_DISMISSED_STORAGE_KEY,
                      "true",
                    )
                  }}
                >
                  <X size={14} aria-hidden="true" />
                </button>
              </Tooltip>
            </div>
          ) : undefined}

          <Input
            label="Name"
            description="Used in Studio and environment history"
            value={form.name}
            maxLength={80}
            required
            spellCheck
            onChange={event => updateForm("name", event.target.value)}
          />

          {form.kind === "actonSimulatedLocalnet" ? (
            <>
              <div className={styles.formGrid}>
                <Select
                  label="Initial state"
                  description="Start clean or fork an existing TON network"
                  value={form.forkNetwork}
                  onChange={event => updateForkNetwork(event.target.value)}
                >
                  <option value="">Clean network</option>
                  <option value="mainnet">Fork mainnet</option>
                  <option value="testnet">Fork testnet</option>
                </Select>
                <Input
                  label="Fork block"
                  description="Leave empty to use the latest available state"
                  type="number"
                  min={1}
                  placeholder="Latest"
                  disabled={!form.forkNetwork}
                  value={form.forkBlockNumber}
                  onChange={event => updateForm("forkBlockNumber", event.target.value)}
                />
              </div>

              <WalletNamesInput
                values={form.accounts}
                walletNames={walletNames}
                onChange={values => updateForm("accounts", values)}
              />

              <Disclosure
                className={styles.compactDisclosure}
                label="Network and mining"
                contentClassName={styles.advancedContent}
              >
                <div className={styles.formGrid}>
                  <Input
                    label="Rate limit"
                    description="Maximum API requests per second"
                    suffix="RPS"
                    type="number"
                    min={1}
                    placeholder="Unlimited"
                    value={form.rateLimit}
                    onChange={event => updateForm("rateLimit", event.target.value)}
                  />
                  <Input
                    label="Response delay"
                    description="Delay TON Center and Emulate API responses"
                    suffix="ms"
                    type="number"
                    min={1}
                    placeholder="None"
                    value={form.responseDelayMs}
                    onChange={event => updateForm("responseDelayMs", event.target.value)}
                  />
                  <Input
                    label="Block time"
                    description="Target interval between automatic blocks; ignored with manual mining"
                    suffix="ms"
                    type="number"
                    min={1}
                    placeholder="Project default"
                    value={form.blockTimeMs}
                    onChange={event => updateForm("blockTimeMs", event.target.value)}
                  />
                </div>
                <div className={styles.checkboxGroup}>
                  <Checkbox
                    label="Manual mining"
                    description="Create blocks only when requested"
                    checked={form.noMining}
                    onChange={event => updateNoMining(event.target.checked)}
                  />
                  <Checkbox
                    label="Mine empty blocks"
                    description="Keep producing blocks when no messages are pending"
                    checked={form.mineEmptyBlocks}
                    disabled={form.noMining}
                    onChange={event => updateForm("mineEmptyBlocks", event.target.checked)}
                  />
                </div>
              </Disclosure>
            </>
          ) : (
            <div className={styles.fullTonFields}>
              <WalletNamesInput
                values={form.accounts}
                walletNames={walletNames}
                onChange={values => updateForm("accounts", values)}
              />
              <AccountImportEditor
                accounts={form.importedAccounts}
                sources={availableImportSources(importSourceEnvironments)}
                onAdd={addImportedAccount}
                onChange={updateImportedAccount}
                onRemove={removeImportedAccount}
              />
              <Disclosure
                className={styles.compactDisclosure}
                label="Network timing"
                contentClassName={styles.advancedContent}
              >
                <div className={styles.formGrid}>
                  <Input
                    label="Block time"
                    description="Target interval between blocks"
                    suffix="ms"
                    type="number"
                    min={1}
                    max={4_294_967_295}
                    placeholder="1,000"
                    value={form.fullTonBlockTimeMs}
                    onChange={event => updateForm("fullTonBlockTimeMs", event.target.value)}
                  />
                  <Input
                    label="Election time"
                    description="Duration of each validator round"
                    suffix="s"
                    type="number"
                    min={4}
                    max={4_294_967_295}
                    placeholder="120"
                    value={form.fullTonElectionTimeSeconds}
                    onChange={event => updateForm("fullTonElectionTimeSeconds", event.target.value)}
                  />
                </div>
              </Disclosure>
            </div>
          )}
        </div>

        <DialogActions className={styles.formActions}>
          <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
            {isSubmitting ? "Close" : "Cancel"}
          </Button>
          <Button
            type="submit"
            variant="primary"
            loading={isSubmitting}
            leadingIcon={<Plus size={16} aria-hidden="true" />}
          >
            Create environment
          </Button>
        </DialogActions>
      </form>
    </Dialog>
  )
}

function createInitialForm(name: string, accounts: readonly string[] = []): EnvironmentFormState {
  return {
    kind: "actonSimulatedLocalnet",
    name,
    forkNetwork: "",
    forkBlockNumber: "",
    accounts: [...new Set(accounts)],
    rateLimit: "",
    responseDelayMs: "",
    blockTimeMs: "",
    fullTonBlockTimeMs: "",
    fullTonElectionTimeSeconds: "",
    noMining: false,
    mineEmptyBlocks: false,
    importedAccounts: [],
  }
}

function defaultEnvironmentName(
  kind: EnvironmentFormState["kind"],
  environments: readonly StudioEnvironment[],
): string {
  const prefix = kind === "actonSimulatedLocalnet" ? "Simulated localnet" : "Full localnet"
  const names = new Set(environments.map(environment => environment.name))
  let suffix = 1

  while (names.has(`${prefix} ${suffix}`)) suffix += 1

  return `${prefix} ${suffix}`
}

function optionalPositiveInteger(
  value: string,
  label: string,
  maximum?: number,
): number | undefined {
  const trimmed = value.trim()
  if (!trimmed) return

  const parsed = Number(trimmed)
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive whole number`)
  }
  if (maximum !== undefined && parsed > maximum) {
    throw new Error(`${label} must not exceed ${maximum}`)
  }
  return parsed
}

function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
