import {useMemo} from "react"
import type {TraceInfo} from "ton-assembly/dist/trace"

export type LinesExecutionData = Record<number, LineExecutionData>

/**
 * Execution data for a single line of code
 */
export interface LineExecutionData {
  /** Gas cost for executing this line */
  readonly gas?: number
  /** Number of times this line was executed */
  readonly executions?: number
}

export function useLineExecutionData(trace: TraceInfo | undefined): LinesExecutionData {
  return useMemo(() => {
    if (!trace || !trace.steps.length) return {}

    const map: Record<number, LineExecutionData> = {}

    for (const step of trace.steps) {
      if (step.loc !== undefined) {
        const line = step.loc.line + 1

        if (!map[line]) {
          map[line] = {}
        }

        map[line] = {
          ...map[line],
          gas: (map[line].gas ?? 0) + step.gasCost,
          executions: (map[line].executions ?? 0) + 1,
        }
      }
    }

    return map
  }, [trace])
}
