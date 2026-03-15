import {
  type BackendContractInfo,
  type TestReport,
  TestStatus,
  type Trace,
} from "@acton/shared-ui/src/types"

export const projectRoot = "/workspace/sample-project"

export const reports: TestReport[] = [
  {
    name: "counter increments",
    suite_name: "counter",
    file_path: `${projectRoot}/tests/counter.spec.tolk`,
    row: 8,
    column: 2,
    duration: {secs: 0, nanos: 125_000_000},
    status: TestStatus.Passed,
    trace_path: "counter-increments.json",
  },
  {
    name: "rejects invalid opcode",
    suite_name: "counter",
    file_path: `${projectRoot}/tests/counter.spec.tolk`,
    row: 23,
    column: 6,
    duration: {secs: 0, nanos: 98_000_000},
    status: TestStatus.Failed,
    message: "Unexpected exit code",
    detailed_message: "Unexpected exit code: got 35, expected 0",
    details: `${projectRoot}/tests/counter.spec.tolk:24:7`,
    trace_path: "rejects-invalid-opcode.json",
  },
]

export const traces: Record<string, Trace> = {
  "counter-increments.json": {
    name: "Counter Tests",
    traces: [
      {
        name: "increment trace",
        transactions: [],
        failed_messages: [],
      },
    ],
    contracts: [],
    wallets: {},
  },
  "rejects-invalid-opcode.json": {
    name: "Counter Tests",
    traces: [
      {
        name: "reject invalid opcode",
        transactions: [],
        failed_messages: [
          {
            error: "External message was not accepted",
            vm_exit_code: 35,
            executor_logs: "vm rejected external message",
          },
        ],
      },
    ],
    contracts: [],
    wallets: {},
  },
}

export const contracts: Record<string, BackendContractInfo> = {}

export const fileContents: Record<string, string> = {
  [`${projectRoot}/tests/counter.spec.tolk`]: `import "@stdlib/testing"

contract Counter {
  storage { value: Int as uint32 }

  fun getValue(): Int {
    return self.value
  }
}

test("counter increments") {
  expect(true).toBe(true)
}

test("rejects invalid opcode") {
  throwUnless(35, false)
}
`,
}

export const config: {project_root: string} = {
  project_root: projectRoot,
}
