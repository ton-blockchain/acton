import {describe, expect, it} from "vitest"

import {classifyLanguageServerProbeFailure} from "./language-server-startup"

describe("language server startup", () => {
  it("recognizes a missing executable", () => {
    expect(classifyLanguageServerProbeFailure("ENOENT", null, "")).toBe("missing")
  })

  it("recognizes an executable that cannot be launched", () => {
    expect(classifyLanguageServerProbeFailure("EACCES", null, "")).toBe("unavailable")
    expect(classifyLanguageServerProbeFailure("ENOEXEC", null, "")).toBe("unavailable")
    expect(classifyLanguageServerProbeFailure("EPERM", null, "")).toBe("unavailable")
  })

  it("recognizes an unsupported language server flag", () => {
    expect(
      classifyLanguageServerProbeFailure(
        undefined,
        2,
        "error: unexpected argument '--stdlib-path' found",
      ),
    ).toBe("unsupported")
  })

  it("recognizes a missing ls subcommand", () => {
    expect(
      classifyLanguageServerProbeFailure(undefined, 2, "error: unrecognized subcommand 'ls'"),
    ).toBe("unsupported")
  })

  it("recognizes clap errors from older versions", () => {
    expect(
      classifyLanguageServerProbeFailure(
        undefined,
        2,
        "Found argument '--stdlib-path' which wasn't expected",
      ),
    ).toBe("unsupported")
  })

  it("ignores error-like output after a successful probe", () => {
    expect(
      classifyLanguageServerProbeFailure(undefined, 0, "unexpected argument in an example"),
    ).toBe(undefined)
  })

  it("does not replace unrelated startup errors with an update prompt", () => {
    expect(classifyLanguageServerProbeFailure(undefined, 1, "failed to load project")).toBe(
      undefined,
    )
  })
})
