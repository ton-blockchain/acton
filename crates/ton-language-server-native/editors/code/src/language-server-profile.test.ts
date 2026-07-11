import {describe, expect, it} from "vitest"

import {renderLanguageServerProfile} from "./language-server-profile"

describe("language server profile", () => {
  it("renders counters and timings as aligned tables", () => {
    expect(
      renderLanguageServerProfile({
        enabled: true,
        counters: {
          "document.change": 15,
          "document.open": 1,
        },
        spans: {
          definition: {count: 10, totalMs: 0.6, averageMs: 0.06},
          "tolk.type_inference": {count: 16, totalMs: 431, averageMs: 26.9375},
        },
      }),
    ).toMatchInlineSnapshot(`
      "Acton Language Server Profile
      Status: enabled

      Counters
        Counter          Count
        document.change  15
        document.open    1

      Spans
        Span                 Count      Total   Average
        definition              10    0.600ms   0.060ms
        tolk.type_inference     16  431.000ms  26.938ms
      "
    `)
  })

  it("explains how to enable an inactive profiler", () => {
    expect(
      renderLanguageServerProfile({enabled: false, counters: {}, spans: {}}),
    ).toMatchInlineSnapshot(`
      "Acton Language Server Profile
      Status: disabled

      Enable profiling and restart the server.
      "
    `)
  })
})
