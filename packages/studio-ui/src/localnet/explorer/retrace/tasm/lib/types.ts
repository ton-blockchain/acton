import tvmSpecData from "@tasm-spec/tvm-specification.json"

import type {FiftInstruction, Instruction, Specification} from "./specification"

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
