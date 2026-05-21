/* eslint-disable unicorn/no-process-exit */

import {mkdtempSync, rmSync} from "node:fs"
import path from "node:path"
import {performance} from "node:perf_hooks"
import {pathToFileURL} from "node:url"

import {Address, type Transaction} from "@ton/core"

import {Localnet} from "./localnet.js"
import {
  formatValue,
  getRegisteredTests,
  isActonAssertionError,
  isTransactionAssertionError,
  type RegisteredTest,
} from "./test.js"
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
      readonly stdout?: string
      readonly stderr?: string
      readonly assertion?: SerializedAssertion
    }
  | {
      readonly type: "coverage"
      readonly id: number
      readonly records: readonly SerializedCoverageRecord[]
    }
  | {
      readonly type: "trace"
      readonly id: number
      readonly records: readonly SerializedTraceRecord[]
    }
  | {
      readonly type: "treasury"
      readonly id: number
      readonly records: readonly SerializedTreasuryRecord[]
    }
  | {readonly type: "fatal"; readonly message: string; readonly stack?: string}

type SerializedAssertion =
  | {
      readonly type: "transaction"
      readonly label: string
      readonly transactions: readonly string[]
      readonly match: SerializedTransactionMatch
      readonly message: string
      readonly row?: number
      readonly column?: number
    }
  | {
      readonly type: "value"
      readonly matcher: string
      readonly actual: string
      readonly expected: string
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

type SerializedCoverageRecord = {
  readonly code: string
  readonly vmLog: string
}

type SerializedTraceRecord = {
  readonly rawTransaction: string
  readonly shardAccountBefore: string
  readonly shardAccount: string
  readonly parentTransaction?: number
  readonly code?: string
  readonly vmLog: string
  readonly executorLogs?: string
  readonly actions?: string
}

type SerializedTreasuryRecord = {
  readonly address: string
  readonly name: string
}

const testFile = process.argv[2]
const outputCapture = installOutputCapture()

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
    const output = outputCapture.start()
    try {
      await registered.fn({localnet})
      emitDiagnosticRecords(registered.id, localnet)
      const captured = output.stop()
      emit({
        durationMs: performance.now() - startedAt,
        id: registered.id,
        stderr: captured.stderr,
        status: "passed",
        stdout: captured.stdout,
        type: "testFinished",
      })
    } catch (error) {
      failed += 1
      emitDiagnosticRecords(registered.id, localnet)
      const captured = output.stop()
      emit({
        assertion: serializeAssertion(error),
        durationMs: performance.now() - startedAt,
        id: registered.id,
        message: errorMessage(error),
        stack: errorStack(error),
        stderr: captured.stderr,
        status: "failed",
        stdout: captured.stdout,
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

function emitDiagnosticRecords(id: number, localnet: Localnet): void {
  const coverageRecords = localnet.consumeCoverageRecords()
  if (coverageRecords.length > 0) {
    emit({id, records: coverageRecords, type: "coverage"})
  }

  const traceRecords = localnet.consumeTraceRecords()
  if (traceRecords.length > 0) {
    emit({id, records: traceRecords, type: "trace"})
  }

  const treasuryRecords = localnet.consumeTreasuryRecords()
  if (treasuryRecords.length > 0) {
    emit({id, records: treasuryRecords, type: "treasury"})
  }
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
  outputCapture.writeEvent(`${EVENT_PREFIX}${JSON.stringify(event)}\n`)
}

function serializeAssertion(error: unknown): SerializedAssertion | undefined {
  if (isActonAssertionError(error)) {
    return {
      actual: formatValue(error.actual),
      expected: formatValue(error.expected),
      matcher: error.matcher,
      message: error.message,
      ...locationFromStack(error.stack),
      type: "value",
    }
  }

  if (isTransactionAssertionError(error)) {
    return {
      label: error.label,
      match: serializeMatch(error.match),
      message: error.message,
      ...locationFromStack(error.stack),
      transactions: error.transactions.map(transaction => serializeTransaction(transaction)),
      type: "transaction",
    }
  }

  return undefined
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

function installOutputCapture(): {
  readonly start: () => {readonly stop: () => {readonly stdout: string; readonly stderr: string}}
  readonly writeEvent: (line: string) => void
} {
  const originalStdoutWrite = process.stdout.write.bind(process.stdout)
  const originalStderrWrite = process.stderr.write.bind(process.stderr)
  const originalConsoleLog = console.log.bind(console)
  const originalConsoleInfo = console.info.bind(console)
  const originalConsoleWarn = console.warn.bind(console)
  const originalConsoleError = console.error.bind(console)
  let active: {stdout: string; stderr: string} | undefined

  process.stdout.write = ((chunk: unknown, encoding?: unknown, callback?: unknown): boolean => {
    if (active) {
      active.stdout += chunkToString(chunk, encoding)
      callWriteCallback(encoding, callback)
      return true
    }
    return originalStdoutWrite(chunk as never, encoding as never, callback as never)
  }) as typeof process.stdout.write

  process.stderr.write = ((chunk: unknown, encoding?: unknown, callback?: unknown): boolean => {
    if (active) {
      active.stderr += chunkToString(chunk, encoding)
      callWriteCallback(encoding, callback)
      return true
    }
    return originalStderrWrite(chunk as never, encoding as never, callback as never)
  }) as typeof process.stderr.write

  console.log = (...args: unknown[]) => {
    if (active) {
      active.stdout += `${formatConsoleArgs(args)}\n`
    } else {
      originalConsoleLog(...args)
    }
  }
  console.info = (...args: unknown[]) => {
    if (active) {
      active.stdout += `${formatConsoleArgs(args)}\n`
    } else {
      originalConsoleInfo(...args)
    }
  }
  console.warn = (...args: unknown[]) => {
    if (active) {
      active.stderr += `${formatConsoleArgs(args)}\n`
    } else {
      originalConsoleWarn(...args)
    }
  }
  console.error = (...args: unknown[]) => {
    if (active) {
      active.stderr += `${formatConsoleArgs(args)}\n`
    } else {
      originalConsoleError(...args)
    }
  }

  return {
    start() {
      const capture = {stderr: "", stdout: ""}
      active = capture
      return {
        stop() {
          if (active === capture) {
            active = undefined
          }
          return capture
        },
      }
    },
    writeEvent(line: string): void {
      originalStdoutWrite(line)
    },
  }
}

function formatConsoleArgs(args: readonly unknown[]): string {
  return args.map(arg => (typeof arg === "string" ? arg : formatValue(arg))).join(" ")
}

function chunkToString(chunk: unknown, encoding: unknown): string {
  if (Buffer.isBuffer(chunk)) {
    return chunk.toString(typeof encoding === "string" ? (encoding as BufferEncoding) : undefined)
  }
  return String(chunk)
}

function callWriteCallback(encoding: unknown, callback: unknown): void {
  const cb = typeof encoding === "function" ? encoding : callback
  if (typeof cb === "function") {
    const writeCallback = cb as () => void
    queueMicrotask(() => {
      writeCallback()
    })
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function errorStack(error: unknown): string | undefined {
  return error instanceof Error ? error.stack : undefined
}
