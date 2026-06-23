export interface Specification {
  readonly instructions: readonly Instruction[]
  readonly fift_instructions: readonly FiftInstruction[]
}

export interface Instruction {
  readonly name: string
  readonly description: InstructionDescription
  readonly layout: InstructionLayout
  readonly signature?: InstructionSignature
}

interface InstructionDescription {
  readonly short: string
  readonly long: string
  readonly operands: readonly string[]
  readonly gas?: readonly GasConsumptionEntry[]
}

export interface GasConsumptionEntry {
  readonly value: number
  readonly description?: string
  readonly formula?: string
}

interface InstructionLayout {
  readonly prefix_str: string
}

interface InstructionSignature {
  readonly stack_string?: string
}

export interface FiftInstruction {
  readonly name: string
  readonly actual_name: string
  readonly arguments?: readonly (number | string)[]
  readonly description?: string
}
