import {
  Button,
  Checkbox,
  Dialog,
  Disclosure,
  InlineAction,
  Input,
  Select,
  useToast,
} from "@acton/ui"
import {Plus, Trash2} from "lucide-react"
import {type FormEvent, useEffect, useRef, useState} from "react"

import {TonClient} from "@acton/explorer-core/api/client"
import {TonAddressInput, type TonAddressSuggestion} from "@acton/transaction-ui"
import {
  type CreateEnvironmentRequest,
  type StudioEnvironment,
  createStudioEnvironment,
} from "../studioApi"
import {WalletNamesInput} from "./WalletNamesInput"

import styles from "./CreateEnvironmentDialog.module.css"

const MAX_FULL_TON_VALIDATORS = 7

interface CreateEnvironmentDialogProps {
  readonly environmentCount: number
  readonly importSourceEnvironments: readonly StudioEnvironment[]
  readonly open: boolean
  readonly walletNames: readonly string[]
  readonly onCreated: (environment: StudioEnvironment) => void
  readonly onOpenChange: (open: boolean) => void
}

interface EnvironmentFormState {
  readonly kind: "actonLocalnet" | "fullTonNetwork"
  readonly name: string
  readonly port: string
  readonly forkNetwork: string
  readonly forkBlockNumber: string
  readonly accounts: readonly string[]
  readonly rateLimit: string
  readonly responseDelayMs: string
  readonly blockIntervalMs: string
  readonly noMining: boolean
  readonly mineEmptyBlocks: boolean
  readonly validators: string
  readonly importedAccounts: readonly ImportedAccountForm[]
}

interface ImportedAccountForm {
  readonly id: number
  readonly sourceEnvironmentId: string
  readonly name: string
  readonly address: string
}

export function CreateEnvironmentDialog({
  environmentCount,
  importSourceEnvironments,
  open,
  walletNames,
  onCreated,
  onOpenChange,
}: CreateEnvironmentDialogProps) {
  const {showToast} = useToast()
  const [form, setForm] = useState<EnvironmentFormState>(() => createInitialForm(environmentCount))
  const [isSubmitting, setIsSubmitting] = useState(false)
  const nextImportedAccountId = useRef(1)

  useEffect(() => {
    if (open) {
      setForm(createInitialForm(environmentCount))
      nextImportedAccountId.current = 1
    }
  }, [environmentCount, open])

  const updateForm = <Key extends keyof EnvironmentFormState>(
    key: Key,
    value: EnvironmentFormState[Key],
  ) => {
    setForm(current => ({...current, [key]: value}))
  }

  const updateKind = (kind: EnvironmentFormState["kind"]) => {
    setForm(current => {
      const currentDefaultName = defaultEnvironmentName(current.kind, environmentCount)
      return {
        ...current,
        kind,
        name:
          current.name === currentDefaultName
            ? defaultEnvironmentName(kind, environmentCount)
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
          form.kind === "actonLocalnet"
            ? {
                kind: "actonLocalnet",
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
                validators: optionalPositiveInteger(
                  form.validators,
                  "Validators",
                  MAX_FULL_TON_VALIDATORS,
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

    setIsSubmitting(true)
    try {
      const environment = await createStudioEnvironment(request)
      onCreated(environment)
      onOpenChange(false)
      showToast({
        title: `${environment.name} is starting`,
        description: "Studio is starting the network in the background",
        variant: "success",
      })
    } catch (error) {
      showToast({
        title: "Failed to create environment",
        description: getErrorMessage(error),
        variant: "error",
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
      dismissible={!isSubmitting}
      contentClassName={styles.dialogContent}
    >
      <form className={styles.form} onSubmit={event => void handleSubmit(event)}>
        <div className={styles.formBody}>
          <Select
            label="Environment type"
            description={
              form.kind === "actonLocalnet"
                ? "Uses the Acton emulator instead of TON validators, starts quickly, uses little disk space, and supports forks, manual mining, time travel, and network controls, but can behave differently from a real TON network in edge cases"
                : "Runs local TON validators and a full indexer, produces blocks through validator nodes, supports actions, and reproduces full-node API behavior, but starts more slowly and uses more memory and disk space"
            }
            value={form.kind}
            autoFocus
            onChange={event => updateKind(event.target.value as EnvironmentFormState["kind"])}
          >
            <option value="actonLocalnet">Simulated localnet</option>
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

          {form.kind === "actonLocalnet" ? (
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
                    description="Delay TonCenter and Emulate API responses"
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
              <Input
                label="Validators"
                description="Validator nodes started for this network. Each additional validator increases the startup time"
                type="number"
                min={1}
                max={MAX_FULL_TON_VALIDATORS}
                value={form.validators}
                onChange={event => updateForm("validators", event.target.value)}
              />
              <AccountImportEditor
                accounts={form.importedAccounts}
                sources={availableImportSources(importSourceEnvironments)}
                onAdd={addImportedAccount}
                onChange={updateImportedAccount}
                onRemove={removeImportedAccount}
              />
            </div>
          )}
        </div>

        <footer className={styles.formActions}>
          <Button
            type="button"
            variant="secondary"
            disabled={isSubmitting}
            onClick={() => onOpenChange(false)}
          >
            Cancel
          </Button>
          <Button
            type="submit"
            variant="primary"
            loading={isSubmitting}
            leadingIcon={<Plus size={16} aria-hidden="true" />}
          >
            Create environment
          </Button>
        </footer>
      </form>
    </Dialog>
  )
}

function createInitialForm(environmentCount: number): EnvironmentFormState {
  return {
    kind: "actonLocalnet",
    name: defaultEnvironmentName("actonLocalnet", environmentCount),
    port: "",
    forkNetwork: "",
    forkBlockNumber: "",
    accounts: [],
    rateLimit: "",
    responseDelayMs: "",
    blockIntervalMs: "",
    noMining: false,
    mineEmptyBlocks: false,
    validators: "1",
    importedAccounts: [],
  }
}

interface AccountImportEditorProps {
  readonly accounts: readonly ImportedAccountForm[]
  readonly sources: readonly StudioEnvironment[]
  readonly onAdd: () => void
  readonly onChange: (
    id: number,
    update: Partial<Pick<ImportedAccountForm, "sourceEnvironmentId" | "name" | "address">>,
  ) => void
  readonly onRemove: (id: number) => void
}

function AccountImportEditor({
  accounts,
  sources,
  onAdd,
  onChange,
  onRemove,
}: AccountImportEditorProps) {
  const [suggestions, setSuggestions] = useState<
    Readonly<Record<string, readonly TonAddressSuggestion[]>>
  >({})
  const loadingSources = useRef(new Set<string>())

  useEffect(() => {
    for (const sourceEnvironmentId of new Set(
      accounts.map(account => account.sourceEnvironmentId),
    )) {
      if (suggestions[sourceEnvironmentId] || loadingSources.current.has(sourceEnvironmentId)) {
        continue
      }
      const source = sources.find(environment => environment.id === sourceEnvironmentId)
      if (!source) continue

      loadingSources.current.add(sourceEnvironmentId)
      void loadAddressSuggestions(source)
        .then(items => {
          setSuggestions(current => ({...current, [sourceEnvironmentId]: items}))
        })
        .catch(() => {
          setSuggestions(current => ({...current, [sourceEnvironmentId]: []}))
        })
        .finally(() => {
          loadingSources.current.delete(sourceEnvironmentId)
        })
    }
  }, [accounts, sources, suggestions])

  return (
    <section className={styles.importPicker} aria-labelledby="account-import-title">
      <header className={styles.importHeader}>
        <div>
          <strong id="account-import-title">Accounts to import</strong>
          <span>Copy active account balance, code, and data into the new network zerostate</span>
        </div>
        <Button
          type="button"
          size="sm"
          variant="secondary"
          leadingIcon={<Plus size={14} aria-hidden="true" />}
          disabled={sources.length === 0}
          onClick={onAdd}
        >
          Add account
        </Button>
      </header>

      {accounts.length === 0 ? (
        <div className={styles.importMessage}>No accounts will be imported</div>
      ) : (
        <div className={styles.importList}>
          <div className={styles.importColumns} aria-hidden="true">
            <span>Source</span>
            <span>Contract name (optional)</span>
            <span>Account address</span>
          </div>
          {accounts.map((account, index) => (
            <div key={account.id} className={styles.importRow}>
              <Select
                aria-label={`Source network for account ${index + 1}`}
                value={account.sourceEnvironmentId}
                onChange={event => onChange(account.id, {sourceEnvironmentId: event.target.value})}
              >
                {sources.map(source => (
                  <option key={source.id} value={source.id}>
                    {source.name}
                  </option>
                ))}
              </Select>
              <Input
                aria-label={`Account ${index + 1} contract name`}
                placeholder={`Account ${index + 1}`}
                value={account.name}
                maxLength={80}
                spellCheck
                onChange={event => onChange(account.id, {name: event.target.value})}
              />
              <TonAddressInput
                ariaLabel={`Account ${index + 1} address`}
                className={styles.importAddressInput}
                suggestions={suggestions[account.sourceEnvironmentId] ?? []}
                value={account.address}
                onValueChange={address => onChange(account.id, {address})}
                onSuggestionSelect={suggestion =>
                  onChange(account.id, {
                    address: suggestion.address,
                    name: account.name.trim() ? account.name : (suggestion.label ?? ""),
                  })
                }
              />
              <InlineAction
                label={`Remove account ${index + 1}`}
                icon={<Trash2 />}
                onClick={() => onRemove(account.id)}
              />
            </div>
          ))}
        </div>
      )}
      <p className={styles.importHint}>
        Select a source to get saved contract completions, or enter any active TON address.
      </p>
    </section>
  )
}

function availableImportSources(environments: readonly StudioEnvironment[]) {
  return environments.filter(
    environment => environment.status === "running" && environment.endpoints.apiV2,
  )
}

function preferredImportSource(environments: readonly StudioEnvironment[]) {
  const sources = availableImportSources(environments)
  return sources.find(environment => environment.id === "mainnet") ?? sources[0]
}

async function loadAddressSuggestions(
  source: StudioEnvironment,
): Promise<readonly TonAddressSuggestion[]> {
  if (!source.capabilities.includes("contracts") || !source.endpoints.apiV3) return []
  const client = new TonClient({
    v2BaseUrl: source.endpoints.apiV2 ?? source.rpcUrl,
    v3BaseUrl: source.endpoints.apiV3,
    addressNameBaseUrl: source.endpoints.control ?? source.rpcUrl,
    localnetControlEnabled: source.capabilities.includes("controlApi"),
    toncenterApiCompatible: source.network.supportsActions,
  })
  const contracts = await client.listContracts()
  return contracts
    .filter(contract => contract.status === "active")
    .map(contract => {
      const name = contract.name ?? contract.abiName
      return {
        address: contract.address,
        label: name,
        description: contract.address,
      }
    })
}

function defaultEnvironmentName(
  kind: EnvironmentFormState["kind"],
  environmentCount: number,
): string {
  return kind === "actonLocalnet"
    ? `Simulated localnet ${environmentCount + 1}`
    : `Full localnet ${environmentCount + 1}`
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
