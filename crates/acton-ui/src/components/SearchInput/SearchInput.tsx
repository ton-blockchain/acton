import {Autocomplete} from "@base-ui/react/autocomplete"
import {Search, X} from "lucide-react"
import {useCallback, useEffect, useRef, useState} from "react"
import type {KeyboardEvent, ReactNode} from "react"

import styles from "./SearchInput.module.css"

export type SearchInputSize = "sm" | "lg"

export interface SearchInputItem {
  readonly id: string
  readonly label: ReactNode
  readonly description?: ReactNode
  readonly icon?: ReactNode
  readonly onSelect: () => void
  readonly onRemove?: () => void
  readonly removeLabel?: string
}

export interface SearchInputProps {
  readonly ariaLabel: string
  readonly autoFocus?: boolean
  readonly className?: string
  readonly invalid?: boolean
  readonly items: readonly SearchInputItem[]
  readonly onOpenChange?: (open: boolean) => void
  readonly onSubmit?: (value: string) => boolean | void
  readonly onValueChange: (value: string) => void
  readonly open?: boolean
  readonly placeholder?: string
  readonly size?: SearchInputSize
  readonly value: string
}

export function SearchInput({
  ariaLabel,
  autoFocus = false,
  className,
  invalid = false,
  items,
  onOpenChange,
  onSubmit,
  onValueChange,
  open,
  placeholder,
  size = "lg",
  value,
}: SearchInputProps) {
  const [uncontrolledOpen, setUncontrolledOpen] = useState(false)
  const [portalContainer, setPortalContainer] = useState<HTMLElement | null>(null)
  const controlRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const highlightedItemRef = useRef<SearchInputItem | undefined>(undefined)
  const isOpen = open ?? uncontrolledOpen

  useEffect(() => {
    if (autoFocus) {
      inputRef.current?.focus()
    }
  }, [autoFocus])

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

  const rootClassName = [styles.root, size === "sm" ? styles.sizeSm : styles.sizeLg, className]
    .filter(Boolean)
    .join(" ")

  return (
    <Autocomplete.Root
      itemToStringValue={item => (typeof item.label === "string" ? item.label : "")}
      items={items}
      mode="none"
      open={isOpen && items.length > 0}
      openOnInputClick
      value={value}
      onItemHighlighted={item => {
        highlightedItemRef.current = item
      }}
      onOpenChange={nextOpen => setOpen(nextOpen)}
      onValueChange={(nextValue, eventDetails) => {
        if (eventDetails.reason !== "item-press") {
          onValueChange(nextValue)
          setOpen(true)
        }
      }}
    >
      <section
        ref={setPortalContainer}
        className={rootClassName}
        role="search"
        aria-label={ariaLabel}
      >
        <div ref={controlRef} className={`${styles.control} ${invalid ? styles.invalid : ""}`}>
          <span className={styles.searchIcon} aria-hidden="true">
            <Search size={size === "sm" ? 16 : 20} />
          </span>
          <Autocomplete.Input
            ref={inputRef}
            type="search"
            className={styles.input}
            aria-invalid={invalid}
            aria-label={ariaLabel}
            autoComplete="off"
            autoCorrect="off"
            autoCapitalize="off"
            placeholder={placeholder}
            spellCheck={false}
            onFocus={() => setOpen(true)}
            onKeyDown={handleKeyDown}
          />
        </div>
      </section>
      {portalContainer && (
        <Autocomplete.Portal container={portalContainer}>
          <Autocomplete.Positioner
            align="start"
            anchor={controlRef}
            className={styles.positioner}
            sideOffset={size === "sm" ? 6 : 8}
          >
            <Autocomplete.Popup
              className={`${styles.dropdown} ${size === "sm" ? styles.dropdownSm : styles.dropdownLg}`}
            >
              <Autocomplete.List className={styles.list}>
                {(item: SearchInputItem) => (
                  <Autocomplete.Item
                    key={item.id}
                    className={styles.item}
                    value={item}
                    onClick={() => {
                      item.onSelect()
                      inputRef.current?.blur()
                    }}
                  >
                    <div
                      className={`${styles.itemButton} ${item.icon ? styles.itemButtonWithIcon : ""}`}
                    >
                      {item.icon && (
                        <span className={styles.itemIcon} aria-hidden="true">
                          {item.icon}
                        </span>
                      )}
                      <span className={styles.itemText}>
                        <span
                          className={item.description ? styles.itemLabelStrong : styles.itemLabel}
                        >
                          {item.label}
                        </span>
                        {item.description && (
                          <span className={styles.itemDescription}>{item.description}</span>
                        )}
                      </span>
                    </div>
                    {item.onRemove && (
                      <button
                        type="button"
                        className={styles.removeButton}
                        aria-label={item.removeLabel ?? "Remove item"}
                        title={item.removeLabel ?? "Remove item"}
                        onClick={event => {
                          event.stopPropagation()
                          item.onRemove?.()
                          if (items.length === 1) {
                            setOpen(false)
                          }
                        }}
                      >
                        <X size={14} />
                      </button>
                    )}
                  </Autocomplete.Item>
                )}
              </Autocomplete.List>
            </Autocomplete.Popup>
          </Autocomplete.Positioner>
        </Autocomplete.Portal>
      )}
    </Autocomplete.Root>
  )
}
