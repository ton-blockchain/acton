import type React from "react"
import {useEffect, useMemo, useRef, useState} from "react"
import {
  FiBookOpen,
  FiCheck,
  FiChevronDown,
  FiChevronRight,
  FiCircle,
  FiMinus,
  FiX,
} from "react-icons/fi"
import {AppIcon, type TestReport, TestStatus, ThemeSwitch, type ThemeMode} from "@acton/shared-ui"

import {Summary} from "../Summary/Summary"

import {DocsSidebarIcon} from "./DocsSidebarIcon"
import styles from "./Sidebar.module.css"

const DOCS_URL = "https://ton-blockchain.github.io/acton/docs/testing/test-ui/overview"

interface SidebarProps {
  readonly reports: TestReport[]
  readonly selectedTest: TestReport | undefined
  readonly onSelectTest: (test: TestReport) => void
  readonly width?: number
  readonly onCollapse?: () => void
  readonly isCollapsed?: boolean
  readonly className?: string
  readonly theme?: ThemeMode
  readonly onToggleTheme?: () => void
}

export const Sidebar: React.FC<SidebarProps> = ({
  reports,
  selectedTest,
  onSelectTest,
  width,
  onCollapse,
  isCollapsed = false,
  className,
  theme,
  onToggleTheme,
}) => {
  const [searchQuery, setSearchQuery] = useState("")
  const [searchShortcutModifier, setSearchShortcutModifier] = useState("⌘")
  const [collapsedSuites, setCollapsedSuites] = useState<Set<string>>(new Set())
  const [statusFilter, setStatusFilter] = useState<Set<TestStatus>>(
    new Set([TestStatus.Passed, TestStatus.Failed, TestStatus.Todo, TestStatus.Skipped]),
  )
  const searchInputRef = useRef<HTMLInputElement>(null)
  const currentTheme = theme ?? "light"

  useEffect(() => {
    if (globalThis.navigator.userAgent.includes("Windows")) {
      setSearchShortcutModifier("Ctrl")
    }
  }, [])

  useEffect(() => {
    const handleSearchShortcut = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "k") {
        return
      }

      event.preventDefault()
      searchInputRef.current?.focus()
      searchInputRef.current?.select()
    }

    globalThis.addEventListener("keydown", handleSearchShortcut)
    return () => globalThis.removeEventListener("keydown", handleSearchShortcut)
  }, [])

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
    <div
      className={`${styles.sidebar} ${className ?? ""}`}
      style={width ? {width: `${width}px`} : undefined}
    >
      <div className={styles.header}>
        <div className={styles.headerTop}>
          <div className={styles.title}>
            <AppIcon theme={currentTheme} size={24} />
            <span className={styles.titleBody}>
              <span className={styles.titleRow}>
                <span className={styles.titleName}>Test UI</span>
              </span>
              <span className={styles.titleMeta}>by Acton</span>
            </span>
          </div>
          <div className={styles.headerButtons}>
            {onCollapse && (
              <button
                type="button"
                onClick={onCollapse}
                className={styles.collapseButton}
                title={isCollapsed ? "Pin sidebar" : "Collapse sidebar"}
                aria-label={isCollapsed ? "Pin Sidebar" : "Collapse Sidebar"}
              >
                <DocsSidebarIcon />
              </button>
            )}
          </div>
        </div>
        <Summary reports={reports} />

        <div className={styles.searchContainer}>
          <DocsSearchIcon />
          <input
            ref={searchInputRef}
            type="text"
            placeholder="Filter tests..."
            className={styles.searchInput}
            value={searchQuery}
            onChange={e => setSearchQuery(e.target.value)}
            aria-label="Filter tests"
          />
          <span className={styles.searchShortcut} aria-hidden="true">
            <kbd>{searchShortcutModifier}</kbd>
            <kbd>K</kbd>
          </span>
        </div>

        <div className={styles.filters}>
          {(Object.values(TestStatus) as TestStatus[]).map(status => (
            <button
              key={status}
              type="button"
              className={`${styles.filterButton} ${statusFilter.has(status) ? styles.activeFilter : ""} ${styles[status.toLowerCase()]}`}
              onClick={() => toggleStatusFilter(status)}
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

      {onToggleTheme && (
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
            theme={currentTheme}
            onToggleTheme={onToggleTheme}
            title={`Switch to ${currentTheme === "light" ? "dark" : "light"} theme`}
          />
        </div>
      )}
    </div>
  )
}

const DocsSearchIcon: React.FC = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    width="24"
    height="24"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
    className={styles.searchIcon}
    aria-hidden="true"
  >
    <circle cx="11" cy="11" r="8" />
    <path d="m21 21-4.3-4.3" />
  </svg>
)
