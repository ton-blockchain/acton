import {describe, expect, test} from "bun:test"
import {createElement} from "react"
import {renderToStaticMarkup} from "react-dom/server"

import {buildStorageDiff} from "../src/components/ParsedValueDiffView/buildStorageDiff"
import {ParsedValueView} from "../src/components/ParsedValueView/ParsedValueView"

describe("ParsedValueView", () => {
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
