import {useCallback, useEffect, useId, useRef} from "react"
import type {ComponentPropsWithRef, ReactNode} from "react"

import {cx} from "../../lib/cx"
import styles from "./Input.module.css"

export type ActonInputSize = "sm" | "md" | "lg"

export type InputProps = Readonly<
  Omit<ComponentPropsWithRef<"input">, "size"> & {
    readonly size?: ActonInputSize
    readonly mono?: boolean
    readonly invalid?: boolean
    readonly leadingIcon?: ReactNode
    readonly shortcut?: string
    readonly label?: ReactNode
    readonly description?: ReactNode
    readonly fieldClassName?: string
  }
>

const sizeClassNames = {
  sm: styles.sizeSm,
  md: styles.sizeMd,
  lg: styles.sizeLg,
} satisfies Record<ActonInputSize, string>

export function Input({
  "aria-describedby": ariaDescribedBy,
  "aria-invalid": ariaInvalid,
  autoCapitalize = "off",
  autoComplete = "off",
  autoCorrect = "off",
  className,
  description,
  disabled,
  fieldClassName,
  id,
  invalid = false,
  label,
  leadingIcon,
  mono = false,
  ref,
  required,
  shortcut,
  size = "md",
  spellCheck = false,
  ...props
}: InputProps) {
  const generatedId = useId()
  const inputRef = useRef<HTMLInputElement>(null)
  const hasField = label !== undefined || description !== undefined
  const inputId = id ?? (hasField ? generatedId : undefined)
  const descriptionId = description === undefined ? undefined : `${inputId}-description`
  const describedBy = [ariaDescribedBy, descriptionId].filter(Boolean).join(" ") || undefined
  const isInvalid =
    invalid ||
    ariaInvalid === true ||
    ariaInvalid === "true" ||
    ariaInvalid === "grammar" ||
    ariaInvalid === "spelling"
  const shortcutModifier = globalThis.navigator?.userAgent.includes("Windows") ? "Ctrl" : "⌘"

  const setInputRef = useCallback(
    (node: HTMLInputElement | null) => {
      inputRef.current = node
      if (typeof ref === "function") {
        return ref(node)
      }
      if (ref) {
        ref.current = node
      }
    },
    [ref],
  )

  useEffect(() => {
    if (shortcut === undefined || disabled) {
      return
    }

    const shortcutKey = shortcut.toLowerCase()
    const handleShortcut = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== shortcutKey) {
        return
      }

      event.preventDefault()
      inputRef.current?.focus()
      inputRef.current?.select()
    }

    globalThis.addEventListener("keydown", handleShortcut)
    return () => globalThis.removeEventListener("keydown", handleShortcut)
  }, [disabled, shortcut])

  const input = (
    <input
      {...props}
      ref={setInputRef}
      id={inputId}
      disabled={disabled}
      required={required}
      autoCapitalize={autoCapitalize}
      autoComplete={autoComplete}
      autoCorrect={autoCorrect}
      spellCheck={spellCheck}
      aria-invalid={isInvalid ? true : ariaInvalid}
      aria-describedby={describedBy}
      className={cx(
        styles.input,
        sizeClassNames[size],
        leadingIcon !== undefined && styles.withLeadingIcon,
        shortcut !== undefined && styles.withShortcut,
        mono && styles.mono,
        isInvalid && styles.invalid,
        className,
      )}
    />
  )
  const hasDecoration = leadingIcon !== undefined || shortcut !== undefined
  const control = hasDecoration ? (
    <div className={styles.control}>
      {leadingIcon === undefined ? undefined : (
        <span className={styles.leadingIcon} aria-hidden="true">
          {leadingIcon}
        </span>
      )}
      {input}
      {shortcut === undefined ? undefined : (
        <span className={styles.shortcut} aria-hidden="true">
          <kbd>{shortcutModifier}</kbd>
          <kbd>{shortcut}</kbd>
        </span>
      )}
    </div>
  ) : (
    input
  )

  if (!hasField) {
    return control
  }

  return (
    <div className={cx(styles.field, fieldClassName)}>
      {label === undefined ? undefined : (
        <label className={styles.label} htmlFor={inputId}>
          {label}
          {required ? (
            <span className={styles.required} aria-hidden="true">
              *
            </span>
          ) : undefined}
        </label>
      )}
      {control}
      {description === undefined ? undefined : (
        <div id={descriptionId} className={styles.description}>
          {description}
        </div>
      )}
    </div>
  )
}
