import {Button, InlineAction, Input, Select} from "@acton/ui"
import {TonClient} from "@acton/explorer-core/api/client"
import {TonAddressInput, type TonAddressSuggestion} from "@acton/transaction-ui"
import {Plus, Trash2} from "lucide-react"
import {useEffect, useId, useRef, useState} from "react"

import type {StudioEnvironment} from "../studioApi"
import styles from "./AccountImportEditor.module.css"

export interface ImportedAccountForm {
  readonly id: number
  readonly sourceEnvironmentId: string
  readonly name: string
  readonly address: string
}

interface AccountImportEditorProps {
  readonly description?: string
  readonly accounts: readonly ImportedAccountForm[]
  readonly sources: readonly StudioEnvironment[]
  readonly onAdd: () => void
  readonly onChange: (
    id: number,
    update: Partial<Pick<ImportedAccountForm, "sourceEnvironmentId" | "name" | "address">>,
  ) => void
  readonly onRemove: (id: number) => void
}

export function AccountImportEditor({
  description = "Copy active account balance, code, and data into the new network zerostate",
  accounts,
  sources,
  onAdd,
  onChange,
  onRemove,
}: AccountImportEditorProps) {
  const titleId = useId()
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
    <section className={styles.importPicker} aria-labelledby={titleId}>
      <header className={styles.importHeader}>
        <div>
          <strong id={titleId}>Accounts to import</strong>
          <span>{description}</span>
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
        Select a source to get saved contract completions, or enter any active TON address
      </p>
    </section>
  )
}

export function availableImportSources(environments: readonly StudioEnvironment[]) {
  return environments.filter(
    environment => environment.status === "running" && environment.endpoints.apiV2,
  )
}

export function preferredImportSource(environments: readonly StudioEnvironment[]) {
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
