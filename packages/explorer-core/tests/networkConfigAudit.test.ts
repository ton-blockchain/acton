import {describe, expect, test} from "bun:test"
import {readFile} from "node:fs/promises"

import {
  findConfigAdditions,
  hasConfigAdditions,
  inspectConfigBoc,
  mergeConfigManifest,
  parseArguments,
  type ConfigManifest,
} from "../scripts/check-network-config"

const configCases = [
  {network: "mainnet", seqno: 84_773_657},
  {network: "testnet", seqno: 80_890_417},
] as const

describe("network config audit", () => {
  for (const {network, seqno} of configCases) {
    test(`the pinned ${network} config is covered by its append-only manifest`, async () => {
      const [rawBoc, manifest] = await Promise.all([
        readFile(
          new URL(`./fixtures/${network}-config-${seqno}.boc.base64`, import.meta.url),
          "utf8",
        ),
        readFile(new URL(`../scripts/config-fields/${network}.json`, import.meta.url), "utf8").then(
          contents => JSON.parse(contents) as ConfigManifest,
        ),
      ])

      const additions = findConfigAdditions(manifest, inspectConfigBoc(rawBoc.trim()))
      expect(additions).toEqual({
        parameterIds: [],
        fields: {},
        parseErrors: {},
      })
      expect(hasConfigAdditions(additions)).toBe(false)
    })
  }

  test("reports only additions and ignores fields absent from the latest config", () => {
    const manifest: ConfigManifest = {
      network: "mainnet",
      parameters: {18: ["mc_cell_price_ps", "old_optional_field"]},
    }
    const additions = findConfigAdditions(manifest, [
      {id: 18, fields: ["mc_cell_price_ps", "new_field"]},
      {id: 46, fields: []},
    ])

    expect(additions).toEqual({
      parameterIds: [46],
      fields: {18: ["new_field"]},
      parseErrors: {},
    })
  })

  test("fixes additions without removing historical fields", () => {
    const manifest: ConfigManifest = {
      network: "testnet",
      parameters: {18: ["mc_cell_price_ps", "old_optional_field"]},
    }
    const parameters = [
      {id: 18, fields: ["mc_cell_price_ps", "new_field"]},
      {id: -123, fields: []},
    ]

    const fixed = mergeConfigManifest(manifest, parameters)

    expect(fixed).toEqual({
      network: "testnet",
      parameters: {
        "-123": [],
        18: ["mc_cell_price_ps", "new_field", "old_optional_field"],
      },
    })
    expect(hasConfigAdditions(findConfigAdditions(fixed, parameters))).toBe(false)
  })

  test("accepts --fix and an optional network", () => {
    expect(parseArguments(["--fix"])).toEqual({network: undefined, bocPath: undefined, fix: true})
    expect(parseArguments(["--network", "testnet", "--boc", "config.boc"])).toEqual({
      network: "testnet",
      bocPath: "config.boc",
      fix: false,
    })
    expect(() => parseArguments(["--network", "localnet"])).toThrow(
      "--network must be mainnet or testnet",
    )
    expect(() => parseArguments(["--boc", "config.boc"])).toThrow("--boc requires --network")
    expect(() => parseArguments(["--update"])).toThrow("Unknown argument: --update")
  })
})
