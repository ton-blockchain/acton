import {Cell, type TupleItem} from "@ton/core"

import {ActonError} from "./errors.js"
import {isRecord} from "./utils.js"

export function tupleItemToLegacyJson(item: TupleItem): unknown {
  switch (item.type) {
    case "null": {
      return ["null", null]
    }
    case "int": {
      return ["num", item.value.toString()]
    }
    case "cell": {
      return ["cell", {bytes: item.cell.toBoc().toString("base64")}]
    }
    case "slice": {
      return ["slice", {bytes: item.cell.toBoc().toString("base64")}]
    }
    case "builder": {
      return ["builder", {bytes: item.cell.toBoc().toString("base64")}]
    }
    case "tuple": {
      return ["tuple", {elements: item.items.map(tupleItem => tupleItemToLegacyJson(tupleItem))}]
    }
    case "nan": {
      throw new ActonError("NaN tuple items are not supported by localnet JSON stack")
    }
  }
}

export function legacyJsonToTupleItem(value: unknown): TupleItem {
  if (!Array.isArray(value) || value.length !== 2 || typeof value[0] !== "string") {
    throw new ActonError("Invalid localnet legacy stack item")
  }

  const entry = value as unknown as readonly [string, unknown]
  const kind = entry[0]
  const payload = entry[1]
  switch (kind) {
    case "null": {
      return {type: "null"}
    }
    case "num": {
      return {type: "int", value: parseStackNumber(payload)}
    }
    case "cell": {
      return {type: "cell", cell: cellFromBytesPayload(payload, "cell")}
    }
    case "slice": {
      return {type: "slice", cell: cellFromBytesPayload(payload, "slice")}
    }
    case "builder": {
      return {type: "builder", cell: cellFromBytesPayload(payload, "builder")}
    }
    case "tuple":
    case "list": {
      return {type: "tuple", items: tupleItemsFromPayload(payload)}
    }
    case "cont": {
      return {type: "slice", cell: cellFromBytesPayload(payload, "cont")}
    }
    default: {
      throw new ActonError(`Unsupported localnet legacy stack item type: ${kind}`)
    }
  }
}

function parseStackNumber(value: unknown): bigint {
  if (typeof value === "number") {
    return BigInt(value)
  }
  if (typeof value !== "string") {
    throw new ActonError("Stack number must be a string or number")
  }
  if (value.startsWith("-0x") || value.startsWith("-0X")) {
    return -BigInt(`0x${value.slice(3)}`)
  }
  return BigInt(value)
}

function cellFromBytesPayload(payload: unknown, kind: string): Cell {
  if (!isRecord(payload) || typeof payload.bytes !== "string") {
    throw new ActonError(`Stack ${kind} item must contain base64 bytes`)
  }
  return Cell.fromBase64(payload.bytes)
}

function tupleItemsFromPayload(payload: unknown): TupleItem[] {
  if (!isRecord(payload) || !Array.isArray(payload.elements)) {
    throw new ActonError("Stack tuple item must contain elements")
  }
  return payload.elements.map(item => legacyJsonToTupleItem(item))
}
