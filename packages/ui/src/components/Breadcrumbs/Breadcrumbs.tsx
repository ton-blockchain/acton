import {ChevronRight} from "lucide-react"
import type {ComponentPropsWithRef, CSSProperties, ReactNode} from "react"

import {cx} from "../../lib/cx"
import {Skeleton} from "../Skeleton"
import styles from "./Breadcrumbs.module.css"

export type BreadcrumbsItem = Readonly<{
  readonly current?: boolean
  readonly id?: string
  readonly label?: ReactNode
  readonly link?: (children: ReactNode, className: string) => ReactNode
  readonly loading?: boolean
  readonly loadingLabel?: string
  readonly preserveEnd?: number
  readonly preserveStart?: number
  readonly skeletonWidth?: CSSProperties["width"]
  readonly truncate?: boolean | "end" | "middle"
}>

export type BreadcrumbsProps = Readonly<
  Omit<ComponentPropsWithRef<"nav">, "children"> & {
    readonly ariaLabel?: string
    readonly items: readonly BreadcrumbsItem[]
    readonly loadingLabel?: string
    readonly separator?: ReactNode
  }
>

export function Breadcrumbs({
  ariaLabel = "Breadcrumb",
  className,
  items,
  loadingLabel = "Loading breadcrumb item",
  ref,
  separator,
  ...props
}: BreadcrumbsProps) {
  const hasLoadingItem = items.some(item => item.loading)

  return (
    <nav
      {...props}
      ref={ref}
      aria-label={ariaLabel}
      aria-busy={hasLoadingItem || undefined}
      className={cx(styles.breadcrumbs, className)}
    >
      <ol className={styles.list}>
        {items.map((item, index) => {
          const isCurrent = item.current ?? (index === items.length - 1 && !item.link)
          const key = item.id ?? `${index}-${getItemKeyLabel(item)}`

          return (
            <li
              key={key}
              className={cx(styles.listItem, item.truncate === false && styles.listItemNoTruncate)}
            >
              {index > 0 ? (
                <span className={styles.separator} aria-hidden="true">
                  {separator ?? <ChevronRight />}
                </span>
              ) : undefined}
              {renderItem(item, isCurrent, loadingLabel)}
            </li>
          )
        })}
      </ol>
    </nav>
  )
}

function renderItem(item: BreadcrumbsItem, isCurrent: boolean, loadingLabel: string) {
  if (item.loading) {
    return (
      <span
        className={cx(
          styles.item,
          styles.loadingItem,
          item.truncate === false && styles.noTruncate,
        )}
        aria-label={item.loadingLabel ?? loadingLabel}
      >
        <Skeleton width={item.skeletonWidth ?? "8rem"} height="0.875rem" />
      </span>
    )
  }

  const label = renderLabel(item)

  if (isCurrent) {
    return (
      <span
        className={cx(styles.item, styles.current, item.truncate === false && styles.noTruncate)}
        aria-current="page"
      >
        {label}
      </span>
    )
  }

  if (item.link) {
    return item.link(
      label,
      cx(styles.item, styles.linkItem, item.truncate === false && styles.noTruncate),
    )
  }

  return (
    <span className={cx(styles.item, item.truncate === false && styles.noTruncate)}>{label}</span>
  )
}

function getItemKeyLabel(item: BreadcrumbsItem) {
  if (typeof item.label === "string" || typeof item.label === "number") return item.label
  if (item.loading) return "loading"
  return "item"
}

function renderLabel(item: BreadcrumbsItem) {
  if (item.truncate !== "middle") {
    return <span className={styles.label}>{item.label}</span>
  }

  if (typeof item.label !== "string" && typeof item.label !== "number") {
    return <span className={styles.label}>{item.label}</span>
  }

  const text = String(item.label)
  const preserveEnd = Math.max(1, item.preserveEnd ?? 8)
  const preserveStart = Math.max(1, item.preserveStart ?? preserveEnd)

  if (text.length <= preserveStart + preserveEnd + 3) {
    return <span className={styles.label}>{text}</span>
  }

  return (
    <span className={styles.label} title={text} aria-label={text}>
      {text.slice(0, preserveStart)}...{text.slice(-preserveEnd)}
    </span>
  )
}
