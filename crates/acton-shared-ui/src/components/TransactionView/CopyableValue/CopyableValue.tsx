import {clsx} from "clsx"
import type React from "react"

import {CopyValueButton} from "../CopyValueButton"

import styles from "./CopyableValue.module.css"

interface CopyableValueProps {
  readonly value: string
  readonly label: string
  readonly children: React.ReactNode
  readonly className?: string
  readonly contentClassName?: string
}

export function CopyableValue({
  value,
  label,
  children,
  className,
  contentClassName,
}: CopyableValueProps): React.JSX.Element {
  return (
    <span className={clsx(styles.copyableValue, className)}>
      <span className={clsx(styles.copyableValueContent, contentClassName)}>{children}</span>
      <CopyValueButton className={styles.copyButton} value={value} label={label} />
    </span>
  )
}
