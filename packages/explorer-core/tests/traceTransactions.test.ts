import {expect, test} from "bun:test"

import {
  createPartialTraceState,
  loadPartialTraceBranches,
  partialTraceTransactionsMap,
  restorePartialTracePath,
} from "../src/api/partialTrace"
import {buildPartialTraceRoot} from "../src/api/traceTransactions"
import type {
  V3Message,
  V3TraceNode,
  V3Transaction,
  V3TransactionDetailsResponse,
} from "../src/api/types"

const message = (
  hash: string,
  createdLt: string,
  source = "source",
  destination = "destination",
): V3Message =>
  ({
    hash,
    created_lt: createdLt,
    source,
    destination,
  }) as V3Message

const transaction = (
  hash: string,
  lt: string,
  {
    inMessage,
    outMessages = [],
    traceId = "trace",
  }: {
    readonly inMessage?: V3Message
    readonly outMessages?: readonly V3Message[]
    readonly traceId?: string
  } = {},
): V3Transaction =>
  ({
    hash,
    lt,
    trace_id: traceId,
    in_msg: inMessage,
    out_msgs: outMessages,
  }) as V3Transaction

const response = (...transactions: V3Transaction[]): V3TransactionDetailsResponse => ({
  address_book: {},
  transactions,
})

const traceShape = (node: V3TraceNode | undefined): unknown =>
  node
    ? {
        hash: node.tx_hash,
        children: node.children?.map(traceShape),
      }
    : undefined

test("partial trace root preserves explicit causal edges", () => {
  const root = buildPartialTraceRoot(
    {
      childB: transaction("child-b", "4"),
      root: transaction("root", "1"),
      childA: transaction("child-a", "3"),
      grandchild: transaction("grandchild", "5"),
    },
    new Map([
      ["child-b", "root"],
      ["child-a", "root"],
      ["grandchild", "child-a"],
    ]),
  )

  expect(traceShape(root)).toMatchInlineSnapshot(`
    {
      "children": [
        {
          "children": [
            {
              "children": [],
              "hash": "grandchild",
            },
          ],
          "hash": "child-a",
        },
        {
          "children": [],
          "hash": "child-b",
        },
      ],
      "hash": "root",
    }
  `)
})

test("partial trace restores the causal path before loading branches", async () => {
  const rootMessage = message("root-to-middle", "2")
  const selectedMessage = message("middle-to-selected", "4")
  const root = transaction("root", "1", {outMessages: [rootMessage]})
  const middle = transaction("middle", "3", {
    inMessage: rootMessage,
    outMessages: [selectedMessage],
  })
  const selected = transaction("selected", "5", {inMessage: selectedMessage})
  const lookups: string[] = []

  const result = await restorePartialTracePath(
    createPartialTraceState({
      traceId: "trace",
      totalTransactionCount: 3,
      origin: root,
      selected,
    }),
    async (messageHash, direction) => {
      lookups.push(`${direction}:${messageHash}`)
      return response(middle)
    },
    () => true,
  )

  expect({
    lookups,
    pathComplete: result.state.pathComplete,
    visible: [...result.state.visibleKeys],
    parents: [...result.state.parentByChildKey],
    transactions: Object.keys(partialTraceTransactionsMap(result.state)),
  }).toMatchInlineSnapshot(`
    {
      "lookups": [
        "out:middle-to-selected",
      ],
      "parents": [
        [
          "selected",
          "middle",
        ],
        [
          "middle",
          "root",
        ],
      ],
      "pathComplete": true,
      "transactions": [
        "selected",
        "middle",
        "root",
      ],
      "visible": [
        "selected",
        "middle",
        "root",
      ],
    }
  `)
})

test("partial trace loads at most ten branches per request", async () => {
  const branchMessages = Array.from({length: 12}, (_, index) =>
    message(`branch-${index + 1}`, String(index + 2)),
  )
  const root = transaction("root", "1", {outMessages: branchMessages})
  const initialState = {
    ...createPartialTraceState({
      traceId: "trace",
      totalTransactionCount: 13,
      origin: root,
      selected: root,
    }),
    pathComplete: true,
  }
  const lookups: string[] = []

  const result = await loadPartialTraceBranches(
    initialState,
    async (messageHash, direction) => {
      lookups.push(`${direction}:${messageHash}`)
      const branchMessage = branchMessages.find(candidate => candidate.hash === messageHash)
      return response(
        transaction(`child-${messageHash}`, String(Number(branchMessage?.created_lt) + 20), {
          inMessage: branchMessage,
        }),
      )
    },
    () => true,
  )

  expect({
    lookups,
    visible: [...result.state.visibleKeys],
    parents: [...result.state.parentByChildKey],
    expandedMessages: [...result.state.expandedMessageKeys],
    failedRequests: result.failedRequests,
  }).toMatchInlineSnapshot(`
    {
      "expandedMessages": [
        "branch-1",
        "branch-2",
        "branch-3",
        "branch-4",
        "branch-5",
        "branch-6",
        "branch-7",
        "branch-8",
        "branch-9",
        "branch-10",
      ],
      "failedRequests": 0,
      "lookups": [
        "in:branch-1",
        "in:branch-2",
        "in:branch-3",
        "in:branch-4",
        "in:branch-5",
        "in:branch-6",
        "in:branch-7",
        "in:branch-8",
        "in:branch-9",
        "in:branch-10",
      ],
      "parents": [
        [
          "child-branch-1",
          "root",
        ],
        [
          "child-branch-2",
          "root",
        ],
        [
          "child-branch-3",
          "root",
        ],
        [
          "child-branch-4",
          "root",
        ],
        [
          "child-branch-5",
          "root",
        ],
        [
          "child-branch-6",
          "root",
        ],
        [
          "child-branch-7",
          "root",
        ],
        [
          "child-branch-8",
          "root",
        ],
        [
          "child-branch-9",
          "root",
        ],
        [
          "child-branch-10",
          "root",
        ],
      ],
      "visible": [
        "root",
        "child-branch-1",
        "child-branch-2",
        "child-branch-3",
        "child-branch-4",
        "child-branch-5",
        "child-branch-6",
        "child-branch-7",
        "child-branch-8",
        "child-branch-9",
        "child-branch-10",
      ],
    }
  `)
})
