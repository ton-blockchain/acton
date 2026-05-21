/* eslint-disable unicorn/no-process-exit */

import {mkdtempSync, rmSync} from "node:fs"
import path from "node:path"
import {performance} from "node:perf_hooks"
import {pathToFileURL} from "node:url"

import {Address, type Transaction} from "@ton/core"

import {Localnet} from "./localnet.js"
import {getRegisteredTests, isTransactionAssertionError, type RegisteredTest} from "./test.js"
import type {TransactionMatch} from "./transactions.js"

const EVENT_PREFIX = "__ACTON_NODE_EVENT__"

type WorkerEvent =
  | {
      readonly type: "tests"
      readonly file: string
      readonly tests: readonly {
        readonly id: number
        readonly name: string
        readonly row: number
        readonly column: number
      }[]
    }
  | {readonly type: "testStarted"; readonly id: number}
  | {
      readonly type: "testFinished"
      readonly id: number
      readonly status: "passed" | "failed"
      readonly durationMs: number
      readonly message?: string
      readonly stack?: string
      readonly assertion?: SerializedAssertion
    }
  | {readonly type: "fatal"; readonly message: string; readonly stack?: string}

type SerializedAssertion = {
  readonly type: "transaction"
  readonly label: string
  readonly transactions: readonly string[]
  readonly match: SerializedTransactionMatch
  readonly message: string
  readonly row?: number
  readonly column?: number
}

type SerializedTransactionMatch = {
  readonly from?: string
  readonly to?: string
  readonly deploy?: boolean
  readonly exitCode?: number
  readonly success?: boolean
}

const testFile = process.argv[2]

if (!testFile) {
  emit({message: "Missing test file path", type: "fatal"})
  process.exit(1)
}

try {
  await import(pathToFileURL(path.resolve(testFile)).href)
} catch (error) {
  emit({
    message: errorMessage(error),
    stack: errorStack(error),
    type: "fatal",
  })
  process.exit(1)
}

const tests = filterTests(getRegisteredTests())
emit({
  file: path.resolve(testFile),
  tests: tests.map(({column, id, name, row}) => ({column, id, name, row})),
  type: "tests",
})

const snapshotDirectory = mkdtempSync("/tmp/acton-node-test-state-")
const snapshotPath = path.join(snapshotDirectory, "state.json")
let failed = 0
let localnet: Localnet | undefined

try {
  localnet = await Localnet.start({autoReset: false})
  await localnet.dumpState(snapshotPath)

  for (const registered of tests) {
    await localnet.loadState(snapshotPath)

    emit({id: registered.id, type: "testStarted"})
    const startedAt = performance.now()
    try {
      await registered.fn({localnet})
      emit({
        durationMs: performance.now() - startedAt,
        id: registered.id,
        status: "passed",
        type: "testFinished",
      })
    } catch (error) {
      failed += 1
      emit({
        assertion: serializeAssertion(error),
        durationMs: performance.now() - startedAt,
        id: registered.id,
        message: errorMessage(error),
        stack: errorStack(error),
        status: "failed",
        type: "testFinished",
      })

      if (process.env.ACTON_TEST_FAIL_FAST === "1") {
        break
      }
    }
  }
} catch (error) {
  failed += 1
  emit({
    message: errorMessage(error),
    stack: errorStack(error),
    type: "fatal",
  })
} finally {
  rmSync(snapshotDirectory, {force: true, recursive: true})
  await localnet?.close()
}

if (failed > 0) {
  process.exit(1)
}

function filterTests(allTests: readonly RegisteredTest[]): readonly RegisteredTest[] {
  const filter = process.env.ACTON_TEST_FILTER
  if (!filter) {
    return allTests
  }
  const regex = new RegExp(filter)
  return allTests.filter(test => regex.test(test.name))
}

function emit(event: WorkerEvent): void {
  process.stdout.write(`${EVENT_PREFIX}${JSON.stringify(event)}\n`)
}

function serializeAssertion(error: unknown): SerializedAssertion | undefined {
  if (!isTransactionAssertionError(error)) {
    return undefined
  }

  return {
    label: error.label,
    match: serializeMatch(error.match),
    message: error.message,
    ...locationFromStack(error.stack),
    transactions: error.transactions.map(transaction => serializeTransaction(transaction)),
    type: "transaction",
  }
}

function locationFromStack(stack: string | undefined): {
  readonly row?: number
  readonly column?: number
} {
  const resolvedTestFile = path.resolve(testFile)
  for (const line of stack?.split("\n") ?? []) {
    if (!line.includes(resolvedTestFile)) {
      continue
    }

    const match = /:(\d+):(\d+)\)?$/.exec(line)
    if (match) {
      return {column: Number(match[2]), row: Number(match[1])}
    }
  }

  return {}
}

function serializeTransaction(transaction: Transaction): string {
  return transaction.raw.toBoc().toString("base64")
}

function serializeMatch(match: TransactionMatch): SerializedTransactionMatch {
  return {
    deploy: match.deploy,
    exitCode: match.exitCode,
    from: serializeAddress(match.from),
    success: match.success,
    to: serializeAddress(match.to),
  }
}

function serializeAddress(address: Address | string | undefined): string | undefined {
  if (address === undefined) {
    return undefined
  }
  return typeof address === "string"
    ? address
    : address.toString({bounceable: true, testOnly: false, urlSafe: true})
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function errorStack(error: unknown): string | undefined {
  return error instanceof Error ? error.stack : undefined
}
