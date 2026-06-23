import type {GasConsumptionEntry} from "@retrace/spec/specification-schema"

import {type AsmInstruction} from "./types"

export function generateAsmDoc(instruction: AsmInstruction): string | null {
  const stackInfo = instruction.instruction.signature?.stack_string
    ? `- Stack (top is on the right): \`${instruction.instruction.signature.stack_string.replace("->", "→")}\``
    : ""

  const gas = formatGasRanges(instruction.instruction.description.gas ?? [])

  const rawShort = instruction.instruction.description.short
  const rawLong = instruction.instruction.description.long

  const short = rawShort === "" ? rawLong : rawShort
  const details = short === rawLong ? "" : rawLong
  const args = instruction.instruction.description.operands.map(it => `[${it}]`).join(" ")

  const actualInstructionDescription = [
    "```",
    instruction.name + " " + args,
    "```",
    stackInfo,
    `- Gas: \`${gas}\``,
    `- Opcode: \`${instruction.instruction.layout.prefix_str}\``,
    "",
    short,
    "",
    details ? "**Details:**\n\n" + details : "",
    "",
  ]

  if (instruction.fiftInstruction) {
    const operandsStr =
      instruction.fiftInstruction.arguments?.map(arg => arg.toString()).join(" ") ?? ""
    const fiftInfoDescription = ` alias of ${instruction.fiftInstruction.actual_name} ${operandsStr}`

    return [
      "```",
      instruction.fiftInstruction.actual_name + fiftInfoDescription,
      "```",
      "",
      instruction.fiftInstruction.description ?? "",
      "",
      "---",
      "",
      "Aliased instruction info:",
      "",
      ...actualInstructionDescription,
    ].join("\n")
  }

  return actualInstructionDescription.join("\n")
}

function formatGasRanges(gasCosts: readonly GasConsumptionEntry[]): string {
  if (!gasCosts || gasCosts.length === 0) {
    return "N/A"
  }

  const formula = gasCosts.find(it => it.formula !== undefined)
  const nonFormulaCosts = gasCosts.filter(it => it.formula === undefined)

  if (nonFormulaCosts.length === 0 && formula?.formula !== undefined) {
    return formula.formula
  }
  const numericValues = nonFormulaCosts.map(it => it.value)
  const sortedCosts = [...numericValues].sort((a, b) => a - b)

  const resultParts: string[] = []
  let startIndex = 0

  for (let i = 0; i < sortedCosts.length; i++) {
    if (i === sortedCosts.length - 1 || sortedCosts[i + 1] !== sortedCosts[i] + 1) {
      if (startIndex === i) {
        resultParts.push(sortedCosts[i].toString())
      } else {
        resultParts.push(`${sortedCosts[startIndex]}-${sortedCosts[i]}`)
      }
      startIndex = i + 1
    }
  }
  const baseGas = resultParts.filter(it => it !== "36").join(" | ")
  if (formula) {
    return `${baseGas} + ${formula.formula}`
  }
  return baseGas
}
