import type React from "react"
import { useEffect, useState } from "react"
import { Summary } from "./components/Summary/Summary"
import { TestList } from "./components/TestList/TestList"
import { TraceView } from "./components/TraceView/TraceView"
import type { TestReport, Trace } from "./types"

export const App: React.FC = () => {
  const [reports, setReports] = useState<TestReport[]>([])
  const [currentTrace, setCurrentTrace] = useState<Trace | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    fetch("/api/reports")
      .then((res) => res.json())
      .then((data) => {
        setReports(data)
        setLoading(false)
      })
      .catch((err) => {
        console.error("Failed to fetch reports", err)
        setLoading(false)
      })
  }, [])

  const handleViewTrace = (name: string) => {
    setLoading(true)
    fetch(`/api/trace/${name}`)
      .then((res) => res.json())
      .then((data) => {
        setCurrentTrace(data)
        setLoading(false)
      })
      .catch((err) => {
        console.error("Failed to fetch trace", err)
        setLoading(false)
      })
  }

  const handleBack = () => {
    setCurrentTrace(null)
  }

  if (loading && reports.length === 0) {
    return <div>Loading...</div>
  }

  return (
    <div style={{ maxWidth: "1000px", margin: "0 auto", padding: "20px" }}>
      {!currentTrace ? (
        <>
          <h1>Acton Test Results</h1>
          <Summary reports={reports} />
          <TestList reports={reports} onViewTrace={handleViewTrace} />
        </>
      ) : (
        <TraceView trace={currentTrace} onBack={handleBack} />
      )}
    </div>
  )
}
