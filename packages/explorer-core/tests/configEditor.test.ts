import {describe, expect, test} from "bun:test"
import {readFileSync} from "node:fs"
import {Address, beginCell} from "@ton/core"

import {parseNetworkConfig} from "../src/api/config"
import {
  decodeParameter,
  encodeParameter,
  objectDraft,
  parameterShape,
  parseParameterBoc,
} from "../src/config/editor/codec"
import {
  decimalValue,
  formatConfigAddress,
  parseConfigAddress,
  scaledInteger,
} from "../src/config/editor/semantics"

const configs = ["mainnet-config-84773657", "testnet-config-80890417"].map(name => ({
  name,
  config: parseNetworkConfig(
    readFileSync(new URL(`./fixtures/${name}.boc.base64`, import.meta.url), "utf8").trim(),
  ),
}))

function parameterFromMainnet(id: number) {
  const mainnet = configs[0]
  if (!mainnet) throw new Error("Mainnet configuration fixture is missing")

  const parameter = mainnet.config.parameters.find(candidate => candidate.id === id)
  if (!parameter) throw new Error(`Mainnet configuration parameter ${id} is missing`)

  return parameter
}

describe("network configuration editor", () => {
  for (const {name, config} of configs) {
    test(`preserves every known parameter bit-for-bit in ${name}`, () => {
      const results = config.parameters
        .filter(parameter => parameterShape(parameter.id))
        .map(parameter => {
          const cell = parseParameterBoc(parameter.rawHex)
          const draft = decodeParameter(parameter.id, cell)
          const rebuilt = encodeParameter(parameter.id, draft)
          return {id: parameter.id, unchanged: cell.equals(rebuilt)}
        })
      expect(results).toMatchSnapshot()
      expect(results.every(result => result.unchanged)).toBe(true)
    })
  }

  test("edits masterchain addresses as friendly or raw addresses, not int256", () => {
    const original = parameterFromMainnet(4)
    const draft = objectDraft(decodeParameter(4, parseParameterBoc(original.rawHex)))
    const address = new Address(-1, Buffer.alloc(32, 0x42))
    draft.dns_root_addr = address.toString()
    expect(decodeParameter(4, encodeParameter(4, draft))).toMatchInlineSnapshot(`
      {
        "dns_root_addr": "-1:4242424242424242424242424242424242424242424242424242424242424242",
        "kind": "ConfigParam__4",
      }
    `)
    draft.dns_root_addr = new Address(0, Buffer.alloc(32)).toString()
    expect(() => encodeParameter(4, draft)).toThrow("workchain -1")
  })

  test("retains exact GRAM values beyond JS integer precision and rejects fractional nanograms", () => {
    const original = parameterFromMainnet(17)
    const draft = objectDraft(decodeParameter(17, parseParameterBoc(original.rawHex)))
    draft.min_stake = "9007199254740993.123456789"
    draft.max_stake = "9907199254740993.123456789"
    draft.min_total_stake = "9907199254740993.123456789"
    expect(decodeParameter(17, encodeParameter(17, draft))).toMatchSnapshot()
    draft.min_stake = "0.0000000001"
    expect(() => encodeParameter(17, draft)).toThrow("cannot be represented exactly")
  })

  test("edits signed parameter lists and detects duplicate keys before serialization", () => {
    const original = parameterFromMainnet(10)
    const draft = objectDraft(decodeParameter(10, parseParameterBoc(original.rawHex)))
    draft.critical_params = [
      {key: "-999", value: {kind: "True"}},
      {key: "0", value: {kind: "True"}},
    ]
    expect(decodeParameter(10, encodeParameter(10, draft))).toMatchSnapshot()
    draft.critical_params.push({key: "0", value: {kind: "True"}})
    expect(() => encodeParameter(10, draft)).toThrow("Duplicate entry")
  })

  test("accepts arbitrary extension cells while rejecting multiple roots and broken BoCs", () => {
    const cell = beginCell()
      .storeUint(42, 32)
      .storeRef(beginCell().storeStringTail("extension"))
      .endCell()
    expect({
      base64: parseParameterBoc(cell.toBoc().toString("base64")).equals(cell),
      hex: parseParameterBoc(cell.toBoc().toString("hex")).equals(cell),
      known: parameterShape(-123_456),
    }).toMatchInlineSnapshot(`
      {
        "base64": true,
        "hex": true,
        "known": undefined,
      }
    `)
    expect(() => parseParameterBoc("invalid")).toThrow()
  })

  test("preserves fractional fixed-point prices and full workchain address keys", () => {
    const value = 18446744073709551615n
    const decimal = decimalValue(value, 65536000000000n)
    const address = `0:${"ab".repeat(32)}`
    expect({
      decimal,
      restored: scaledInteger(decimal, 65536000000000n).toString(),
      address: formatConfigAddress(parseConfigAddress(address, true), true),
    }).toMatchSnapshot()
  })
})
