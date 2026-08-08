import type {AccountHistorySortOrder} from "../api/client"
import type {V3Action} from "../api/types"

export const MAX_AUTO_LOADED_ACTIONS_PER_TRACE = 10

export interface AccountActionPageCursor {
  readonly offset: number
  readonly startLt?: string
  readonly endLt?: string
}

export interface AutomaticActionPageResult {
  readonly actions: V3Action[]
  readonly collapsedTraceIds: readonly string[]
  readonly cursor: AccountActionPageCursor
  readonly hasMore: boolean
}

export interface StreamedActionsResult {
  readonly actions: V3Action[]
  readonly collapsedTraceIds: readonly string[]
}

interface BoundedActionPage {
  readonly addedActions: V3Action[]
  readonly collapsedTraceIds: readonly string[]
  readonly skippedActionIds: ReadonlySet<string>
}

/**
 * Adds an account-actions page while keeping large traces bounded.
 *
 * Once the visible limit is exceeded, the remainder of the trace is omitted.
 * If that trace reaches the end of the API page, the next cursor jumps across
 * its shared trace_end_lt instead of downloading every omitted action.
 */
export function mergeAutomaticActionPage(
  current: readonly V3Action[],
  page: readonly V3Action[],
  cursor: AccountActionPageCursor,
  sort: AccountHistorySortOrder,
  pageSize: number,
  maxActionsPerTrace = MAX_AUTO_LOADED_ACTIONS_PER_TRACE,
): AutomaticActionPageResult {
  const {addedActions, collapsedTraceIds, skippedActionIds} = collectBoundedActionPage(
    current,
    page,
    maxActionsPerTrace,
  )
  const actions = [...current, ...addedActions]
  const hasMore = page.length === pageSize
  const lastAction = page.at(-1)
  const nextCursor =
    hasMore &&
    lastAction !== undefined &&
    skippedActionIds.has(lastAction.action_id) &&
    normalizedTraceId(lastAction)
      ? (cursorAfterTrace(lastAction, sort) ?? {
          ...cursor,
          offset: cursor.offset + page.length,
        })
      : {...cursor, offset: cursor.offset + page.length}

  return {
    actions,
    collapsedTraceIds,
    cursor: nextCursor,
    hasMore,
  }
}

export function mergeStreamedActions(
  current: readonly V3Action[],
  streamed: readonly V3Action[],
  sort: AccountHistorySortOrder,
  maxActionsPerTrace = MAX_AUTO_LOADED_ACTIONS_PER_TRACE,
): StreamedActionsResult {
  const {addedActions, collapsedTraceIds} = collectBoundedActionPage(
    current,
    streamed,
    maxActionsPerTrace,
  )
  return {
    actions: sort === "desc" ? [...addedActions, ...current] : [...current, ...addedActions],
    collapsedTraceIds,
  }
}

function collectBoundedActionPage(
  current: readonly V3Action[],
  page: readonly V3Action[],
  maxActionsPerTrace: number,
): BoundedActionPage {
  const seenActionIds = new Set(current.map(action => action.action_id))
  const traceCounts = countActionsByTrace(current)
  const addedActions: V3Action[] = []
  const collapsedTraceIds = new Set<string>()
  const skippedActionIds = new Set<string>()

  for (const action of page) {
    if (seenActionIds.has(action.action_id)) {
      continue
    }
    seenActionIds.add(action.action_id)

    const traceId = normalizedTraceId(action)
    if (traceId) {
      const count = traceCounts.get(traceId) ?? 0
      if (count >= maxActionsPerTrace) {
        collapsedTraceIds.add(traceId)
        skippedActionIds.add(action.action_id)
        continue
      }
      traceCounts.set(traceId, count + 1)
    }

    addedActions.push(action)
  }

  return {
    addedActions,
    collapsedTraceIds: [...collapsedTraceIds],
    skippedActionIds,
  }
}

export function countActionsForTrace(actions: readonly V3Action[], traceId: string): number {
  return actions.reduce(
    (count, action) => count + (normalizedTraceId(action) === traceId ? 1 : 0),
    0,
  )
}

function countActionsByTrace(actions: readonly V3Action[]): Map<string, number> {
  const counts = new Map<string, number>()
  for (const action of actions) {
    const traceId = normalizedTraceId(action)
    if (traceId) {
      counts.set(traceId, (counts.get(traceId) ?? 0) + 1)
    }
  }
  return counts
}

function normalizedTraceId(action: V3Action): string | undefined {
  return action.trace_id?.trim() || undefined
}

function cursorAfterTrace(
  action: V3Action,
  sort: AccountHistorySortOrder,
): AccountActionPageCursor | undefined {
  try {
    const traceEndLt = BigInt(action.trace_end_lt)
    if (sort === "desc") {
      if (traceEndLt <= 0n) return undefined
      return {offset: 0, endLt: (traceEndLt - 1n).toString()}
    }
    return {offset: 0, startLt: (traceEndLt + 1n).toString()}
  } catch {
    return undefined
  }
}
