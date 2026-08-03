import {describe, expect, test} from "bun:test"
import {Address, beginCell, external, storeMessage} from "@ton/core"

import {parseRawMessageBoc} from "../src/retrace/txTrace/lib/emulateRawMessage"

const DESTINATION = Address.parse("EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c")

describe("raw message BoC parser", () => {
  test("accepts equivalent hex, 0x-prefixed hex, and base64 messages", () => {
    const message = beginCell()
      .store(
        storeMessage(external({to: DESTINATION, body: beginCell().storeUint(0xab, 8).endCell()})),
      )
      .endCell()
    const hex = message.toBoc().toString("hex")
    const base64 = message.toBoc().toString("base64")

    expect({
      hex: parseRawMessageBoc(hex).equals(message),
      prefixedHex: parseRawMessageBoc(`0x${hex}`).equals(message),
      base64: parseRawMessageBoc(base64).equals(message),
    }).toMatchSnapshot()
  })

  test("rejects empty, malformed, and non-message cells with useful errors", () => {
    const nonMessageBoc = beginCell().storeUint(1, 1).endCell().toBoc().toString("hex")

    expect(() => parseRawMessageBoc(" ")).toThrow("Message BOC cannot be empty")
    expect(() => parseRawMessageBoc("not a BOC")).toThrow(
      "Message BOC must be encoded as hex or base64",
    )
    expect(() => parseRawMessageBoc(nonMessageBoc)).toThrow()
  })
})
