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
    readonly suffix?: ReactNode
    readonly shortcut?: string
    readonly label?: ReactNode
    readonly labelAction?: ReactNode
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
  labelAction,
  leadingIcon,
  mono = false,
  ref,
  required,
  shortcut,
  size = "md",
  spellCheck = false,
  suffix,
  ...props
}: InputProps) {
  const generatedId = useId()
  const inputRef = useRef<HTMLInputElement>(null)
  const hasField = label !== undefined || labelAction !== undefined || description !== undefined
  const inputId = id ?? (hasField || suffix !== undefined ? generatedId : undefined)
  const descriptionId = description === undefined ? undefined : `${inputId}-description`
  const suffixId = suffix === undefined ? undefined : `${inputId}-suffix`
  const describedBy =
    [ariaDescribedBy, descriptionId, suffixId].filter(Boolean).join(" ") || undefined
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
        suffix !== undefined && styles.withSuffix,
        shortcut !== undefined && suffix !== undefined && styles.withShortcutAndSuffix,
        mono && styles.mono,
        isInvalid && styles.invalid,
        className,
      )}
    />
  )
  const hasTrailingDecoration = suffix !== undefined || shortcut !== undefined
  const hasDecoration = leadingIcon !== undefined || hasTrailingDecoration
  const control = hasDecoration ? (
    <div className={styles.control}>
      {leadingIcon === undefined ? undefined : (
        <span className={styles.leadingIcon} aria-hidden="true">
          {leadingIcon}
        </span>
      )}
      {input}
      {hasTrailingDecoration ? (
        <span className={styles.trailingDecoration}>
          {suffix === undefined ? undefined : (
            <span id={suffixId} className={styles.suffix}>
              {suffix}
            </span>
          )}
          {shortcut === undefined ? undefined : (
            <span className={styles.shortcut} aria-hidden="true">
              <kbd>{shortcutModifier}</kbd>
              <kbd>{shortcut}</kbd>
            </span>
          )}
        </span>
      ) : undefined}
    </div>
  ) : (
    input
  )

  if (!hasField) {
    return control
  }

  return (
    <div className={cx(styles.field, fieldClassName)}>
      {label === undefined && labelAction === undefined ? undefined : (
        <div className={styles.labelRow}>
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
          {labelAction === undefined ? undefined : (
            <div className={styles.labelAction}>{labelAction}</div>
          )}
        </div>
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
