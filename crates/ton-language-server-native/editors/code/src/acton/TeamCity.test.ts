import {describe, expect, it} from "vitest"

import {extractTeamCityFileHint, parseTeamCityMessage} from "./TeamCity"

describe("TeamCity service message parser", () => {
  it("keeps test duration attributes for finished tests", () => {
    const message = parseTeamCityMessage(
      "##teamcity[testFinished name='test alpha' nodeId='2' duration='37']",
    )

    expect(message).toEqual({
      name: "testFinished",
      attributes: {
        name: "test alpha",
        nodeId: "2",
        duration: "37",
      },
    })
  })

  it("keeps test duration attributes for failed tests", () => {
    const message = parseTeamCityMessage(
      "##teamcity[testFailed name='test alpha' duration='41' message='boom']",
    )

    expect(message).toEqual({
      name: "testFailed",
      attributes: {
        name: "test alpha",
        duration: "41",
        message: "boom",
      },
    })
  })

  it("extracts file paths and falls back to the suite name", () => {
    expect(extractTeamCityFileHint("file:///workspace/tests/counter.tolk", "fallback.tolk")).toBe(
      "/workspace/tests/counter.tolk",
    )
    expect(extractTeamCityFileHint("not a URL", "fallback.tolk")).toBe("fallback.tolk")
  })
})
