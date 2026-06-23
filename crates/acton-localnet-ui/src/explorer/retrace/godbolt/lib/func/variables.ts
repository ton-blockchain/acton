import {trace} from "ton-assembly"

export type {FuncVar} from "ton-assembly/dist/trace/func-mapping"

const getFuncTypeString = (type: trace.FuncType): string => {
  switch (type) {
    case trace.FuncType.INT:
      return "int"
    case trace.FuncType.CELL:
      return "cell"
    case trace.FuncType.SLICE:
      return "slice"
    case trace.FuncType.BUILDER:
      return "builder"
    case trace.FuncType.CONT:
      return "cont"
    case trace.FuncType.TUPLE:
      return "tuple"
    case trace.FuncType.TYPE:
      return "type"
    default:
      // eslint-disable-next-line @typescript-eslint/restrict-template-expressions
      return `type(${type})`
  }
}

const getVariableKind = (flags: number): string => {
  const flagStrings: string[] = []

  if (flags & trace.FuncVarFlag.IN) return "Parameter"
  if (flags & trace.FuncVarFlag.NAMED) return "Local variable"
  if (flags & trace.FuncVarFlag.TMP) return "Temp variable"

  return flagStrings.length > 0 ? flagStrings.join(", ") : "var"
}

export const formatVariablesForHover = (variables: trace.FuncVar[]): string => {
  if (variables.length === 0) {
    return "No variables in scope"
  }

  const variablesList = [...variables]
    .reverse()
    .map(variable => {
      const kind = `${getVariableKind(variable.flags)}`
      const name = `${variable.name}`
      const type = getFuncTypeString(variable.type)
      const value = variable.value ?? ""
      const valuePresentation = value.length === 0 ? "" : ` = ${value}`
      return `${kind} \`${name}: ${type}\`${valuePresentation}\n`
    })
    .join("\n")

  return `**Live variables:**\n\n${variablesList}`
}
