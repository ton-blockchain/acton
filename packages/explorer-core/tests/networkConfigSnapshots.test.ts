import {describe, expect, test} from "bun:test"
import {readFile} from "node:fs/promises"

import {parseNetworkConfig, type NetworkConfig} from "../src/api/config"

describe("real network configuration snapshots", () => {
  for (const fixture of HISTORICAL_MAINNET_CONFIG_FIXTURES) {
    test(`parses mainnet config with global version ${fixture.version} at masterchain block ${fixture.seqno}`, async () => {
      const rawBoc = await readNetworkConfigFixture("mainnet", fixture.seqno)
      const config = parseNetworkConfig(rawBoc)

      expect(readGlobalVersion(config)).toEqual({
        version: fixture.version,
        capabilities: fixture.capabilities,
      })
      expect(toNetworkConfigSnapshot(config)).toMatchSnapshot()
    })
  }

  test("parses mainnet config at masterchain block 84773657", async () => {
    const rawBoc = await readNetworkConfigFixture("mainnet", 84_773_657)
    const config = parseNetworkConfig(rawBoc)

    expect(readGlobalVersion(config)).toEqual({version: 15, capabilities: 0x3een})
    expect(toNetworkConfigSnapshot(config)).toMatchSnapshot()
  })

  test("parses testnet config at masterchain block 80890417", async () => {
    const rawBoc = await readNetworkConfigFixture("testnet", 80_890_417)

    expect(toNetworkConfigSnapshot(parseNetworkConfig(rawBoc))).toMatchSnapshot()
  })
})

const HISTORICAL_MAINNET_CONFIG_FIXTURES = [
  {seqno: 1, version: 0, capabilities: 0x002n},
  {seqno: 2_908_199, version: 1, capabilities: 0x00en},
  {seqno: 3_127_942, version: 2, capabilities: 0x02en},
  // Global version 3 was never activated on mainnet.
  {seqno: 34_875_663, version: 4, capabilities: 0x02en},
  {seqno: 35_865_387, version: 5, capabilities: 0x02en},
  {seqno: 36_746_858, version: 6, capabilities: 0x02en},
  {seqno: 37_375_729, version: 7, capabilities: 0x02en},
  {seqno: 39_939_169, version: 8, capabilities: 0x1een},
  {seqno: 44_891_369, version: 9, capabilities: 0x1een},
  {seqno: 47_557_457, version: 10, capabilities: 0x1een},
  {seqno: 49_524_026, version: 11, capabilities: 0x1een},
  {seqno: 53_939_517, version: 12, capabilities: 0x1een},
  {seqno: 59_015_496, version: 13, capabilities: 0x1een},
  {seqno: 71_304_031, version: 14, capabilities: 0x3een},
] as const

async function readNetworkConfigFixture(
  network: "mainnet" | "testnet",
  seqno: number,
): Promise<string> {
  const url = new URL(`./fixtures/${network}-config-${seqno}.boc.base64`, import.meta.url)
  return (await readFile(url, "utf8")).trim()
}

function readGlobalVersion(config: NetworkConfig) {
  return config.parameters.find(parameter => parameter.id === 8)?.globalVersion
}

function toNetworkConfigSnapshot(config: NetworkConfig) {
  return {
    address: config.configAddress,
    parameters: config.parameters.map(({parsedValue, rawHex, ...parameter}) => ({
      ...parameter,
      hasParsedValue: parsedValue !== undefined,
      hasRawHex: rawHex.length > 0,
    })),
    hasRootRawHex: config.rawHex.length > 0,
  }
}
