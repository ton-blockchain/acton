import {useId} from "react"
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
  fieldClassName,
  id,
  invalid = false,
  label,
  leadingIcon,
  mono = false,
  ref,
  required,
  size = "md",
  spellCheck = false,
  ...props
}: InputProps) {
  const generatedId = useId()
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

  const input = (
    <input
      {...props}
      ref={ref}
      id={inputId}
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
        mono && styles.mono,
        isInvalid && styles.invalid,
        className,
      )}
    />
  )
  const control =
    leadingIcon === undefined ? (
      input
    ) : (
      <div className={styles.control}>
        <span className={styles.leadingIcon} aria-hidden="true">
          {leadingIcon}
        </span>
        {input}
      </div>
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
