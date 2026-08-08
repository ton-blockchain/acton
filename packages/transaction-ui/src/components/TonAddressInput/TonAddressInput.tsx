import {SearchInput, type SearchInputItem} from "@acton/ui"
import {useMemo, useState} from "react"
import type {ReactNode} from "react"

import {isTonAddress, type TonAddressKind} from "../../lib/tonAddress"
import styles from "./TonAddressInput.module.css"

export interface TonAddressSuggestion {
  readonly address: string
  readonly label?: string
  readonly description?: string
}

export interface TonAddressInputProps {
  readonly value: string
  readonly onValueChange: (value: string) => void
  readonly onSuggestionSelect?: (suggestion: TonAddressSuggestion) => void
  readonly kind?: TonAddressKind
  readonly suggestions?: readonly TonAddressSuggestion[]
  readonly label?: string
  readonly labelAction?: ReactNode
  readonly ariaLabel?: string
  readonly className?: string
  readonly fieldClassName?: string
  readonly placeholder?: string
  readonly disabled?: boolean
  readonly invalid?: boolean
  readonly required?: boolean
}

export function TonAddressInput({
  value,
  onValueChange,
  onSuggestionSelect,
  kind = "internal",
  suggestions = [],
  label,
  labelAction,
  ariaLabel = label ?? "TON address",
  className,
  fieldClassName,
  placeholder = addressPlaceholder(kind),
  disabled = false,
  invalid = false,
  required = false,
}: TonAddressInputProps) {
  const [filterSuggestions, setFilterSuggestions] = useState(false)
  const query = filterSuggestions ? value.trim().toLocaleLowerCase() : ""
  const items = useMemo<readonly SearchInputItem[]>(
    () =>
      suggestions
        .filter(suggestion => {
          if (!query) return true
          return [suggestion.address, suggestion.label, suggestion.description].some(candidate =>
            candidate?.toLocaleLowerCase().includes(query),
          )
        })
        .map((suggestion, index) => ({
          id: `${suggestion.address}:${index}`,
          label: suggestion.label ?? suggestion.address,
          description: suggestion.description,
          onSelect: () => {
            onValueChange(suggestion.address)
            onSuggestionSelect?.(suggestion)
          },
        })),
    [onSuggestionSelect, onValueChange, query, suggestions],
  )
  const hasInvalidAddress = value.trim().length > 0 ? !isTonAddress(value, kind) : required

  const input = (
    <SearchInput
      ariaLabel={ariaLabel}
      disabled={disabled}
      inputClassName={className}
      invalid={invalid || hasInvalidAddress}
      items={items}
      onFocus={() => setFilterSuggestions(false)}
      onValueChange={nextValue => {
        setFilterSuggestions(true)
        onValueChange(nextValue)
      }}
      placeholder={placeholder}
      size="md"
      value={value}
      variant="field"
    />
  )

  if (!label && !labelAction) return input

  return (
    <div className={`${styles.field} ${fieldClassName ?? ""}`}>
      <div className={styles.labelRow}>
        {label && (
          <span className={styles.label}>
            {label}
            {required && <span className={styles.required}>*</span>}
          </span>
        )}
        {labelAction && <div className={styles.labelAction}>{labelAction}</div>}
      </div>
      {input}
    </div>
  )
}

function addressPlaceholder(kind: TonAddressKind): string {
  if (kind === "external") return "External<bits:value>"
  if (kind === "any") return "EQ…, External<…>, or addr_none"
  return "EQ… or 0:…"
}
