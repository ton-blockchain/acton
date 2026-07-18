import {Check, ChevronDown, ChevronRight, Copy, LoaderCircle} from "lucide-react"
import {
  useEffect,
  useId,
  useState,
  type ComponentPropsWithRef,
  type CSSProperties,
  type MouseEvent,
  type ReactNode,
} from "react"

import {cx} from "../../lib/cx"
import {SkeletonText} from "../Skeleton"
import styles from "./RawDataBlock.module.css"

export type RawDataBlockVariant = "embedded" | "standalone"

export type RawDataBlockProps = Readonly<
  Omit<ComponentPropsWithRef<"div">, "children" | "title"> & {
    readonly children?: ReactNode
    readonly codeClassName?: string
    readonly collapsible?: boolean
    readonly contentClassName?: string
    readonly copyLabel?: string
    readonly copyValue?: string
    /** Fully rendered content that replaces the built-in pre/code presentation. */
    readonly customContent?: ReactNode
    readonly defaultExpanded?: boolean
    readonly empty?: boolean
    readonly emptyContent?: ReactNode
    readonly expanded?: boolean
    readonly loading?: boolean
    readonly loadingLabel?: string
    readonly maxHeight?: CSSProperties["maxHeight"]
    readonly onCopy?: (value: string) => Promise<void> | void
    readonly onCopyError?: (error: unknown) => void
    readonly onExpandedChange?: (expanded: boolean) => void
    readonly resetDelay?: number
    readonly showCopy?: boolean
    readonly title?: ReactNode
    readonly titleLabel?: string
    readonly value: string
    readonly variant?: RawDataBlockVariant
    readonly wrap?: boolean
  }
>

const variantClassNames = {
  embedded: styles.variantEmbedded,
  standalone: styles.variantStandalone,
} satisfies Record<RawDataBlockVariant, string>

export function RawDataBlock({
  "aria-busy": ariaBusy,
  children,
  className,
  codeClassName,
  collapsible = false,
  contentClassName,
  copyLabel = "raw data",
  copyValue,
  customContent,
  defaultExpanded = true,
  empty = false,
  emptyContent = "No data available",
  expanded,
  loading = false,
  loadingLabel = "Loading raw data",
  maxHeight,
  onCopy,
  onCopyError,
  onExpandedChange,
  ref,
  resetDelay = 1600,
  showCopy = true,
  style,
  title,
  titleLabel,
  value,
  variant = "standalone",
  wrap = true,
  ...props
}: RawDataBlockProps) {
  const generatedId = useId()
  const [isCopied, setIsCopied] = useState(false)
  const [uncontrolledExpanded, setUncontrolledExpanded] = useState(defaultExpanded)
  const hasTitle = title !== undefined && title !== null
  const canCollapse = collapsible && hasTitle
  const isExpanded = canCollapse ? (expanded ?? uncontrolledExpanded) : true
  const valueToCopy = copyValue ?? value
  const copyTitle = isCopied ? `Copied ${copyLabel}` : `Copy ${copyLabel}`
  const canCopy = !loading && !empty && showCopy && valueToCopy.length > 0
  const resolvedTitleLabel =
    titleLabel ?? (typeof title === "string" ? title : undefined) ?? copyLabel
  const contentId = `${generatedId}-content`

  useEffect(() => {
    setIsCopied(false)
  }, [valueToCopy])

  useEffect(() => {
    if (!isCopied || resetDelay <= 0) return

    const timer = globalThis.setTimeout(() => setIsCopied(false), resetDelay)
    return () => globalThis.clearTimeout(timer)
  }, [isCopied, resetDelay])

  const handleCopy = async (event: MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation()

    try {
      if (onCopy) {
        await onCopy(valueToCopy)
      } else {
        await navigator.clipboard.writeText(valueToCopy)
      }

      setIsCopied(true)
    } catch (error) {
      onCopyError?.(error)
    }
  }

  const toggleExpanded = () => {
    const nextExpanded = !isExpanded

    if (expanded === undefined) {
      setUncontrolledExpanded(nextExpanded)
    }

    onExpandedChange?.(nextExpanded)
  }

  const copyButton = canCopy ? (
    <button
      type="button"
      className={cx(
        styles.copyButton,
        hasTitle ? styles.headerCopyButton : styles.floatingCopyButton,
      )}
      onClick={event => void handleCopy(event)}
      aria-label={copyTitle}
      title={copyTitle}
    >
      {isCopied ? <Check size={14} aria-hidden="true" /> : <Copy size={14} aria-hidden="true" />}
    </button>
  ) : undefined

  const loadingIndicator = loading && canCollapse && (
    <span className={styles.loadingIndicator} role="status" aria-label={loadingLabel}>
      <LoaderCircle size={14} aria-hidden="true" />
    </span>
  )

  return (
    <div
      {...props}
      ref={ref}
      aria-busy={loading ? true : ariaBusy}
      data-expanded={hasTitle ? String(isExpanded) : undefined}
      data-has-title={hasTitle ? "true" : undefined}
      data-loading={loading ? "true" : undefined}
      className={cx(styles.rawDataBlock, variantClassNames[variant], className)}
      style={getRawDataBlockStyle(style, maxHeight)}
    >
      {hasTitle && (
        <div className={styles.header}>
          {canCollapse ? (
            <button
              type="button"
              className={styles.headerToggle}
              aria-controls={contentId}
              aria-expanded={isExpanded}
              aria-label={`${isExpanded ? "Collapse" : "Expand"} ${resolvedTitleLabel}`}
              onClick={toggleExpanded}
            >
              <span className={styles.headerIcon} aria-hidden="true">
                {isExpanded ? <ChevronDown /> : <ChevronRight />}
              </span>
              <span className={styles.headerTitle}>{title}</span>
            </button>
          ) : (
            <div className={styles.headerTitle}>{title}</div>
          )}
          {loadingIndicator || copyButton}
        </div>
      )}

      {isExpanded && (
        <div
          id={hasTitle ? contentId : undefined}
          hidden={loading && canCollapse}
          className={cx(
            styles.content,
            empty && styles.emptyContent,
            loading && !canCollapse && styles.loadingContent,
            contentClassName,
          )}
          role={loading && !canCollapse ? "status" : undefined}
          aria-label={loading && !canCollapse ? loadingLabel : undefined}
        >
          {loading && !canCollapse ? (
            <SkeletonText lineCount={3} />
          ) : empty ? (
            <div className={styles.empty}>{emptyContent}</div>
          ) : customContent !== undefined && customContent !== null ? (
            customContent
          ) : (
            <pre className={cx(styles.pre, wrap && styles.preWrap)}>
              <code className={codeClassName}>{children ?? value}</code>
            </pre>
          )}
        </div>
      )}

      {!hasTitle && copyButton}
    </div>
  )
}

function getRawDataBlockStyle(
  style: CSSProperties | undefined,
  maxHeight: CSSProperties["maxHeight"] | undefined,
) {
  return {
    ...style,
    ...(maxHeight === undefined ? {} : {"--acton-raw-data-max-height": toCssSize(maxHeight)}),
  } as CSSProperties
}

function toCssSize(value: CSSProperties["maxHeight"]) {
  return typeof value === "number" ? `${value}px` : value
}
