import {describe, expect, test} from "bun:test"
import {generateCode} from "@ton-community/tlb-codegen"
import {createHash} from "node:crypto"
import {readFileSync} from "node:fs"

import editorSchema from "../src/config/editor/schema.generated.json"
import {configTlbSource, getConfigParameterTlb, type ConfigTlbCatalog} from "../src/config/tlb"
import generated from "../src/config/tlb.generated.json"
import parameterIds from "../src/config/tlb.parameters.generated.json"
import pinnedSource from "../src/cell-inspector/block.tlb.source.json"

const catalog: ConfigTlbCatalog = generated

describe("configuration TL-B catalog", () => {
  test("uses the decoder's pinned source and covers every editor parameter", () => {
    const source = readFileSync(new URL("../src/cell-inspector/block.tlb", import.meta.url), "utf8")
    const decoder = readFileSync(
      new URL("../src/cell-inspector/block.tlb.generated.ts", import.meta.url),
      "utf8",
    )

    expect({
      sourceMatches: createHash("sha256").update(source).digest("hex") === configTlbSource.sha256,
      manifestMatches:
        pinnedSource.revision === configTlbSource.revision &&
        pinnedSource.sha256 === configTlbSource.sha256,
      indexMatches:
        JSON.stringify(parameterIds) ===
        JSON.stringify(Object.keys(catalog.parameters).map(Number)),
      decoderRevisionMatches: decoder.includes(`ton-blockchain/ton@${configTlbSource.revision}/`),
      parametersMatch:
        JSON.stringify(Object.keys(catalog.parameters)) ===
        JSON.stringify(Object.keys(editorSchema.parameters)),
      sourceTokensPreserved: Object.values(catalog.declarations).every(declaration =>
        source.replace(/\s/g, "").includes(declaration.replace(/\s/g, "")),
      ),
      sourceLocationsMatch: Object.values(catalog.parameters).every(parameter => {
        const root = parameter.roots[0]
        const declaration = root === undefined ? undefined : catalog.declarations[root]
        return (
          Number.isInteger(parameter.line) &&
          parameter.line > 0 &&
          declaration !== undefined &&
          source
            .split("\n")
            .slice(parameter.line - 1)
            .join("\n")
            .replace(/\s/g, "")
            .startsWith(declaration.replace(/\s/g, ""))
        )
      }),
      unknown: getConfigParameterTlb(9999),
      negative: getConfigParameterTlb(-123),
    }).toMatchInlineSnapshot(`
      {
        "decoderRevisionMatches": true,
        "indexMatches": true,
        "manifestMatches": true,
        "negative": undefined,
        "parametersMatch": true,
        "sourceLocationsMatch": true,
        "sourceMatches": true,
        "sourceTokensPreserved": true,
        "unknown": undefined,
      }
    `)
  })

  test("every displayed schema compiles independently with all dependencies", () => {
    for (const id of Object.keys(catalog.parameters)) {
      const source = getConfigParameterTlb(Number(id))
      if (!source) throw new Error(`Missing TL-B for ConfigParam ${id}`)

      generateCode(`${source.declaration}\n\n${source.dependencies}`, "typescript")
    }
  })

  test("keeps root declarations first, exact tags and all recursive constructors", () => {
    for (const id of [0, 7, 9, 11, 12, 17, 20, 31, 34, 46]) {
      const source = getConfigParameterTlb(id)

      expect(source?.declaration).toMatchSnapshot(`ConfigParam ${id}`)
      expect(source?.dependencies).toMatchSnapshot(`ConfigParam ${id} additional types`)
    }
  })
})
