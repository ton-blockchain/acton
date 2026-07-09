import {useMemo} from "react"
import {Checkbox} from "@acton/ui"
import {renderTy, type SymTable, type UnionVariant} from "@ton/tolk-abi-to-typescript"

import {SAMPLE_ADDRESS, sampleAbiValueForTy} from "../api/abiDynamic"
import styles from "./AbiValueEditor.module.css"

interface AbiValueEditorProps {
  readonly symbols: SymTable
  readonly tyIdx: number
  readonly value: unknown
  readonly onChange: (value: unknown) => void
  readonly disabled?: boolean
  readonly label?: string
}

export function AbiValueEditor({
  symbols,
  tyIdx,
  value,
  onChange,
  disabled = false,
  label,
}: AbiValueEditorProps) {
  return (
    <div className={styles.editor}>
      <AbiValueEditorNode
        symbols={symbols}
        tyIdx={tyIdx}
        value={value}
        onChange={onChange}
        disabled={disabled}
        label={label}
      />
    </div>
  )
}

interface AbiValueEditorNodeProps extends AbiValueEditorProps {
  readonly nested?: boolean
}

function AbiValueEditorNode({
  symbols,
  tyIdx,
  value,
  onChange,
  disabled = false,
  label,
  nested = false,
}: AbiValueEditorNodeProps) {
  const ty = tryTyByIdx(symbols, tyIdx)
  const typeLabel = useMemo(() => safeRenderTy(symbols, tyIdx), [symbols, tyIdx])
  if (!ty) {
    return (
      <div className={styles.field}>
        {label && <span className={styles.fieldLabel}>{label}</span>}
        <span className={styles.emptyValue}>Unknown type #{tyIdx}</span>
      </div>
    )
  }

  switch (ty.kind) {
    case "bool": {
      return (
        <Checkbox
          checked={value === true}
          onChange={event => onChange(event.target.checked)}
          disabled={disabled}
          label={label ?? typeLabel}
          className={styles.checkboxControl}
        />
      )
    }
    case "int":
    case "intN":
    case "uintN":
    case "varintN":
    case "varuintN":
    case "coins":
    case "EnumRef":
    case "string":
    case "address":
    case "addressExt":
    case "addressAny": {
      return (
        <label className={styles.field}>
          {label && <span className={styles.fieldLabel}>{label}</span>}
          <input
            className={styles.textInput}
            value={formatScalarValue(value)}
            onChange={event => onChange(event.target.value)}
            placeholder={inputPlaceholder(ty.kind, typeLabel)}
            disabled={disabled}
          />
          {!label && <span className={styles.typeLabel}>{typeLabel}</span>}
        </label>
      )
    }
    case "addressOpt": {
      const isNull = value === null || value === undefined || value === ""
      return (
        <div className={nested ? styles.nested : styles.group}>
          <div className={styles.rowHeader}>
            <span className={styles.groupTitle}>{label ?? typeLabel}</span>
            <Checkbox
              checked={isNull}
              onChange={event => onChange(event.target.checked ? null : SAMPLE_ADDRESS)}
              disabled={disabled}
              label="null"
              className={styles.nullCheckbox}
            />
          </div>
          {!isNull && (
            <input
              className={styles.textInput}
              value={formatScalarValue(value)}
              onChange={event => onChange(event.target.value)}
              placeholder={inputPlaceholder(ty.kind, typeLabel)}
              disabled={disabled}
            />
          )}
        </div>
      )
    }
    case "cell":
    case "builder":
    case "slice":
    case "remaining":
    case "bitsN": {
      return (
        <label className={styles.field}>
          {label && <span className={styles.fieldLabel}>{label}</span>}
          <textarea
            className={styles.textArea}
            value={typeof value === "string" ? value : ""}
            onChange={event => onChange(event.target.value)}
            placeholder="Hex or base64 BoC"
            spellCheck={false}
            disabled={disabled}
          />
          {!label && <span className={styles.typeLabel}>{typeLabel}</span>}
        </label>
      )
    }
    case "nullable": {
      const isNull = value === null || value === undefined
      return (
        <div className={nested ? styles.nested : styles.group}>
          <div className={styles.rowHeader}>
            <span className={styles.groupTitle}>{label ?? typeLabel}</span>
            <Checkbox
              checked={isNull}
              onChange={event =>
                onChange(event.target.checked ? null : sampleAbiValueForTy(symbols, ty.inner_ty_idx))
              }
              disabled={disabled}
              label="null"
              className={styles.nullCheckbox}
            />
          </div>
          {!isNull && (
            <AbiValueEditorNode
              symbols={symbols}
              tyIdx={ty.inner_ty_idx}
              value={value}
              onChange={onChange}
              disabled={disabled}
              nested
            />
          )}
        </div>
      )
    }
    case "arrayOf":
    case "lispListOf": {
      const items = Array.isArray(value) ? value : []
      return (
        <div className={styles.group}>
          <CollectionHeader
            label={label ?? typeLabel}
            disabled={disabled}
            onAdd={() => onChange([...items, sampleAbiValueForTy(symbols, ty.inner_ty_idx)])}
          />
          <div className={styles.collection}>
            {items.length === 0 && <span className={styles.emptyValue}>Empty list</span>}
            {items.map((item, index) => (
              <div className={styles.collectionItem} key={index}>
                <AbiValueEditorNode
                  symbols={symbols}
                  tyIdx={ty.inner_ty_idx}
                  value={item}
                  onChange={next => onChange(items.map((entry, itemIndex) => (itemIndex === index ? next : entry)))}
                  disabled={disabled}
                  label={`#${index}`}
                  nested
                />
                <RemoveButton
                  disabled={disabled}
                  onClick={() => onChange(items.filter((_entry, itemIndex) => itemIndex !== index))}
                />
              </div>
            ))}
          </div>
        </div>
      )
    }
    case "tensor":
    case "shapedTuple": {
      const items = Array.isArray(value) ? value : []
      return (
        <div className={styles.group}>
          <span className={styles.groupTitle}>{label ?? typeLabel}</span>
          {ty.items_ty_idx.map((itemTyIdx, index) => (
            <AbiValueEditorNode
              key={`${itemTyIdx}:${index}`}
              symbols={symbols}
              tyIdx={itemTyIdx}
              value={items[index] ?? sampleAbiValueForTy(symbols, itemTyIdx)}
              onChange={next => {
                const nextItems = [...items]
                nextItems[index] = next
                onChange(nextItems)
              }}
              disabled={disabled}
              label={`#${index}`}
              nested
            />
          ))}
        </div>
      )
    }
    case "mapKV": {
      const entries = isRecord(value) ? Object.entries(value) : []
      return (
        <div className={styles.group}>
          <CollectionHeader
            label={label ?? typeLabel}
            disabled={disabled}
            onAdd={() => {
              const key = nextMapKey(entries)
              onChange({...recordValue(value), [key]: sampleAbiValueForTy(symbols, ty.value_ty_idx)})
            }}
          />
          <div className={styles.collection}>
            {entries.length === 0 && <span className={styles.emptyValue}>Empty map</span>}
            {entries.map(([key, item]) => (
              <div className={`${styles.collectionItem} ${styles.mapItem}`} key={key}>
                <label className={styles.field}>
                  <span className={styles.fieldLabel}>Key</span>
                  <input
                    className={styles.textInput}
                    value={key}
                    onChange={event => renameMapKey(value, key, event.target.value, onChange)}
                    disabled={disabled}
                  />
                </label>
                <AbiValueEditorNode
                  symbols={symbols}
                  tyIdx={ty.value_ty_idx}
                  value={item}
                  onChange={next => onChange({...recordValue(value), [key]: next})}
                  disabled={disabled}
                  label="Value"
                  nested
                />
                <RemoveButton
                  disabled={disabled}
                  onClick={() => {
                    const next = {...recordValue(value)}
                    delete next[key]
                    onChange(next)
                  }}
                />
              </div>
            ))}
          </div>
        </div>
      )
    }
    case "cellOf": {
      const current = isRecord(value) ? value.ref : undefined
      return (
        <div className={styles.group}>
          <span className={styles.groupTitle}>{label ?? typeLabel}</span>
          <AbiValueEditorNode
            symbols={symbols}
            tyIdx={ty.inner_ty_idx}
            value={current ?? sampleAbiValueForTy(symbols, ty.inner_ty_idx)}
            onChange={next => onChange({ref: next})}
            disabled={disabled}
            label="Referenced value"
            nested
          />
        </div>
      )
    }
    case "StructRef": {
      const fields = safeStructFields(symbols, tyIdx)
      return (
        <div className={styles.group}>
          <span className={styles.groupTitle}>{label ?? ty.struct_name}</span>
          {fields.length === 0 && <span className={styles.emptyValue}>No fields</span>}
          {fields.map(field => (
            <AbiValueEditorNode
              key={field.name}
              symbols={symbols}
              tyIdx={field.ty_idx}
              value={isRecord(value) ? value[field.name] : sampleAbiValueForTy(symbols, field.ty_idx)}
              onChange={next => onChange({...recordValue(value), [field.name]: next})}
              disabled={disabled}
              label={field.name}
              nested
            />
          ))}
        </div>
      )
    }
    case "AliasRef": {
      const targetTyIdx = safeAliasTargetTyIdx(symbols, tyIdx)
      if (targetTyIdx === undefined) {
        return <span className={styles.emptyValue}>Unknown alias {ty.alias_name}</span>
      }
      return (
        <AbiValueEditorNode
          symbols={symbols}
          tyIdx={targetTyIdx}
          value={value}
          onChange={onChange}
          disabled={disabled}
          label={label ?? ty.alias_name}
          nested={nested}
        />
      )
    }
    case "union": {
      const variants = createUnionLabels(symbols, ty.variants)
      const currentLabel = isRecord(value) && typeof value.$ === "string" ? value.$ : variants[0]?.labelStr
      const selected = variants.find(variant => variant.labelStr === currentLabel) ?? variants[0]
      if (!selected) {
        return <span className={styles.emptyValue}>Empty union</span>
      }
      const selectedValue = unionValueForEditor(value, selected, symbols)
      return (
        <div className={styles.group}>
          <label className={styles.field}>
            <span className={styles.fieldLabel}>{label ?? typeLabel}</span>
            <select
              className={styles.selectInput}
              value={selected.labelStr}
              onChange={event => {
                const next = variants.find(variant => variant.labelStr === event.target.value) ?? variants[0]
                onChange(unionValueFromEditor(next, sampleAbiValueForTy(symbols, next.variant_ty_idx)))
              }}
              disabled={disabled}
            >
              {variants.map(variant => (
                <option key={variant.labelStr} value={variant.labelStr}>
                  {variant.labelStr || "null"}
                </option>
              ))}
            </select>
          </label>
          {tryTyByIdx(symbols, selected.variant_ty_idx)?.kind === "nullLiteral" ? (
            <span className={styles.emptyValue}>null</span>
          ) : (
            <AbiValueEditorNode
              symbols={symbols}
              tyIdx={selected.variant_ty_idx}
              value={selectedValue}
              onChange={next => onChange(unionValueFromEditor(selected, next))}
              disabled={disabled}
              nested
            />
          )}
        </div>
      )
    }
    case "void":
    case "nullLiteral": {
      return (
        <div className={styles.field}>
          {label && <span className={styles.fieldLabel}>{label}</span>}
          <span className={styles.emptyValue}>null</span>
        </div>
      )
    }
    default: {
      return (
        <label className={styles.field}>
          {label && <span className={styles.fieldLabel}>{label}</span>}
          <textarea
            className={styles.textArea}
            value={typeof value === "string" ? value : JSON.stringify(value ?? null, null, 2)}
            onChange={event => onChange(event.target.value)}
            disabled={disabled}
          />
          <span className={styles.typeLabel}>{typeLabel}</span>
        </label>
      )
    }
  }
}

function CollectionHeader({
  label,
  disabled,
  onAdd,
}: {
  readonly label: string
  readonly disabled: boolean
  readonly onAdd: () => void
}) {
  return (
    <div className={styles.collectionHeader}>
      <span className={styles.groupTitle}>{label}</span>
      <button type="button" className={styles.smallButton} onClick={onAdd} disabled={disabled}>
        Add
      </button>
    </div>
  )
}

function RemoveButton({
  disabled,
  onClick,
}: {
  readonly disabled: boolean
  readonly onClick: () => void
}) {
  return (
    <button
      type="button"
      className={`${styles.smallButton} ${styles.removeButton}`}
      onClick={onClick}
      disabled={disabled}
    >
      Remove
    </button>
  )
}

type FieldInfo = {
  readonly name: string
  readonly ty_idx: number
}

function safeStructFields(symbols: SymTable, tyIdx: number): readonly FieldInfo[] {
  try {
    return symbols.structFieldsOf(tyIdx, false)
  } catch {
    return []
  }
}

function safeAliasTargetTyIdx(symbols: SymTable, tyIdx: number): number | undefined {
  try {
    return symbols.aliasTargetOf(tyIdx).ty_idx
  } catch {
    return undefined
  }
}

function safeRenderTy(symbols: SymTable, tyIdx: number): string {
  try {
    return renderTy(symbols, tyIdx)
  } catch {
    return `ty#${tyIdx}`
  }
}

function tryTyByIdx(symbols: SymTable, tyIdx: number): ReturnType<SymTable["tyByIdx"]> | undefined {
  try {
    return symbols.tyByIdx(tyIdx)
  } catch {
    return undefined
  }
}

function inputPlaceholder(kind: string, typeLabel: string): string {
  if (kind === "address" || kind === "addressOpt" || kind === "addressExt" || kind === "addressAny") {
    return "EQ..."
  }
  if (kind === "string") {
    return "string"
  }
  return typeLabel
}

function formatScalarValue(value: unknown): string {
  if (typeof value === "string") return value
  if (value === null || value === undefined) return ""
  return String(value)
}

function recordValue(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {}
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function nextMapKey(entries: readonly [string, unknown][]): string {
  let index = entries.length
  let key = String(index)
  const keys = new Set(entries.map(([entryKey]) => entryKey))
  while (keys.has(key)) {
    index += 1
    key = String(index)
  }
  return key
}

function renameMapKey(
  value: unknown,
  oldKey: string,
  newKey: string,
  onChange: (value: unknown) => void,
) {
  const next: Record<string, unknown> = {}
  for (const [key, item] of Object.entries(recordValue(value))) {
    next[key === oldKey ? newKey : key] = item
  }
  onChange(next)
}

function createUnionLabels(
  symbols: SymTable,
  variants: readonly UnionVariant[],
): readonly (UnionVariant & {readonly labelStr: string; readonly hasValueField: boolean})[] {
  const labels = variants.map(variant => createTypeLabel(symbols, variant.variant_ty_idx))
  const duplicatedLabels = new Set(labels.filter((label, index) => labels.indexOf(label) !== index))

  return variants.map((variant, index) => {
    const label = labels[index]
    const labelTy = tryTyByIdx(symbols, variant.variant_ty_idx)
    const fullLabel = duplicatedLabels.has(label)
      ? safeRenderTy(symbols, variant.variant_ty_idx)
      : label
    return {
      ...variant,
      labelStr: labelTy?.kind === "nullLiteral" ? "" : fullLabel,
      hasValueField: duplicatedLabels.has(label) ? true : !isStructWithOwnLabel(symbols, variant.variant_ty_idx),
    }
  })
}

function createTypeLabel(symbols: SymTable, tyIdx: number): string {
  const ty = tryTyByIdx(symbols, tyIdx)
  if (!ty) {
    return `ty#${tyIdx}`
  }

  switch (ty.kind) {
    case "StructRef": {
      return ty.struct_name
    }
    case "AliasRef": {
      return ty.alias_name
    }
    case "nullLiteral": {
      return ""
    }
    default: {
      return safeRenderTy(symbols, tyIdx)
    }
  }
}

function isStructWithOwnLabel(symbols: SymTable, tyIdx: number): boolean {
  const ty = tryTyByIdx(symbols, tyIdx)
  return ty?.kind === "StructRef" || ty?.kind === "AliasRef"
}

function unionValueForEditor(
  value: unknown,
  variant: UnionVariant & {readonly labelStr: string; readonly hasValueField: boolean},
  symbols: SymTable,
): unknown {
  if (!isRecord(value)) {
    return sampleAbiValueForTy(symbols, variant.variant_ty_idx)
  }
  if (variant.hasValueField) {
    return value.value ?? sampleAbiValueForTy(symbols, variant.variant_ty_idx)
  }
  const {$: _label, ...rest} = value
  return Object.keys(rest).length > 0 ? rest : sampleAbiValueForTy(symbols, variant.variant_ty_idx)
}

function unionValueFromEditor(
  variant: UnionVariant & {readonly labelStr: string; readonly hasValueField: boolean},
  value: unknown,
): unknown {
  if (variant.hasValueField) {
    return {$: variant.labelStr, value}
  }
  if (isRecord(value)) {
    return {$: variant.labelStr, ...value}
  }
  return {$: variant.labelStr}
}
