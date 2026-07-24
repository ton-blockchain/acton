import {CellType, type Cell} from "@ton/core"

export function codeLookupHashHex(code: Cell): string {
  if (code.type === CellType.Library) {
    const slice = code.beginParse(true)
    slice.skip(8)
    return slice.loadBuffer(32).toString("hex")
  }

  return code.hash().toString("hex")
}
