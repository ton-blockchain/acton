import {Button, Checkbox, Input, Select} from "@acton/ui"
import {Plus, Trash2} from "lucide-react"

import {GLOBAL_CAPABILITIES} from "../../components/GlobalCapabilities"
import {getNoncriticalParameterMetadata} from "../configParameter30"
import {
  defaultDraft,
  objectDraft,
  resolveShape,
  selectedShape,
  type Draft,
  type Shape,
} from "./codec"
import {fieldLabel, fieldSemantics, type FieldContext} from "./semantics"
import styles from "./ConfigEditor.module.css"

interface ConfigFieldsProps {
  readonly shape: Shape
  readonly value: Draft
  readonly context: FieldContext
  readonly onChange: (value: Draft) => void
  readonly label?: string
}

/** TL-B structure supplies completeness; field semantics supply human input units. */
export function ConfigFields({shape, value, context, onChange, label}: ConfigFieldsProps) {
  const resolved = resolveShape(shape)
  const title = label ?? fieldLabel(context.field)
  const owner = shape.type === "ref" ? shape.name : context.owner

  if (resolved.type === "literal" || resolved.type === "undefined") return null
  if (resolved.type === "maybe") {
    return (
      <div className={styles.group}>
        <Checkbox
          label={title}
          checked={value !== null}
          onChange={event =>
            onChange(event.currentTarget.checked ? defaultDraft(resolved.value) : null)
          }
        />
        {value !== null && (
          <ConfigFields
            shape={resolved.value}
            value={value}
            context={context}
            label={title}
            onChange={onChange}
          />
        )}
      </div>
    )
  }
  if (resolved.type === "union") {
    const selected = selectedShape(shape, value)
    const index = resolved.options.indexOf(selected)
    return (
      <div className={styles.group}>
        <Select
          label={`${title || "Value"} format`}
          value={index}
          onChange={event => {
            const option = resolved.options[Number(event.currentTarget.value)]
            if (!option) return
            const next = defaultDraft(option)
            // Retain common fields when switching a schema version; newly introduced
            // fields still need explicit input instead of silently disappearing.
            const previous = objectDraft(value)
            const record = objectDraft(next)
            for (const key of Object.keys(record))
              if (key !== "kind" && key in previous) record[key] = previous[key] ?? null
            onChange(record)
          }}
        >
          {resolved.options.map((option, optionIndex) => (
            <option key={option.type === "ref" ? option.name : optionIndex} value={optionIndex}>
              {option.type === "ref"
                ? fieldLabel(option.name.replace(/^[^_]+_/, ""))
                : `Format ${optionIndex + 1}`}
            </option>
          ))}
        </Select>
        <ConfigFields
          shape={selected}
          value={value}
          context={context}
          label={title}
          onChange={onChange}
        />
      </div>
    )
  }
  if (resolved.type === "struct") {
    const record = objectDraft(value)
    if (record.kind === "Bool")
      return (
        <Checkbox
          label={title}
          checked={record.value === true}
          onChange={event => onChange({...record, value: event.currentTarget.checked})}
        />
      )
    const fields = Object.entries(resolved.fields).filter(([field]) => field !== "kind")
    return (
      <div className={styles.fields}>
        {fields.map(([field, nested]) => (
          <ConfigFields
            key={field}
            shape={nested}
            value={record[field] ?? null}
            context={
              field === "anon0"
                ? {...context, owner}
                : {parameter: context.parameter, field, owner, parentField: context.field}
            }
            label={field === "anon0" ? title : undefined}
            onChange={next => onChange({...record, [field]: next})}
          />
        ))}
      </div>
    )
  }
  if (resolved.type === "map")
    return (
      <MapFields
        shape={resolved}
        value={value}
        context={context}
        onChange={onChange}
        label={title}
      />
    )
  if (resolved.type === "boolean")
    return (
      <Checkbox
        label={title}
        checked={value === true}
        onChange={event => onChange(event.currentTarget.checked)}
      />
    )

  const semantics = fieldSemantics(context)
  const isAddress = semantics.format === "address" || semantics.format === "wide-address"

  const technical =
    isAddress || semantics.format === "hash" || resolved.type === "buffer"
  const input = (
    <Input
      fieldClassName={technical || context.field === "weight" ? styles.wideField : undefined}
      label={title || "Value"}
      value={String(value ?? "")}
      size="md"
      mono={technical}
      // Chrome ignores "off" for fields it recognizes as postal addresses.
      autoComplete={isAddress ? "new-password" : "off"}
      suffix={semantics.unit}
      description={
        semantics.format === "address"
          ? "Raw or friendly masterchain address"
          : semantics.format === "wide-address"
            ? "Raw or friendly address, including its workchain"
            : undefined
      }
      onChange={event => onChange(event.currentTarget.value)}
    />
  )

  if (context.field === "capabilities") {
    let bits = 0n
    try {
      bits = BigInt(String(value))
    } catch {
      /* Keep incomplete numeric input editable */
    }
    return (
      <div className={styles.group}>
        {input}
        <div className={styles.flags}>
          {GLOBAL_CAPABILITIES.map(flag => (
            <Checkbox
              key={flag.value}
              label={flag.name}
              description={flag.description}
              checked={(bits & BigInt(flag.value)) !== 0n}
              onChange={event =>
                onChange(
                  String(
                    event.currentTarget.checked
                      ? bits | BigInt(flag.value)
                      : bits & ~BigInt(flag.value),
                  ),
                )
              }
            />
          ))}
        </div>
      </div>
    )
  }
  if (semantics.format === "date") {
    const seconds = Number(value)
    const date =
      Number.isFinite(seconds) && seconds >= 0 && seconds <= 4_294_967_295 && value !== ""
        ? new Date(seconds * 1000).toISOString().slice(0, 19)
        : ""
    return (
      <div className={styles.group}>
        <Input
          label={`${title} (UTC)`}
          type="datetime-local"
          step="1"
          value={date}
          onChange={event => {
            const time = Date.parse(`${event.currentTarget.value}Z`)
            onChange(Number.isFinite(time) ? String(time / 1000) : "")
          }}
        />
        <Input
          label="Unix timestamp"
          value={String(value ?? "")}
          onChange={event => onChange(event.currentTarget.value)}
        />
      </div>
    )
  }
  return input
}

function MapFields({
  shape,
  value,
  context,
  onChange,
  label,
}: ConfigFieldsProps & {readonly shape: Extract<Shape, {type: "map"}>}) {
  const entries = value as {key: Draft; value: Draft}[]
  const valueShape = resolveShape(shape.value)
  const emptyValue =
    valueShape.type === "struct" && Object.keys(valueShape.fields).every(field => field === "kind")
  const noncritical = context.field === "noncritical_params"
  return (
    <fieldset className={styles.map}>
      <legend>
        {label || "Entries"} <span className={styles.muted}>({entries.length})</span>
      </legend>
      {entries.map((entry, index) => {
        const change = (field: "key" | "value", next: Draft) =>
          onChange(entries.map((item, i) => (i === index ? {...item, [field]: next} : item)))
        const metadata = noncritical
          ? getNoncriticalParameterMetadata(Number(entry.key))
          : undefined
        return (
          <div className={styles.mapEntry} key={`${context.field}-${index}`}>
            <div className={styles.mapKey}>
              <ConfigFields
                shape={shape.key}
                value={entry.key}
                context={{...context, mapRole: "key"}}
                onChange={next => change("key", next)}
                label={
                  [9, 10].includes(context.parameter)
                    ? "Parameter number"
                    : noncritical
                      ? "Setting number"
                      : [31, 44].includes(context.parameter)
                        ? "Address"
                        : context.parameter === 45
                          ? "Code hash"
                          : "Key"
                }
              />
              <Button
                variant="ghost"
                size="icon"
                aria-label={`Remove entry ${index + 1}`}
                onClick={() => onChange(entries.filter((_, i) => i !== index))}
              >
                <Trash2 size={15} />
              </Button>
            </div>
            {metadata && (
              <p className={styles.hint}>
                {metadata.name} — {metadata.description?.replace(/\.$/, "")}
              </p>
            )}
            {!emptyValue && (
              <ConfigFields
                shape={shape.value}
                value={entry.value}
                context={{...context, mapRole: "value", mapKey: String(entry.key)}}
                label="Value"
                onChange={next => change("value", next)}
              />
            )}
          </div>
        )
      })}
      <Button
        variant="outline"
        size="sm"
        leadingIcon={<Plus size={14} />}
        onClick={() =>
          onChange([...entries, {key: defaultDraft(shape.key), value: defaultDraft(shape.value)}])
        }
      >
        Add entry
      </Button>
    </fieldset>
  )
}
