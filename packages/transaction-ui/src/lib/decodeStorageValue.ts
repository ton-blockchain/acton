import type {DictionaryKey, DictionaryValue} from "@ton/core"
import {type Address, type BitString, Dictionary, type Slice} from "@ton/core"
import {type DynamicCtx, renderTy, unpackFromSliceDynamic} from "@ton/tolk-abi-to-typescript"

type FallbackDictionaryKey = Address | bigint | BitString

// TODO: remove this fallback once @ton/tolk-abi-to-typescript supports bitsN map keys.
const createFallbackDictionaryKey = (
  ctx: DynamicCtx,
  tyIdx: number,
): DictionaryKey<FallbackDictionaryKey> => {
  const ty = ctx.symbols.tyByIdx(tyIdx)

  switch (ty.kind) {
    case "intN": {
      return Dictionary.Keys.BigInt(ty.n)
    }
    case "uintN": {
      return Dictionary.Keys.BigUint(ty.n)
    }
    case "bitsN": {
      return Dictionary.Keys.BitString(ty.n)
    }
    case "address": {
      return Dictionary.Keys.Address()
    }
    case "AliasRef": {
      const aliasRef = ctx.symbols.getAlias(ty.alias_name)
      if (aliasRef.custom_pack_unpack?.unpack_from_slice) {
        throw new Error(`Unsupported dictionary key alias: ${ty.alias_name}`)
      }

      return createFallbackDictionaryKey(ctx, ctx.symbols.aliasTargetOf(tyIdx).ty_idx)
    }
    default: {
      throw new Error(`Unsupported dictionary key type: ${renderTy(ctx.symbols, tyIdx)}`)
    }
  }
}

const createFallbackDictionaryValue = (
  ctx: DynamicCtx,
  tyIdx: number,
): DictionaryValue<unknown> => ({
  serialize() {
    throw new Error("Storage dictionary fallback is read-only.")
  },
  parse(parser) {
    const value = unpackStorageValueWithDictionaryFallback(ctx, tyIdx, parser)
    parser.endParse()
    return value
  },
})

/**
 * Read-only compatibility decoder for storage containing maps with bitsN keys.
 *
 * `unpackFromSliceDynamic` is always the primary decoder. Its dictionary key factory currently
 * supports intN, uintN, and address, but rejects bitsN even though @ton/core can represent such
 * keys with `Dictionary.Keys.BitString`. A problematic map may be nested inside a struct, alias,
 * cell, or collection, and the public dynamic decoder has no hook for replacing only its map key
 * factory, so the fallback has to walk the enclosing ABI types recursively as well.
 *
 * Keep this implementation read-only and remove it when the upstream runtime supports bitsN keys.
 */
function unpackStorageValueWithDictionaryFallback(
  ctx: DynamicCtx,
  tyIdx: number,
  parser: Slice,
): unknown {
  const ty = ctx.symbols.tyByIdx(tyIdx)

  switch (ty.kind) {
    case "void": {
      return undefined
    }
    case "intN": {
      return parser.loadIntBig(ty.n)
    }
    case "uintN": {
      return parser.loadUintBig(ty.n)
    }
    case "varintN": {
      return parser.loadVarIntBig(Math.log2(ty.n))
    }
    case "varuintN": {
      return parser.loadVarUintBig(Math.log2(ty.n))
    }
    case "coins": {
      return parser.loadCoins()
    }
    case "bool": {
      return parser.loadBoolean()
    }
    case "cell": {
      return parser.loadRef()
    }
    case "string": {
      return parser.loadStringRefTail()
    }
    case "remaining": {
      const rest = parser.clone()
      parser.loadBits(parser.remainingBits)
      while (parser.remainingRefs > 0) {
        parser.loadRef()
      }
      return rest
    }
    case "address": {
      return parser.loadAddress()
    }
    case "addressOpt": {
      return parser.loadMaybeAddress()
    }
    case "addressExt": {
      return parser.loadExternalAddress()
    }
    case "addressAny": {
      const address = parser.loadAddressAny()
      return address === null ? "none" : address
    }
    case "bitsN": {
      return parser.loadBits(ty.n)
    }
    case "nullLiteral": {
      return null
    }
    case "nullable": {
      return parser.loadBoolean()
        ? unpackStorageValueWithDictionaryFallback(ctx, ty.inner_ty_idx, parser)
        : null
    }
    case "cellOf": {
      const refParser = parser.loadRef().beginParse()
      const value = unpackStorageValueWithDictionaryFallback(ctx, ty.inner_ty_idx, refParser)
      refParser.endParse()
      return {ref: value}
    }
    case "arrayOf": {
      const length = parser.loadUint(8)
      let head = parser.loadMaybeRef()
      const values: unknown[] = []

      while (head) {
        const chunk = head.beginParse()
        head = chunk.loadMaybeRef()
        while (chunk.remainingBits > 0 || chunk.remainingRefs > 0) {
          values.push(unpackStorageValueWithDictionaryFallback(ctx, ty.inner_ty_idx, chunk))
        }
      }

      if (values.length !== length) {
        throw new Error(`Array length mismatch: expected ${length}, got ${values.length}`)
      }

      return values
    }
    case "lispListOf": {
      const values: unknown[] = []
      let head = parser.loadRef().beginParse()

      while (head.remainingRefs > 0) {
        const tail = head.loadRef()
        const value = unpackStorageValueWithDictionaryFallback(ctx, ty.inner_ty_idx, head)
        head.endParse()
        values.unshift(value)
        head = tail.beginParse()
      }

      return values
    }
    case "tensor":
    case "shapedTuple": {
      return ty.items_ty_idx.map(itemTyIdx =>
        unpackStorageValueWithDictionaryFallback(ctx, itemTyIdx, parser),
      )
    }
    case "mapKV": {
      return parser.loadDict(
        createFallbackDictionaryKey(ctx, ty.key_ty_idx),
        createFallbackDictionaryValue(ctx, ty.value_ty_idx),
      )
    }
    case "EnumRef": {
      const enumRef = ctx.symbols.getEnum(ty.enum_name)
      if (enumRef.custom_pack_unpack?.unpack_from_slice) {
        throw new Error(`Unsupported enum: ${ty.enum_name}`)
      }

      return unpackStorageValueWithDictionaryFallback(ctx, enumRef.encoded_as_ty_idx, parser)
    }
    case "StructRef": {
      const structRef = ctx.symbols.getStruct(ty.struct_name)
      if (structRef.custom_pack_unpack?.unpack_from_slice) {
        throw new Error(`Unsupported struct: ${ty.struct_name}`)
      }

      const value: Record<string, unknown> = {$: ty.struct_name}
      if (structRef.prefix) {
        const prefix = parser.loadUint(structRef.prefix.prefix_len)
        if (prefix !== structRef.prefix.prefix_num) {
          throw new Error(`Incorrect prefix for ${ty.struct_name}`)
        }
      }

      for (const field of ctx.symbols.structFieldsOf(tyIdx, false)) {
        value[field.name] = unpackStorageValueWithDictionaryFallback(ctx, field.ty_idx, parser)
      }

      return value
    }
    case "AliasRef": {
      const aliasRef = ctx.symbols.getAlias(ty.alias_name)
      if (aliasRef.custom_pack_unpack?.unpack_from_slice) {
        throw new Error(`Unsupported alias: ${ty.alias_name}`)
      }

      return unpackStorageValueWithDictionaryFallback(
        ctx,
        ctx.symbols.aliasTargetOf(tyIdx).ty_idx,
        parser,
      )
    }
    default: {
      throw new Error(`Unsupported storage type: ${renderTy(ctx.symbols, tyIdx)}`)
    }
  }
}

/**
 * Decodes one complete storage value using the runtime unpacker and its bitsN dictionary fallback.
 * The input slice is cloned, so a failed attempt never leaves it partially consumed.
 */
export function unpackStorageValue(ctx: DynamicCtx, tyIdx: number, source: Slice): unknown {
  const parser = source.clone()
  let decoded: unknown

  try {
    decoded = unpackFromSliceDynamic(ctx, tyIdx, parser) as unknown
  } catch {
    // A NonStandardDictKey from a nested map is wrapped by the runtime as CantUnpackDynamic,
    // so its exact error class cannot be checked reliably here. Retry with the compatibility
    // decoder for ABI shapes such as mapKV<bitsN, void>.
    const fallbackParser = source.clone()
    const fallbackDecoded = unpackStorageValueWithDictionaryFallback(ctx, tyIdx, fallbackParser)
    fallbackParser.endParse()
    return fallbackDecoded
  }

  // Keep incomplete runtime decodes rejected instead of reinterpreting them with the fallback.
  parser.endParse()
  return decoded
}
