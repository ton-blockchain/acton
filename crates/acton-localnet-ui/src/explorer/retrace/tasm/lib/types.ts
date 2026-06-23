import type {
  FiftInstruction,
  Instruction,
  Specification,
} from "@retrace/spec/specification-schema"
import tvmSpecData from "@retrace/spec/gen/tvm-specification.json"

export interface AsmInstruction {
  readonly name: string
  readonly instruction: Instruction
  readonly fiftInstruction?: FiftInstruction
}

export function instructionSpecification(): Specification {
  return tvmSpecData as unknown as Specification
}

export function findInstruction(name: string): AsmInstruction | undefined {
  const data = instructionSpecification()

  const instruction = data?.instructions.find(i => i.name === name)
  if (instruction) {
    return {name, instruction}
  }

  const fiftInstruction = data?.fift_instructions.find(i => i.name === name)
  if (fiftInstruction) {
    const instruction = data?.instructions.find(i => i.name === fiftInstruction.actual_name)
    if (instruction) {
      return {name, instruction, fiftInstruction}
    }
  }

  return undefined
}
