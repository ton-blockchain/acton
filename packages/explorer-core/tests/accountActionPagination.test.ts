import {expect, test} from "bun:test"

import type {V3Action} from "../src/api/types"
import {
  MAX_AUTO_LOADED_ACTIONS_PER_TRACE,
  mergeAutomaticActionPage,
  mergeStreamedActions,
} from "../src/pages/accountActionPagination"

function action(traceId: string, index: number, traceEndLt = "1000"): V3Action {
  return {
    action_id: `${traceId}-action-${index}`,
    trace_id: traceId,
    trace_end_lt: traceEndLt,
  } as V3Action
}

test("large descending traces are capped and skipped with a trace boundary", () => {
  const page = Array.from({length: 20}, (_, index) => action("bulk", index))

  const result = mergeAutomaticActionPage([], page, {offset: 0}, "desc", 20)

  expect(result.actions.map(item => item.action_id)).toEqual(
    page.slice(0, MAX_AUTO_LOADED_ACTIONS_PER_TRACE).map(item => item.action_id),
  )
  expect(result.collapsedTraceIds).toEqual(["bulk"])
  expect(result.cursor).toEqual({offset: 0, endLt: "999"})
  expect(result.hasMore).toBe(true)
})

test("large ascending traces skip forward without downloading their remaining actions", () => {
  const page = Array.from({length: 20}, (_, index) => action("bulk", index, "1000"))

  const result = mergeAutomaticActionPage([], page, {offset: 0}, "asc", 20)

  expect(result.actions).toHaveLength(MAX_AUTO_LOADED_ACTIONS_PER_TRACE)
  expect(result.cursor).toEqual({offset: 0, startLt: "1001"})
})

test("ordinary pages keep offset pagination until a trace exceeds the threshold", () => {
  const firstPage = [
    ...Array.from({length: 8}, (_, index) => action("first", index, "3000")),
    ...Array.from({length: 12}, (_, index) => action("second", index, "2000")),
  ]

  const result = mergeAutomaticActionPage([], firstPage, {offset: 0}, "desc", 20)

  expect(result.actions.filter(item => item.trace_id === "first")).toHaveLength(8)
  expect(result.actions.filter(item => item.trace_id === "second")).toHaveLength(10)
  expect(result.collapsedTraceIds).toEqual(["second"])
  expect(result.cursor).toEqual({offset: 0, endLt: "1999"})
})

test("a partial final trace continues with an offset so its size can be detected", () => {
  const page = [
    ...Array.from({length: 12}, (_, index) => action("first", index, "3000")),
    ...Array.from({length: 8}, (_, index) => action("second", index, "2000")),
  ]

  const result = mergeAutomaticActionPage([], page, {offset: 40, endLt: "4000"}, "desc", 20)

  expect(result.collapsedTraceIds).toEqual(["first"])
  expect(result.cursor).toEqual({offset: 60, endLt: "4000"})
})

test("streamed actions keep the selected order, remove duplicates, and cap large traces", () => {
  const current = [action("old", 0, "3000")]
  const streamed = [
    action("new", 0, "4000"),
    action("old", 0, "3000"),
    ...Array.from({length: 12}, (_, index) => action("bulk", index, "5000")),
  ]

  expect({
    ascending: mergeStreamedActions(current, streamed, "asc"),
    descending: mergeStreamedActions(current, streamed, "desc"),
  }).toMatchSnapshot()
})
