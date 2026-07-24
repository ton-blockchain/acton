import type {Localnet} from "./localnet.js"

export {ton} from "./amount.js"
export type {ContractHandle} from "./contract.js"
export {Localnet} from "./localnet.js"
export {
  expectFailedTx,
  expectSuccessfulDeploy,
  expectSuccessfulTx,
  findTransaction,
  isTransactionAssertionError,
  transactionExitCode,
  TransactionAssertionError,
  transactionSucceeded,
} from "./transactions.js"
export type {
  FailedTransactionMatch,
  TransactionEndpointsMatch,
  TransactionMatch,
} from "./transactions.js"

export type ActonTestContext = {
  readonly localnet: Localnet
}

export type ActonTestFunction = (context: ActonTestContext) => Promise<void> | void

export type RegisteredTest = {
  readonly id: number
  readonly name: string
  readonly fn: ActonTestFunction
  readonly row: number
  readonly column: number
}

export class ActonAssertionError extends Error {
  constructor(
    readonly matcher: string,
    readonly actual: unknown,
    readonly expected: unknown,
    message: string,
  ) {
    super(message)
  }
}

const tests: RegisteredTest[] = []

export function test(name: string, fn: ActonTestFunction): void {
  tests.push({
    column: 1,
    fn,
    id: tests.length,
    name,
    row: locationFromStack().row,
  })
}

export function getRegisteredTests(): readonly RegisteredTest[] {
  return tests
}

export function expect<T>(actual: T): {
  readonly toBe: (expected: T) => void
  readonly toEqual: (expected: T) => void
} {
  return {
    toBe(expected: T): void {
      if (!Object.is(actual, expected)) {
        throw new ActonAssertionError(
          "toBe",
          actual,
          expected,
          `expect(<actual>).toBe(<expected>)`,
        )
      }
    },
    toEqual(expected: T): void {
      if (!deepEqual(actual, expected)) {
        throw new ActonAssertionError(
          "toEqual",
          actual,
          expected,
          `expect(<actual>).toEqual(<expected>)`,
        )
      }
    },
  }
}

export function isActonAssertionError(error: unknown): error is ActonAssertionError {
  return error instanceof ActonAssertionError
}

function locationFromStack(): {readonly row: number} {
  const stack = new Error("capture test registration stack").stack ?? ""
  const line = stack.split("\n").find(item => item.includes(".test.") && /:\d+:\d+\)?$/.test(item))
  const match = /:(\d+):\d+\)?$/.exec(line ?? "")
  return {row: match ? Number(match[1]) : 1}
}

function deepEqual(left: unknown, right: unknown): boolean {
  return formatValue(left) === formatValue(right)
}

export function formatValue(value: unknown): string {
  if (typeof value === "string") {
    return JSON.stringify(value)
  }
  if (typeof value === "bigint") {
    return `${value}n`
  }
  if (value instanceof Error) {
    return `${value.name}: ${value.message}`
  }
  if (typeof value === "object" && value !== null) {
    try {
      return JSON.stringify(value, jsonValueReplacer, 2)
    } catch {
      return Object.prototype.toString.call(value)
    }
  }
  return String(value)
}

function jsonValueReplacer(_key: string, nested: unknown): unknown {
  return typeof nested === "bigint" ? `${nested}n` : nested
}
