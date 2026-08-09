import {X} from "lucide-react"
import {useEffect, useId, useMemo, useRef, useState} from "react"
import type {KeyboardEvent, ReactNode} from "react"

import {cx} from "../../lib/cx"
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
  const listboxId = `${inputId}-options`
  const descriptionId = description ? `${inputId}-description` : undefined
  const inputRef = useRef<HTMLInputElement>(null)
  const [query, setQuery] = useState("")
  const [isFocused, setIsFocused] = useState(false)
  const [isListDismissed, setIsListDismissed] = useState(false)
  const [activeIndex, setActiveIndex] = useState(0)
  const suggestions = useMemo(
    () => getSuggestions(query, options, values),
    [options, query, values],
  )
  const open = isFocused && !isListDismissed && !disabled && suggestions.length > 0
  const activeSuggestion = open ? suggestions[Math.min(activeIndex, suggestions.length - 1)] : null

  useEffect(() => {
    inputRef.current?.setCustomValidity(
      required && values.length === 0 ? "Select at least one value." : "",
    )
  }, [required, values.length])

  const selectValue = (value: string) => {
    onValuesChange([...values, value])
    setQuery("")
    setActiveIndex(0)
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

    if (event.key === "ArrowDown") {
      event.preventDefault()
      setActiveIndex(index => (index + 1) % suggestions.length)
    } else if (event.key === "ArrowUp") {
      event.preventDefault()
      setActiveIndex(index => (index - 1 + suggestions.length) % suggestions.length)
    } else if (event.key === "Enter" && activeSuggestion) {
      event.preventDefault()
      selectValue(activeSuggestion)
    } else if (event.key === "Tab" && !event.shiftKey && activeSuggestion) {
      event.preventDefault()
      selectValue(activeSuggestion)
    } else if (event.key === "Escape") {
      setIsListDismissed(true)
    }
  }

  return (
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
        <div className={cx(styles.control, invalid && styles.invalid, disabled && styles.disabled)}>
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
          <input
            ref={inputRef}
            id={inputId}
            className={styles.input}
            value={query}
            disabled={disabled}
            required={required && values.length === 0}
            placeholder={values.length === 0 ? placeholder : undefined}
            autoComplete="off"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck={false}
            role="combobox"
            aria-autocomplete="list"
            aria-controls={open ? listboxId : undefined}
            aria-expanded={open}
            aria-invalid={invalid || undefined}
            aria-describedby={descriptionId}
            aria-activedescendant={
              activeSuggestion ? multiValueOptionId(inputId, activeSuggestion) : undefined
            }
            onBlur={() => {
              setIsFocused(false)
              setIsListDismissed(false)
            }}
            onChange={event => {
              setQuery(event.target.value)
              setIsListDismissed(false)
              setActiveIndex(0)
            }}
            onFocus={() => {
              setIsFocused(true)
              setIsListDismissed(false)
            }}
            onKeyDown={handleKeyDown}
          />
        </div>
        {open ? (
          <div id={listboxId} className={styles.options} role="listbox">
            {suggestions.map((suggestion, index) => (
              <button
                key={suggestion}
                id={multiValueOptionId(inputId, suggestion)}
                type="button"
                tabIndex={-1}
                className={styles.option}
                role="option"
                aria-selected={suggestion === activeSuggestion}
                data-active={index === Math.min(activeIndex, suggestions.length - 1) || undefined}
                onMouseDown={event => event.preventDefault()}
                onClick={() => selectValue(suggestion)}
              >
                {suggestion}
              </button>
            ))}
          </div>
        ) : null}
      </div>
      {description ? (
        <div id={descriptionId} className={styles.description}>
          {description}
        </div>
      ) : null}
    </div>
  )
}

function getSuggestions(query: string, options: readonly string[], values: readonly string[]) {
  const normalizedQuery = query.trim().toLowerCase()
  const selected = new Set(values)
  return options.filter(option => {
    return !selected.has(option) && option.toLowerCase().includes(normalizedQuery)
  })
}

function multiValueOptionId(inputId: string, value: string) {
  return `${inputId}-option-${value.replaceAll(/[^a-zA-Z0-9_-]/g, "-")}`
}
