import {describe, expect, it} from "vitest"

import {stripAnsiCodes} from "./ansi"

describe("ANSI output", () => {
  it("removes terminal color sequences", () => {
    expect(stripAnsiCodes("\u001B[31merror\u001B[0m")).toBe("error")
  })
})
