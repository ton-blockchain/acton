import {
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  Disclosure,
  Input,
  Select,
  useToast,
} from "@acton/ui"
import {Plus} from "lucide-react"
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

interface CreateEnvironmentDialogProps {
  readonly environments: readonly StudioEnvironment[]
  readonly importSourceEnvironments: readonly StudioEnvironment[]
  readonly open: boolean
  readonly walletNames: readonly string[]
  readonly onCreated: (environment: StudioEnvironment) => void
  readonly onOpenChange: (open: boolean) => void
}

interface EnvironmentFormState {
  readonly kind: "actonSimulatedLocalnet" | "fullTonNetwork"
  readonly name: string
  readonly port: string
  readonly forkNetwork: string
  readonly forkBlockNumber: string
  readonly accounts: readonly string[]
  readonly rateLimit: string
  readonly responseDelayMs: string
  readonly blockIntervalMs: string
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
  onCreated,
  onOpenChange,
}: CreateEnvironmentDialogProps) {
  const {showToast, updateToast} = useToast()
  const simulatedDefaultName = defaultEnvironmentName("actonSimulatedLocalnet", environments)
  const fullDefaultName = defaultEnvironmentName("fullTonNetwork", environments)
  const [form, setForm] = useState<EnvironmentFormState>(() =>
    createInitialForm(simulatedDefaultName),
  )
  const [isSubmitting, setIsSubmitting] = useState(false)
  const nextImportedAccountId = useRef(1)

  useEffect(() => {
    if (open) {
      setForm(createInitialForm(simulatedDefaultName))
      nextImportedAccountId.current = 1
    }
  }, [open, simulatedDefaultName])

  const updateForm = <Key extends keyof EnvironmentFormState>(
    key: Key,
    value: EnvironmentFormState[Key],
  ) => {
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
                port: optionalPositiveInteger(form.port, "Local port"),
                forkNetwork: form.forkNetwork || undefined,
                forkBlockNumber: form.forkNetwork
                  ? optionalPositiveInteger(form.forkBlockNumber, "Fork block")
                  : undefined,
                accounts: form.accounts,
                rateLimit: optionalPositiveInteger(form.rateLimit, "Rate limit"),
                responseDelayMs: optionalPositiveInteger(form.responseDelayMs, "Response delay"),
                blockIntervalMs: optionalPositiveInteger(form.blockIntervalMs, "Block interval"),
                noMining: form.noMining,
                mineEmptyBlocks: form.noMining ? false : form.mineEmptyBlocks,
              }
            : {
                kind: "fullTonNetwork",
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
              form.kind === "actonSimulatedLocalnet"
                ? "Fast local TON environment with compatible blocks, APIs, forks, mining controls, and time travel; Acton's custom simplified implementation, not a real TON network"
                : "Runs a complete local TON network and full indexer, supports actions, and reproduces full-node API behavior, but starts more slowly and uses more memory and disk space"
            }
            value={form.kind}
            autoFocus
            onChange={event => updateKind(event.target.value as EnvironmentFormState["kind"])}
          >
            <option value="actonSimulatedLocalnet">Simulated localnet</option>
            <option value="fullTonNetwork">Full localnet</option>
          </Select>

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
                <Input
                  label="Local port"
                  description="Leave empty to select the first available port"
                  type="number"
                  min={1}
                  max={65_535}
                  placeholder="Automatic"
                  value={form.port}
                  onChange={event => updateForm("port", event.target.value)}
                />
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

              <Disclosure label="Network and mining" contentClassName={styles.advancedContent}>
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
                    label="Block interval"
                    description="Leave empty to use the Acton project setting"
                    suffix="ms"
                    type="number"
                    min={1}
                    placeholder="Project default"
                    value={form.blockIntervalMs}
                    onChange={event => updateForm("blockIntervalMs", event.target.value)}
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
              <AccountImportEditor
                accounts={form.importedAccounts}
                sources={availableImportSources(importSourceEnvironments)}
                onAdd={addImportedAccount}
                onChange={updateImportedAccount}
                onRemove={removeImportedAccount}
              />
              <Disclosure label="Network timing" contentClassName={styles.advancedContent}>
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

function createInitialForm(name: string): EnvironmentFormState {
  return {
    kind: "actonSimulatedLocalnet",
    name,
    port: "",
    forkNetwork: "",
    forkBlockNumber: "",
    accounts: [],
    rateLimit: "",
    responseDelayMs: "",
    blockIntervalMs: "",
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
