import {describe, expect, it} from "vitest"

import {
  parseTomlAssignmentKey,
  parseStringTomlTable,
  parseTomlTableHeaderPath,
  parseTopLevelTomlTableKeys,
} from "./acton-toml"

describe("Acton TOML helpers", () => {
  it("parses top-level table keys with quoted names", () => {
    const content = `
[contracts.Counter]
src = "contracts/counter.tolk"

[contracts."Jetton.Minter"]
src = "contracts/jetton-minter.tolk"

[contracts."Jetton.Minter".test]
skip = true
`

    expect(parseTopLevelTomlTableKeys(content, "contracts")).toEqual(["Counter", "Jetton.Minter"])
  })

  it("parses string tables with inline comments", () => {
    const content = `
[import-mappings] # aliases
"@contracts" = "contracts" # main contracts
libs = "libs#core"
`

    expect([...parseStringTomlTable(content, "import-mappings")]).toEqual([
      ["@contracts", "contracts"],
      ["libs", "libs#core"],
    ])
  })

  it("parses table header paths with comments and quoted segments", () => {
    expect(parseTomlTableHeaderPath('[contracts."Jetton.Minter"] # contract')).toEqual([
      "contracts",
      "Jetton.Minter",
    ])
    expect(parseTomlTableHeaderPath("[wrappers.typescript]")).toEqual(["wrappers", "typescript"])
    expect(parseTomlTableHeaderPath('script = "build"')).toBeNull()
  })

  it("parses assignment keys with quoted names", () => {
    expect(parseTomlAssignmentKey('  "deploy.main" = "acton run deploy" # script')).toEqual({
      key: "deploy.main",
      start: 2,
      end: 15,
    })
    expect(parseTomlAssignmentKey('value = "a=b"')).toEqual({
      key: "value",
      start: 0,
      end: 5,
    })
    expect(parseTomlAssignmentKey("# value = ignored")).toBeNull()
  })
})
