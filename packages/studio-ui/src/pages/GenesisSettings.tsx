import {Button, Checkbox, Disclosure, Input} from "@acton/ui"
import {Download, Upload} from "lucide-react"
import {type Ref, useEffect, useImperativeHandle, useRef, useState} from "react"

import {
  type GenesisIssue,
  type GenesisSection,
  parseGenesisToml,
  StudioRequestError,
} from "../studioApi"
import styles from "./CreateEnvironmentDialog.module.css"

interface GenesisSettingsProps {
  readonly ref?: Ref<GenesisSettingsHandle>
  readonly value: string
  readonly disabled: boolean
  readonly onChange: (value: string) => void
  readonly onReadyChange: (ready: boolean) => void
}

/** Creation validates the current draft and focuses the first invalid field */
export interface GenesisSettingsHandle {
  validate(): Promise<boolean>
}

const sectionNames: Record<string, string> = {
  "": "Network, validators and consensus",
  workchain: "Workchain sharding",
  storage: "Storage prices",
  gas: "Workchain gas limits",
  gas_masterchain: "Masterchain gas limits",
  forwarding: "Workchain forwarding fees",
  forwarding_masterchain: "Masterchain forwarding fees",
  catchain: "Validator rotation",
  voting: "Configuration voting",
  critical_voting: "Critical configuration voting",
  "block_limits.bytes": "Workchain block size limits",
  "block_limits.gas": "Workchain block gas limits",
  "block_limits.logical_time": "Workchain logical time limits",
  "masterchain_block_limits.bytes": "Masterchain block size limits",
  "masterchain_block_limits.gas": "Masterchain block gas limits",
  "masterchain_block_limits.logical_time": "Masterchain logical time limits",
}

/** Edit scalar values as strings so protocol integers retain all their bits */
export function GenesisSettings({
  ref,
  value,
  disabled,
  onChange,
  onReadyChange,
}: GenesisSettingsProps) {
  const initialValue = useRef(value)
  const fileInput = useRef<HTMLInputElement>(null)
  const disclosure = useRef<HTMLDetailsElement>(null)
  const requestId = useRef(0)
  const validationTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const [sections, setSections] = useState<readonly GenesisSection[]>([])
  const [busy, setBusy] = useState(true)
  const [error, setError] = useState<string>()
  const [importError, setImportError] = useState<string>()
  const [issues, setIssues] = useState<readonly GenesisIssue[]>([])
  const [edited, setEdited] = useState(false)

  useEffect(() => {
    let active = true
    parseGenesisToml(initialValue.current)
      .then(result => {
        if (active) {
          setSections(result.sections)
          setIssues(result.issues)
          revealIssues(result.issues)
        }
      })
      .catch(error => {
        if (active) setError(error instanceof Error ? error.message : String(error))
      })
      .finally(() => {
        if (active) setBusy(false)
      })
    return () => {
      active = false
      requestId.current += 1
    }
  }, [])

  useEffect(
    () => onReadyChange(!busy && !importError && sections.length > 0),
    [busy, importError, sections.length, onReadyChange],
  )

  // After the first edit, recheck dependencies without moving keyboard focus.
  useEffect(() => {
    if (!edited || busy || importError) return
    validationTimer.current = globalThis.setTimeout(() => void validate(false), 350)
    return () => globalThis.clearTimeout(validationTimer.current)
  }, [sections, edited, busy, importError])

  useImperativeHandle(ref, () => ({validate: () => validate(true)}))

  function revealIssues(next: readonly GenesisIssue[], focus = false) {
    if (next.length === 0) return
    if (disclosure.current) disclosure.current.open = true
    // Imported fields render before they can be revealed or focused.
    requestAnimationFrame(() => {
      const paths = new Set(next.flatMap(issue => issue.fields))
      let first: HTMLInputElement | undefined
      for (const input of disclosure.current?.querySelectorAll<HTMLInputElement>("input[name]") ??
        []) {
        if (!paths.has(input.name)) continue
        first ??= input
        let parent = input.closest("details")
        while (parent) {
          parent.open = true
          parent = parent.parentElement?.closest("details") ?? null
        }
      }
      if (focus) first?.focus()
    })
  }

  async function validate(focus: boolean): Promise<boolean> {
    globalThis.clearTimeout(validationTimer.current)
    if (busy || importError || sections.length === 0) return false
    const id = ++requestId.current
    const inputIssues = sections.flatMap(section =>
      section.fields.flatMap(field => {
        if (field.boolean) return []
        const value = field.value.trim()
        let message: string | undefined
        if (!/^-?[0-9]+$/.test(value)) {
          message = value ? "Enter a whole number" : "Enter a value"
        } else if (BigInt(value) < -(2n ** 63n) || BigInt(value) > 2n ** 63n - 1n) {
          message = "Value exceeds the supported integer range"
        }
        return message ? [{fields: [fieldPath(section.path, field.key)], message}] : []
      }),
    )
    try {
      const next =
        inputIssues.length > 0 ? inputIssues : (await parseGenesisToml(toToml(sections))).issues
      if (id !== requestId.current) return false
      setIssues(next)
      setError(undefined)
      revealIssues(next, focus)
      return next.length === 0
    } catch (error) {
      if (id !== requestId.current) return false
      const message = error instanceof Error ? error.message : String(error)
      if (error instanceof StudioRequestError && error.fields.length > 0) {
        const next = [{fields: error.fields, message}]
        setIssues(next)
        setError(undefined)
        revealIssues(next, focus)
      } else {
        setError(message)
        if (disclosure.current) disclosure.current.open = true
      }
      return false
    }
  }

  async function load(source: string) {
    const id = ++requestId.current
    setBusy(true)
    setError(undefined)
    setImportError(undefined)
    try {
      const result = await parseGenesisToml(source)
      if (id !== requestId.current) return
      setSections(result.sections)
      setIssues(result.issues)
      setEdited(false)
      onChange(result.toml)
      revealIssues(result.issues, true)
    } catch (error) {
      if (id === requestId.current) {
        setImportError(error instanceof Error ? error.message : String(error))
      }
    } finally {
      if (id === requestId.current) setBusy(false)
    }
  }

  function update(sectionPath: string, key: string, value: string) {
    requestId.current += 1
    setImportError(undefined)
    setError(undefined)
    setEdited(true)
    const next = sections.map(section =>
      section.path === sectionPath
        ? {
            ...section,
            fields: section.fields.map(field => (field.key === key ? {...field, value} : field)),
          }
        : section,
    )
    setSections(next)
    onChange(toToml(next))
  }

  async function download() {
    if (!(await validate(true))) return
    setBusy(true)
    setError(undefined)
    try {
      const result = await parseGenesisToml(toToml(sections))
      const url = URL.createObjectURL(new Blob([result.toml], {type: "application/toml"}))
      const anchor = document.createElement("a")
      anchor.href = url
      anchor.download = "genesis.toml"
      anchor.click()
      URL.revokeObjectURL(url)
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Disclosure
      ref={disclosure}
      data-invalid={issues.length > 0}
      className={styles.compactDisclosure}
      label="Genesis configuration"
      contentClassName={styles.advancedContent}
    >
      <p className={styles.genesisDescription}>
        Import genesis.toml or edit the values below · These settings are fixed when the network is
        created
      </p>
      <div className={styles.genesisActions}>
        <Button
          type="button"
          variant="secondary"
          disabled={disabled || busy}
          leadingIcon={<Upload size={16} aria-hidden="true" />}
          onClick={() => fileInput.current?.click()}
        >
          Import genesis.toml
        </Button>
        <Button
          type="button"
          variant="secondary"
          disabled={disabled || busy || Boolean(importError) || sections.length === 0}
          leadingIcon={<Download size={16} aria-hidden="true" />}
          onClick={() => void download()}
        >
          Export genesis.toml
        </Button>
        <Button
          type="button"
          variant="ghost"
          disabled={disabled || busy}
          onClick={() => void load("")}
        >
          Reset defaults
        </Button>
        <input
          ref={fileInput}
          hidden
          type="file"
          accept=".toml,application/toml,text/plain"
          aria-label="Import genesis.toml"
          onChange={async event => {
            const file = event.target.files?.[0]
            event.target.value = ""
            if (!file) return
            if (file.size > 64 * 1024) {
              setImportError("Genesis files must be smaller than 64 KiB")
              return
            }
            try {
              setBusy(true)
              await load(await file.text())
            } catch (error) {
              setBusy(false)
              setImportError(error instanceof Error ? error.message : String(error))
            }
          }}
        />
      </div>
      {busy && <p className={styles.genesisDescription}>Loading genesis settings…</p>}
      {importError && (
        <div role="alert" className={styles.genesisError}>
          Could not load genesis settings · Existing values are unchanged
          <br />
          {importError}
        </div>
      )}
      {error && (
        <p role="alert" className={styles.genesisError}>
          {error}
        </p>
      )}
      {issues.length > 0 && (
        <p role="status" className={styles.genesisError}>
          Check the highlighted fields · For related values, adjust either value to satisfy the
          condition
        </p>
      )}
      <div className={styles.genesisSections}>
        {sections.map(section => (
          <Disclosure
            key={section.path}
            data-invalid={section.fields.some(field =>
              issues.some(issue => issue.fields.includes(fieldPath(section.path, field.key))),
            )}
            className={styles.genesisSection}
            label={sectionNames[section.path] ?? title(section.path)}
            contentClassName={styles.formGrid}
          >
            {section.fields.map(field => {
              const label = title(field.key)
              const path = fieldPath(section.path, field.key)
              const messages = issues
                .filter(issue => issue.fields.includes(path))
                .map(issue => issue.message)
              return field.boolean ? (
                <div key={field.key} className={styles.genesisCheckbox}>
                  <Checkbox
                    name={path}
                    label={label}
                    checked={field.value === "true"}
                    disabled={disabled || busy}
                    onChange={event =>
                      update(section.path, field.key, String(event.target.checked))
                    }
                  />
                </div>
              ) : (
                <Input
                  key={field.key}
                  name={path}
                  label={label}
                  invalid={messages.length > 0}
                  description={
                    messages.length > 0 ? (
                      <span className={styles.genesisError}>{messages.join(" · ")}</span>
                    ) : (
                      description(section.path, field.key)
                    )
                  }
                  suffix={unit(section.path, field.key)}
                  value={field.value}
                  inputMode="numeric"
                  pattern="-?[0-9]+"
                  required
                  disabled={disabled || busy}
                  onBlur={() => void validate(false)}
                  onInvalid={event => {
                    void validate(true)
                    let disclosure = event.currentTarget.closest("details")
                    while (disclosure) {
                      disclosure.open = true
                      disclosure = disclosure.parentElement?.closest("details") ?? null
                    }
                  }}
                  onChange={event => {
                    update(section.path, field.key, event.target.value)
                  }}
                />
              )
            })}
          </Disclosure>
        ))}
      </div>
    </Disclosure>
  )
}

function toToml(sections: readonly GenesisSection[]): string {
  return sections
    .map(
      section =>
        `${section.path ? `[${section.path}]\n` : ""}${section.fields
          .map(field => {
            const value = field.value.trim()
            return `${field.key} = ${/^-?[0-9]+$/.test(value) ? BigInt(value).toString() : value}`
          })
          .join("\n")}\n`,
    )
    .join("\n")
}

function fieldPath(section: string, key: string): string {
  return section ? `${section}.${key}` : key
}

function title(key: string): string {
  const labels: Record<string, string> = {
    global_id: "Network ID",
    global_version: "Protocol version",
    global_capabilities: "Protocol capabilities",
    elected_for_seconds: "Validator round duration",
    election_start_before_seconds: "Elections start offset",
    election_end_before_seconds: "Elections close offset",
    original_validator_set_valid_for_seconds: "Initial validator set lifetime",
    stakes_frozen_for_seconds: "Stake freeze duration",
    min_split: "Minimum shard split depth",
    max_split: "Maximum shard split depth",
  }
  if (labels[key]) return labels[key]
  const text = key.replaceAll(/[_.]/g, " ").replace(/\s+(ms|seconds)$/, "")
  return text.charAt(0).toUpperCase() + text.slice(1)
}

function description(section: string, key: string): string | undefined {
  if (section === "storage") return "Nanograms per second, scaled by 65,536"
  if (section.startsWith("forwarding")) {
    if (key === "lump_price") return "Fixed charge per message"
    if (key === "bit_price") return "Nanograms per bit, scaled by 65,536"
    return "Fraction or multiplier scaled by 65,536"
  }
  if (!section) {
    if (key === "election_start_before_seconds")
      return "Time before the round ends when elections open"
    if (key === "election_end_before_seconds")
      return "Time before the round ends when elections close"
    if (key.startsWith("cell_price"))
      return "Forwarding price per cell in nanograms, scaled by 65,536"
    if (key.startsWith("gas_price")) return "Nanograms per gas unit, scaled by 65,536"
    if (key === "global_id") return "Negative identifier for this local network"
    if (key === "global_capabilities") return "Protocol capability bitmask in decimal"
    if (key === "min_validators") return "Elections require this many validators to participate"
  }
  if (section.includes("block_limits")) return "Underload < soft < hard"
  if (section.includes("voting") && key.endsWith("price")) return "Nanograms per second"
  return undefined
}

function unit(section: string, key: string): string | undefined {
  if (!section && (key.endsWith("_balance") || key.endsWith("_stake"))) return "GRAM"
  if (key.endsWith("_seconds")) return "s"
  if (key.endsWith("_ms")) return "ms"
  if (section.startsWith("forwarding") && key === "lump_price") return "nanograms"
  if (section.startsWith("gas") && (key.endsWith("due_limit") || key === "flat_gas_price"))
    return "nanograms"
  if (section.endsWith(".bytes")) return "bytes"
  return undefined
}
