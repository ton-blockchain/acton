import {describe, expect, test} from "bun:test"
import {createElement} from "react"
import {renderToStaticMarkup} from "react-dom/server"

import {buildStorageDiff} from "../src/components/ParsedValueDiffView/buildStorageDiff"
import {ParsedValueView} from "../src/components/ParsedValueView/ParsedValueView"
import {ThemeProvider} from "../src/components/Theme/ThemeProvider"

describe("ParsedValueView", () => {
  test("renders Cell Inspector actions for parsed cells and slices", () => {
    const cellMarkup = renderToStaticMarkup(
      createElement(ParsedValueView, {
        value: {
          kind: "scalar",
          value: "Cell 012345…abcdef",
          rawValue: "b5ee9c72",
          typeName: "Cell",
        },
        onCellInspect: () => undefined,
      }),
    )
    const sliceMarkup = renderToStaticMarkup(
      createElement(ParsedValueView, {
        value: {
          kind: "scalar",
          value: "Slice 012345…abcdef",
          rawValue: "b5ee9c72",
          typeName: "Slice",
        },
        onCellInspect: () => undefined,
      }),
    )

    expect(cellMarkup).toContain('aria-label="Inspect cell"')
    expect(sliceMarkup).toContain('aria-label="Inspect slice"')
  })

  test("renders only uint256 map keys as hexadecimal", () => {
    const markup = renderToStaticMarkup(
      createElement(ParsedValueView, {
        value: {
          kind: "map",
          entries: [
            {
              key: {kind: "scalar", value: "255", typeName: "uint256"},
              value: {kind: "scalar", value: "1"},
            },
            {
              key: {kind: "scalar", value: "255", typeName: "uint32"},
              value: {kind: "scalar", value: "2"},
            },
          ],
        },
      }),
    )

    expect(markup).toMatchInlineSnapshot(
      `"<div><span>map</span><div class="undefined  "><div><div><div>Key</div><div><span>0xff</span></div></div><div><div>Value</div><div><span>1</span></div></div></div><div><div><div>Key</div><div><span>255</span></div></div><div><div>Value</div><div><span>2</span></div></div></div></div></div>"`,
    )
  })

  test("renders information controls for annotated map keys", () => {
    const markup = renderToStaticMarkup(
      createElement(
        ThemeProvider,
        {defaultTheme: "light"},
        createElement(ParsedValueView, {
          value: {
            kind: "map",
            entries: [
              {
                key: {kind: "scalar", value: "Max leader-window desync"},
                keyInfo: {
                  id: 10,
                  description:
                    "Maximum tolerated future leader-window distance for inbound Simplex traffic.",
                },
                value: {kind: "scalar", value: "64"},
              },
              {
                key: {kind: "scalar", value: "Certificate gossip neighbors"},
                keyInfo: {id: 15},
                value: {kind: "scalar", value: "20"},
              },
            ],
          },
        }),
      ),
    )

    expect(markup).toContain('aria-label="About parameter ID 10"')
    expect(markup).toContain('aria-label="About parameter ID 15"')
  })

  test("keeps hexadecimal uint256 keys in storage diffs", () => {
    const value = {
      kind: "map" as const,
      typeName: "map<uint256, bool>",
      entries: [
        {
          key: {kind: "scalar" as const, value: "255", typeName: "uint256"},
          value: {kind: "boolean" as const, value: true},
        },
      ],
    }

    expect(
      buildStorageDiff(
        {name: "Permissions", value},
        {
          name: "Permissions",
          value: {
            ...value,
            entries: [{...value.entries[0], value: {kind: "boolean", value: false}}],
          },
        },
      ),
    ).toMatchInlineSnapshot(`
      {
        "entries": [
          {
            "key": "0xff",
            "value": {
              "after": {
                "kind": "boolean",
                "value": false,
              },
              "before": {
                "kind": "boolean",
                "value": true,
              },
              "kind": "leaf",
              "status": "changed",
            },
          },
        ],
        "kind": "object",
        "objectKind": "map",
        "status": "changed",
        "typeName": "map<uint256, bool>",
      }
    `)
  })
})
