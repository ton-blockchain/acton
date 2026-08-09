export interface ShortenMiddleOptions {
  readonly end?: number
  readonly maxLength?: number
  readonly separator?: string
  readonly start?: number
}

/** Shortens a string in the middle and preserves both ends */
export function shortenMiddle(value: string, options: ShortenMiddleOptions = {}): string {
  const separator = options.separator ?? "…"
  let start = normalizeEdgeLength(options.start, 6)
  let end = normalizeEdgeLength(options.end, start)

  if (options.maxLength !== undefined) {
    const visibleLength = Math.max(0, Math.trunc(options.maxLength) - separator.length)
    start = Math.ceil(visibleLength / 2)
    end = Math.floor(visibleLength / 2)
  }

  if (value.length <= start + end + separator.length) return value
  return `${value.slice(0, start)}${separator}${end > 0 ? value.slice(-end) : ""}`
}

/** Truncates a string at the end and keeps the result within the requested length */
export function truncateEnd(value: string, maxLength: number, separator = "…"): string {
  const normalizedMaxLength = Number.isFinite(maxLength)
    ? Math.max(0, Math.trunc(maxLength))
    : value.length
  if (value.length <= normalizedMaxLength) return value
  if (normalizedMaxLength === 0) return ""
  if (separator.length >= normalizedMaxLength) return separator.slice(0, normalizedMaxLength)
  return `${value.slice(0, normalizedMaxLength - separator.length)}${separator}`
}

export interface HumanizeIdentifierOptions {
  readonly capitalize?: boolean
  readonly fallback?: string
}

/** Converts a machine identifier to a readable label */
export function humanizeIdentifier(
  value: string | null | undefined,
  options: HumanizeIdentifierOptions = {},
): string {
  const normalized = value?.trim().replaceAll("_", " ").replaceAll("-", " ") ?? ""
  if (!normalized) return options.fallback ?? "—"
  if (!options.capitalize) return normalized
  return normalized.charAt(0).toUpperCase() + normalized.slice(1)
}

export interface CompilerLabelValue {
  readonly language?: string | null
  readonly version?: string | null
}

/** Formats a compiler language and version as one label */
export function formatCompilerLabel(
  compiler: CompilerLabelValue | null | undefined,
  fallback = "Unknown",
): string {
  const language = compiler?.language?.trim() ?? ""
  const version = compiler?.version?.trim() ?? ""
  if (!language) return fallback
  return version ? `${language} ${version}` : language
}

function normalizeEdgeLength(value: number | undefined, fallback: number): number {
  return value === undefined || !Number.isFinite(value) ? fallback : Math.max(0, Math.trunc(value))
}
