import {
  Button,
  CountValue,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  formatDateTime,
  formatNumberValue,
  NumberValue,
  Percentage,
  Skeleton,
} from "@acton/ui"
import {CircleAlert, RefreshCw} from "lucide-react"
import {useCallback, useEffect, useMemo, useState} from "react"
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts"

import type {
  VerificationStatisticsHistoryItem,
  VerificationStatisticsHistoryResponse,
  VerificationStatisticsResponse,
  VerifierApi,
} from "../lib/api"
import styles from "./StatisticsPage.module.css"

interface StatisticsPageProps {
  readonly api: VerifierApi
}

interface LanguageStatistics {
  readonly fill: string
  readonly label: string
  readonly language: string
  readonly total: number
  readonly versions: readonly VersionRow[]
}

interface VersionRow {
  readonly total: number
  readonly version: string
}

interface HistoryLanguage {
  readonly color: string
  readonly key: string
  readonly label: string
}

interface HistorySeries {
  readonly cumulative: readonly HistoryPoint[]
  readonly languages: readonly HistoryLanguage[]
  readonly monthly: readonly HistoryPoint[]
}

interface HistoryPoint {
  [key: string]: number
  timestamp: number
}

const LANGUAGE_COLORS: Readonly<Record<string, string>> = {
  func: "var(--acton-color-warning)",
  tact: "var(--acton-color-success)",
  tolk: "var(--acton-color-accent)",
}
const LANGUAGE_LABELS: Readonly<Record<string, string>> = {
  func: "FunC",
  tact: "Tact",
  tolk: "Tolk",
}
const FALLBACK_LANGUAGE_COLOR = "var(--acton-color-text-subtle)"
const LEGEND_SKELETON_KEYS = ["first", "second", "third"] as const
const CHART_TOOLTIP_STYLE = {
  background: "var(--acton-color-surface-raised)",
  border: "1px solid var(--acton-color-border)",
  borderRadius: "10px",
  color: "var(--acton-color-text)",
}
const MONTH_TICK_FORMAT = {display: "month-short", locale: "en", timeZone: "UTC"} as const
const MONTH_TOOLTIP_FORMAT = {display: "month-long", locale: "en", timeZone: "UTC"} as const

function normalizedCount(value: number): number {
  return Number.isFinite(value) ? Math.max(0, Math.trunc(value)) : 0
}

function languageLabel(language: string): string {
  const value = language.trim()
  return (
    LANGUAGE_LABELS[value.toLowerCase()] ??
    (value ? value[0].toUpperCase() + value.slice(1) : "Unknown")
  )
}

function languageColor(language: string): string {
  return LANGUAGE_COLORS[language.trim().toLowerCase()] ?? FALLBACK_LANGUAGE_COLOR
}

function monthStart(timestamp: number): number {
  const date = new Date(timestamp * 1000)
  return Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), 1)
}

function nextMonth(timestamp: number): number {
  const date = new Date(timestamp)
  return Date.UTC(date.getUTCFullYear(), date.getUTCMonth() + 1, 1)
}

function buildHistorySeries(items: readonly VerificationStatisticsHistoryItem[]): HistorySeries {
  const monthlyCounts = new Map<number, Map<string, number>>()
  const languageTotals = new Map<string, number>()

  for (const item of items) {
    if (!Number.isFinite(item.timestamp) || item.timestamp < 0) continue

    const language = item.compiler.trim().toLowerCase() || "unknown"
    const month = monthStart(item.timestamp)
    const monthCounts = monthlyCounts.get(month) ?? new Map<string, number>()
    monthCounts.set(language, (monthCounts.get(language) ?? 0) + 1)
    monthlyCounts.set(month, monthCounts)
    languageTotals.set(language, (languageTotals.get(language) ?? 0) + 1)
  }

  const months = [...monthlyCounts.keys()].sort((left, right) => left - right)
  const languages = [...languageTotals.entries()]
    .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]))
    .map(([key]) => ({
      color: languageColor(key),
      key,
      label: languageLabel(key),
    }))
  const [firstMonth] = months
  const lastMonth = months.at(-1)
  if (firstMonth === undefined || lastMonth === undefined) {
    return {cumulative: [], languages, monthly: []}
  }

  const cumulative: HistoryPoint[] = []
  const monthly: HistoryPoint[] = []
  const runningTotals = new Map<string, number>()

  for (let timestamp = firstMonth; timestamp <= lastMonth; timestamp = nextMonth(timestamp)) {
    const monthCounts = monthlyCounts.get(timestamp)
    const monthlyPoint: HistoryPoint = {timestamp}
    const cumulativePoint: HistoryPoint = {timestamp}

    for (const language of languages) {
      const count = monthCounts?.get(language.key) ?? 0
      const cumulativeCount = (runningTotals.get(language.key) ?? 0) + count
      runningTotals.set(language.key, cumulativeCount)
      monthlyPoint[language.key] = count
      cumulativePoint[language.key] = cumulativeCount
    }

    monthly.push(monthlyPoint)
    cumulative.push(cumulativePoint)
  }

  return {cumulative, languages, monthly}
}

export function StatisticsPage({api}: StatisticsPageProps) {
  const [statistics, setStatistics] = useState<VerificationStatisticsResponse>()
  const [history, setHistory] = useState<VerificationStatisticsHistoryResponse>()
  const [error, setError] = useState<string>()
  const [isLoading, setIsLoading] = useState(true)

  const loadStatistics = useCallback(() => {
    setIsLoading(true)
    setError(undefined)

    Promise.all([api.fetchStatistics(), api.fetchStatisticsHistory()])
      .then(([statisticsResponse, historyResponse]) => {
        setStatistics(statisticsResponse)
        setHistory(historyResponse)
      })
      .catch(error => {
        setStatistics(undefined)
        setHistory(undefined)
        setError(error instanceof Error ? error.message : String(error))
      })
      .finally(() => {
        setIsLoading(false)
      })
  }, [api])

  useEffect(() => {
    loadStatistics()
  }, [loadStatistics])

  const total = normalizedCount(statistics?.total ?? 0)
  const languages = useMemo<readonly LanguageStatistics[]>(
    () =>
      [...(statistics?.languages ?? [])]
        .map(language => {
          const languageTotal = normalizedCount(language.total)
          return {
            fill: languageColor(language.language),
            label: languageLabel(language.language),
            language: language.language,
            total: languageTotal,
            versions: [...language.versions]
              .sort((left, right) => normalizedCount(right.total) - normalizedCount(left.total))
              .map(version => ({
                total: normalizedCount(version.total),
                version: version.version || "Unknown",
              })),
          }
        })
        .sort((left, right) => right.total - left.total),
    [statistics],
  )
  const versionCount = languages.reduce((count, language) => count + language.versions.length, 0)
  const historySeries = useMemo(() => buildHistorySeries(history?.items ?? []), [history])
  const compilerLanguages = [...languages].sort(
    (left, right) =>
      Number(right.language.toLowerCase() === "tolk") -
      Number(left.language.toLowerCase() === "tolk"),
  )

  return (
    <section className={styles.container}>
      <header className={styles.hero}>
        <h1 className={styles.title}>Verification statistics</h1>
        <p className={styles.subtitle}>
          Language and compiler coverage across indexed verified contracts.
        </p>
      </header>

      {error ? (
        <section className={styles.errorPanel} role="alert">
          <CircleAlert size={22} aria-hidden="true" />
          <div className={styles.errorCopy}>
            <strong>Statistics are unavailable</strong>
            <span>{error}</span>
          </div>
          <Button
            size="sm"
            variant="outline"
            leadingIcon={<RefreshCw size={15} />}
            onClick={loadStatistics}
          >
            Retry
          </Button>
        </section>
      ) : (
        <>
          <section
            className={styles.summary}
            aria-busy={isLoading}
            aria-label={isLoading ? "Loading verification statistics" : undefined}
          >
            <div className={styles.totalPanel}>
              <span className={styles.summaryLabel}>Verified contracts</span>
              {isLoading ? (
                <Skeleton width="11rem" height="4.5rem" radius="md" />
              ) : (
                <strong className={styles.totalValue}>
                  <NumberValue value={total} />
                </strong>
              )}
              <span className={styles.totalCaption}>
                {isLoading ? (
                  "Reading the verifier registry"
                ) : (
                  <>
                    <span>Across </span>
                    <CountValue singular="compiler language" value={languages.length} />
                  </>
                )}
              </span>
            </div>

            <div className={styles.distributionPanel}>
              <div className={styles.sectionHeading}>
                <div>
                  <span className={styles.summaryLabel}>Language distribution</span>
                  <h2>Verified source by language</h2>
                </div>
              </div>

              <div className={styles.distributionContent}>
                <div className={styles.chart}>
                  {isLoading ? (
                    <Skeleton shape="circle" width="10rem" height="10rem" />
                  ) : languages.length > 0 ? (
                    <>
                      <ResponsiveContainer width="100%" height="100%">
                        <PieChart accessibilityLayer>
                          <Pie
                            data={languages}
                            dataKey="total"
                            nameKey="label"
                            innerRadius="66%"
                            outerRadius="94%"
                            paddingAngle={2}
                            stroke="none"
                            isAnimationActive={false}
                          />
                          <Tooltip
                            contentStyle={CHART_TOOLTIP_STYLE}
                            itemStyle={{color: "var(--acton-color-text)"}}
                            formatter={value => [
                              formatNumberValue(Array.isArray(value) ? value[0] : value),
                              "Contracts",
                            ]}
                          />
                        </PieChart>
                      </ResponsiveContainer>
                      <div className={styles.chartCenter} aria-hidden="true">
                        <strong>
                          <NumberValue value={total} />
                        </strong>
                        <span>Total</span>
                      </div>
                    </>
                  ) : (
                    <span className={styles.emptyChart}>No data</span>
                  )}
                </div>

                <div className={styles.languageLegend}>
                  {isLoading
                    ? LEGEND_SKELETON_KEYS.map(key => (
                        <div className={styles.legendSkeleton} key={key}>
                          <Skeleton width="8rem" />
                          <Skeleton width="4rem" />
                        </div>
                      ))
                    : languages.map(language => (
                        <div className={styles.legendRow} key={language.language}>
                          <div className={styles.legendIdentity}>
                            <span
                              className={styles.legendDot}
                              style={{backgroundColor: language.fill}}
                              aria-hidden="true"
                            />
                            <span>{language.label}</span>
                          </div>
                          <div className={styles.legendValue}>
                            <strong>
                              <NumberValue value={language.total} />
                            </strong>
                            <span>
                              <Percentage
                                maximumFractionDigits={1}
                                total={total}
                                value={language.total}
                              />
                            </span>
                          </div>
                        </div>
                      ))}
                </div>
              </div>
            </div>
          </section>

          <section className={styles.historySection}>
            <header className={styles.historyHeading}>
              <div>
                <h2>Verification history</h2>
                <p>Registry growth and monthly verification activity.</p>
              </div>
              <span>
                {isLoading ? (
                  "Loading"
                ) : (
                  <CountValue singular="record" value={history?.items.length ?? 0} />
                )}
              </span>
            </header>

            <div className={styles.historyGrid} aria-busy={isLoading}>
              <article className={styles.historyPanel}>
                <header className={styles.historyPanelHeading}>
                  <span className={styles.summaryLabel}>Registry growth</span>
                  <h3>Cumulative verified contracts</h3>
                </header>
                <div className={styles.historyChart}>
                  {isLoading ? (
                    <Skeleton width="100%" height="100%" radius="md" />
                  ) : historySeries.cumulative.length > 0 ? (
                    <ResponsiveContainer width="100%" height="100%">
                      <AreaChart
                        accessibilityLayer
                        data={historySeries.cumulative}
                        margin={{top: 10, right: 8, bottom: 0, left: 0}}
                      >
                        <CartesianGrid
                          vertical={false}
                          stroke="var(--acton-color-border)"
                          strokeDasharray="3 3"
                        />
                        <XAxis
                          axisLine={false}
                          dataKey="timestamp"
                          minTickGap={32}
                          tick={{fill: "var(--acton-color-text-muted)", fontSize: 12}}
                          tickFormatter={timestamp => formatDateTime(timestamp, MONTH_TICK_FORMAT)}
                          tickLine={false}
                        />
                        <YAxis
                          allowDecimals={false}
                          axisLine={false}
                          tick={{fill: "var(--acton-color-text-muted)", fontSize: 12}}
                          tickFormatter={value => formatNumberValue(value)}
                          tickLine={false}
                          width={46}
                        />
                        <Tooltip
                          contentStyle={CHART_TOOLTIP_STYLE}
                          itemStyle={{color: "var(--acton-color-text)"}}
                          labelFormatter={value =>
                            formatDateTime(Number(value), MONTH_TOOLTIP_FORMAT)
                          }
                          formatter={(value, name) => [
                            formatNumberValue(Array.isArray(value) ? value[0] : value),
                            name,
                          ]}
                        />
                        {historySeries.languages.map(language => (
                          <Area
                            key={language.key}
                            dataKey={language.key}
                            fill={language.color}
                            fillOpacity={0.16}
                            isAnimationActive={false}
                            name={language.label}
                            stackId="contracts"
                            stroke={language.color}
                            strokeWidth={2}
                            type="monotone"
                          />
                        ))}
                      </AreaChart>
                    </ResponsiveContainer>
                  ) : (
                    <span className={styles.historyEmpty}>No verification history indexed yet</span>
                  )}
                </div>
              </article>

              <article className={styles.historyPanel}>
                <header className={styles.historyPanelHeading}>
                  <span className={styles.summaryLabel}>Monthly activity</span>
                  <h3>Verifications by month</h3>
                </header>
                <div className={styles.historyChart}>
                  {isLoading ? (
                    <Skeleton width="100%" height="100%" radius="md" />
                  ) : historySeries.monthly.length > 0 ? (
                    <ResponsiveContainer width="100%" height="100%">
                      <BarChart
                        accessibilityLayer
                        data={historySeries.monthly}
                        margin={{top: 10, right: 8, bottom: 0, left: 0}}
                      >
                        <CartesianGrid
                          vertical={false}
                          stroke="var(--acton-color-border)"
                          strokeDasharray="3 3"
                        />
                        <XAxis
                          axisLine={false}
                          dataKey="timestamp"
                          minTickGap={32}
                          tick={{fill: "var(--acton-color-text-muted)", fontSize: 12}}
                          tickFormatter={timestamp => formatDateTime(timestamp, MONTH_TICK_FORMAT)}
                          tickLine={false}
                        />
                        <YAxis
                          allowDecimals={false}
                          axisLine={false}
                          tick={{fill: "var(--acton-color-text-muted)", fontSize: 12}}
                          tickFormatter={value => formatNumberValue(value)}
                          tickLine={false}
                          width={46}
                        />
                        <Tooltip
                          contentStyle={CHART_TOOLTIP_STYLE}
                          cursor={{fill: "var(--acton-color-surface-hover)"}}
                          itemStyle={{color: "var(--acton-color-text)"}}
                          labelFormatter={value =>
                            formatDateTime(Number(value), MONTH_TOOLTIP_FORMAT)
                          }
                          formatter={(value, name) => [
                            formatNumberValue(Array.isArray(value) ? value[0] : value),
                            name,
                          ]}
                        />
                        {historySeries.languages.map(language => (
                          <Bar
                            key={language.key}
                            dataKey={language.key}
                            fill={language.color}
                            isAnimationActive={false}
                            name={language.label}
                            stackId="contracts"
                          />
                        ))}
                      </BarChart>
                    </ResponsiveContainer>
                  ) : (
                    <span className={styles.historyEmpty}>No verification history indexed yet</span>
                  )}
                </div>
              </article>
            </div>
          </section>

          <section className={styles.compilerSection}>
            <header className={styles.compilerHeading}>
              <h2>Versions by language</h2>
              <span>
                {isLoading ? "Loading" : <CountValue singular="version" value={versionCount} />}
              </span>
            </header>

            {isLoading ? (
              <DataTable title="Compiler versions" meta="Loading" minWidth="36rem">
                <DataTableTable aria-label="Loading compiler statistics">
                  <DataTableHead>
                    <DataTableRow>
                      <DataTableHeaderCell columnWidth="40%">Version</DataTableHeaderCell>
                      <DataTableHeaderCell align="right">Contracts</DataTableHeaderCell>
                      <DataTableHeaderCell align="right">Language share</DataTableHeaderCell>
                      <DataTableHeaderCell align="right">Registry share</DataTableHeaderCell>
                    </DataTableRow>
                  </DataTableHead>
                  <DataTableBody>
                    <DataTableSkeletonRows
                      columns={4}
                      rows={6}
                      widths={["8rem", "4rem", "4rem", "4rem"]}
                      alignments={["left", "right", "right", "right"]}
                    />
                  </DataTableBody>
                </DataTableTable>
              </DataTable>
            ) : languages.length === 0 ? (
              <DataTable title="Compiler versions" minWidth="36rem">
                <DataTableTable aria-label="Verified contracts by compiler version">
                  <DataTableBody>
                    <DataTableEmpty colSpan={4}>No compiler statistics indexed yet</DataTableEmpty>
                  </DataTableBody>
                </DataTableTable>
              </DataTable>
            ) : (
              <div className={styles.versionTables}>
                {compilerLanguages.map(language => (
                  <DataTable
                    key={language.language}
                    title={
                      <span className={styles.languageTitle}>
                        <span
                          className={styles.legendDot}
                          style={{backgroundColor: language.fill}}
                          aria-hidden="true"
                        />
                        {language.label}
                      </span>
                    }
                    meta={
                      <>
                        <CountValue singular="contract" value={language.total} /> ·{" "}
                        <CountValue singular="version" value={language.versions.length} />
                      </>
                    }
                    minWidth="36rem"
                  >
                    <DataTableTable
                      aria-label={`${language.label} verified contracts by compiler version`}
                    >
                      <DataTableHead>
                        <DataTableRow>
                          <DataTableHeaderCell columnWidth="40%">Version</DataTableHeaderCell>
                          <DataTableHeaderCell align="right">Contracts</DataTableHeaderCell>
                          <DataTableHeaderCell align="right">Language share</DataTableHeaderCell>
                          <DataTableHeaderCell align="right">Registry share</DataTableHeaderCell>
                        </DataTableRow>
                      </DataTableHead>
                      <DataTableBody>
                        {language.versions.length === 0 ? (
                          <DataTableEmpty colSpan={4}>No versions indexed</DataTableEmpty>
                        ) : (
                          language.versions.map(row => (
                            <DataTableRow key={`${language.language}:${row.version}`} hover>
                              <DataTableCell mono>{row.version}</DataTableCell>
                              <DataTableCell align="right" tone="strong">
                                <NumberValue value={row.total} />
                              </DataTableCell>
                              <DataTableCell align="right" tone="muted">
                                <Percentage
                                  maximumFractionDigits={1}
                                  total={language.total}
                                  value={row.total}
                                />
                              </DataTableCell>
                              <DataTableCell align="right" tone="muted">
                                <Percentage
                                  maximumFractionDigits={1}
                                  total={total}
                                  value={row.total}
                                />
                              </DataTableCell>
                            </DataTableRow>
                          ))
                        )}
                      </DataTableBody>
                    </DataTableTable>
                  </DataTable>
                ))}
              </div>
            )}
          </section>
        </>
      )}
    </section>
  )
}
