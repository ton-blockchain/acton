import type {ComponentPropsWithRef, ReactNode} from "react"

import type {ActonInlineActionsVisibility} from "../InlineActions"
import {TechnicalValue} from "../TechnicalValue"

export interface SourceLocationData {
  readonly column?: number | null
  readonly file: string
  readonly line?: number | null
}

export interface SourcePathFormatOptions {
  readonly maxSegments?: number
  readonly projectRoot?: string
}

export interface SourceLocationValueProps
  extends Omit<ComponentPropsWithRef<"span">, "children">,
    SourcePathFormatOptions {
  readonly copyable?: boolean
  readonly copyVisibility?: ActonInlineActionsVisibility
  readonly fallback?: ReactNode
  readonly value: SourceLocationData | null | undefined
}

/** Formats a source path relative to a project root */
export function formatSourcePath(file: string, options: SourcePathFormatOptions = {}): string {
  const normalizedFile = normalizePath(file)
  const normalizedRoot = normalizePath(options.projectRoot ?? "").replace(/\/$/, "")
  let displayPath = normalizedFile

  if (normalizedRoot && normalizedFile.startsWith(`${normalizedRoot}/`)) {
    displayPath = normalizedFile.slice(normalizedRoot.length + 1)
  } else if (normalizedRoot && normalizedFile === normalizedRoot) {
    displayPath = normalizedFile.split("/").at(-1) ?? normalizedFile
  }

  const maxSegments = Math.max(0, Math.trunc(options.maxSegments ?? 4))
  const segments = displayPath.split("/").filter(Boolean)
  if (maxSegments > 0 && segments.length > maxSegments) {
    return `…/${segments.slice(-maxSegments).join("/")}`
  }
  return displayPath || file
}

/** Formats a source path with its line and column */
export function formatSourceLocation(
  value: SourceLocationData,
  options: SourcePathFormatOptions = {},
): string {
  const path = formatSourcePath(value.file, options)
  if (value.line === null || value.line === undefined) return path
  if (value.column === null || value.column === undefined) return `${path}:${value.line}`
  return `${path}:${value.line}:${value.column}`
}

export function SourceLocationValue({
  value,
  copyable = false,
  copyVisibility = "hover",
  fallback = "—",
  maxSegments,
  projectRoot,
  ...props
}: SourceLocationValueProps) {
  if (!value?.file.trim()) return fallback

  const fullValue = formatSourceLocation(value, {maxSegments: Number.MAX_SAFE_INTEGER})
  const displayValue = formatSourceLocation(value, {maxSegments, projectRoot})
  return (
    <TechnicalValue
      {...props}
      copyable={copyable}
      copyLabel="source location"
      copyVisibility={copyVisibility}
      displayValue={displayValue}
      shorten={false}
      value={fullValue}
    />
  )
}

function normalizePath(value: string): string {
  return value.trim().replaceAll("\\", "/")
}
