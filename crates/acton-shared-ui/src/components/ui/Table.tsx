import * as React from "react";
import styles from "./Table.module.css";

interface TableProps extends React.HTMLAttributes<HTMLTableElement> {}
export const Table: React.FC<TableProps> = ({ className, ...props }) => (
  <div className={styles.tableWrapper}>
    <table className={`${styles.table} ${className ?? ""}`} {...props} />
  </div>
);

interface TableHeaderProps extends React.HTMLAttributes<HTMLTableSectionElement> {}
export const TableHeader: React.FC<TableHeaderProps> = ({ className, ...props }) => (
  <thead className={`${styles.header} ${className ?? ""}`} {...props} />
);

interface TableBodyProps extends React.HTMLAttributes<HTMLTableSectionElement> {}
export const TableBody: React.FC<TableBodyProps> = ({ className, ...props }) => (
  <tbody className={`${styles.body} ${className ?? ""}`} {...props} />
);

interface TableFooterProps extends React.HTMLAttributes<HTMLTableSectionElement> {}
export const TableFooter: React.FC<TableFooterProps> = ({ className, ...props }) => (
  <tfoot className={`${styles.footer} ${className ?? ""}`} {...props} />
);

interface TableRowProps extends React.HTMLAttributes<HTMLTableRowElement> {}
export const TableRow: React.FC<TableRowProps> = ({ className, ...props }) => (
  <tr className={`${styles.row} ${className ?? ""}`} {...props} />
);

interface TableHeadProps extends React.ThHTMLAttributes<HTMLTableCellElement> {}
export const TableHead: React.FC<TableHeadProps> = ({ className, ...props }) => (
  <th className={`${styles.head} ${className ?? ""}`} {...props} />
);

interface TableCellProps extends React.TdHTMLAttributes<HTMLTableCellElement> {}
export const TableCell: React.FC<TableCellProps> = ({ className, ...props }) => (
  <td className={`${styles.cell} ${className ?? ""}`} {...props} />
);

interface TableCaptionProps extends React.HTMLAttributes<HTMLTableCaptionElement> {}
export const TableCaption: React.FC<TableCaptionProps> = ({ className, ...props }) => (
  <caption className={`${styles.caption} ${className ?? ""}`} {...props} />
);
