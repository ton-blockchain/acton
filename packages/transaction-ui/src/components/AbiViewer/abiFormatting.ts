import {renderTy, type ContractABI, type SymTable, type Ty} from "@ton/tolk-abi-to-typescript"

export type AbiDeclaration = Readonly<ContractABI["declarations"][number]>

type AbiEnumMemberWithDescription = Readonly<{readonly description?: string}>

export function formatTolkIdentifier(value: string): string {
  if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(value)) {
    return value
  }
  return `\`${value.replaceAll("\\", "\\\\").replaceAll("`", "\\`")}\``
}

export function formatType(symbols: SymTable, tyIdx: number): string {
  try {
    return renderTy(symbols, tyIdx)
  } catch {
    const ty = tryTyByIdx(symbols, tyIdx)
    return ty ? formatTyFallback(ty, symbols) : "unknown"
  }
}

export function formatGetMethodSignature(
  method: ContractABI["get_methods"][number],
  symbols: SymTable,
): string {
  const parameters = method.parameters
    .map(
      parameter =>
        `${formatTolkIdentifier(parameter.name)}: ${formatType(symbols, parameter.ty_idx)}`,
    )
    .join(", ")
  return `get fun ${formatTolkIdentifier(method.name)}(${parameters}): ${formatType(
    symbols,
    method.return_ty_idx,
  )}`
}

export function formatAbiTyDeclaration(symbols: SymTable, tyIdx: number): string {
  const declaration = getAbiTyDeclaration(symbols, tyIdx)
  return declaration
    ? formatDeclarationTolk(declaration, symbols)
    : formatTypeBlock(symbols, tyIdx, 0)
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

export function formatDeclarationTolk(declaration: AbiDeclaration, symbols: SymTable): string {
  switch (declaration.kind) {
    case "struct": {
      const prefix = declaration.prefix ? ` (${formatTolkPrefix(declaration.prefix)})` : ""
      if (declaration.fields.length === 0) {
        return `struct${prefix} ${formatTolkIdentifier(declaration.name)} {}`
      }
      const fields = declaration.fields
        .map(field => {
          const comment = field.description ? `${formatTolkDocComment(field.description, 4)}\n` : ""
          return `${comment}    ${formatTolkIdentifier(field.name)}: ${formatType(
            symbols,
            field.client_ty_idx ?? field.ty_idx,
          )}`
        })
        .join("\n")
      return `struct${prefix} ${formatTolkIdentifier(declaration.name)} {\n${fields}\n}`
    }
    case "alias":
      return `type ${formatTolkIdentifier(declaration.name)} = ${formatType(
        symbols,
        declaration.target_ty_idx,
      )}`
    case "enum": {
      const members = declaration.members
        .map(member => {
          const description = (member as AbiEnumMemberWithDescription).description
          const comment = description ? `${formatTolkDocComment(description, 4)}\n` : ""
          return `${comment}    ${formatTolkIdentifier(member.name)} = ${member.value}`
        })
        .join("\n")
      return `enum ${formatTolkIdentifier(declaration.name)} {\n${members}\n}`
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
    case "StructRef": {
      const fields = symbols.structFieldsOf(tyIdx, false)
      if (fields.length === 0) return `${formatTolkIdentifier(ty.struct_name)} {}`

      const baseIndent = "    ".repeat(depth)
      const fieldIndent = "    ".repeat(depth + 1)
      const fieldLines = fields
        .map(
          field =>
            `${fieldIndent}${formatTolkIdentifier(field.name)}: ${formatTypeBlock(
              symbols,
              field.ty_idx,
              depth + 1,
              new Set(visited),
            ).trimStart()}`,
        )
        .join("\n")
      return `${baseIndent}${formatTolkIdentifier(ty.struct_name)} {\n${fieldLines}\n${baseIndent}}`
    }
    case "AliasRef": {
      const targetTyIdx = tryAliasTargetTyIdx(symbols, tyIdx)
      return targetTyIdx === undefined
        ? formatTolkIdentifier(ty.alias_name)
        : `${"    ".repeat(depth)}${formatTolkIdentifier(ty.alias_name)} =\n${formatTypeBlock(
            symbols,
            targetTyIdx,
            depth + 1,
            visited,
          )}`
    }
    case "union":
      return `${"    ".repeat(depth)}${ty.variants
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
        .join(`\n${"    ".repeat(depth)}| `)}`
    case "nullable":
      return `${formatTypeBlock(symbols, ty.inner_ty_idx, depth, visited)}?`
    default:
      return `${"    ".repeat(depth)}${formatType(symbols, tyIdx)}`
  }
}

function formatTyFallback(ty: Ty, symbols: SymTable): string {
  switch (ty.kind) {
    case "intN":
    case "uintN":
    case "varintN":
    case "varuintN":
    case "bitsN":
      return `${ty.kind}<${ty.n}>`
    case "StructRef":
      return formatGenericName(ty.struct_name, ty.type_args_ty_idx, symbols)
    case "AliasRef":
      return formatGenericName(ty.alias_name, ty.type_args_ty_idx, symbols)
    case "EnumRef":
      return formatTolkIdentifier(ty.enum_name)
    case "nullable":
      return `${formatType(symbols, ty.inner_ty_idx)}?`
    case "cellOf":
    case "arrayOf":
    case "lispListOf":
      return `${ty.kind}<${formatType(symbols, ty.inner_ty_idx)}>`
    case "tensor":
    case "shapedTuple":
      return `${ty.kind}<${ty.items_ty_idx
        .map(itemTyIdx => formatType(symbols, itemTyIdx))
        .join(", ")}>`
    case "mapKV":
      return `map<${formatType(symbols, ty.key_ty_idx)}, ${formatType(symbols, ty.value_ty_idx)}>`
    case "genericT":
      return ty.name_t
    case "union":
      return ty.variants.map(variant => formatType(symbols, variant.variant_ty_idx)).join(" | ")
    default:
      return ty.kind
  }
}

function formatGenericName(
  name: string,
  typeArgsTyIdx: readonly number[] | undefined,
  symbols: SymTable,
): string {
  if (typeArgsTyIdx === undefined || typeArgsTyIdx.length === 0) {
    return formatTolkIdentifier(name)
  }
  return `${formatTolkIdentifier(name)}<${typeArgsTyIdx
    .map(tyIdx => formatType(symbols, tyIdx))
    .join(", ")}>`
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
  if (prefix.prefix_len % 4 === 0) {
    return `0x${(prefix.prefix_num >>> 0)
      .toString(16)
      .padStart(Math.max(1, prefix.prefix_len / 4), "0")}`
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
