import {useCallback, useEffect, useRef, useState} from "react"

import {
  type TestRunOutput,
  type TestRunRecord,
  type TestRunSummary,
  cancelStudioTestRun,
  fetchStudioTestRun,
  fetchStudioTestRunOutput,
  fetchStudioTestRuns,
  subscribeToStudioTestRuns,
} from "../studioApi"

const EMPTY_OUTPUT: TestRunOutput = {stdout: "", stderr: ""}
const FALLBACK_POLL_INTERVAL_MS = 5000
const LIVE_REFRESH_DELAY_MS = 100

export interface StudioTestRunsState {
  readonly runs: readonly TestRunSummary[]
  readonly selectedRunId?: string
  readonly selectedRun?: TestRunRecord
  readonly output: TestRunOutput
  readonly isLoading: boolean
  readonly isCancelling: boolean
  readonly error?: string
  readonly cancelSelectedRun: () => Promise<void>
  readonly refresh: () => Promise<void>
  readonly selectRun: (runId: string) => void
  readonly setStartedRun: (run: TestRunRecord) => void
}

type TestRunSelectionHandler = (runId: string | undefined, replace: boolean) => void

export function useStudioTestRuns(
  enabled: boolean,
  selectedRunId: string | undefined,
  onSelectedRunIdChange: TestRunSelectionHandler,
): StudioTestRunsState {
  const [runs, setRuns] = useState<readonly TestRunSummary[]>([])
  const [selectedRun, setSelectedRun] = useState<TestRunRecord>()
  const [output, setOutput] = useState<TestRunOutput>(EMPTY_OUTPUT)
  const [isLoading, setIsLoading] = useState(true)
  const [isCancelling, setIsCancelling] = useState(false)
  const [runsError, setRunsError] = useState<string>()
  const [selectedRunError, setSelectedRunError] = useState<string>()
  const selectedRunIdRef = useRef(selectedRunId)
  const runsRequestRef = useRef(0)
  const selectedRunRequestRef = useRef(0)
  const selectedRunKnown = selectedRunId ? runs.some(run => run.id === selectedRunId) : false

  useEffect(() => {
    selectedRunIdRef.current = selectedRunId
  }, [selectedRunId])

  const refreshRuns = useCallback(
    async (signal?: AbortSignal) => {
      const requestId = ++runsRequestRef.current
      try {
        const nextRuns = await fetchStudioTestRuns(signal)
        if (requestId !== runsRequestRef.current) return
        setRuns(nextRuns)
        const current = selectedRunIdRef.current
        if (current && !nextRuns.some(run => run.id === current)) {
          onSelectedRunIdChange(undefined, true)
        }
        setRunsError(undefined)
      } catch (error) {
        if (isAbortError(error)) return
        if (requestId === runsRequestRef.current) setRunsError(getErrorMessage(error))
      } finally {
        if (requestId === runsRequestRef.current) setIsLoading(false)
      }
    },
    [onSelectedRunIdChange],
  )

  const refreshSelectedRun = useCallback(
    async (runId: string, signal?: AbortSignal, includeOutput = true) => {
      const requestId = ++selectedRunRequestRef.current
      try {
        const [run, runOutput] = await Promise.all([
          fetchStudioTestRun(runId, signal),
          includeOutput
            ? fetchStudioTestRunOutput(runId, signal)
            : Promise.resolve<TestRunOutput | undefined>(undefined),
        ])
        if (requestId !== selectedRunRequestRef.current || selectedRunIdRef.current !== runId) {
          return
        }
        setSelectedRun(run)
        if (runOutput) setOutput(runOutput)
        setSelectedRunError(undefined)
      } catch (error) {
        if (!isAbortError(error) && requestId === selectedRunRequestRef.current) {
          setSelectedRunError(getErrorMessage(error))
        }
      }
    },
    [],
  )

  useEffect(() => {
    if (!enabled) return

    const controller = new AbortController()
    void refreshRuns(controller.signal)
    return () => controller.abort()
  }, [enabled, refreshRuns])

  useEffect(() => {
    if (!enabled) return
    if (!selectedRunId) {
      selectedRunRequestRef.current += 1
      setSelectedRun(undefined)
      setOutput(EMPTY_OUTPUT)
      setSelectedRunError(undefined)
      return
    }
    if (!selectedRunKnown) return

    const controller = new AbortController()
    setSelectedRun(current => (current?.id === selectedRunId ? current : undefined))
    setOutput(EMPTY_OUTPUT)
    void refreshSelectedRun(selectedRunId, controller.signal)
    return () => controller.abort()
  }, [enabled, refreshSelectedRun, selectedRunId, selectedRunKnown])

  useEffect(() => {
    if (!enabled) return

    let refreshTimer: ReturnType<typeof globalThis.setTimeout> | undefined
    let streamOpen = false
    let includeOutputInRefresh = false
    const scheduleSelectedRefresh = (runId: string, includeOutput: boolean) => {
      if (runId !== selectedRunIdRef.current) return
      includeOutputInRefresh ||= includeOutput
      if (refreshTimer !== undefined) return
      refreshTimer = globalThis.setTimeout(() => {
        refreshTimer = undefined
        const selected = selectedRunIdRef.current
        if (selected) {
          const shouldIncludeOutput = includeOutputInRefresh
          includeOutputInRefresh = false
          void refreshSelectedRun(selected, undefined, shouldIncludeOutput)
        }
      }, LIVE_REFRESH_DELAY_MS)
    }
    const unsubscribe = subscribeToStudioTestRuns(
      event => {
        if (event.type === "output") {
          if (event.data.runId === selectedRunIdRef.current) {
            setOutput(current => ({
              ...current,
              [event.data.stream]: current[event.data.stream] + event.data.chunk,
            }))
          }
          return
        }
        if (event.type === "reporterEvent") return

        setRuns(current => upsertRunSummary(current, event.data.run))
        scheduleSelectedRefresh(
          event.data.run.id,
          event.data.run.status !== "running" && event.data.run.status !== "queued",
        )
      },
      () => {
        streamOpen = false
      },
      () => {
        streamOpen = true
        void refreshRuns()
        const selected = selectedRunIdRef.current
        if (selected) void refreshSelectedRun(selected)
      },
    )
    const polling = globalThis.setInterval(() => {
      if (streamOpen) return
      void refreshRuns()
      const selected = selectedRunIdRef.current
      if (selected) void refreshSelectedRun(selected)
    }, FALLBACK_POLL_INTERVAL_MS)

    return () => {
      if (refreshTimer !== undefined) globalThis.clearTimeout(refreshTimer)
      globalThis.clearInterval(polling)
      unsubscribe()
    }
  }, [enabled, refreshRuns, refreshSelectedRun])

  const setStartedRun = useCallback(
    (run: TestRunRecord) => {
      setRuns(current => [run, ...current.filter(candidate => candidate.id !== run.id)])
      setSelectedRun(run)
      setOutput(EMPTY_OUTPUT)
      setSelectedRunError(undefined)
      onSelectedRunIdChange(run.id, false)
    },
    [onSelectedRunIdChange],
  )

  const cancelSelectedRun = useCallback(async () => {
    if (selectedRun?.source !== "studio" || selectedRun.status !== "running") return

    setIsCancelling(true)
    try {
      const run = await cancelStudioTestRun(selectedRun.id)
      setSelectedRun(run)
      await refreshRuns()
    } catch (error) {
      setSelectedRunError(getErrorMessage(error))
      setIsCancelling(false)
    }
  }, [refreshRuns, selectedRun])

  useEffect(() => {
    if (selectedRun?.status !== "running") setIsCancelling(false)
  }, [selectedRun?.status])

  const refresh = useCallback(async () => {
    await refreshRuns()
    const selected = selectedRunIdRef.current
    if (selected) await refreshSelectedRun(selected)
  }, [refreshRuns, refreshSelectedRun])

  return {
    runs,
    selectedRunId,
    selectedRun,
    output,
    isLoading,
    isCancelling,
    error: selectedRunId ? (selectedRunError ?? runsError) : runsError,
    cancelSelectedRun,
    refresh,
    selectRun: runId => onSelectedRunIdChange(runId, false),
    setStartedRun,
  }
}

function upsertRunSummary(
  runs: readonly TestRunSummary[],
  nextRun: TestRunSummary,
): readonly TestRunSummary[] {
  return [...runs.filter(run => run.id !== nextRun.id), nextRun].sort((left, right) =>
    right.startedAt.localeCompare(left.startedAt),
  )
}

function isAbortError(error: unknown) {
  return error instanceof DOMException && error.name === "AbortError"
}

function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
