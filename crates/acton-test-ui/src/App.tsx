import type React from "react"
import { useEffect, useState } from "react"
import { Sidebar } from "./components/Sidebar/Sidebar"
import { TestDetails } from "./components/TestDetails/TestDetails"
import type { TestReport, Trace } from "./types"

export const App: React.FC = () => {
  const [reports, setReports] = useState<TestReport[]>([])
  const [selectedTest, setSelectedTest] = useState<TestReport | null>(null)
  const [currentTrace, setCurrentTrace] = useState<Trace | null>(null)
  const [loading, setLoading] = useState(true)
  const [detailsLoading, setDetailsLoading] = useState(false)

  useEffect(() => {
    fetch("/api/reports")
      .then((res) => res.json())
      .then((data) => {
        setReports(data)
        if (data.length > 0 && !selectedTest) {
          handleSelectTest(data[0])
        }
        setLoading(false)
      })
      .catch((err) => {
        console.error("Failed to fetch reports", err)
        setLoading(false)
      })
  }, [])

  const handleSelectTest = (test: TestReport) => {
    setSelectedTest(test)
    if (test.trace_path) {
      setDetailsLoading(true)
      fetch(`/api/trace/${test.trace_path}`)
        .then((res) => res.json())
        .then((data) => {
          setCurrentTrace(data)
          setDetailsLoading(false)
        })
        .catch((err) => {
          console.error("Failed to fetch trace", err)
          setCurrentTrace(null)
          setDetailsLoading(false)
        })
    } else {
      setCurrentTrace(null)
    }
  }

  if (loading && reports.length === 0) {
    return (
      <div
        style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "100vh" }}
      >
        Loading...
      </div>
    )
  }

  return (
    <div style={{ display: "flex", height: "100vh", overflow: "hidden" }}>
      <Sidebar reports={reports} selectedTest={selectedTest} onSelectTest={handleSelectTest} />
      <div style={{ flex: 1, position: "relative" }}>
        {selectedTest ? (
          <>
            {detailsLoading && (
              <div
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  right: 0,
                  bottom: 0,
                  backgroundColor: "rgba(255, 255, 255, 0.7)",
                  display: "flex",
                  justifyContent: "center",
                  alignItems: "center",
                  zIndex: 10,
                }}
              >
                Loading trace...
              </div>
            )}
            <TestDetails test={selectedTest} trace={currentTrace} />
          </>
        ) : (
          <div
            style={{
              display: "flex",
              justifyContent: "center",
              alignItems: "center",
              height: "100%",
            }}
          >
            Select a test to see details
          </div>
        )}
      </div>
    </div>
  )
}
