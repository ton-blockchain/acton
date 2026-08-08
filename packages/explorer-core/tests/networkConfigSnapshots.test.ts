import {describe, expect, test} from "bun:test"
import {readFile} from "node:fs/promises"

import {parseNetworkConfig, type NetworkConfig} from "../src/api/config"

describe("real network configuration snapshots", () => {
  test("parses mainnet config at masterchain block 84773657", async () => {
    const rawBoc = await readNetworkConfigFixture("mainnet", 84_773_657)

    expect(toNetworkConfigSnapshot(parseNetworkConfig(rawBoc))).toMatchSnapshot()
  })

  test("parses testnet config at masterchain block 76769489", async () => {
    const rawBoc = await readNetworkConfigFixture("testnet", 76_769_489)

    expect(toNetworkConfigSnapshot(parseNetworkConfig(rawBoc))).toMatchSnapshot()
  })
})

async function readNetworkConfigFixture(
  network: "mainnet" | "testnet",
  seqno: number,
): Promise<string> {
  const url = new URL(`./fixtures/${network}-config-${seqno}.boc.base64`, import.meta.url)
  return (await readFile(url, "utf8")).trim()
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
