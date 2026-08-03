import path from "node:path";
import semver from "semver";

import { buildSourceMap, normalizeSourcePath } from "./common.mjs";
import { importTolk } from "./registry.mjs";

const SOURCE_MAP_MIN_TOLK_VERSION = "1.4.0";

export async function compileTolk(input) {
  const sources = buildSourceMap(input.sources);
  const importMappings = buildImportMappings(input.import_mappings);
  const entrypointFileName = normalizeSourcePath(input.entrypoint);
  const includeSourceMapData = supportsSourceMapData(input.compiler_version);
  const { runTolkCompiler } = await importTolk(input.compiler_version);
  const result = await runTolkCompiler({
    entrypointFileName,
    pathMappings: pathMappingsObject(importMappings),
    fsReadCallback: (requestedPath) => readSourceContent(requestedPath, sources, importMappings),
    withSourceMapData: includeSourceMapData,
  });

  if (result.status === "error") {
    return { status: "compile_error", error: result.message };
  }

  return {
    status: "ok",
    code_hash: String(result.codeHashHex).toLowerCase(),
    generated_sources: generatedSources(entrypointFileName, result),
    source_map: includeSourceMapData ? buildSourceMapData(result) : undefined,
  };
}

function readSourceContent(requestedPath, sources, importMappings) {
  const resolvedPath = resolveSourcePath(requestedPath, sources, importMappings);
  const content = sources.get(resolvedPath);
  if (content === undefined) {
    throw new Error(`source was not provided: ${requestedPath}`);
  }
  return content;
}

function generatedSources(entrypointFileName, result) {
  if (result.abiJson === undefined || result.abiJson === null) {
    return [];
  }

  return [
    {
      path: generatedAbiPath(entrypointFileName),
      content: `${JSON.stringify(result.abiJson, null, 2)}\n`,
    },
  ];
}

function generatedAbiPath(entrypointFileName) {
  const parsed = path.posix.parse(normalizeSourcePath(entrypointFileName));
  const name = parsed.name || "contract";
  return path.posix.join("output", `${name}.abi.json`);
}

function buildSourceMapData(result) {
  const requiredFields = [
    "codeBoc64",
    "symbolTypesJson",
    "debugMarksJson",
    "debugMarksBase64",
  ];
  const missingFields = requiredFields.filter((field) => result[field] === undefined);
  if (missingFields.length > 0) {
    throw new Error(
      `Tolk compiler did not return source map data fields: ${missingFields.join(", ")}`,
    );
  }

  return {
    code_boc64: result.codeBoc64,
    symbol_types_json: result.symbolTypesJson,
    debug_marks_json: result.debugMarksJson,
    debug_marks_base64: result.debugMarksBase64,
  };
}

function buildImportMappings(inputMappings) {
  if (inputMappings === undefined) {
    return [];
  }

  return Object.entries(inputMappings)
    .map(([prefix, target]) => ({
      prefix: normalizeSourcePath(prefix),
      target: normalizeSourcePath(target),
    }))
    .sort((left, right) => right.prefix.length - left.prefix.length);
}

function pathMappingsObject(importMappings) {
  return Object.fromEntries(importMappings.map((mapping) => [mapping.prefix, mapping.target]));
}

function resolveSourcePath(requestedPath, sources, importMappings) {
  const sourcePath = normalizeSourcePath(requestedPath);
  const candidates = [sourcePath];

  for (const mapping of importMappings) {
    const suffix = mappedSuffix(sourcePath, mapping.prefix);
    if (suffix === undefined) {
      continue;
    }

    candidates.push(joinMappingTarget(mapping.target, suffix));
  }

  for (const candidate of candidates) {
    const normalizedCandidate = normalizeSourcePath(candidate);
    if (sources.has(normalizedCandidate)) {
      return normalizedCandidate;
    }

    if (!path.posix.extname(normalizedCandidate)) {
      const tolkCandidate = `${normalizedCandidate}.tolk`;
      if (sources.has(tolkCandidate)) {
        return tolkCandidate;
      }
    }
  }

  return sourcePath;
}

function mappedSuffix(sourcePath, prefix) {
  if (sourcePath === prefix) {
    return "";
  }

  const prefixWithSlash = `${prefix}/`;
  if (sourcePath.startsWith(prefixWithSlash)) {
    return sourcePath.slice(prefixWithSlash.length);
  }

  return undefined;
}

function joinMappingTarget(target, suffix) {
  if (suffix.length === 0) {
    return target;
  }

  return path.posix.join(target, suffix);
}

function supportsSourceMapData(version) {
  const parsed = semver.coerce(version);
  return parsed !== null && semver.gte(parsed, SOURCE_MAP_MIN_TOLK_VERSION);
}
