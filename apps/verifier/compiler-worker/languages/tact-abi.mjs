import path from "node:path";
import { getMethodId } from "@ton/core";

import { normalizeSourcePath } from "./common.mjs";
import { importTolk, SUPPORTED_TOLK_VERSIONS } from "./registry.mjs";

/** @typedef {import("@ton/core").ABIReceiver} ABIReceiver */
/** @typedef {import("@ton/core").ABIGetter} ABIGetter */
/** @typedef {import("@ton/core").ABIType} ABIType */
/** @typedef {import("@ton/core").ABITypeRef} ABITypeRef */
/** @typedef {import("@ton/core").ContractABI} ContractABI */
/** @typedef {{ path: string, content: string }} GeneratedSource */
/** @typedef {{ name: string, type: ABITypeRef }} ABIArgument */
/** @typedef {{ type: ABIType, metadataFields: import("@ton/core").ABIField[] }} TactStorage */
/**
 * @typedef {{
 *   name?: string,
 *   abi?: string | ContractABI,
 *   compiler?: { version?: string },
 *   init?: {
 *     args?: ABIArgument[],
 *     prefix?: { bits?: number, value?: number },
 *     deployment?: { kind?: string, system?: string | null },
 *   },
 * }} TactPackage
 */
/** @typedef {{ contractName: string, getters: Map<string, ABIGetter>, source: string }} GeneratedTolkSource */
/** @typedef {(name: string, selected: Set<string>, serializable?: boolean) => void} CollectStruct */

const PRIMITIVE_TYPES = new Set([
  "address",
  "bool",
  "builder",
  "cell",
  "fixed-bytes",
  "int",
  "slice",
  "string",
  "uint",
]);

const TOLK_KEYWORDS = new Set([
  "asm",
  "assert",
  "break",
  "catch",
  "const",
  "continue",
  "contract",
  "do",
  "else",
  "enum",
  "export",
  "extern",
  "false",
  "for",
  "fun",
  "get",
  "global",
  "if",
  "import",
  "in",
  "inline",
  "lazy",
  "match",
  "mutate",
  "null",
  "operator",
  "redef",
  "repeat",
  "return",
  "self",
  "struct",
  "throw",
  "true",
  "try",
  "type",
  "val",
  "var",
  "while",
]);

const TOLK_GETTER_NAME_CONFLICTS = new Set(["address", "random"]);

/**
 * @param {TactPackage} tactPackage
 * @param {GeneratedSource[]} generatedSources
 * @returns {Promise<GeneratedSource[] | undefined>}
 */
export async function generatedTolkAbiSources(tactPackage, generatedSources) {
  try {
    const tactAbi = findMainTactAbi(tactPackage, generatedSources);
    if (tactAbi === undefined) {
      return undefined;
    }

    const compilerVersion = SUPPORTED_TOLK_VERSIONS[0];
    if (compilerVersion === undefined) {
      return undefined;
    }

    const storage = await findTactStorage(
      tactPackage,
      tactAbi,
      generatedSources,
    ).catch(() => undefined);
    const types = await prepareTactTypes(tactPackage, tactAbi, storage);
    if (types === undefined) {
      return undefined;
    }
    const generated = tactAbiToTolk(tactAbi, types);
    const typesPath = path.posix.join(
      "output",
      `${generated.contractName}.types.tolk`,
    );
    const { runTolkCompiler } = await importTolk(compilerVersion);
    const result = await runTolkCompiler({
      entrypointFileName: typesPath,
      allowNoEntrypoint: true,
      pathMappings: {},
      fsReadCallback: (requestedPath) => {
        if (normalizeSourcePath(requestedPath) === typesPath) {
          return generated.source;
        }
        throw new Error(
          `generated Tact ABI source was not provided: ${requestedPath}`,
        );
      },
    });

    if (
      result.status === "error" ||
      result.abiJson === undefined ||
      result.abiJson === null
    ) {
      return undefined;
    }

    // Tolk emits ABI entries only for `get fun`, whose method ID is derived
    // from the function name. Some valid Tact names collide with Tolk symbols
    // and need a temporary suffix. Restore their names and locally calculated
    // method IDs after Tolk has produced the parameter and return type indices.
    for (const getter of result.abiJson.get_methods ?? []) {
      const tactGetter = generated.getters.get(getter.name);
      if (tactGetter === undefined) {
        continue;
      }
      getter.name = tactGetter.name;
      getter.tvm_method_id = Number.isInteger(tactGetter.methodId)
        ? tactGetter.methodId
        : getMethodId(tactGetter.name);
    }

    return [
      { path: typesPath, content: generated.source },
      {
        path: path.posix.join("output", `${generated.contractName}.abi.json`),
        content: `${JSON.stringify(result.abiJson, null, 2)}\n`,
      },
    ];
  } catch {
    return undefined;
  }
}

/**
 * Tact stores runtime metadata before contract fields, but does not expose it
 * through ContractABI. Package metadata tells us whether the storage has a
 * child-code system cell and a deployment-state bit. Older Tact ABIs also omit
 * the contract fields themselves, so recover those from the verified sources.
 *
 * @param {TactPackage} tactPackage
 * @param {ContractABI} tactAbi
 * @param {GeneratedSource[]} generatedSources
 * @returns {Promise<TactStorage | undefined>}
 */
async function findTactStorage(
  tactPackage,
  tactAbi,
  generatedSources,
) {
  const storageName = `${tactAbi.name}$Data`;
  const existing = (Array.isArray(tactAbi.types) ? tactAbi.types : []).find(
    (type) => type?.name === storageName,
  );
  const init = tactPackage?.init;
  if (init === undefined) {
    return existing === undefined
      ? undefined
      : { type: existing, metadataFields: [] };
  }

  const metadataFields = [];
  if (
    init.deployment?.kind === "system-cell" &&
    typeof init.deployment.system === "string" &&
    init.deployment.system.length > 0
  ) {
    metadataFields.push({
      name: "tactSystemCell",
      type: { kind: "simple", type: "cell", optional: false },
    });
  }

  if (init.prefix !== undefined) {
    if (init.prefix.bits !== 1) {
      return undefined;
    }
    metadataFields.push({
      name: "tactDeploymentCompleted",
      type: { kind: "simple", type: "bool", optional: false },
    });
  }

  let fields = existing?.fields;
  if (!Array.isArray(fields)) {
    if (init.prefix === undefined) {
      fields = Array.isArray(init.args)
        ? init.args.map((argument) => ({
            name: argument.name,
            type: argument.type,
          }))
        : undefined;
    } else {
      fields = await findTactContractFields(
        tactPackage,
        generatedSources,
      );
    }
  }
  if (!Array.isArray(fields)) {
    return undefined;
  }

  const usedFieldNames = new Set(fields.map((field) => field.name));
  for (const field of metadataFields) {
    const base = field.name;
    let suffix = 2;
    while (usedFieldNames.has(field.name)) {
      field.name = `${base}${suffix}`;
      suffix += 1;
    }
    usedFieldNames.add(field.name);
  }

  return {
    type: {
      name: storageName,
      header: null,
      fields,
    },
    metadataFields,
  };
}

/**
 * Tact serializes a large struct into a chain of cells. Reuse the allocator
 * from the exact compiler version that produced the verified package, then
 * expose every continuation as an explicit `Cell<T>` in the Tolk ABI.
 *
 * @param {TactPackage} tactPackage
 * @param {ContractABI} tactAbi
 * @param {TactStorage | undefined} storage
 * @returns {Promise<ABIType[] | undefined>}
 */
async function prepareTactTypes(tactPackage, tactAbi, storage) {
  const storageName = `${tactAbi.name}$Data`;
  const abiTypes = Array.isArray(tactAbi.types) ? tactAbi.types : [];
  const originalTypes =
    tactPackage.init !== undefined && storage === undefined
      ? abiTypes.filter((type) => type?.name !== storageName)
      : abiTypes;
  let types = originalTypes;
  if (storage !== undefined) {
    types = [
      ...originalTypes.filter((type) => type?.name !== storage.type.name),
      storage.type,
    ];
  }

  const version = tactPackage.compiler?.version;
  if (typeof version !== "string") {
    return undefined;
  }

  try {
    const allocatorModule = await import(
      `tact-${version}/dist/storage/allocator.js`
    );
    const allocate =
      allocatorModule.allocate ?? allocatorModule.default?.allocate;
    const getAllocationOperationFromField =
      allocatorModule.getAllocationOperationFromField ??
      allocatorModule.default?.getAllocationOperationFromField;
    if (
      typeof allocate !== "function" ||
      typeof getAllocationOperationFromField !== "function"
    ) {
      return undefined;
    }

    const typesByName = new Map(types.map((type) => [type.name, type]));
    const allocations = new Map();
    const failedTypes = new Set();
    const visiting = new Set();

    /**
     * @param {string} name
     * @returns {{ root: object, size: { bits: number, refs: number } } | undefined}
     */
    const resolveAllocation = (name) => {
      const cached = allocations.get(name);
      if (cached !== undefined) {
        return cached;
      }
      if (failedTypes.has(name) || visiting.has(name)) {
        return undefined;
      }
      const type = typesByName.get(name);
      if (type === undefined) {
        return undefined;
      }

      visiting.add(name);
      try {
        const isStorage = type.name === storage?.type.name;
        const headerBits =
          type.header === null || type.header === undefined ? 0 : 32;
        const ops = (Array.isArray(type.fields) ? type.fields : []).map(
          (field) => ({
            name: field.name,
            type: field.type,
            op: getAllocationOperationFromField(field.type, (referencedName) => {
              const referenced = resolveAllocation(referencedName);
              if (referenced === undefined) {
                throw new Error(
                  `Tact ABI type cannot be allocated: ${referencedName}`,
                );
              }
              return referenced.size;
            }),
          }),
        );
        const root = allocate({
          ops,
          reserved: isStorage
            ? // Tact reserves one root reference for its internal system cell,
              // even when this particular contract does not end up storing it.
              { bits: 0, refs: 1 }
            : { bits: headerBits, refs: 0 },
        });
        const resolved = {
          root,
          size: {
            bits: root.size.bits + (isStorage ? 0 : headerBits),
            refs: root.size.refs + (isStorage ? 1 : 0),
          },
        };
        allocations.set(name, resolved);
        return resolved;
      } catch {
        failedTypes.add(name);
        return undefined;
      } finally {
        visiting.delete(name);
      }
    };

    for (const type of types) {
      resolveAllocation(type.name);
    }

    const usedTypeNames = new Set(types.map((type) => type.name));
    const result = [];
    for (const type of types) {
      const typeAllocation = allocations.get(type.name);
      if (typeAllocation === undefined) {
        if (type.name === storage?.type.name) {
          return undefined;
        }
        result.push(type);
        continue;
      }
      const cells = [];
      for (
        let cell = typeAllocation.root;
        cell !== null;
        cell = cell.next
      ) {
        cells.push(cell);
      }

      const names = [type.name];
      for (let index = 1; index < cells.length; index += 1) {
        const base = `${type.name}$Continuation${index === 1 ? "" : index}`;
        let name = base;
        let suffix = 2;
        while (usedTypeNames.has(name)) {
          name = `${base}_${suffix}`;
          suffix += 1;
        }
        usedTypeNames.add(name);
        names.push(name);
      }

      for (let index = 0; index < cells.length; index += 1) {
        const fields = cells[index].ops.map((op) => ({
          name: op.name,
          type: op.type,
        }));
        if (index === 0 && type.name === storage?.type.name) {
          fields.unshift(...storage.metadataFields);
        }
        if (index + 1 < cells.length) {
          const usedFieldNames = new Set(fields.map((field) => field.name));
          let continuationName = "tactContinuation";
          let suffix = 2;
          while (usedFieldNames.has(continuationName)) {
            continuationName = `tactContinuation${suffix}`;
            suffix += 1;
          }
          fields.push({
            name: continuationName,
            type: {
              kind: "simple",
              type: names[index + 1],
              optional: false,
              format: "ref",
            },
          });
        }
        result.push({
          name: names[index],
          header: index === 0 ? (type.header ?? null) : null,
          fields,
        });
      }
    }
    return result;
  } catch {
    return undefined;
  }
}

/**
 * @param {TactPackage} tactPackage
 * @param {GeneratedSource[]} generatedSources
 * @returns {Promise<import("@ton/core").ABIField[] | undefined>}
 */
async function findTactContractFields(tactPackage, generatedSources) {
  const version = tactPackage.compiler?.version;
  if (typeof version !== "string") {
    return undefined;
  }

  const parsedItems = [];
  for (const source of generatedSources) {
    if (!source.path.endsWith(".tact")) {
      continue;
    }
    const module = await parseTactModule(version, source);
    const items = Array.isArray(module?.entries)
      ? module.entries
      : Array.isArray(module?.items)
        ? module.items
        : [];
    parsedItems.push(...items);
  }

  const contract = parsedItems.find(
    (item) =>
      (item?.kind === "def_contract" || item?.kind === "contract") &&
      astName(item.name) === tactPackage.name,
  );
  if (contract === undefined) {
    return undefined;
  }

  const contractFields = [];
  for (const declaration of Array.isArray(contract.declarations)
    ? contract.declarations
    : []) {
    if (
      declaration?.kind !== "def_field" &&
      declaration?.kind !== "field_decl"
    ) {
      continue;
    }
    const field = astFieldToAbi(declaration);
    if (field === undefined) {
      return undefined;
    }
    contractFields.push(field);
  }
  return contractFields;
}

/**
 * @param {string} version
 * @param {GeneratedSource} source
 * @returns {Promise<object>}
 */
async function parseTactModule(version, source) {
  const grammar = await import(`tact-${version}/dist/grammar/grammar.js`);
  const parse = grammar.parse ?? grammar.default?.parse;
  if (typeof parse === "function") {
    return parse(source.content, source.path, "user");
  }

  const [parserModule, astModule] = await Promise.all([
    import(`tact-${version}/dist/grammar/index.js`),
    import(`tact-${version}/dist/ast/ast-helpers.js`),
  ]);
  const getParser = parserModule.getParser ?? parserModule.default?.getParser;
  const getAstFactory =
    astModule.getAstFactory ?? astModule.default?.getAstFactory;
  if (typeof getParser !== "function" || typeof getAstFactory !== "function") {
    throw new Error(`Tact ${version} parser is unavailable`);
  }
  return getParser(getAstFactory()).parse({
    path: source.path,
    code: source.content,
    origin: "user",
  });
}

/**
 * @param {unknown} declaration
 * @returns {import("@ton/core").ABIField | undefined}
 */
function astFieldToAbi(declaration) {
  if (
    declaration === null ||
    typeof declaration !== "object" ||
    (declaration.kind !== "def_field" && declaration.kind !== "field_decl")
  ) {
    return undefined;
  }
  const name = astName(declaration.name);
  if (name === undefined) {
    return undefined;
  }
  const type = astTypeToAbi(declaration.type, astName(declaration.as));
  return type === undefined ? undefined : { name, type };
}

/**
 * @param {unknown} type
 * @param {string | undefined} format
 * @returns {ABITypeRef | undefined}
 */
function astTypeToAbi(type, format) {
  if (type === null || typeof type !== "object") {
    return undefined;
  }

  if (type.kind === "type_ref_map" || type.kind === "map_type") {
    const key = astName(type.key ?? type.keyType);
    const value = astName(type.value ?? type.valueType);
    const keyFormat = astName(type.keyAs ?? type.keyStorageType);
    const valueFormat = astName(type.valueAs ?? type.valueStorageType);
    if (key === undefined || value === undefined) {
      return undefined;
    }
    return astMapTypeToAbi(key, keyFormat, value, valueFormat);
  }

  let optional = false;
  let typeName;
  if (type.kind === "type_ref_simple") {
    typeName = astName(type.name);
    optional = type.optional === true;
  } else if (type.kind === "type_id") {
    typeName = astName(type);
  } else if (
    type.kind === "optional_type" &&
    type.typeArg?.kind === "type_id"
  ) {
    typeName = astName(type.typeArg);
    optional = true;
  }
  if (typeName === undefined) {
    return undefined;
  }

  if (typeName === "Int") {
    const integer = tactIntegerFormat(format);
    return integer === undefined
      ? undefined
      : { kind: "simple", ...integer, optional };
  }
  if (typeName === "Bool" || typeName === "Address") {
    if (format !== undefined) {
      return undefined;
    }
    return {
      kind: "simple",
      type: typeName === "Bool" ? "bool" : "address",
      optional,
    };
  }
  if (typeName === "Cell" || typeName === "Slice" || typeName === "Builder") {
    const primitive = typeName.toLowerCase();
    if (format === undefined) {
      return { kind: "simple", type: primitive, optional };
    }
    if (format === "remaining") {
      return {
        kind: "simple",
        type: primitive,
        optional,
        format: "remainder",
      };
    }
    if (typeName === "Slice" && (format === "bytes32" || format === "bytes64")) {
      return {
        kind: "simple",
        type: "fixed-bytes",
        optional,
        format: Number(format.slice(5)),
      };
    }
    return undefined;
  }
  if (typeName === "String") {
    return format === undefined
      ? { kind: "simple", type: "string", optional }
      : undefined;
  }
  if (typeName === "StringBuilder") {
    return undefined;
  }
  if (format !== undefined && format !== "reference") {
    return undefined;
  }
  return {
    kind: "simple",
    type: typeName,
    optional,
    ...(format === "reference" ? { format: "ref" } : {}),
  };
}

/**
 * @param {string} key
 * @param {string | undefined} keyFormat
 * @param {string} value
 * @param {string | undefined} valueFormat
 * @returns {ABITypeRef | undefined}
 */
function astMapTypeToAbi(key, keyFormat, value, valueFormat) {
  let abiKey;
  let abiKeyFormat;
  if (key === "Int") {
    const integer = tactIntegerFormat(keyFormat);
    if (integer === undefined || integer.format === "coins") {
      return undefined;
    }
    abiKey = integer.type;
    abiKeyFormat = integer.format;
  } else if (key === "Address" && keyFormat === undefined) {
    abiKey = "address";
  } else {
    return undefined;
  }

  let abiValue;
  let abiValueFormat;
  if (value === "Int") {
    const integer = tactIntegerFormat(valueFormat);
    if (integer === undefined) {
      return undefined;
    }
    abiValue = integer.type;
    abiValueFormat = integer.format;
  } else if (
    (value === "Bool" || value === "Address") &&
    valueFormat === undefined
  ) {
    abiValue = value === "Bool" ? "bool" : "address";
  } else if (value === "Cell" && valueFormat === undefined) {
    abiValue = "cell";
    abiValueFormat = "ref";
  } else if (
    !["Slice", "Builder", "String", "StringBuilder"].includes(value) &&
    (valueFormat === undefined || valueFormat === "reference")
  ) {
    abiValue = value;
    abiValueFormat = "ref";
  } else {
    return undefined;
  }

  return {
    kind: "dict",
    key: abiKey,
    keyFormat: abiKeyFormat,
    value: abiValue,
    valueFormat: abiValueFormat,
  };
}

/**
 * @param {string | undefined} format
 * @returns {{ type: string, format: string | number } | undefined}
 */
function tactIntegerFormat(format) {
  if (format === undefined || format === "int257") {
    return { type: "int", format: 257 };
  }
  if (
    format === "coins" ||
    format === "varuint16" ||
    format === "varuint32"
  ) {
    return { type: "uint", format };
  }
  if (format === "varint16" || format === "varint32") {
    return { type: "int", format };
  }
  const fixed = /^(u?int)(\d+)$/.exec(format);
  const bits = fixed === null ? 0 : Number(fixed[2]);
  return bits > 0 && bits <= 256
    ? { type: fixed[1] === "uint" ? "uint" : "int", format: bits }
    : undefined;
}

/**
 * @param {unknown} value
 * @returns {string | undefined}
 */
function astName(value) {
  if (typeof value === "string") {
    return value;
  }
  if (value !== null && typeof value === "object") {
    if (typeof value.value === "string") {
      return value.value;
    }
    if (typeof value.text === "string") {
      return value.text;
    }
  }
  return undefined;
}

/**
 * @param {ContractABI} tactAbi
 * @param {ABIType[]} types
 * @returns {GeneratedTolkSource}
 */
function tactAbiToTolk(tactAbi, types) {
  if (
    tactAbi === null ||
    typeof tactAbi !== "object" ||
    Array.isArray(tactAbi)
  ) {
    throw new Error("Tact ABI must be a JSON object");
  }
  if (typeof tactAbi.name !== "string" || tactAbi.name.length === 0) {
    throw new Error("Tact ABI contract name is required");
  }

  const usedTypeNames = new Set();
  const contractName = uniqueIdentifier(
    tactAbi.name,
    "Contract",
    usedTypeNames,
  );
  const typeNames = new Map();
  for (const type of types) {
    if (typeof type?.name !== "string" || type.name.length === 0) {
      throw new Error("Tact ABI type name is required");
    }
    const originalName = identifier(type.name, "Type");
    const prefixedName = originalName.startsWith(contractName)
      ? originalName
      : `${contractName}${originalName}`;
    typeNames.set(
      type.name,
      uniqueIdentifier(prefixedName, "Type", usedTypeNames),
    );
  }

  const typeName = (name) => typeNames.get(name) ?? identifier(name, "Type");
  const typesByName = new Map(types.map((type) => [type.name, type]));
  const selectedTypes = new Set();

  /**
   * @param {ABITypeRef} type
   * @param {Set<string>} selected
   * @param {boolean} [serializable]
   * @param {Set<string>} [visiting]
   */
  function collectTypeReference(
    type,
    selected,
    serializable = false,
    visiting = new Set(),
  ) {
    renderType(type, typeName);
    if (
      serializable &&
      type.kind === "simple" &&
      (type.type === "slice" || type.type === "builder") &&
      type.format !== "remainder"
    ) {
      throw new Error(
        `Tact ABI type cannot be serialized by Tolk: ${type.type}`,
      );
    }
    if (type.kind === "dict") {
      if (!PRIMITIVE_TYPES.has(type.key)) {
        collectStruct(type.key, selected, true, visiting);
      }
      if (!PRIMITIVE_TYPES.has(type.value)) {
        collectStruct(type.value, selected, true, visiting);
      }
      return;
    }
    if (!PRIMITIVE_TYPES.has(type.type)) {
      collectStruct(
        type.type,
        selected,
        serializable || type.format === "ref",
        visiting,
      );
    }
  }

  /**
   * @param {string} name
   * @param {Set<string>} selected
   * @param {boolean} [serializable]
   * @param {Set<string>} [visiting]
   */
  function collectStruct(
    name,
    selected,
    serializable = false,
    visiting = new Set(),
  ) {
    const visitKey = `${serializable ? "serialized" : "stack"}:${name}`;
    if (visiting.has(visitKey)) {
      return;
    }
    const type = typesByName.get(name);
    if (type === undefined) {
      throw new Error(`Tact ABI type is not defined: ${name}`);
    }
    renderStruct(type, typeName);
    visiting.add(visitKey);
    selected.add(name);
    for (const field of Array.isArray(type.fields) ? type.fields : []) {
      collectTypeReference(field?.type, selected, serializable, visiting);
    }
    visiting.delete(visitKey);
  }

  const contractProperties = [];
  const storageTypeName = `${tactAbi.name}$Data`;
  const storageName = typeNames.get(storageTypeName);
  if (storageName !== undefined) {
    collectStruct(storageTypeName, selectedTypes, true);
    contractProperties.push(`    storage: ${storageName}`);
  }

  const receivers = Array.isArray(tactAbi.receivers) ? tactAbi.receivers : [];
  const internalMessages = typedReceiverNames(
    receivers,
    "internal",
    typeName,
    selectedTypes,
    collectStruct,
  );
  const externalMessages = typedReceiverNames(
    receivers,
    "external",
    typeName,
    selectedTypes,
    collectStruct,
  );
  const aliases = [];
  addMessageProperty({
    property: "incomingMessages",
    suffix: "IncomingMessage",
    messages: internalMessages,
    contractName,
    contractProperties,
    aliases,
    usedTypeNames,
  });
  addMessageProperty({
    property: "incomingExternal",
    suffix: "IncomingExternalMessage",
    messages: externalMessages,
    contractName,
    contractProperties,
    aliases,
    usedTypeNames,
  });

  const errors = renderErrors(tactAbi.errors, contractName, usedTypeNames);
  if (errors !== undefined) {
    contractProperties.push(`    thrownErrors: ${errors.name}`);
  }

  const lines = [
    "// Generated from the verified Tact contract ABI.",
    `contract ${contractName} {`,
    ...contractProperties,
    "}",
  ];

  const rawGetters = Array.isArray(tactAbi.getters) ? tactAbi.getters : [];
  for (const getter of rawGetters) {
    for (const argument of Array.isArray(getter?.arguments)
      ? getter.arguments
      : []) {
      collectTypeReference(argument?.type, selectedTypes);
    }
    if (getter?.returnType) {
      collectTypeReference(getter.returnType, selectedTypes);
    }
  }

  for (const type of types.filter((type) => selectedTypes.has(type.name))) {
    lines.push("", renderStruct(type, typeName));
  }
  for (const alias of aliases) {
    lines.push("", alias);
  }
  if (errors !== undefined) {
    lines.push("", errors.source);
  }

  const getters = new Map();
  const usedGetterNames = new Set();
  for (const getter of rawGetters) {
    if (typeof getter?.name !== "string" || getter.name.length === 0) {
      throw new Error("Tact ABI getter name is required");
    }
    const sourceGetterName = TOLK_GETTER_NAME_CONFLICTS.has(getter.name)
      ? `${getter.name}_`
      : getter.name;
    const getterName = uniqueIdentifier(
      sourceGetterName,
      "getter",
      usedGetterNames,
    );
    const usedArgumentNames = new Set();
    const argumentsSource = (
      Array.isArray(getter.arguments) ? getter.arguments : []
    )
      .map((argument, index) => {
        const name = uniqueIdentifier(
          argument?.name,
          `argument${index + 1}`,
          usedArgumentNames,
        );
        return `${name}: ${renderType(argument?.type, typeName)}`;
      })
      .join(", ");
    const returnType = getter.returnType
      ? renderType(getter.returnType, typeName)
      : "void";
    const tactMethodId = Number.isInteger(getter.methodId)
      ? getter.methodId
      : getMethodId(getter.name);
    lines.push("");
    if (getterName !== getter.name) {
      lines.push(`// Tact getter name: ${getter.name}`);
    }
    if (tactMethodId !== getMethodId(getterName)) {
      lines.push(`// Tact method ID: ${tactMethodId}`);
    }
    lines.push(
      `get fun ${getterName}(${argumentsSource}): ${returnType} {`,
      "    throw 0;",
      "}",
    );
    getters.set(getterName, getter);
  }

  lines.push("");
  return { contractName, getters, source: lines.join("\n") };
}

/**
 * @param {TactPackage} tactPackage
 * @param {GeneratedSource[]} generatedSources
 * @returns {ContractABI | undefined}
 */
function findMainTactAbi(tactPackage, generatedSources) {
  const packageName = tactPackage?.name;
  for (const source of generatedSources) {
    if (!source.path.endsWith(".abi")) {
      continue;
    }
    const abi = JSON.parse(source.content);
    if (abi?.name === packageName) {
      return abi;
    }
  }

  if (typeof tactPackage?.abi === "string") {
    const abi = JSON.parse(tactPackage.abi);
    if (abi?.name === packageName) {
      return abi;
    }
  } else if (tactPackage?.abi?.name === packageName) {
    return tactPackage.abi;
  }

  return undefined;
}

/**
 * @param {ABIReceiver[]} receivers
 * @param {"internal" | "external"} receiverKind
 * @param {(name: string) => string} typeName
 * @param {Set<string>} selectedTypes
 * @param {CollectStruct} collectStruct
 * @returns {string[]}
 */
function typedReceiverNames(
  receivers,
  receiverKind,
  typeName,
  selectedTypes,
  collectStruct,
) {
  const names = [];
  const seen = new Set();
  for (const receiver of receivers) {
    const message = receiver?.message;
    if (
      receiver?.receiver !== receiverKind ||
      message?.kind !== "typed" ||
      typeof message.type !== "string"
    ) {
      continue;
    }
    const messageTypes = new Set(selectedTypes);
    try {
      collectStruct(message.type, messageTypes, true);
    } catch {
      continue;
    }
    selectedTypes.clear();
    for (const selected of messageTypes) {
      selectedTypes.add(selected);
    }

    const name = typeName(message.type);
    if (!seen.has(name)) {
      names.push(name);
      seen.add(name);
    }
  }
  return names;
}

/**
 * @param {{
 *   property: string,
 *   suffix: string,
 *   messages: string[],
 *   contractName: string,
 *   contractProperties: string[],
 *   aliases: string[],
 *   usedTypeNames: Set<string>,
 * }} options
 */
function addMessageProperty({
  property,
  suffix,
  messages,
  contractName,
  contractProperties,
  aliases,
  usedTypeNames,
}) {
  if (messages.length === 0) {
    return;
  }
  if (messages.length === 1) {
    contractProperties.push(`    ${property}: ${messages[0]}`);
    return;
  }

  const aliasName = uniqueIdentifier(
    `${contractName}${suffix}`,
    suffix,
    usedTypeNames,
  );
  contractProperties.push(`    ${property}: ${aliasName}`);
  aliases.push(
    `type ${aliasName} =\n${messages.map((name) => `    | ${name}`).join("\n")}`,
  );
}

/**
 * @param {ABIType} type
 * @param {(name: string) => string} typeName
 * @returns {string}
 */
function renderStruct(type, typeName) {
  const name = typeName(type.name);
  let header = "";
  if (type.header !== null && type.header !== undefined) {
    if (
      !Number.isInteger(type.header) ||
      type.header < 0 ||
      type.header > 0xffffffff
    ) {
      throw new Error(
        `invalid Tact ABI header for ${type.name}: ${type.header}`,
      );
    }
    header = ` (0x${type.header.toString(16).padStart(8, "0")})`;
  }

  const usedFieldNames = new Set();
  const fields = (Array.isArray(type.fields) ? type.fields : []).map(
    (field, index) => {
      const fieldName = uniqueIdentifier(
        field?.name,
        `field${index + 1}`,
        usedFieldNames,
      );
      return `    ${fieldName}: ${renderType(field?.type, typeName)}`;
    },
  );
  return [`struct${header} ${name} {`, ...fields, "}"].join("\n");
}

/**
 * @param {ABITypeRef} type
 * @param {(name: string) => string} typeName
 * @returns {string}
 */
function renderType(type, typeName) {
  if (type === null || typeof type !== "object" || Array.isArray(type)) {
    throw new Error("invalid Tact ABI type reference");
  }
  if (type.kind === "dict") {
    const key = renderDictionaryPart(type.key, type.keyFormat, typeName);
    const value = renderDictionaryPart(type.value, type.valueFormat, typeName);
    return `map<${key}, ${value}>`;
  }
  if (type.kind !== "simple" || typeof type.type !== "string") {
    throw new Error(`unsupported Tact ABI type kind: ${String(type.kind)}`);
  }

  let rendered;
  if (type.format === "remainder") {
    rendered = "RemainingBitsAndRefs";
  } else if (type.format === "ref") {
    rendered = `Cell<${typeName(type.type)}>`;
  } else if (type.type === "fixed-bytes") {
    if (!Number.isInteger(type.format) || type.format <= 0) {
      throw new Error(`invalid fixed-bytes format: ${String(type.format)}`);
    }
    rendered = `bits${type.format * 8}`;
  } else if (type.type === "int" || type.type === "uint") {
    rendered = renderInteger(type.type, type.format);
  } else if (PRIMITIVE_TYPES.has(type.type)) {
    rendered = type.type;
  } else {
    rendered = typeName(type.type);
  }

  return type.optional === true ? `${rendered}?` : rendered;
}

/**
 * @param {string} type
 * @param {string | number | boolean | null | undefined} format
 * @param {(name: string) => string} typeName
 * @returns {string}
 */
function renderDictionaryPart(type, format, typeName) {
  if (typeof type !== "string") {
    throw new Error("invalid Tact ABI dictionary type");
  }
  if (type === "int" || type === "uint") {
    return renderInteger(type, format);
  }
  if (type === "cell") {
    if (format !== undefined && format !== null && format !== "ref") {
      throw new Error(`unsupported Tact ABI dictionary cell format: ${format}`);
    }
    return "cell";
  }
  if (PRIMITIVE_TYPES.has(type)) {
    if (format === "ref") {
      throw new Error(`unsupported Tact ABI dictionary ref type: ${type}`);
    }
    return type;
  }
  if (format === undefined || format === null || format === "ref") {
    return `Cell<${typeName(type)}>`;
  }
  throw new Error(`unsupported Tact ABI dictionary format: ${format}`);
}

/**
 * @param {string} type
 * @param {string | number | boolean | null | undefined} format
 * @returns {string}
 */
function renderInteger(type, format) {
  if (format === undefined || format === null || format === 257) {
    return "int257";
  }
  if (
    format === "coins" ||
    format === "varint16" ||
    format === "varint32" ||
    format === "varuint16" ||
    format === "varuint32"
  ) {
    return format;
  }
  if (Number.isInteger(format) && format > 0 && format <= 256) {
    return `${type}${format}`;
  }
  throw new Error(`unsupported Tact ABI integer format: ${String(format)}`);
}

/**
 * @param {ContractABI["errors"]} rawErrors
 * @param {string} contractName
 * @param {Set<string>} usedTypeNames
 * @returns {{ name: string, source: string } | undefined}
 */
function renderErrors(rawErrors, contractName, usedTypeNames) {
  if (
    rawErrors === null ||
    typeof rawErrors !== "object" ||
    Array.isArray(rawErrors)
  ) {
    return undefined;
  }
  const entries = Object.entries(rawErrors)
    .map(([code, value]) => ({ code: Number(code), message: value?.message }))
    .filter(
      ({ code, message }) =>
        Number.isInteger(code) && typeof message === "string",
    )
    .sort((left, right) => left.code - right.code);
  if (entries.length === 0) {
    return undefined;
  }

  const name = uniqueIdentifier(
    `${contractName}Errors`,
    "TactErrors",
    usedTypeNames,
  );
  const usedMembers = new Set();
  const lines = [`enum ${name} {`];
  for (const entry of entries) {
    const comment = entry.message.replace(/[\r\n]+/g, " ").trim();
    if (comment.length > 0) {
      lines.push(`    /// ${comment}`);
    }
    const suggestedName = entry.message
      .match(/[A-Za-z0-9]+/g)
      ?.map((part) => `${part[0].toUpperCase()}${part.slice(1)}`)
      .join("");
    const member = uniqueIdentifier(
      suggestedName,
      `Error${entry.code}`,
      usedMembers,
    );
    lines.push(`    ${member} = ${entry.code}`);
  }
  lines.push("}");
  return { name, source: lines.join("\n") };
}

/**
 * @param {unknown} value
 * @param {string} fallback
 * @param {Set<string>} used
 * @returns {string}
 */
function uniqueIdentifier(value, fallback, used) {
  const base = identifier(value, fallback);
  let candidate = base;
  let suffix = 2;
  while (used.has(candidate)) {
    candidate = `${base}${suffix}`;
    suffix += 1;
  }
  used.add(candidate);
  return candidate;
}

/**
 * @param {unknown} value
 * @param {string} fallback
 * @returns {string}
 */
function identifier(value, fallback) {
  const text = typeof value === "string" ? value : "";
  let result = text.replace(/\$+([A-Za-z0-9_])/g, (_, next) =>
    next.toUpperCase(),
  );
  result = result.replace(/[^A-Za-z0-9_]/g, "_");
  if (result.length === 0) {
    result = fallback;
  }
  if (!/^[A-Za-z_]/.test(result)) {
    result = `_${result}`;
  }
  if (TOLK_KEYWORDS.has(result)) {
    result = `${result}_`;
  }
  return result;
}
