import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react"
import {Checkbox, formatGramAmount, InlineAction, Input, parseGramAmount, Select} from "@acton/ui"
import {renderTy, type SymTable, type UnionVariant} from "@ton/tolk-abi-to-typescript"
import {Plus, Trash2} from "lucide-react"

import {abiValueToFormValue, SAMPLE_ADDRESS, sampleAbiValueForTy} from "../../lib/abiValue"
import type {TonAddressKind} from "../../lib/tonAddress"
import {TonAddressInput, type TonAddressSuggestion} from "../TonAddressInput/TonAddressInput"
import styles from "./AbiValueEditor.module.css"

const AddressSuggestionsContext = createContext<readonly TonAddressSuggestion[]>([])

export interface AbiValueEditorProps {
  readonly symbols: SymTable
  readonly tyIdx: number
  readonly value: unknown
  /** A decoded ABI value to apply when the editor becomes available. */
  readonly initialValue?: unknown
  readonly onChange: (value: unknown) => void
  readonly disabled?: boolean
  readonly invalid?: boolean
  readonly label?: string
  readonly addressSuggestions?: readonly TonAddressSuggestion[]
}

export function AbiValueEditor({
  symbols,
  tyIdx,
  value,
  initialValue,
  onChange,
  disabled = false,
  invalid = false,
  label,
  addressSuggestions = [],
}: AbiValueEditorProps) {
  const appliedInitialValue = useRef<unknown>(undefined)

  useEffect(() => {
    if (initialValue === undefined || appliedInitialValue.current === initialValue) {
      return
    }

    appliedInitialValue.current = initialValue
    onChange(abiValueToFormValue(initialValue))
  }, [initialValue, onChange])

  return (
    <AddressSuggestionsContext.Provider value={addressSuggestions}>
      <div
        className={`${styles.editor} ${invalid ? styles.invalid : ""}`}
        aria-invalid={invalid || undefined}
      >
        <AbiValueEditorNode
          symbols={symbols}
          tyIdx={tyIdx}
          value={value}
          onChange={onChange}
          disabled={disabled}
          label={label}
        />
      </div>
    </AddressSuggestionsContext.Provider>
  )
}

function TonCoinsInput({
  label,
  typeLabel,
  hideHeader = false,
  value,
  onChange,
  disabled,
}: {
  readonly label: string
  readonly typeLabel: string
  readonly hideHeader?: boolean
  readonly value: unknown
  readonly onChange: (value: unknown) => void
  readonly disabled: boolean
}) {
  const nanoValue = formatScalarValue(value)
  const [draft, setDraft] = useState(() =>
    formatGramAmount(nanoValue, {fallback: "", showUnit: false}),
  )

  useEffect(() => {
    setDraft(current => {
      const currentNano = parseGramAmount(current)?.toString()
      return currentNano === nanoValue
        ? current
        : formatGramAmount(nanoValue, {fallback: "", showUnit: false})
    })
  }, [nanoValue])

  return (
    <div className={styles.field}>
      <FieldHeader label={label} typeLabel={typeLabel} hidden={hideHeader} />
      <Input
        className={styles.tonInput}
        suffix="GRAM"
        value={draft}
        onChange={event => {
          const next = event.target.value
          setDraft(next)
          const nextNano = parseGramAmount(next)?.toString()
          if (nextNano !== undefined) {
            onChange(nextNano)
          }
        }}
        onBlur={() =>
          setDraft(formatGramAmount(formatScalarValue(value), {fallback: "", showUnit: false}))
        }
        inputMode="decimal"
        placeholder="0.1"
        disabled={disabled}
        aria-label={label}
      />
    </div>
  )
}

interface AbiValueEditorNodeProps extends AbiValueEditorProps {
  readonly hideHeader?: boolean
}

function AbiValueEditorNode({
  symbols,
  tyIdx,
  value,
  onChange,
  disabled = false,
  label,
  hideHeader = false,
}: AbiValueEditorNodeProps) {
  const addressSuggestions = useContext(AddressSuggestionsContext)
  const ty = tryTyByIdx(symbols, tyIdx)
  const typeLabel = useMemo(() => safeRenderTy(symbols, tyIdx), [symbols, tyIdx])
  if (!ty) {
    return (
      <div className={styles.field}>
        <FieldHeader label={label} typeLabel={`ty#${tyIdx}`} hidden={hideHeader} />
        <span className={styles.emptyValue}>Unknown type #{tyIdx}</span>
      </div>
    )
  }

  switch (ty.kind) {
    case "bool": {
      return (
        <div className={styles.field}>
          <FieldHeader label={label} typeLabel={typeLabel} hidden={hideHeader} />
          <div className={styles.booleanInput}>
            <Checkbox
              checked={value === true}
              onChange={event => onChange(event.target.checked)}
              disabled={disabled}
              label={value === true ? "true" : "false"}
              className={styles.checkboxControl}
            />
          </div>
        </div>
      )
    }
    case "int":
    case "intN":
    case "uintN":
    case "varintN":
    case "varuintN":
    case "EnumRef":
    case "string": {
      return (
        <label className={styles.field}>
          <FieldHeader label={label} typeLabel={typeLabel} hidden={hideHeader} />
          <input
            className={styles.textInput}
            value={formatScalarValue(value)}
            onChange={event => onChange(event.target.value)}
            placeholder={inputPlaceholder(ty.kind, typeLabel)}
            disabled={disabled}
          />
        </label>
      )
    }
    case "address":
    case "addressExt":
    case "addressAny": {
      return (
        <div className={styles.field}>
          <FieldHeader label={label} typeLabel={typeLabel} hidden={hideHeader} />
          <TonAddressInput
            ariaLabel={label ?? typeLabel}
            className={styles.addressInput}
            kind={tonAddressKindForAbiKind(ty.kind)}
            value={formatScalarValue(value)}
            onValueChange={onChange}
            suggestions={addressSuggestions}
            disabled={disabled}
            required
          />
        </div>
      )
    }
    case "coins": {
      if (label && isTonFieldName(label)) {
        return (
          <TonCoinsInput
            label={label}
            typeLabel={typeLabel}
            hideHeader={hideHeader}
            value={value}
            onChange={onChange}
            disabled={disabled}
          />
        )
      }

      return (
        <label className={styles.field}>
          <FieldHeader label={label} typeLabel={typeLabel} hidden={hideHeader} />
          <input
            className={styles.textInput}
            value={formatScalarValue(value)}
            onChange={event => onChange(event.target.value)}
            placeholder={inputPlaceholder(ty.kind, typeLabel)}
            disabled={disabled}
          />
        </label>
      )
    }
    case "addressOpt": {
      const isNull = value === null || value === undefined
      return (
        <div className={styles.field}>
          <FieldHeader
            label={label}
            typeLabel={typeLabel}
            hidden={hideHeader}
            action={
              <Checkbox
                checked={isNull}
                onChange={event => onChange(event.target.checked ? null : SAMPLE_ADDRESS)}
                disabled={disabled}
                label="Null"
                className={styles.nullCheckbox}
              />
            }
          />
          {isNull ? (
            <NullValue />
          ) : (
            <TonAddressInput
              ariaLabel={label ?? typeLabel}
              className={styles.addressInput}
              kind="internal"
              value={formatScalarValue(value)}
              onValueChange={onChange}
              suggestions={addressSuggestions}
              disabled={disabled}
              required
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
          <FieldHeader label={label} typeLabel={typeLabel} hidden={hideHeader} />
          <textarea
            className={styles.textArea}
            value={typeof value === "string" ? value : ""}
            onChange={event => onChange(event.target.value)}
            placeholder="Hex BoC"
            spellCheck={false}
            disabled={disabled}
          />
        </label>
      )
    }
    case "nullable": {
      const isNull = value === null || value === undefined
      const innerTy = tryTyByIdx(symbols, ty.inner_ty_idx)
      return (
        <div className={styles.field}>
          <FieldHeader
            label={label}
            typeLabel={typeLabel}
            hidden={hideHeader}
            action={
              <Checkbox
                checked={isNull}
                onChange={event =>
                  onChange(
                    event.target.checked ? null : sampleAbiValueForTy(symbols, ty.inner_ty_idx),
                  )
                }
                disabled={disabled}
                label="Null"
                className={styles.nullCheckbox}
              />
            }
          />
          {isNull ? (
            <NullValue />
          ) : innerTy?.kind === "coins" && label && isTonFieldName(label) ? (
            <TonCoinsInput
              label={label}
              typeLabel={safeRenderTy(symbols, ty.inner_ty_idx)}
              hideHeader
              value={value}
              onChange={onChange}
              disabled={disabled}
            />
          ) : (
            <div className={styles.optionalValue}>
              <AbiValueEditorNode
                symbols={symbols}
                tyIdx={ty.inner_ty_idx}
                value={value}
                onChange={onChange}
                disabled={disabled}
                hideHeader
              />
            </div>
          )}
        </div>
      )
    }
    case "arrayOf":
    case "lispListOf": {
      const items = Array.isArray(value) ? value : []
      return (
        <div className={hideHeader ? styles.flattenedGroup : styles.group}>
          <CollectionHeader
            label={label ?? (ty.kind === "arrayOf" ? "Array" : "List")}
            typeLabel={typeLabel}
            hidden={hideHeader}
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
                  onChange={next =>
                    onChange(items.map((entry, itemIndex) => (itemIndex === index ? next : entry)))
                  }
                  disabled={disabled}
                  label={`#${index}`}
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
          <FieldHeader label={label ?? "Tuple"} typeLabel={typeLabel} hidden={hideHeader} group />
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
            label={label ?? "Map"}
            typeLabel={typeLabel}
            hidden={hideHeader}
            disabled={disabled}
            onAdd={() => {
              const key = nextMapKey(entries)
              onChange({
                ...recordValue(value),
                [key]: sampleAbiValueForTy(symbols, ty.value_ty_idx),
              })
            }}
          />
          <div className={styles.collection}>
            {entries.length === 0 && <span className={styles.emptyValue}>Empty map</span>}
            {entries.map(([key, item]) => (
              <div className={`${styles.collectionItem} ${styles.mapItem}`} key={key}>
                <AbiMapKeyInput
                  symbols={symbols}
                  tyIdx={ty.key_ty_idx}
                  value={key}
                  onChange={next => renameMapKey(value, key, next, onChange)}
                  disabled={disabled}
                />
                <AbiValueEditorNode
                  symbols={symbols}
                  tyIdx={ty.value_ty_idx}
                  value={item}
                  onChange={next => onChange({...recordValue(value), [key]: next})}
                  disabled={disabled}
                  label="Value"
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
        <AbiValueEditorNode
          symbols={symbols}
          tyIdx={ty.inner_ty_idx}
          value={current ?? sampleAbiValueForTy(symbols, ty.inner_ty_idx)}
          onChange={next => onChange({ref: next})}
          disabled={disabled}
          label={label}
          hideHeader={hideHeader}
        />
      )
    }
    case "StructRef": {
      const fields = safeStructFields(symbols, tyIdx)
      return (
        <div className={hideHeader ? styles.flattenedGroup : styles.group}>
          <FieldHeader
            label={label ?? ty.struct_name}
            typeLabel={label ? typeLabel : "struct"}
            hidden={hideHeader}
            group
          />
          {fields.length === 0 && <span className={styles.emptyValue}>No fields</span>}
          {fields.map(field => (
            <AbiValueEditorNode
              key={field.name}
              symbols={symbols}
              tyIdx={field.ty_idx}
              value={
                isRecord(value) ? value[field.name] : sampleAbiValueForTy(symbols, field.ty_idx)
              }
              onChange={next => onChange({...recordValue(value), [field.name]: next})}
              disabled={disabled}
              label={field.name}
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
          hideHeader={hideHeader}
        />
      )
    }
    case "union": {
      const variants = createUnionLabels(symbols, ty.variants)
      const currentLabel =
        isRecord(value) && typeof value.$ === "string" ? value.$ : variants[0]?.labelStr
      const selected = variants.find(variant => variant.labelStr === currentLabel) ?? variants[0]
      if (!selected) {
        return <span className={styles.emptyValue}>Empty union</span>
      }
      const selectedValue = unionValueForEditor(value, selected, symbols)
      const selectedHasOwnStructure = isStructWithOwnLabel(symbols, selected.variant_ty_idx)
      return (
        <div className={styles.group}>
          <FieldHeader label={label ?? "Union"} typeLabel={typeLabel} hidden={hideHeader} group />
          <Select
            fieldClassName={`${styles.field} ${styles.variantField}`}
            label="Variant"
            value={selected.labelStr}
            onChange={event => {
              const next =
                variants.find(variant => variant.labelStr === event.target.value) ?? variants[0]
              onChange(
                unionValueFromEditor(next, sampleAbiValueForTy(symbols, next.variant_ty_idx)),
              )
            }}
            disabled={disabled}
          >
            {variants.map(variant => (
              <option key={variant.labelStr} value={variant.labelStr}>
                {variant.labelStr || "null"}
              </option>
            ))}
          </Select>
          {tryTyByIdx(symbols, selected.variant_ty_idx)?.kind === "nullLiteral" ? (
            <span className={styles.emptyValue}>null</span>
          ) : (
            <AbiValueEditorNode
              symbols={symbols}
              tyIdx={selected.variant_ty_idx}
              value={selectedValue}
              onChange={next => onChange(unionValueFromEditor(selected, next))}
              disabled={disabled}
              label={selectedHasOwnStructure ? undefined : "Value"}
              hideHeader={selectedHasOwnStructure}
            />
          )}
        </div>
      )
    }
    case "void":
    case "nullLiteral": {
      return (
        <div className={styles.field}>
          <FieldHeader label={label} typeLabel={typeLabel} hidden={hideHeader} />
          <NullValue />
        </div>
      )
    }
    default: {
      return (
        <label className={styles.field}>
          <FieldHeader label={label} typeLabel={typeLabel} hidden={hideHeader} />
          <textarea
            className={styles.textArea}
            value={typeof value === "string" ? value : JSON.stringify(value ?? null, null, 2)}
            onChange={event => onChange(event.target.value)}
            disabled={disabled}
          />
        </label>
      )
    }
  }
}

function AbiMapKeyInput({
  symbols,
  tyIdx,
  value,
  onChange,
  disabled,
}: {
  readonly symbols: SymTable
  readonly tyIdx: number
  readonly value: string
  readonly onChange: (value: string) => void
  readonly disabled: boolean
}) {
  const addressSuggestions = useContext(AddressSuggestionsContext)
  const typeLabel = safeRenderTy(symbols, tyIdx)
  const addressKind = tonAddressKindForTy(symbols, tyIdx)

  return (
    <div className={styles.field}>
      <FieldHeader label="Key" typeLabel={typeLabel} />
      {addressKind ? (
        <TonAddressInput
          ariaLabel="Key"
          className={styles.addressInput}
          kind={addressKind}
          value={value}
          onValueChange={onChange}
          suggestions={addressSuggestions}
          disabled={disabled}
          required
        />
      ) : (
        <input
          className={styles.textInput}
          value={value}
          onChange={event => onChange(event.target.value)}
          disabled={disabled}
          aria-label="Key"
        />
      )}
    </div>
  )
}

function CollectionHeader({
  label,
  typeLabel,
  hideType,
  hidden,
  disabled,
  onAdd,
}: {
  readonly label: string
  readonly typeLabel: string
  readonly hideType?: boolean
  readonly hidden?: boolean
  readonly disabled: boolean
  readonly onAdd: () => void
}) {
  return (
    <FieldHeader
      label={label}
      typeLabel={typeLabel}
      hideType={hideType}
      hidden={hidden}
      group
      action={
        <InlineAction
          className={styles.collectionAddAction}
          icon={<Plus />}
          label="Add item"
          onClick={onAdd}
          disabled={disabled}
        />
      }
    />
  )
}

function FieldHeader({
  label,
  typeLabel,
  hideType = false,
  hidden = false,
  group = false,
  action,
}: {
  readonly label?: string
  readonly typeLabel: string
  readonly hideType?: boolean
  readonly hidden?: boolean
  readonly group?: boolean
  readonly action?: ReactNode
}) {
  if (hidden && !action) return null

  const title = label ?? typeLabel
  return (
    <div className={`${styles.fieldHeader} ${group ? styles.groupHeader : ""}`}>
      {!hidden && <span className={styles.fieldLabel}>{title}</span>}
      {!hidden && !hideType && title !== typeLabel && (
        <span className={styles.typeLabel}>{typeLabel}</span>
      )}
      {action && <div className={styles.fieldAction}>{action}</div>}
    </div>
  )
}

function NullValue() {
  return <span className={styles.nullValue}>null</span>
}

function RemoveButton({
  disabled,
  onClick,
}: {
  readonly disabled: boolean
  readonly onClick: () => void
}) {
  return (
    <InlineAction
      icon={<Trash2 />}
      label="Remove item"
      variant="danger"
      onClick={onClick}
      disabled={disabled}
    />
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
  if (kind === "string") {
    return "string"
  }
  return typeLabel
}

function tonAddressKindForAbiKind(kind: "address" | "addressExt" | "addressAny"): TonAddressKind {
  if (kind === "addressExt") return "external"
  if (kind === "addressAny") return "any"
  return "internal"
}

function tonAddressKindForTy(
  symbols: SymTable,
  tyIdx: number,
  visited = new Set<number>(),
): TonAddressKind | undefined {
  if (visited.has(tyIdx)) return undefined
  visited.add(tyIdx)

  const ty = tryTyByIdx(symbols, tyIdx)
  if (!ty) return undefined
  if (ty.kind === "address" || ty.kind === "addressOpt") return "internal"
  if (ty.kind === "addressExt") return "external"
  if (ty.kind === "addressAny") return "any"
  if (ty.kind === "AliasRef") {
    const targetTyIdx = safeAliasTargetTyIdx(symbols, tyIdx)
    return targetTyIdx === undefined
      ? undefined
      : tonAddressKindForTy(symbols, targetTyIdx, visited)
  }
  return undefined
}

function formatScalarValue(value: unknown): string {
  if (typeof value === "string") return value
  if (value === null || value === undefined) return ""
  return String(value)
}

function isTonFieldName(label: string): boolean {
  const words = label
    .replaceAll(/([a-z0-9])([A-Z])/g, "$1 $2")
    .split(/[^a-zA-Z0-9]+/)
    .map(word => word.toLowerCase())

  return words.some(word => word === "ton" || word === "gram" || word === "grams")
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
      hasValueField: duplicatedLabels.has(label)
        ? true
        : !isStructWithOwnLabel(symbols, variant.variant_ty_idx),
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
