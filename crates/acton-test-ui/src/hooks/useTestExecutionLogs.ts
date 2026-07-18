import {useEffect, useState} from "react"

import type {TestExecutionLogs, TestReport} from "@acton/shared-ui"

import {isAbortError} from "./request"

interface TestExecutionLogsState {
  readonly key: string
  readonly logs: TestExecutionLogs | undefined
  readonly loading: boolean
}

export function useTestExecutionLogs(
  test: Pick<TestReport, "file_path" | "name" | "row" | "column">,
) {
  const key = `${test.file_path}:${test.row}:${test.column}:${test.name}`
  const [state, setState] = useState<TestExecutionLogsState>({
    key,
    logs: undefined,
    loading: true,
  })

  useEffect(() => {
    const controller = new AbortController()
    const params = new URLSearchParams({
      file_path: test.file_path,
      name: test.name,
      row: test.row.toString(),
      column: test.column.toString(),
    })

    setState({key, logs: undefined, loading: true})

    const loadLogs = async () => {
      try {
        const response = await fetch(`/api/test-logs?${params.toString()}`, {
          signal: controller.signal,
        })
        if (!response.ok) {
          throw new Error(`Failed to fetch test logs: ${response.status}`)
        }

        const logs = (await response.json()) as TestExecutionLogs
        setState({key, logs, loading: false})
      } catch (error) {
        if (isAbortError(error)) return

        console.error("Failed to fetch test logs", error)
        setState({key, logs: {}, loading: false})
      }
    }

    void loadLogs()
    return () => controller.abort()
  }, [key, test.column, test.file_path, test.name, test.row])

  if (state.key === key) {
    return {executionLogs: state.logs, isLoadingExecutionLogs: state.loading}
  }

  return {executionLogs: undefined, isLoadingExecutionLogs: true}
}
