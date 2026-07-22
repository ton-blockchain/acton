import type {ComponentPropsWithRef} from "react"

import {cx} from "../../lib/cx"
import styles from "./InlineLoader.module.css"

export type InlineLoaderProps = Readonly<
  Omit<ComponentPropsWithRef<"div">, "children"> & {
    readonly message?: string
    readonly subtext?: string
  }
>

export function InlineLoader({
  "aria-live": ariaLive = "polite",
  className,
  message = "Loading",
  ref,
  role = "status",
  subtext,
  ...props
}: InlineLoaderProps) {
  return (
    <div
      {...props}
      ref={ref}
      role={role}
      aria-live={ariaLive}
      className={cx(styles.inlineLoader, className)}
    >
      <span className={styles.spinnerWrapper} aria-hidden="true">
        <span className={styles.spinner} />
        <span className={styles.spinnerGlow} />
      </span>
      {message && <span className={styles.message}>{message}</span>}
      {subtext && (
        <span className={styles.subtext}>
          <span>{subtext}</span>
          <span className={styles.dots} aria-hidden="true">
            <span className={styles.dot}>.</span>
            <span className={styles.dot}>.</span>
            <span className={styles.dot}>.</span>
          </span>
        </span>
      )}
    </div>
  )
}
