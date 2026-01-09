import type React from "react"
import { useMemo, useState } from "react"
import {
  FiCheck,
  FiChevronDown,
  FiChevronLeft,
  FiChevronRight,
  FiCircle,
  FiMinus,
  FiMoon,
  FiSearch,
  FiSun,
  FiX,
} from "react-icons/fi"
import { type TestReport, TestStatus } from "../../types"
import { AppIcon } from "../common/AppIcon"
import { Summary } from "../Summary/Summary"
import styles from "./Sidebar.module.css"

interface SidebarProps {
  readonly reports: TestReport[]
  readonly selectedTest: TestReport | null
  readonly onSelectTest: (test: TestReport) => void
  readonly width?: number
  readonly onCollapse?: () => void
  readonly theme?: string
  readonly onToggleTheme?: () => void
}

export const Sidebar: React.FC<SidebarProps> = ({
  reports,
  selectedTest,
  onSelectTest,
  width,
  onCollapse,
  theme,
  onToggleTheme,
}) => {
  const [searchQuery, setSearchQuery] = useState("")
  const [collapsedSuites, setCollapsedSuites] = useState<Set<string>>(new Set())
  const [statusFilter, setStatusFilter] = useState<Set<TestStatus>>(
    new Set([TestStatus.Passed, TestStatus.Failed, TestStatus.Todo, TestStatus.Skipped]),
  )

  const toggleSuite = (suiteName: string) => {
    setCollapsedSuites((prev) => {
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
    setStatusFilter((prev) => {
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
      case TestStatus.Passed:
        return <FiCheck className={styles.passed} />
      case TestStatus.Failed:
        return <FiX className={styles.failed} />
      case TestStatus.Skipped:
        return <FiCircle className={styles.skipped} />
      case TestStatus.Todo:
        return <FiMinus className={styles.todo} />
      default:
        return null
    }
  }

  const getSuiteStatus = (suiteReports: TestReport[]) => {
    const hasFailed = suiteReports.some((r) => r.status === TestStatus.Failed)
    const allPassed = suiteReports.every((r) => r.status === TestStatus.Passed)
    return { hasFailed, allPassed }
  }

  return (
    <div className={styles.sidebar} style={width ? { width: `${width}px` } : undefined}>
      <div className={styles.header}>
        <div className={styles.headerTop}>
          <div className={styles.title}>
            <AppIcon theme={theme ?? "light"} />
            Acton Tests
          </div>
          <div className={styles.headerButtons}>
            {onToggleTheme && (
              <button
                type="button"
                onClick={onToggleTheme}
                className={styles.themeButton}
                title={`Switch to ${theme === "light" ? "dark" : "light"} theme`}
              >
                {theme === "light" ? <FiMoon /> : <FiSun />}
              </button>
            )}
            {onCollapse && (
              <button
                type="button"
                onClick={onCollapse}
                className={styles.collapseButton}
                title="Collapse sidebar"
              >
                <FiChevronLeft />
              </button>
            )}
          </div>
        </div>
        <Summary reports={reports} />

        <div className={styles.searchContainer}>
          <FiSearch className={styles.searchIcon} />
          <input
            type="text"
            placeholder="Search tests..."
            className={styles.searchInput}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
        </div>

        <div className={styles.filters}>
          {(Object.values(TestStatus) as TestStatus[]).map((status) => (
            <button
              key={status}
              type="button"
              className={`${styles.filterButton} ${statusFilter.has(status) ? styles.activeFilter : ""} ${styles[status.toLowerCase()]}`}
              onClick={() => toggleStatusFilter(status)}
              title={status}
            >
              {getStatusIcon(status)}
            </button>
          ))}
        </div>
      </div>

      <div className={styles.content}>
        {Object.entries(filteredSuites).map(([suiteName, suiteReports]) => {
          const isCollapsed = collapsedSuites.has(suiteName)
          const { hasFailed, allPassed } = getSuiteStatus(suiteReports)

          return (
            <div key={suiteName} className={styles.suite}>
              <button
                type="button"
                className={`${styles.suiteHeader} ${hasFailed ? styles.suiteFailed : ""}`}
                onClick={() => toggleSuite(suiteName)}
              >
                <span className={styles.suiteToggle}>
                  {isCollapsed ? <FiChevronRight /> : <FiChevronDown />}
                </span>
                <span className={styles.suiteIcon}>
                  {hasFailed ? (
                    <FiX className={styles.failed} />
                  ) : allPassed ? (
                    <FiCheck className={styles.passed} />
                  ) : null}
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
                    const isFailed = report.status === TestStatus.Failed
                    return (
                      <button
                        key={`${report.name}-${idx}`}
                        type="button"
                        className={`${styles.testItem} ${isSelected ? styles.selected : ""} ${isFailed ? styles.testFailed : ""}`}
                        onClick={() => onSelectTest(report)}
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
    </div>
  )
}
