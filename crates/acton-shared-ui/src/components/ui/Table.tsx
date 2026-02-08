import * as React from "react"

import styles from "./Table.module.css"

export type TableProps = Readonly<React.HTMLAttributes<HTMLTableElement>>
export const Table: React.FC<TableProps> = ({className, ...properties}) => (
  <div className={styles.tableWrapper}>
    <table className={`${styles.table} ${className ?? ""}`} {...properties} />
  </div>
)

export type TableHeaderProps = Readonly<React.HTMLAttributes<HTMLTableSectionElement>>
export const TableHeader: React.FC<TableHeaderProps> = ({className, ...properties}) => (
  <thead className={`${styles.header} ${className ?? ""}`} {...properties} />
)

export type TableBodyProps = Readonly<React.HTMLAttributes<HTMLTableSectionElement>>
export const TableBody: React.FC<TableBodyProps> = ({className, ...properties}) => (
  <tbody className={`${styles.body} ${className ?? ""}`} {...properties} />
)

export type TableFooterProps = Readonly<React.HTMLAttributes<HTMLTableSectionElement>>
export const TableFooter: React.FC<TableFooterProps> = ({className, ...properties}) => (
  <tfoot className={`${styles.footer} ${className ?? ""}`} {...properties} />
)

export type TableRowProps = Readonly<React.HTMLAttributes<HTMLTableRowElement>>
export const TableRow: React.FC<TableRowProps> = ({className, ...properties}) => (
  <tr className={`${styles.row} ${className ?? ""}`} {...properties} />
)

export type TableHeadProps = Readonly<React.ThHTMLAttributes<HTMLTableCellElement>>
export const TableHead: React.FC<TableHeadProps> = ({className, ...properties}) => (
  <th className={`${styles.head} ${className ?? ""}`} {...properties} />
)

export type TableCellProps = Readonly<React.TdHTMLAttributes<HTMLTableCellElement>>
export const TableCell: React.FC<TableCellProps> = ({className, ...properties}) => (
  <td className={`${styles.cell} ${className ?? ""}`} {...properties} />
)

export type TableCaptionProps = Readonly<React.HTMLAttributes<HTMLTableCaptionElement>>
export const TableCaption: React.FC<TableCaptionProps> = ({className, ...properties}) => (
  <caption className={`${styles.caption} ${className ?? ""}`} {...properties} />
)
