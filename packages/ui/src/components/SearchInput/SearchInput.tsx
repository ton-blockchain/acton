import {Autocomplete} from "@base-ui/react/autocomplete"
import {Search, X} from "lucide-react"
import {Fragment, useCallback, useEffect, useRef, useState} from "react"
import type {KeyboardEvent, ReactNode} from "react"

import {Input} from "../Input/Input"
import {useTheme} from "../Theme/ThemeProvider"
import {Tooltip} from "../Tooltip"
import styles from "./SearchInput.module.css"

export type SearchInputSize = "sm" | "md" | "lg"
export type SearchInputVariant = "search" | "field"

export interface SearchInputItem {
  readonly id: string
  readonly label: ReactNode
  readonly description?: ReactNode
  readonly icon?: ReactNode
  readonly onSelect: () => void
  readonly onRemove?: () => void
  readonly removeLabel?: string
  /** Consecutive items with the same group share a visible result heading */
  readonly group?: string
}

export interface SearchInputProps {
  readonly ariaLabel: string
  readonly autoFocus?: boolean
  readonly className?: string
  readonly disabled?: boolean
  readonly inputClassName?: string
  readonly invalid?: boolean
  /** Keeps results inside a containing surface, such as Studio's search dialog */
  readonly inline?: boolean
  /** Shown below an inline field when its caller has no results to display */
  readonly emptyContent?: ReactNode
  readonly items: readonly SearchInputItem[]
  readonly onOpenChange?: (open: boolean) => void
  readonly onFocus?: () => void
  readonly onSubmit?: (value: string) => boolean | void
  readonly onValueChange: (value: string) => void
  readonly open?: boolean
  readonly placeholder?: string
  readonly size?: SearchInputSize
  readonly shortcut?: string
  readonly value: string
  readonly variant?: SearchInputVariant
}

export function SearchInput({
  ariaLabel,
  autoFocus = false,
  className,
  disabled = false,
  inputClassName,
  invalid = false,
  inline = false,
  emptyContent,
  items,
  onOpenChange,
  onFocus,
  onSubmit,
  onValueChange,
  open,
  placeholder,
  size = "lg",
  shortcut,
  value,
  variant = "search",
}: SearchInputProps) {
  const {theme} = useTheme()
  const [uncontrolledOpen, setUncontrolledOpen] = useState(false)
  const controlRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const highlightedItemRef = useRef<SearchInputItem | undefined>(undefined)
  const isOpen = open ?? uncontrolledOpen

  useEffect(() => {
    if (shortcut === undefined || disabled) return

    const shortcutKey = shortcut.toLowerCase()
    const handleShortcut = (event: globalThis.KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== shortcutKey) return

      event.preventDefault()
      inputRef.current?.focus()
      inputRef.current?.select()
    }

    globalThis.addEventListener("keydown", handleShortcut)
    return () => globalThis.removeEventListener("keydown", handleShortcut)
  }, [disabled, shortcut])

  useEffect(() => {
    if (autoFocus && !disabled) {
      inputRef.current?.focus()
    }
  }, [autoFocus, disabled])

  const setOpen = useCallback(
    (nextOpen: boolean) => {
      if (open === undefined) {
        setUncontrolledOpen(nextOpen)
      }
      onOpenChange?.(nextOpen)
    },
    [onOpenChange, open],
  )

  const submit = useCallback(() => {
    if (onSubmit?.(value) !== false) {
      setOpen(false)
      inputRef.current?.blur()
    }
  }, [onSubmit, setOpen, value])

  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Enter" && !highlightedItemRef.current) {
        event.preventDefault()
        submit()
      }
    },
    [submit],
  )

  const sizeClassName =
    size === "sm" ? styles.sizeSm : size === "md" ? styles.sizeMd : styles.sizeLg
  const rootClassName = [
    styles.root,
    sizeClassName,
    variant === "field" ? styles.fieldVariant : styles.searchVariant,
    className,
  ]
    .filter(Boolean)
    .join(" ")
  const iconSize = size === "sm" ? 16 : size === "md" ? 18 : 20

  // Both presentations share Base UI's collection and keyboard selection behavior.
  const results = (
    <Autocomplete.List className={`${styles.list} ${inline ? styles.inlineList : ""}`}>
      {items.map((item, index) => (
        <Fragment key={item.id}>
          {item.group && item.group !== items[index - 1]?.group && (
            <div className={styles.groupLabel} role="presentation">
              {item.group}
            </div>
          )}
          <Autocomplete.Item
            className={styles.item}
            value={item}
            onClick={() => {
              item.onSelect()
              inputRef.current?.blur()
            }}
          >
            <div className={`${styles.itemButton} ${item.icon ? styles.itemButtonWithIcon : ""}`}>
              {item.icon && (
                <span className={styles.itemIcon} aria-hidden="true">
                  {item.icon}
                </span>
              )}
              <span className={styles.itemText}>
                <span className={item.description ? styles.itemLabelStrong : styles.itemLabel}>
                  {item.label}
                </span>
                {item.description && (
                  <span className={styles.itemDescription}>{item.description}</span>
                )}
              </span>
            </div>
            {item.onRemove && (
              <Tooltip content={item.removeLabel ?? "Remove item"}>
                <button
                  type="button"
                  className={styles.removeButton}
                  aria-label={item.removeLabel ?? "Remove item"}
                  onClick={event => {
                    event.stopPropagation()
                    item.onRemove?.()
                    if (items.length === 1 && !inline) setOpen(false)
                  }}
                >
                  <X size={14} />
                </button>
              </Tooltip>
            )}
          </Autocomplete.Item>
        </Fragment>
      ))}
    </Autocomplete.List>
  )

  return (
    <Autocomplete.Root
      itemToStringValue={item => (typeof item.label === "string" ? item.label : "")}
      items={items}
      mode="none"
      inline={inline}
      autoHighlight={inline ? "always" : false}
      open={!disabled && (inline || (isOpen && items.length > 0))}
      openOnInputClick
      value={value}
      onItemHighlighted={item => {
        highlightedItemRef.current = item
      }}
      onOpenChange={nextOpen => {
        if (!inline) setOpen(nextOpen)
      }}
      onValueChange={(nextValue, eventDetails) => {
        if (eventDetails.reason !== "item-press") {
          onValueChange(nextValue)
          setOpen(true)
        }
      }}
    >
      <div
        className={rootClassName}
        role={variant === "search" ? "search" : undefined}
        aria-label={variant === "search" ? ariaLabel : undefined}
      >
        <div
          ref={controlRef}
          className={`${styles.control} ${
            variant === "search" && invalid ? styles.invalid : ""
          } ${variant === "search" && disabled ? styles.disabled : ""}`}
        >
          <span className={styles.searchIcon} aria-hidden="true">
            <Search size={iconSize} />
          </span>
          <Autocomplete.Input
            ref={inputRef}
            type={variant === "search" ? "search" : "text"}
            className={variant === "search" ? styles.input : inputClassName}
            render={variant === "field" ? <Input size={size} invalid={invalid} /> : undefined}
            aria-invalid={invalid}
            aria-label={ariaLabel}
            autoComplete="off"
            autoCorrect="off"
            autoCapitalize="off"
            disabled={disabled}
            placeholder={placeholder}
            spellCheck={false}
            onFocus={() => {
              if (!disabled) {
                onFocus?.()
                setOpen(true)
              }
            }}
            onKeyDown={handleKeyDown}
          />
        </div>
      </div>
      {inline ? (
        <div className={styles.inlineResults}>
          {results}
          {items.length === 0 && emptyContent}
        </div>
      ) : (
        <Autocomplete.Portal>
          <Autocomplete.Positioner
            align="start"
            anchor={controlRef}
            className={styles.positioner}
            data-theme={theme}
            sideOffset={size === "sm" ? 6 : 8}
          >
            <Autocomplete.Popup
              className={`${styles.dropdown} ${size === "sm" ? styles.dropdownSm : styles.dropdownLg}`}
            >
              {results}
            </Autocomplete.Popup>
          </Autocomplete.Positioner>
        </Autocomplete.Portal>
      )}
    </Autocomplete.Root>
  )
}
