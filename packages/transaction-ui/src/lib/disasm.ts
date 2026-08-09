export interface DisasmResult {
  readonly disasm: string
  readonly isEmptyCell: boolean
  readonly isEmbeddedData: boolean
}

export async function disassembleBocHex(bocHex: string): Promise<DisasmResult> {
  const normalizedHex = bocHex.trim()
  if (normalizedHex.length === 0) {
    throw new Error("Empty code BOC")
  }

  const {Cell, runtime, text} = await import("@ton/tasm")
  const cell = Cell.fromHex(normalizedHex)
  const isEmptyCell = cell.bits.length === 0 && cell.refs.length === 0
  const instructions = runtime.decompileCell(cell)

  return {
    disasm: text.print(instructions),
    isEmptyCell,
    isEmbeddedData: instructions.length === 1 && instructions[0]?.$ === "PSEUDO_PUSHSLICE",
  }
}
