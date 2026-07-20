import type {ComponentPropsWithRef, CSSProperties, ReactNode} from "react"
import {ChevronDown} from "lucide-react"

import {cx} from "../../lib/cx"
import {Skeleton} from "../Skeleton"
import styles from "./DataTable.module.css"

export type DataTableAlign = "center" | "left" | "right"
export type DataTableCellTone = "default" | "muted" | "strong" | "subtle"
export type DataTableLayout = "auto" | "fixed"

export type DataTableProps = Readonly<
  Omit<ComponentPropsWithRef<"section">, "title"> & {
    readonly actions?: ReactNode
    readonly meta?: ReactNode
    readonly minWidth?: CSSProperties["minWidth"]
    readonly title?: ReactNode
    readonly titleId?: string
  }
>

export type DataTableTableProps = Readonly<
  ComponentPropsWithRef<"table"> & {
    readonly layout?: DataTableLayout
    readonly rowDividers?: boolean
  }
>

export type DataTableRowProps = Readonly<
  ComponentPropsWithRef<"tr"> & {
    readonly groupChild?: boolean
    readonly hover?: boolean
    readonly interactive?: boolean
    readonly selected?: boolean
  }
>

export type DataTableCellProps = Readonly<
  ComponentPropsWithRef<"td"> & {
    readonly align?: DataTableAlign
    readonly columnWidth?: CSSProperties["width"]
    readonly mono?: boolean
    readonly tone?: DataTableCellTone
    readonly truncate?: boolean
  }
>

export type DataTableHeaderCellProps = Readonly<
  ComponentPropsWithRef<"th"> & {
    readonly align?: DataTableAlign
    readonly columnWidth?: CSSProperties["width"]
    readonly truncate?: boolean
  }
>

export type DataTableGroupRowProps = Readonly<
  Omit<ComponentPropsWithRef<"tr">, "children"> & {
    readonly children: ReactNode
    readonly colSpan: number
    readonly disabled?: boolean
    readonly expanded?: boolean
    readonly onToggle?: () => void
  }
>

export type DataTableEmptyProps = Readonly<
  Omit<ComponentPropsWithRef<"tr">, "children"> & {
    readonly children: ReactNode
    readonly colSpan: number
  }
>

export type DataTableSkeletonRowsProps = Readonly<{
  readonly alignments?: readonly DataTableAlign[]
  readonly columns: number
  readonly rowKeyPrefix?: string
  readonly rows?: number
  readonly widths?: readonly CSSProperties["width"][]
}>

const alignClassNames = {
  center: styles.alignCenter,
  left: styles.alignLeft,
  right: styles.alignRight,
} satisfies Record<DataTableAlign, string>

const layoutClassNames = {
  auto: styles.layoutAuto,
  fixed: styles.layoutFixed,
} satisfies Record<DataTableLayout, string>

const toneClassNames = {
  default: styles.toneDefault,
  muted: styles.toneMuted,
  strong: styles.toneStrong,
  subtle: styles.toneSubtle,
} satisfies Record<DataTableCellTone, string>

export function DataTable({
  actions,
  children,
  className,
  meta,
  minWidth = "42rem",
  ref,
  style,
  title,
  titleId,
  ...props
}: DataTableProps) {
  const hasTitle = title !== undefined && title !== null
  const hasMeta = meta !== undefined && meta !== null
  const hasActions = actions !== undefined && actions !== null
  const hasHeader = hasTitle || hasMeta || hasActions

  return (
    <section
      {...props}
      ref={ref}
      className={cx(styles.dataTable, className)}
      style={getDataTableStyle(style, minWidth)}
    >
      <div className={styles.inner}>
        {hasHeader ? (
          <div className={styles.titleBar}>
            {hasTitle ? (
              <h2 id={titleId} className={styles.title}>
                {title}
              </h2>
            ) : (
              <span />
            )}
            <div className={styles.titleEnd}>
              {hasMeta ? <span className={styles.meta}>{meta}</span> : undefined}
              {hasActions ? <div className={styles.actions}>{actions}</div> : undefined}
            </div>
          </div>
        ) : undefined}
        {children}
      </div>
    </section>
  )
}

export function DataTableTable({
  className,
  layout = "fixed",
  ref,
  rowDividers = true,
  ...props
}: DataTableTableProps) {
  return (
    <table
      {...props}
      ref={ref}
      className={cx(
        styles.table,
        layoutClassNames[layout],
        !rowDividers && styles.tableWithoutRowDividers,
        className,
      )}
    />
  )
}

export function DataTableHead({className, ref, ...props}: ComponentPropsWithRef<"thead">) {
  return <thead {...props} ref={ref} className={cx(styles.head, className)} />
}

export function DataTableBody({className, ref, ...props}: ComponentPropsWithRef<"tbody">) {
  return <tbody {...props} ref={ref} className={cx(styles.body, className)} />
}

export function DataTableFooter({className, ref, ...props}: ComponentPropsWithRef<"tfoot">) {
  return <tfoot {...props} ref={ref} className={cx(styles.footer, className)} />
}

export function DataTableRow({
  className,
  groupChild = false,
  hover = false,
  interactive = false,
  ref,
  selected = false,
  ...props
}: DataTableRowProps) {
  return (
    <tr
      {...props}
      ref={ref}
      className={cx(
        styles.row,
        groupChild && styles.rowGroupChild,
        hover && styles.rowHover,
        interactive && styles.rowInteractive,
        selected && styles.rowSelected,
        className,
      )}
    />
  )
}

export function DataTableHeaderCell({
  align = "left",
  className,
  columnWidth,
  ref,
  scope = "col",
  style,
  truncate = false,
  ...props
}: DataTableHeaderCellProps) {
  return (
    <th
      {...props}
      ref={ref}
      scope={scope}
      className={cx(
        styles.headerCell,
        alignClassNames[align],
        truncate && styles.truncate,
        className,
      )}
      style={getCellStyle(style, columnWidth)}
    />
  )
}

export function DataTableCell({
  align = "left",
  className,
  columnWidth,
  mono = false,
  ref,
  style,
  tone = "default",
  truncate = false,
  ...props
}: DataTableCellProps) {
  return (
    <td
      {...props}
      ref={ref}
      className={cx(
        styles.cell,
        alignClassNames[align],
        toneClassNames[tone],
        mono && styles.mono,
        truncate && styles.truncate,
        className,
      )}
      style={getCellStyle(style, columnWidth)}
    />
  )
}

export function DataTableGroupRow({
  children,
  className,
  colSpan,
  disabled = false,
  expanded = false,
  onToggle,
  ref,
  ...props
}: DataTableGroupRowProps) {
  return (
    <tr {...props} ref={ref} className={cx(styles.groupRow, className)}>
      <td className={styles.groupCell} colSpan={colSpan}>
        {onToggle ? (
          <button
            type="button"
            className={styles.groupToggle}
            aria-expanded={expanded}
            disabled={disabled}
            onClick={onToggle}
          >
            <ChevronDown
              className={cx(styles.groupIcon, expanded && styles.groupIconExpanded)}
              size={14}
              aria-hidden="true"
            />
            <span className={styles.groupLabel}>{children}</span>
          </button>
        ) : (
          <span className={styles.groupLabel}>{children}</span>
        )}
      </td>
    </tr>
  )
}

export function DataTableEmpty({children, className, colSpan, ref, ...props}: DataTableEmptyProps) {
  return (
    <tr {...props} ref={ref} className={cx(styles.emptyRow, className)}>
      <td className={styles.emptyCell} colSpan={colSpan}>
        <div className={styles.emptyContent}>{children}</div>
      </td>
    </tr>
  )
}

export function DataTableSkeletonRows({
  alignments,
  columns,
  rowKeyPrefix = "data-table-skeleton-row",
  rows = 3,
  widths,
}: DataTableSkeletonRowsProps) {
  const rowCount = Math.max(1, rows)
  const columnCount = Math.max(1, columns)

  return (
    <>
      {Array.from({length: rowCount}).map((_, rowIndex) => (
        <DataTableRow key={`${rowKeyPrefix}-${rowIndex}`} aria-hidden="true">
          {Array.from({length: columnCount}).map((__, columnIndex) => {
            const align = alignments?.[columnIndex] ?? "left"

            return (
              <DataTableCell key={columnIndex} align={align}>
                <Skeleton
                  className={cx(
                    styles.skeleton,
                    align === "center" && styles.skeletonCenter,
                    align === "right" && styles.skeletonRight,
                  )}
                  width={widths?.[columnIndex] ?? getDefaultSkeletonWidth(columnIndex)}
                />
              </DataTableCell>
            )
          })}
        </DataTableRow>
      ))}
    </>
  )
}

function getDataTableStyle(style: CSSProperties | undefined, minWidth: CSSProperties["minWidth"]) {
  return {
    ...style,
    "--acton-data-table-min-width": toCssSize(minWidth),
  } as CSSProperties
}

function getCellStyle(style: CSSProperties | undefined, width: CSSProperties["width"]) {
  return {
    ...style,
    ...(width === undefined ? {} : {width: toCssSize(width)}),
  } as CSSProperties
}

function toCssSize(value: CSSProperties["width"]) {
  return typeof value === "number" ? `${value}px` : value
}

function getDefaultSkeletonWidth(index: number) {
  if (index % 4 === 0) return "72%"
  if (index % 4 === 1) return "54%"
  if (index % 4 === 2) return "64%"
  return "46%"
}
