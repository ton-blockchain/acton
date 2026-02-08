import * as React from "react"
import {useCallback, useEffect, useRef, useState} from "react"
import {FiChevronRight} from "react-icons/fi"

import type {TestReport, Trace} from "@acton/shared-ui"

import styles from "./App.module.css"
import {Sidebar} from "./components/Sidebar/Sidebar"
import {TestDetails} from "./components/TestDetails/TestDetails"

export const App: React.FC = () => {
  const [reports, setReports] = useState<TestReport[]>([])
  const [selectedTest, setSelectedTest] = useState<TestReport | undefined>()
  const [currentTrace, setCurrentTrace] = useState<Trace | undefined>()
  const [projectRoot, setProjectRoot] = useState<string>("")
  const [theme, setTheme] = useState(() => {
    return (
      localStorage.getItem("theme") ||
      (globalThis.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
    )
  })
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    document.documentElement.classList.toggle("dark-theme", theme === "dark")
    localStorage.setItem("theme", theme)
  }, [theme])

  const toggleTheme = useCallback(() => {
    setTheme(prev => (prev === "light" ? "dark" : "light"))
  }, [])
  const [sidebarWidth, setSidebarWidth] = useState(() => {
    const saved = localStorage.getItem("sidebarWidth")
    return saved ? Number.parseInt(saved, 10) : 350
  })
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState(() => {
    return localStorage.getItem("isSidebarCollapsed") === "true"
  })
  const [isHoveredResizer, setIsHoveredResizer] = useState(false)
  const isResizing = useRef(false)
  const lastWidth = useRef(sidebarWidth)

  const handleSelectTest = useCallback((test: TestReport) => {
    setSelectedTest(test)
    localStorage.setItem("selectedTest", `${test.suite_name}::${test.name}`)
    if (test.trace_path) {
      void fetch(`/api/trace/${test.trace_path}`)
        .then(async res => (await res.json()) as Trace)
        .then(data => {
          setCurrentTrace(data)
        })
        .catch(error => {
          console.error("Failed to fetch trace", error)
          setCurrentTrace(undefined)
        })
    } else {
      setCurrentTrace(undefined)
    }
  }, [])

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (!isResizing.current) return
    const newWidth = Math.max(200, Math.min(800, e.clientX))
    setSidebarWidth(newWidth)
    localStorage.setItem("sidebarWidth", newWidth.toString())
    lastWidth.current = newWidth
  }, [])

  const stopResizing = useCallback(() => {
    isResizing.current = false
    document.removeEventListener("mousemove", handleMouseMove)
    document.removeEventListener("mouseup", stopResizing)
    document.body.style.cursor = ""
    document.body.style.userSelect = ""
  }, [handleMouseMove])

  const startResizing = useCallback(() => {
    if (isSidebarCollapsed) return
    isResizing.current = true
    document.addEventListener("mousemove", handleMouseMove)
    document.addEventListener("mouseup", stopResizing)
    document.body.style.cursor = "col-resize"
    document.body.style.userSelect = "none"
  }, [handleMouseMove, stopResizing, isSidebarCollapsed])

  const toggleSidebar = useCallback(() => {
    setIsSidebarCollapsed(prev => {
      const newState = !prev
      localStorage.setItem("isSidebarCollapsed", newState.toString())
      return newState
    })
  }, [])

  useEffect(() => {
    void fetch("/api/config")
      .then(async (res) => (await res.json()) as {project_root: string})
      .then((data) => {
        setProjectRoot(data.project_root);
      })
      .catch((error) => {
        console.error("Failed to fetch config", error);
      });

    void fetch("/api/reports")
      .then(async (res) => (await res.json()) as TestReport[])
      .then((data) => {
        setReports(data);
        if (data.length > 0 && !selectedTest) {
          const savedTestId = localStorage.getItem("selectedTest");
          let testToSelect = data[0];

          if (savedTestId) {
            const found = data.find((t) => `${t.suite_name}::${t.name}` === savedTestId);
            if (found) {
              testToSelect = found;
            }
          }
          handleSelectTest(testToSelect);
        }
        setLoading(false);
      })
      .catch((error) => {
        console.error("Failed to fetch reports", error);
        setLoading(false);
      });
  }, [handleSelectTest, selectedTest]);

  if (loading && reports.length === 0) {
    return <div className={styles.loadingContainer}>Loading...</div>
  }

  return (
    <div className={styles.app}>
      {!isSidebarCollapsed && (
        <Sidebar
          reports={reports}
          selectedTest={selectedTest}
          onSelectTest={handleSelectTest}
          width={sidebarWidth}
          onCollapse={toggleSidebar}
          theme={theme}
          onToggleTheme={toggleTheme}
        />
      )}

      {isSidebarCollapsed && (
        <button
          type="button"
          onClick={toggleSidebar}
          className={styles.expandButton}
          title="Expand sidebar"
        >
          <FiChevronRight size={20} />
        </button>
      )}

      {/* eslint-disable-next-line jsx-a11y/no-noninteractive-element-interactions */}
      <div
        onMouseDown={startResizing}
        onMouseEnter={() => setIsHoveredResizer(true)}
        onMouseLeave={() => setIsHoveredResizer(false)}
        role="separator"
        aria-valuenow={isSidebarCollapsed ? 0 : sidebarWidth}
        aria-valuemin={200}
        aria-valuemax={800}
        aria-label="Resize sidebar"
        className={`${styles.resizer} ${isSidebarCollapsed ? "" : styles.resizerActive} ${
          isHoveredResizer && !isSidebarCollapsed ? styles.resizerHovered : ""
        }`}
      />

      <div className={styles.mainContent}>
        {selectedTest ? (
          <TestDetails test={selectedTest} trace={currentTrace} projectRoot={projectRoot} />
        ) : (
          <div className={styles.noSelection}>Select a test to see details</div>
        )}
      </div>
    </div>
  )
}
