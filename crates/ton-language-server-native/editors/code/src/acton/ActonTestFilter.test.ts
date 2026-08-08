import {describe, expect, it} from "vitest"

import {
  CompileCommand,
  DisasmCommand,
  InitCommand,
  ScriptCommand,
  TestCommand,
  TestMode,
  UpdateCommand,
} from "./ActonCommand"
import {createActonTestFilterPattern, escapeRustRegexLiteral} from "./ActonTestFilter"

describe("Acton test filtering", () => {
  it("escapes Rust regex metacharacters in test names", () => {
    expect(escapeRustRegexLiteral("test teamcity '|[]")).toBe("test teamcity '\\|\\[\\]")
    expect(escapeRustRegexLiteral("test .+*?(){}^$\\")).toBe(
      "test \\.\\+\\*\\?\\(\\)\\{\\}\\^\\$\\\\",
    )
  })

  it("builds an exact filter for one test", () => {
    expect(createActonTestFilterPattern(["test transfer"])).toBe("^test transfer$")
  })

  it("builds a narrowed alternation filter for multiple tests", () => {
    expect(createActonTestFilterPattern(["test alpha", "test beta"])).toBe(
      "^(?:test alpha|test beta)$",
    )
  })

  it("deduplicates selected test names", () => {
    expect(createActonTestFilterPattern(["test alpha", "test alpha"])).toBe("^test alpha$")
  })

  it("passes explicit filters to directory test commands", () => {
    const command = new TestCommand(TestMode.DIRECTORY, "tests")
    command.filterPattern = "^(?:test alpha|test beta)$"

    expect(command.getArguments()).toContain("--filter")
    expect(command.getArguments()).toEqual([
      "--color",
      "always",
      "--reporter",
      "console,teamcity",
      "--filter",
      "^(?:test alpha|test beta)$",
      "tests",
    ])
  })

  it("uses exact filter pattern instead of function name when provided", () => {
    const command = new TestCommand(
      TestMode.FUNCTION,
      "tests/counter.test.tolk",
      "test counter",
      false,
      false,
      "lcov",
      "",
      false,
      "",
      false,
      "",
      "console,teamcity",
      "^test counter$",
    )

    expect(command.getArguments()).toEqual([
      "--color",
      "always",
      "--reporter",
      "console,teamcity",
      "--filter",
      "^test counter$",
      "tests/counter.test.tolk",
    ])
  })

  it("adds full backtrace while preserving the exact filter", () => {
    const command = new TestCommand(TestMode.DIRECTORY, "tests")
    command.filterPattern = "^test alpha$"
    command.backtraceFull = true

    expect(command.getArguments()).toEqual([
      "--color",
      "always",
      "--reporter",
      "console,teamcity",
      "--backtrace",
      "full",
      "--filter",
      "^test alpha$",
      "tests",
    ])
  })

  it("adds full backtrace to script commands before the script path", () => {
    const command = new ScriptCommand("scripts/deploy.tolk")
    command.backtraceFull = true

    expect(command.getArguments()).toEqual([
      "--color",
      "always",
      "--backtrace",
      "full",
      "scripts/deploy.tolk",
    ])
  })

  it("builds JSON disasm commands from inline BoC with source maps", () => {
    const command = new DisasmCommand("", "te6ccgEBAQEAAgAAAA==")
    command.json = true
    command.sourceMapFile = "/tmp/counter.source-map.json"

    expect(command.getArguments()).toEqual([
      "--color",
      "never",
      "--json",
      "--source-map",
      "/tmp/counter.source-map.json",
      "--string",
      "te6ccgEBAQEAAgAAAA==",
    ])
  })

  it("builds JSON compile commands with source maps", () => {
    const command = new CompileCommand("contracts/counter.tolk")
    command.json = true
    command.sourceMapFile = "/tmp/counter.source-map.json"
    command.allowNoEntrypoint = true

    expect(command.getArguments()).toEqual([
      "--color",
      "never",
      "--json",
      "--source-map",
      "/tmp/counter.source-map.json",
      "--allow-no-entrypoint",
      "contracts/counter.tolk",
    ])
  })

  it("builds dApp init commands with default scaffold path", () => {
    expect(new InitCommand(true).getArguments()).toEqual(["--create-dapp"])
  })

  it("builds dApp init commands with custom scaffold path", () => {
    expect(new InitCommand(true, "frontend").getArguments()).toEqual(["--create-dapp", "frontend"])
  })

  it("builds JSON update check commands without colored output", () => {
    expect(new UpdateCommand(true).getArguments()).toEqual(["--color", "never", "--check"])
  })

  it("builds interactive update commands without confirmation flags", () => {
    expect(new UpdateCommand().getArguments()).toEqual([])
  })
})
