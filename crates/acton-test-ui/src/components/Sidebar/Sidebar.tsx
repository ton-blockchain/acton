import type React from "react"
import {PanelLeft} from "lucide-react"
import {useMemo, useState} from "react"
import {
  FiBookOpen,
  FiCheck,
  FiChevronDown,
  FiChevronRight,
  FiCircle,
  FiMinus,
  FiSearch,
  FiX,
} from "react-icons/fi"
import {Input, ThemeSwitch, type ThemeMode} from "@acton/ui"
import {type TestReport, TestStatus} from "@acton/shared-ui"

import styles from "./Sidebar.module.css"

const DOCS_URL = "https://ton-blockchain.github.io/acton/docs/testing/test-ui/overview"

interface SidebarProps {
  readonly reports: TestReport[]
  readonly selectedTest: TestReport | undefined
  readonly onSelectTest: (test: TestReport) => void
  readonly width: number
  readonly onCollapse: () => void
  readonly isCollapsed: boolean
  readonly className: string
  readonly theme: ThemeMode
  readonly onToggleTheme: () => void
}

export const Sidebar: React.FC<SidebarProps> = ({
  reports,
  selectedTest,
  onSelectTest,
  width,
  onCollapse,
  isCollapsed,
  className,
  theme,
  onToggleTheme,
}) => {
  const [searchQuery, setSearchQuery] = useState("")
  const [collapsedSuites, setCollapsedSuites] = useState<Set<string>>(new Set())
  const [statusFilter, setStatusFilter] = useState<Set<TestStatus>>(
    new Set([TestStatus.Passed, TestStatus.Failed, TestStatus.Todo, TestStatus.Skipped]),
  )
  const passedCount = reports.filter(report => report.status === TestStatus.Passed).length
  const failedCount = reports.filter(report => report.status === TestStatus.Failed).length
  const appIconColor = theme === "dark" ? "#ecebeb" : "#4DB8FF"

  const toggleSuite = (suiteName: string) => {
    setCollapsedSuites(prev => {
      const next = new Set(prev)
      if (next.has(suiteName)) {
        next.delete(suiteName)
      } else {
        next.add(suiteName)
      }
      return next
    })
  }

  const toggleStatusFilter = (status: TestStatus) => {
    setStatusFilter(prev => {
      const next = new Set(prev)
      if (next.has(status)) {
        next.delete(status)
      } else {
        next.add(status)
      }
      return next
    })
  }

  const filteredSuites = useMemo(() => {
    const suites: Record<string, TestReport[]> = {}

    for (const report of reports) {
      const matchesSearch = report.name.toLowerCase().includes(searchQuery.toLowerCase())
      const matchesStatus = statusFilter.has(report.status)

      if (matchesSearch && matchesStatus) {
        if (!suites[report.suite_name]) {
          suites[report.suite_name] = []
        }
        suites[report.suite_name].push(report)
      }
    }

    return suites
  }, [reports, searchQuery, statusFilter])

  const getStatusIcon = (status: TestStatus) => {
    switch (status) {
      case TestStatus.Passed: {
        return <FiCheck className={styles.passed} />
      }
      case TestStatus.Failed: {
        return <FiX className={styles.failed} />
      }
      case TestStatus.Skipped: {
        return <FiCircle className={styles.skipped} />
      }
      case TestStatus.Todo: {
        return <FiMinus className={styles.todo} />
      }
      default: {
        return
      }
    }
  }

  const getSuiteStatus = (suiteReports: TestReport[]) => {
    const hasFailed = suiteReports.some(r => r.status === TestStatus.Failed)
    const allPassed = suiteReports.every(r => r.status === TestStatus.Passed)
    return {hasFailed, allPassed}
  }

  return (
    <div className={`${styles.sidebar} ${className}`} style={{width}}>
      <div className={styles.header}>
        <div className={styles.headerTop}>
          <div className={styles.title}>
            <svg
              width="24"
              height="24"
              viewBox="0 0 237 237"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
              role="img"
              aria-label="TON logo"
            >
              <path
                d="M118.2 0C183.49 0 236.41 52.92 236.41 118.21C236.41 183.49 183.49 236.41 118.2 236.41C52.92 236.41 0 183.49 0 118.21C0 52.92 52.92 0 118.2 0ZM74.1 62.2C57.68 62.2 47.27 79.91 55.53 94.23L109.96 188.58C113.62 194.92 122.78 194.92 126.44 188.58L180.88 94.23C189.13 79.93 178.72 62.2 162.31 62.2H74.1ZM162.29 78.84C166.03 78.84 168.23 82.81 166.45 85.91L137.86 137.09L137.85 137.1L126.51 159.05V78.84H162.29ZM109.87 78.85V159.02L98.54 137.09L98.53 137.08L69.93 85.92L69.85 85.77C68.21 82.7 70.41 78.85 74.09 78.85H109.87Z"
                fill={appIconColor}
              />
            </svg>
            <span className={styles.titleBody}>
              <span className={styles.titleRow}>
                <span className={styles.titleName}>Test UI</span>
              </span>
              <span className={styles.titleMeta}>by Acton</span>
            </span>
          </div>
          <div className={styles.headerButtons}>
            <button
              type="button"
              onClick={onCollapse}
              className={styles.collapseButton}
              title={isCollapsed ? "Pin sidebar" : "Collapse sidebar"}
              aria-label={isCollapsed ? "Pin Sidebar" : "Collapse Sidebar"}
            >
              <PanelLeft aria-hidden="true" />
            </button>
          </div>
        </div>
        <div className={styles.summary}>
          <div className={styles.summaryCard} data-testid="summary-total">
            <span className={styles.summaryCount}>{reports.length}</span>
            <span className={styles.summaryLabel}>Total</span>
          </div>
          <div
            className={`${styles.summaryCard} ${styles.summaryPassed}`}
            data-testid="summary-passed"
          >
            <span className={styles.summaryCount}>{passedCount}</span>
            <span className={styles.summaryLabel}>Passed</span>
          </div>
          <div
            className={`${styles.summaryCard} ${styles.summaryFailed}`}
            data-testid="summary-failed"
          >
            <span className={styles.summaryCount}>{failedCount}</span>
            <span className={styles.summaryLabel}>Failed</span>
          </div>
        </div>

        <Input
          type="search"
          size="sm"
          placeholder="Filter tests..."
          value={searchQuery}
          onChange={event => setSearchQuery(event.target.value)}
          aria-label="Filter tests"
          leadingIcon={<FiSearch className={styles.searchIcon} />}
          shortcut="K"
        />

        <div className={styles.filters}>
          {(Object.values(TestStatus) as TestStatus[]).map(status => (
            <button
              key={status}
              type="button"
              className={`${styles.filterButton} ${statusFilter.has(status) ? styles.activeFilter : ""} ${styles[status.toLowerCase()]}`}
              onClick={() => toggleStatusFilter(status)}
              aria-label={`Show ${status} tests`}
              aria-pressed={statusFilter.has(status)}
              title={`Show ${status} tests`}
            >
              {getStatusIcon(status)}
            </button>
          ))}
        </div>
      </div>

      <div className={styles.content}>
        {Object.entries(filteredSuites).map(([suiteName, suiteReports]) => {
          const isCollapsed = collapsedSuites.has(suiteName)
          const {hasFailed, allPassed} = getSuiteStatus(suiteReports)

          return (
            <div key={suiteName} className={styles.suite}>
              <button
                type="button"
                className={styles.suiteHeader}
                onClick={() => toggleSuite(suiteName)}
                aria-expanded={!isCollapsed}
              >
                <span className={styles.suiteToggle}>
                  {isCollapsed ? <FiChevronRight /> : <FiChevronDown />}
                </span>
                <span className={styles.suiteIcon}>
                  {hasFailed ? (
                    <FiX className={styles.failed} />
                  ) : allPassed ? (
                    <FiCheck className={styles.passed} />
                  ) : undefined}
                </span>
                <span className={styles.suiteName}>{suiteName}</span>
                <span className={styles.suiteCount}>{suiteReports.length}</span>
              </button>

              {!isCollapsed && (
                <div className={styles.testList}>
                  {suiteReports.map((report, idx) => {
                    const isSelected =
                      selectedTest?.name === report.name &&
                      selectedTest?.suite_name === report.suite_name
                    return (
                      <button
                        key={`${report.name}-${idx}`}
                        type="button"
                        className={`${styles.testItem} ${isSelected ? styles.selected : ""}`}
                        onClick={() => onSelectTest(report)}
                        aria-current={isSelected ? "true" : undefined}
                      >
                        <span className={styles.testStatusIcon}>
                          {getStatusIcon(report.status)}
                        </span>
                        <span className={styles.testName}>{report.name}</span>
                      </button>
                    )
                  })}
                </div>
              )}
            </div>
          )
        })}
      </div>

      <div className={styles.footer}>
        <a
          className={styles.documentationButton}
          href={DOCS_URL}
          target="_blank"
          rel="noreferrer"
          title="Open documentation"
          aria-label="Open documentation"
        >
          <FiBookOpen />
        </a>
        <ThemeSwitch
          theme={theme}
          onToggleTheme={onToggleTheme}
          title={`Switch to ${theme === "light" ? "dark" : "light"} theme`}
          aria-label={`Switch to ${theme === "light" ? "dark" : "light"} theme`}
        />
      </div>
    </div>
  )
}
