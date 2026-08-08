import {readFileSync} from "node:fs"
import * as path from "node:path"

import {describe, expect, it} from "vitest"
import process from "node:process"

interface GrammarPattern {
  readonly name?: string
  readonly match?: string
  readonly captures?: Record<string, GrammarPattern>
  readonly begin?: string
  readonly beginCaptures?: Record<string, GrammarPattern>
  readonly end?: string
  readonly patterns?: readonly GrammarPattern[]
}

interface TolkGrammar {
  readonly patterns: readonly GrammarPattern[]
}

describe("Tolk TextMate grammar", () => {
  it("scopes integer literals with compiler-compatible separators", () => {
    const grammar = readTolkGrammar()
    const numericPattern = grammar.patterns.find(pattern => pattern.name === "constant.numeric")
    const numericRegex = new RegExp(numericPattern?.match ?? "", "u")

    expect(numericRegex.exec("100_000")?.[0]).toBe("100_000")
    expect(numericRegex.exec("0xFF_FF")?.[0]).toBe("0xFF_FF")
    expect(numericRegex.exec("0b1010_0011")?.[0]).toBe("0b1010_0011")
    expect(numericRegex.exec("123_")?.[0]).toBe("123_")
    expect(numericRegex.exec("0b0_____1")?.[0]).toBe("0b0_____1")
    expect(numericRegex.exec("0b_0____1")?.[0]).toBe("0b_0____1")
    expect(numericRegex.exec("0x")?.[0]).toBe("0x")
    expect(numericRegex.exec("0b")?.[0]).toBe("0b")
    expect(numericRegex.exec("0x_FF")?.[0]).toBe("0x_FF")
    expect(numericRegex.exec("0b_")?.[0]).toBe("0b_")

    expect(numericRegex.exec("_100")).toBeNull()
  })

  it("scopes only ordinary string escapes accepted by the compiler", () => {
    const grammar = readTolkGrammar()
    const stringPattern = grammar.patterns.find(
      pattern => pattern.name === "string.quoted.double.tolk",
    )
    const escapePattern = stringPattern?.patterns?.find(
      pattern => pattern.name === "constant.character.escape.tolk",
    )
    const escapeRegex = new RegExp(escapePattern?.match ?? "", "u")

    for (const validEscape of ["\\n", "\\r", "\\t", "\\\\", "\\'", '\\"']) {
      expect(escapeRegex.exec(validEscape)?.[0]).toBe(validEscape)
    }

    expect(escapeRegex.exec("\\0")).toBeNull()
    expect(escapeRegex.exec("\\u")).toBeNull()
    expect(escapeRegex.exec("\\u1234")).toBeNull()
  })

  it("keeps triple-quoted string escapes broad for asm strings", () => {
    const grammar = readTolkGrammar()
    const stringPattern = grammar.patterns.find(
      pattern => pattern.name === "string.quoted.triple.tolk",
    )
    const escapePattern = stringPattern?.patterns?.find(
      pattern => pattern.name === "constant.character.escape.tolk",
    )
    const escapeRegex = new RegExp(escapePattern?.match ?? "", "u")

    for (const escapedText of ["\\n", "\\0", "\\u", "\\u1234", "\\x"]) {
      expect(escapeRegex.exec(escapedText)?.[0]).toBe(escapedText.slice(0, 2))
    }
  })

  it("scopes builtin and stdlib type names used by the compiler", () => {
    const grammar = readTolkGrammar()
    const typePatterns = grammar.patterns.filter(pattern => pattern.name === "storage.type")
    const typeRegexes = typePatterns.map(pattern => new RegExp(pattern.match ?? "", "u"))

    for (const typeName of [
      "int",
      "bool",
      "cell",
      "slice",
      "builder",
      "continuation",
      "string",
      "address",
      "any_address",
      "coins",
      "never",
      "void",
      "unknown",
      "array",
      "map",
      "dict",
      "tuple",
      "Cell",
      "int257",
      "uint256",
      "bits1023",
      "bytes1023",
      "varint16",
      "varint32",
      "varuint16",
      "varuint32",
    ]) {
      expect(typeRegexes.some(regex => regex.exec(typeName)?.[0] === typeName)).toBe(true)
    }

    expect(typeRegexes.some(regex => regex.exec("varint8")?.[0] === "varint8")).toBe(false)
    expect(typeRegexes.some(regex => regex.exec("varuint64")?.[0] === "varuint64")).toBe(false)
  })

  it("does not scope identifiers after dots as types", () => {
    const grammar = readTolkGrammar()
    const storageTypeRegexes = grammar.patterns
      .filter(pattern => pattern.name === "storage.type")
      .map(pattern => new RegExp(pattern.match ?? "", "u"))
    const typeParameterPattern = grammar.patterns.find(
      pattern => pattern.name === "entity.name.type.parameter",
    )
    const nominalTypePattern = grammar.patterns.find(pattern => pattern.name === "entity.name.type")
    const typeParameterRegex = new RegExp(typeParameterPattern?.match ?? "", "u")
    const nominalTypeRegex = new RegExp(nominalTypePattern?.match ?? "", "u")

    expect(storageTypeRegexes.some(regex => regex.exec(".int")?.[0] === "int")).toBe(false)
    expect(storageTypeRegexes.some(regex => regex.exec(".address")?.[0] === "address")).toBe(false)
    expect(storageTypeRegexes.some(regex => regex.exec(".Cell")?.[0] === "Cell")).toBe(false)
    expect(typeParameterRegex.exec(".T")).toBeNull()
    expect(nominalTypeRegex.exec(".SendResultList")).toBeNull()

    expect(storageTypeRegexes.some(regex => regex.exec("int")?.[0] === "int")).toBe(true)
    expect(typeParameterRegex.exec("T")?.[0]).toBe("T")
    expect(nominalTypeRegex.exec("SendResultList")?.[0]).toBe("SendResultList")
  })

  it("scopes chained method calls as functions", () => {
    const grammar = readTolkGrammar()
    const functionPattern = grammar.patterns.find(
      pattern => pattern.captures?.["2"]?.name === "entity.name.function",
    )
    const functionRegex = new RegExp(functionPattern?.match ?? "", "u")
    const scopedFunctionName = (source: string): string | null =>
      functionRegex.exec(source)?.[2] ?? null

    expect(functionPattern?.captures?.["1"]?.name).toBe("keyword.operator.accessor")
    expect(scopedFunctionName(".toHaveSuccessfulTx<ProcessGovernanceDecision>(")).toBe(
      "toHaveSuccessfulTx",
    )
    expect(scopedFunctionName(".bar()")).toBe("bar")
    expect(scopedFunctionName(".bar<A>()")).toBe("bar")
    expect(scopedFunctionName("bar()")).toBe("bar")

    expect(scopedFunctionName(".bar")).toBeNull()
    expect(scopedFunctionName(".bar<A>")).toBeNull()
  })

  it("scopes dotted property accesses after method calls", () => {
    const grammar = readTolkGrammar()
    const functionPatternIndex = grammar.patterns.findIndex(
      pattern => pattern.captures?.["2"]?.name === "entity.name.function",
    )
    const propertyPatternIndex = grammar.patterns.findIndex(
      pattern => pattern.captures?.["2"]?.name === "variable.other.property",
    )
    const propertyPattern = grammar.patterns[propertyPatternIndex]
    const propertyRegex = new RegExp(propertyPattern.match ?? "", "u")
    const scopedPropertyName = (source: string): string | null =>
      propertyRegex.exec(source)?.[2] ?? null

    expect(functionPatternIndex).toBeGreaterThanOrEqual(0)
    expect(propertyPatternIndex).toBeGreaterThan(functionPatternIndex)
    expect(propertyPattern.captures?.["1"]?.name).toBe("keyword.operator.accessor")
    expect(scopedPropertyName("deployer.address")).toBe("address")
    expect(scopedPropertyName("start.context_idx")).toBe("context_idx")
    expect(scopedPropertyName("foo.`strange-field`")).toBe("`strange-field`")

    expect(firstMatchedScope(grammar, ".bar()")).toBe("entity.name.function")
    expect(firstMatchedScope(grammar, ".bar<A>()")).toBe("entity.name.function")
    expect(firstMatchedScope(grammar, ".bar")).toBe("variable.other.property")
    expect(firstMatchedScope(grammar, ".bar<A>")).toBe("variable.other.property")
  })

  it("does not scope every identifier as a variable", () => {
    const grammar = readTolkGrammar()

    expect(grammar.patterns.some(pattern => pattern.name === "variable.name")).toBe(false)
  })

  it("scopes dotted annotation names without scoping dots", () => {
    const grammar = readTolkGrammar()
    const annotationPattern = grammar.patterns.find(pattern => pattern.begin?.startsWith("(@)"))
    const segmentPattern = annotationPattern?.patterns?.[0]

    expect(annotationPattern?.begin).toBe("(@)(?=[A-Za-z0-9_])")
    expect(annotationPattern?.beginCaptures?.["1"]?.name).toBe("entity.name.function.decorator")
    expect(annotationPattern?.end).toBe("(?![A-Za-z0-9_.])")
    expect(segmentPattern?.name).toBe("entity.name.function.decorator")
    expect(segmentPattern?.match).toBe("[A-Za-z0-9_]+")

    const segmentRegex = new RegExp(segmentPattern?.match ?? "", "u")
    expect(segmentRegex.exec("abi")?.[0]).toBe("abi")
    expect(segmentRegex.exec("clientType")?.[0]).toBe("clientType")
    expect(segmentRegex.exec(".")).toBeNull()
  })
})

function readTolkGrammar(): TolkGrammar {
  const grammarPath = path.join(process.cwd(), "syntaxes/tolk.tmLanguage.json")
  return JSON.parse(readFileSync(grammarPath, "utf8")) as TolkGrammar
}

function firstMatchedScope(grammar: TolkGrammar, source: string): string | undefined {
  return grammar.patterns
    .map((pattern, index) => {
      const scope = pattern.captures?.["2"]?.name
      if (
        pattern.match === undefined ||
        (scope !== "entity.name.function" && scope !== "variable.other.property")
      ) {
        return undefined
      }

      const match = new RegExp(pattern.match, "u").exec(source)
      if (match === null) {
        return undefined
      }

      return {index, match, pattern}
    })
    .filter(result => result !== undefined)
    .sort((left, right) => left.match.index - right.match.index || left.index - right.index)[0]
    ?.pattern.captures?.["2"]?.name
}
