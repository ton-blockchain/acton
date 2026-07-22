import {ChevronDown} from "lucide-react"
import {useId} from "react"
import type {ComponentPropsWithRef, ReactNode} from "react"

import {cx} from "../../lib/cx"
import styles from "./Select.module.css"

export type ActonSelectSize = "sm" | "md" | "lg"

export type SelectProps = Readonly<
  Omit<ComponentPropsWithRef<"select">, "size"> & {
    readonly size?: ActonSelectSize
    readonly invalid?: boolean
    readonly label?: ReactNode
    readonly description?: ReactNode
    readonly fieldClassName?: string
  }
>

const sizeClassNames = {
  sm: styles.sizeSm,
  md: styles.sizeMd,
  lg: styles.sizeLg,
} satisfies Record<ActonSelectSize, string>

export function Select({
  "aria-describedby": ariaDescribedBy,
  "aria-invalid": ariaInvalid,
  children,
  className,
  description,
  disabled,
  fieldClassName,
  id,
  invalid = false,
  label,
  ref,
  required,
  size = "md",
  ...props
}: SelectProps) {
  const generatedId = useId()
  const hasField = label !== undefined || description !== undefined
  const selectId = id ?? (hasField ? generatedId : undefined)
  const descriptionId = description === undefined ? undefined : `${selectId}-description`
  const describedBy = [ariaDescribedBy, descriptionId].filter(Boolean).join(" ") || undefined
  const isInvalid =
    invalid ||
    ariaInvalid === true ||
    ariaInvalid === "true" ||
    ariaInvalid === "grammar" ||
    ariaInvalid === "spelling"

  const control = (
    <div className={cx(styles.control, sizeClassNames[size])}>
      <select
        {...props}
        ref={ref}
        id={selectId}
        disabled={disabled}
        required={required}
        aria-invalid={isInvalid ? true : ariaInvalid}
        aria-describedby={describedBy}
        className={cx(styles.select, isInvalid && styles.invalid, className)}
      >
        {children}
      </select>
      <ChevronDown className={styles.chevron} strokeWidth={2} aria-hidden="true" />
    </div>
  )

  if (!hasField) {
    return control
  }

  return (
    <div className={cx(styles.field, fieldClassName)}>
      {label === undefined ? undefined : (
        <label className={styles.label} htmlFor={selectId}>
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
