export interface DisasmResult {
  readonly disasm: string
  readonly isEmptyCell: boolean
}

export async function disassembleBocHex(bocHex: string): Promise<DisasmResult> {
  const normalizedHex = bocHex.trim()
  if (normalizedHex.length === 0) {
    throw new Error("Empty code BOC")
  }

  const {Cell, runtime, text} = await import("ton-assembly")
  const cell = Cell.fromHex(normalizedHex)
  const isEmptyCell = cell.bits.length === 0 && cell.refs.length === 0

  return {
    disasm: text.print(runtime.decompileCell(cell)),
    isEmptyCell,
  }
}
