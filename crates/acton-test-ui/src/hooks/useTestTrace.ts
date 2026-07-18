import {useEffect, useState} from "react"

import type {TestReport, Trace} from "@acton/shared-ui"

import {getErrorMessage, isAbortError} from "./request"

interface TestTraceState {
  readonly key: string | undefined
  readonly trace: Trace | undefined
  readonly error: string | undefined
  readonly loading: boolean
}

const EMPTY_TRACE_STATE: TestTraceState = {
  key: undefined,
  trace: undefined,
  error: undefined,
  loading: false,
}

export function useTestTrace(test: TestReport | undefined) {
  const tracePath = test?.trace_path
  const key = tracePath ? `${test.suite_name}:${test.name}:${tracePath}` : undefined
  const [state, setState] = useState<TestTraceState>(EMPTY_TRACE_STATE)

  useEffect(() => {
    if (tracePath === undefined || key === undefined || test === undefined) {
      setState(EMPTY_TRACE_STATE)
      return
    }

    const controller = new AbortController()
    const selectedTest = test
    setState({key, trace: undefined, error: undefined, loading: true})

    const loadTrace = async () => {
      try {
        const response = await fetch(`/api/trace/${encodeURIComponent(tracePath)}`, {
          signal: controller.signal,
        })
        const trace = await parseTraceResponse(response, tracePath)
        setState({key, trace, error: undefined, loading: false})
      } catch (error) {
        if (isAbortError(error)) return

        console.error("Failed to fetch trace", {
          suite: selectedTest.suite_name,
          test: selectedTest.name,
          tracePath,
          error,
        })
        setState({key, trace: undefined, error: getErrorMessage(error), loading: false})
      }
    }

    void loadTrace()
    return () => controller.abort()
  }, [key, test?.name, test?.suite_name, tracePath])

  if (key === undefined) return EMPTY_TRACE_STATE
  if (state.key === key) return state

  return {key, trace: undefined, error: undefined, loading: true}
}

async function parseTraceResponse(
  response: Response,
  tracePath: string,
): Promise<Trace | undefined> {
  if (response.status === 204) return

  const body = await response.text()
  if (!response.ok) {
    throw new Error(formatResponseError(response, body))
  }
  if (body.trim().length === 0) return

  try {
    return JSON.parse(body) as Trace
  } catch (error) {
    throw new Error(`Trace ${tracePath} is not valid JSON: ${getErrorMessage(error)}`)
  }
}

function formatResponseError(response: Response, body: string): string {
  const status = `${response.status} ${response.statusText}`.trim()
  const trimmedBody = body.trim()
  if (trimmedBody.length === 0) return status

  try {
    const json = JSON.parse(trimmedBody) as {error?: unknown}
    if (typeof json.error === "string" && json.error.trim().length > 0) {
      return `${status}: ${json.error}`
    }
  } catch {
    // Fall through to the raw response body below.
  }

  return `${status}: ${trimmedBody.slice(0, 500)}`
}
