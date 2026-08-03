import {Button, Checkbox, Dialog, Disclosure, Input, Select, useToast} from "@acton/ui"
import {Plus} from "lucide-react"
import {type FormEvent, useEffect, useState} from "react"

import {
  type CreateEnvironmentRequest,
  type StudioEnvironment,
  createStudioEnvironment,
} from "../studioApi"
import {WalletNamesInput} from "./WalletNamesInput"

import styles from "./CreateEnvironmentDialog.module.css"

interface CreateEnvironmentDialogProps {
  readonly environmentCount: number
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
}

export function CreateEnvironmentDialog({
  environmentCount,
  open,
  walletNames,
  onCreated,
  onOpenChange,
}: CreateEnvironmentDialogProps) {
  const {showToast} = useToast()
  const [form, setForm] = useState<EnvironmentFormState>(() => createInitialForm(environmentCount))
  const [isSubmitting, setIsSubmitting] = useState(false)

  useEffect(() => {
    if (open) setForm(createInitialForm(environmentCount))
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

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()

    let request: CreateEnvironmentRequest
    try {
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
                validators: optionalPositiveInteger(form.validators, "Validators"),
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
      const endpoint =
        environment.endpoints.apiV3 ?? environment.endpoints.apiV2 ?? environment.endpoints.control
      showToast({
        title: `${environment.name} is starting`,
        description: endpoint
          ? `The network will be available at ${endpoint}`
          : "Studio is preparing the network",
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
      description="Choose the network model for this workspace"
      maxWidth="44rem"
      dismissible={!isSubmitting}
      contentClassName={styles.dialogContent}
    >
      <form className={styles.form} onSubmit={event => void handleSubmit(event)}>
        <Select
          label="Environment type"
          description="Use the fast Acton runtime or start a validator-backed TON network"
          value={form.kind}
          autoFocus
          onChange={event => updateKind(event.target.value as EnvironmentFormState["kind"])}
        >
          <option value="actonLocalnet">Fast local network</option>
          <option value="fullTonNetwork">Full TON network</option>
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
          <div>
            <Input
              label="Validators"
              description="Validator nodes started for this network"
              type="number"
              min={1}
              max={100}
              value={form.validators}
              onChange={event => updateForm("validators", event.target.value)}
            />
          </div>
        )}

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
  }
}

function defaultEnvironmentName(
  kind: EnvironmentFormState["kind"],
  environmentCount: number,
): string {
  return kind === "actonLocalnet"
    ? `Localnet ${environmentCount + 1}`
    : `Full TON network ${environmentCount + 1}`
}

function optionalPositiveInteger(value: string, label: string): number | undefined {
  const trimmed = value.trim()
  if (!trimmed) return

  const parsed = Number(trimmed)
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive whole number`)
  }
  return parsed
}

function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
