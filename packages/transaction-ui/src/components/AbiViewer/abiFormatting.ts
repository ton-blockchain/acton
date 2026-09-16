import {
  renderTy,
  type ABIConstExpression,
  type ContractABI,
  type SymTable,
  type Ty,
} from "@ton/tolk-abi-to-typescript"

export type AbiDeclaration = Readonly<ContractABI["declarations"][number]>

type AbiEnumMemberWithDescription = Readonly<{readonly description?: string}>

const TOLK_KEYWORDS = new Set([
  "tolk",
  "import",
  "global",
  "const",
  "type",
  "struct",
  "enum",
  "contract",
  "fun",
  "get",
  "mutate",
  "asm",
  "builtin",
  "var",
  "val",
  "return",
  "repeat",
  "if",
  "else",
  "do",
  "while",
  "break",
  "continue",
  "throw",
  "assert",
  "try",
  "catch",
  "lazy",
  "is",
  "as",
  "match",
  "true",
  "false",
  "null",
])

/** Keeps catalog counts and ABI details consistent by excluding successful exit code 0. */
export function getAbiThrownErrors(errors: readonly ContractABI["thrown_errors"][number][]) {
  return errors.filter(error => error.err_code !== 0)
}

export function formatTolkIdentifier(value: string): string {
  if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(value) && !TOLK_KEYWORDS.has(value)) {
    return value
  }
  return `\`${value.replaceAll("\\", "\\\\").replaceAll("`", "\\`")}\``
}

/** Formats type references without expanding declarations, so recursive storage stays finite. */
export function formatType(symbols: SymTable, tyIdx: number): string {
  const visiting = new Set<number>()

  // Preserve escaped identifiers inside containers and generic arguments.
  // The ABI library supplies the primitive spellings.
  function render(index: number): string {
    const ty = tryTyByIdx(symbols, index)
    if (!ty || visiting.has(index)) return "unknown"
    visiting.add(index)

    let result: string
    switch (ty.kind) {
      case "StructRef":
        result = formatGenericName(ty.struct_name, ty.type_args_ty_idx?.map(render))
        break
      case "AliasRef":
        result = formatGenericName(ty.alias_name, ty.type_args_ty_idx?.map(render))
        break
      case "EnumRef":
        result = formatTolkIdentifier(ty.enum_name)
        break
      case "genericT":
        result = formatTolkIdentifier(ty.name_t)
        break
      case "nullable":
        result = `${render(ty.inner_ty_idx)}?`
        break
      case "cellOf":
        result = `Cell<${render(ty.inner_ty_idx)}>`
        break
      case "arrayOf":
        result = `array<${render(ty.inner_ty_idx)}>`
        break
      case "lispListOf":
        result = `lisp_list<${render(ty.inner_ty_idx)}>`
        break
      case "tensor":
        result = `(${ty.items_ty_idx.map(render).join(", ")})`
        break
      case "shapedTuple":
        result = `[${ty.items_ty_idx.map(render).join(", ")}]`
        break
      case "mapKV":
        result = `map<${render(ty.key_ty_idx)}, ${render(ty.value_ty_idx)}>`
        break
      case "union":
        result = ty.variants.map(variant => render(variant.variant_ty_idx)).join(" | ")
        break
      default:
        result = renderTy(symbols, index)
    }

    visiting.delete(index)
    return result
  }

  return render(tyIdx)
}

export function formatGetMethodSignature(
  method: ContractABI["get_methods"][number],
  symbols: SymTable,
): string {
  const parameters = method.parameters
    .map(
      parameter =>
        `${formatTolkIdentifier(parameter.name)}: ${formatType(symbols, parameter.ty_idx)}${formatAbiDefault(parameter.default_value, symbols, parameter.ty_idx)}`,
    )
    .join(", ")
  return `get fun ${formatTolkIdentifier(method.name)}(${parameters}): ${formatType(
    symbols,
    method.return_ty_idx,
  )}`
}

export function formatAbiTyDeclaration(symbols: SymTable, tyIdx: number): string {
  return formatTypeBlock(symbols, tyIdx, 0)
}

export function getAbiTyDeclaration(symbols: SymTable, tyIdx: number): AbiDeclaration | undefined {
  const ty = tryTyByIdx(symbols, tyIdx)
  if (!ty) return undefined

  switch (ty.kind) {
    case "StructRef":
      return tryGetStruct(symbols, ty.struct_name)
    case "AliasRef":
      return tryGetAlias(symbols, ty.alias_name)
    case "EnumRef":
      return tryGetEnum(symbols, ty.enum_name)
    default:
      return undefined
  }
}

/** Names declarations with template parameters, and references with their concrete arguments. */
export function formatDeclarationName(
  declaration: AbiDeclaration,
  symbols: SymTable,
  tyIdx = declaration.ty_idx,
): string {
  return tyIdx === declaration.ty_idx
    ? formatGenericName(
        declaration.name,
        "type_params" in declaration
          ? declaration.type_params?.map(formatTolkIdentifier)
          : undefined,
      )
    : formatType(symbols, tyIdx)
}

/** Renders either a template or a concrete ABI instantiation without changing the symbol table. */
export function formatDeclarationTolk(
  declaration: AbiDeclaration,
  symbols: SymTable,
  tyIdx = declaration.ty_idx,
): string {
  const name = formatDeclarationName(declaration, symbols, tyIdx)
  const serializers = declaration.custom_pack_unpack
  const customSerialization = [
    serializers?.pack_to_builder ? "packToBuilder" : undefined,
    serializers?.unpack_from_slice ? "unpackFromSlice" : undefined,
  ].filter(Boolean)
  const serializationComment =
    customSerialization.length > 0
      ? `// Custom serialization: ${customSerialization.join(", ")}\n`
      : ""

  switch (declaration.kind) {
    case "struct": {
      const prefixValue = declaration.prefix ? formatTolkPrefix(declaration.prefix) : ""
      const prefix = prefixValue ? ` (${prefixValue})` : ""
      // Stack fields retain the declared type; clientType is a separate ABI annotation.
      const resolvedFields = symbols.structFieldsOf(tyIdx, true)
      if (resolvedFields.length === 0) {
        return `${serializationComment}struct${prefix} ${name} {}`
      }
      const fields = resolvedFields
        .map(field => {
          const comment = field.description ? `${formatTolkDocComment(field.description, 4)}\n` : ""
          const clientType =
            field.client_ty_idx === undefined
              ? ""
              : `    @abi.clientType(${formatType(symbols, field.client_ty_idx)})\n`
          return `${comment}${clientType}    ${formatTolkIdentifier(field.name)}: ${formatType(
            symbols,
            field.ty_idx,
          )}${formatAbiDefault(field.default_value, symbols, field.ty_idx)}`
        })
        .join("\n")
      return `${serializationComment}struct${prefix} ${name} {\n${fields}\n}`
    }
    case "alias":
      return `${serializationComment}type ${name} = ${formatType(
        symbols,
        symbols.aliasTargetOf(tyIdx).ty_idx,
      )}`
    case "enum": {
      const members = declaration.members
        .map(member => {
          const description = (member as AbiEnumMemberWithDescription).description
          const comment = description ? `${formatTolkDocComment(description, 4)}\n` : ""
          return `${comment}    ${formatTolkIdentifier(member.name)} = ${member.value}`
        })
        .join("\n")
      return `${serializationComment}enum ${name}: ${formatType(symbols, declaration.encoded_as_ty_idx)} {\n${members}\n}`
    }
  }
}

export function tryTyByIdx(symbols: SymTable, tyIdx: number): Ty | undefined {
  try {
    return symbols.tyByIdx(tyIdx)
  } catch {
    return undefined
  }
}

function formatTypeBlock(
  symbols: SymTable,
  tyIdx: number,
  depth: number,
  visited = new Set<number>(),
): string {
  if (visited.has(tyIdx)) return formatType(symbols, tyIdx)

  const ty = tryTyByIdx(symbols, tyIdx)
  if (!ty) return "unknown"
  visited.add(tyIdx)

  switch (ty.kind) {
    case "StructRef":
    case "AliasRef":
    case "EnumRef": {
      const declaration = getAbiTyDeclaration(symbols, tyIdx)
      const definition = declaration
        ? formatDeclarationTolk(declaration, symbols, tyIdx)
        : formatType(symbols, tyIdx)
      return definition
        .split("\n")
        .map(line => `${"    ".repeat(depth)}${line}`)
        .join("\n")
    }
    case "union":
      return ty.variants
        .map(variant => {
          const prefix = formatTolkPrefix(variant)
          const formatted = formatTypeBlock(
            symbols,
            variant.variant_ty_idx,
            depth + 1,
            new Set(visited),
          )
          return `${formatted}${prefix ? ` /* ${prefix} */` : ""}`
        })
        .join(`\n${"    ".repeat(depth)}| `)
    case "nullable": {
      const name = formatType(symbols, tyIdx)
      const definition = formatTypeBlock(symbols, ty.inner_ty_idx, depth, visited)
      return definition.trim() === formatType(symbols, ty.inner_ty_idx)
        ? name
        : `${name}\n\n${definition}`
    }
    default:
      return `${"    ".repeat(depth)}${formatType(symbols, tyIdx)}`
  }
}

function formatGenericName(name: string, typeArgs: readonly string[] | undefined): string {
  if (typeArgs === undefined || typeArgs.length === 0) {
    return formatTolkIdentifier(name)
  }
  return `${formatTolkIdentifier(name)}<${typeArgs.join(", ")}>`
}

/** ABI constants are already evaluated; preserve their value and casts in displayed Tolk. */
export function formatAbiDefault(
  value: ABIConstExpression | undefined,
  symbols: SymTable,
  tyIdx: number,
): string {
  return value === undefined ? "" : ` = ${formatConstExpression(value, symbols, tyIdx)}`
}

function formatConstExpression(
  value: ABIConstExpression,
  symbols: SymTable,
  tyIdx?: number,
): string {
  // A constant's field type supplies generic arguments that are absent from its
  // nested object values. Resolve aliases before descending into containers.
  let resolvedTyIdx = tyIdx
  const visited = new Set<number>()
  while (
    resolvedTyIdx !== undefined &&
    tryTyByIdx(symbols, resolvedTyIdx)?.kind === "AliasRef" &&
    !visited.has(resolvedTyIdx)
  ) {
    visited.add(resolvedTyIdx)
    resolvedTyIdx = tryAliasTargetTyIdx(symbols, resolvedTyIdx)
  }
  const ty = resolvedTyIdx === undefined ? undefined : tryTyByIdx(symbols, resolvedTyIdx)

  switch (value.kind) {
    case "int":
      return value.v
    case "bool":
      return String(value.v)
    case "null":
      return "null"
    case "string":
      return JSON.stringify(value.str)
    case "slice":
      return `${JSON.stringify(value.hex)}.hexToSlice()`
    case "address":
      return `address(${JSON.stringify(value.addr)})`
    case "tensor":
    case "shapedTuple": {
      const items = value.items
        .map((item, index) => {
          const itemTyIdx =
            ty?.kind === "tensor" || ty?.kind === "shapedTuple"
              ? ty.items_ty_idx[index]
              : ty?.kind === "arrayOf" || ty?.kind === "lispListOf"
                ? ty.inner_ty_idx
                : undefined
          return formatConstExpression(item, symbols, itemTyIdx)
        })
        .join(", ")
      return value.kind === "tensor" ? `(${items})` : `[${items}]`
    }
    case "castTo":
      return `(${formatConstExpression(value.inner, symbols, value.cast_to_ty_idx)} as ${formatType(symbols, value.cast_to_ty_idx)})`
    case "object": {
      // Object constants name generic instantiations (e.g. Message<uint32>),
      // while declarations are indexed by the template name (Message).
      const fields =
        ty?.kind === "StructRef"
          ? symbols.structFieldsOf(resolvedTyIdx!, true)
          : tryGetStruct(symbols, value.struct_name)?.fields
      const typeName =
        ty?.kind === "StructRef"
          ? formatType(symbols, resolvedTyIdx!)
          : formatTolkIdentifier(value.struct_name)
      const values = value.fields.map((field, index) => {
        const name = fields?.[index]?.name
        const expression = formatConstExpression(field, symbols, fields?.[index]?.ty_idx)
        return name === undefined ? expression : `${formatTolkIdentifier(name)}: ${expression}`
      })
      return `${typeName} { ${values.join(", ")} }`
    }
  }
}

export function formatTolkDocComment(description: string, indentSpaces: number): string {
  const pad = " ".repeat(indentSpaces)
  return description
    .split(/\r?\n/)
    .map(line => `${pad}/// ${line.trim()}`)
    .join("\n")
}

function formatTolkPrefix(prefix: {
  readonly prefix_num: number
  readonly prefix_len: number
}): string {
  if (prefix.prefix_len === 0) return ""
  if (prefix.prefix_len % 4 === 0) {
    return `0x${prefix.prefix_num.toString(16).padStart(Math.max(1, prefix.prefix_len / 4), "0")}`
  }
  return `0b${prefix.prefix_num.toString(2).padStart(prefix.prefix_len, "0")}`
}

function tryGetStruct(
  symbols: SymTable,
  name: string,
): Extract<AbiDeclaration, {kind: "struct"}> | undefined {
  try {
    return symbols.getStruct(name)
  } catch {
    return undefined
  }
}

function tryGetAlias(
  symbols: SymTable,
  name: string,
): Extract<AbiDeclaration, {kind: "alias"}> | undefined {
  try {
    return symbols.getAlias(name)
  } catch {
    return undefined
  }
}

function tryGetEnum(
  symbols: SymTable,
  name: string,
): Extract<AbiDeclaration, {kind: "enum"}> | undefined {
  try {
    return symbols.getEnum(name)
  } catch {
    return undefined
  }
}

export function tryAliasTargetTyIdx(symbols: SymTable, tyIdx: number): number | undefined {
  try {
    return symbols.aliasTargetOf(tyIdx).ty_idx
  } catch {
    return undefined
  }
}
