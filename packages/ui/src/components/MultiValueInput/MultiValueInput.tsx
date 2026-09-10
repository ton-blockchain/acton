import {Autocomplete} from "@base-ui/react/autocomplete"
import {X} from "lucide-react"
import {useEffect, useId, useMemo, useRef, useState} from "react"
import type {KeyboardEvent, ReactNode} from "react"

import {cx} from "../../lib/cx"
import {useTheme} from "../Theme/ThemeProvider"
import styles from "./MultiValueInput.module.css"

export interface MultiValueInputProps {
  readonly className?: string
  readonly description?: ReactNode
  readonly disabled?: boolean
  readonly id?: string
  readonly invalid?: boolean
  readonly label?: ReactNode
  readonly options: readonly string[]
  readonly placeholder?: string
  readonly required?: boolean
  readonly values: readonly string[]
  readonly onValuesChange: (values: readonly string[]) => void
}

export function MultiValueInput({
  className,
  description,
  disabled = false,
  id,
  invalid = false,
  label,
  options,
  placeholder,
  required = false,
  values,
  onValuesChange,
}: MultiValueInputProps) {
  const generatedId = useId()
  const inputId = id ?? generatedId
  const descriptionId = description ? `${inputId}-description` : undefined
  const {theme} = useTheme()
  const controlRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const highlightedItemRef = useRef<string | undefined>(undefined)
  const [query, setQuery] = useState("")
  const [isOpen, setOpen] = useState(false)
  const suggestions = useMemo(
    () => getSuggestions(query, options, values),
    [options, query, values],
  )
  const open = isOpen && !disabled && suggestions.length > 0

  useEffect(() => {
    inputRef.current?.setCustomValidity(
      required && values.length === 0 ? "Select at least one value" : "",
    )
  }, [required, values.length])

  const selectValue = (value: string) => {
    onValuesChange([...values, value])
    setQuery("")
    inputRef.current?.focus()
  }

  const removeValue = (value: string) => {
    onValuesChange(values.filter(candidate => candidate !== value))
  }

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Backspace" && query.length === 0 && values.length > 0) {
      event.preventDefault()
      onValuesChange(values.slice(0, -1))
      return
    }
    if (!open) return

    if (event.key === "Tab" && !event.shiftKey && highlightedItemRef.current) {
      event.preventDefault()
      selectValue(highlightedItemRef.current)
    }
  }

  return (
    <Autocomplete.Root
      items={suggestions}
      mode="none"
      autoHighlight="always"
      open={open}
      openOnInputClick
      value={query}
      onItemHighlighted={item => {
        highlightedItemRef.current = item
      }}
      onOpenChange={(nextOpen, details) => {
        if (details.reason !== "item-press") setOpen(nextOpen)
      }}
      onValueChange={(nextValue, details) => {
        if (details.reason !== "item-press") {
          setQuery(nextValue)
          setOpen(true)
        }
      }}
    >
      <div className={cx(styles.field, className)}>
        {label ? (
          <label className={styles.label} htmlFor={inputId}>
            {label}
            {required ? (
              <span className={styles.required} aria-hidden="true">
                *
              </span>
            ) : null}
          </label>
        ) : null}
        <div className={styles.controlWrap}>
          <div
            ref={controlRef}
            className={cx(styles.control, invalid && styles.invalid, disabled && styles.disabled)}
          >
            {values.map(value => (
              <span key={value} className={styles.value}>
                <span className={styles.valueLabel}>{value}</span>
                <button
                  type="button"
                  className={styles.removeButton}
                  aria-label={`Remove ${value}`}
                  disabled={disabled}
                  onMouseDown={event => event.preventDefault()}
                  onClick={() => removeValue(value)}
                >
                  <X size={13} aria-hidden="true" />
                </button>
              </span>
            ))}
            <Autocomplete.Input
              ref={inputRef}
              id={inputId}
              className={styles.input}
              disabled={disabled}
              required={required && values.length === 0}
              placeholder={values.length === 0 ? placeholder : undefined}
              autoComplete="off"
              autoCorrect="off"
              autoCapitalize="off"
              spellCheck={false}
              aria-invalid={invalid || undefined}
              aria-describedby={descriptionId}
              onFocus={() => setOpen(true)}
              onKeyDown={handleKeyDown}
            />
          </div>
          <Autocomplete.Portal>
            <Autocomplete.Positioner
              anchor={controlRef}
              align="start"
              sideOffset={4}
              className={styles.positioner}
              data-theme={theme}
            >
              <Autocomplete.Popup className={styles.popup}>
                <Autocomplete.List className={styles.options}>
                  {(suggestion: string) => (
                    <Autocomplete.Item
                      key={suggestion}
                      value={suggestion}
                      className={styles.option}
                      onClick={() => selectValue(suggestion)}
                    >
                      {suggestion}
                    </Autocomplete.Item>
                  )}
                </Autocomplete.List>
              </Autocomplete.Popup>
            </Autocomplete.Positioner>
          </Autocomplete.Portal>
        </div>
        {description ? (
          <div id={descriptionId} className={styles.description}>
            {description}
          </div>
        ) : null}
      </div>
    </Autocomplete.Root>
  )
}

function getSuggestions(query: string, options: readonly string[], values: readonly string[]) {
  const normalizedQuery = query.trim().toLowerCase()
  const selected = new Set(values)
  return options.filter(option => {
    return !selected.has(option) && option.toLowerCase().includes(normalizedQuery)
  })
}
