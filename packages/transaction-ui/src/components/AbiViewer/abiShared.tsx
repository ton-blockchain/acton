import type {MouseEvent, ReactNode} from "react"
import {HighlightedCode} from "@acton/ui"
import {Link2} from "lucide-react"

import styles from "./AbiViewer.module.css"

export type AbiSymbolAnchorKind = "declaration" | "error" | "get-method" | "message" | "storage"

export function abiSymbolAnchorId(
  kind: AbiSymbolAnchorKind,
  name: string,
  suffix?: string,
): string {
  const slug = [name, suffix]
    .filter((part): part is string => Boolean(part))
    .join("-")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
  return `abi-${kind}-${slug || "symbol"}`
}

export function AbiSection({
  title,
  count,
  children,
}: {
  readonly title: string
  readonly count: number
  readonly children: ReactNode
}) {
  return (
    <section className={styles.section}>
      <header className={styles.sectionHeader}>
        <h4>{title}</h4>
        <span className={styles.count}>{count}</span>
      </header>
      {children}
    </section>
  )
}

export function AbiSymbolAnchor({
  show,
  id,
  label,
  onClick,
}: {
  readonly show: boolean
  readonly id: string
  readonly label: string
  readonly onClick?: (event: MouseEvent<HTMLAnchorElement>) => void
}) {
  if (!show) return null

  return (
    <a className={styles.symbolAnchor} href={`#${id}`} aria-label={label} onClick={onClick}>
      <Link2 size={12} aria-hidden="true" />
    </a>
  )
}

export function TolkCode({value, wrap = false}: {readonly value: string; readonly wrap?: boolean}) {
  return (
    <div className={styles.tolkCode}>
      <HighlightedCode
        className={styles.highlightedCode}
        value={value}
        language="tolk"
        wrap={wrap}
      />
    </div>
  )
}

export function scrollToAbiSymbol(target: HTMLElement | null): void {
  if (!target) return
  const headerOffset = 116
  const top = target.getBoundingClientRect().top + globalThis.scrollY - headerOffset
  globalThis.scrollTo({top: Math.max(0, top), behavior: "auto"})
}
