import path from "node:path";
import { Cell } from "@ton/core";

export async function readStdin(stream) {
  const chunks = [];
  for await (const chunk of stream) {
    chunks.push(chunk);
  }
  return Buffer.concat(chunks).toString("utf8");
}

export function writeOutput(stream, output) {
  stream.write(`${JSON.stringify(output)}\n`);
}

export function validateInput(input) {
  if (typeof input?.language !== "string" || input.language.length === 0) {
    throw new Error("language is required");
  }
  if (typeof input.compiler_version !== "string" || input.compiler_version.length === 0) {
    throw new Error("compiler_version is required");
  }
  if (typeof input.entrypoint !== "string" || input.entrypoint.length === 0) {
    throw new Error("entrypoint is required");
  }
  if (!Array.isArray(input.sources) || input.sources.length === 0) {
    throw new Error("sources are required");
  }
  if (input.import_mappings !== undefined && !isPlainObject(input.import_mappings)) {
    throw new Error("import_mappings must be an object");
  }
}

export function buildSourceMap(inputSources) {
  const sources = new Map();
  for (const source of inputSources) {
    if (typeof source?.path !== "string" || source.path.length === 0) {
      throw new Error("source path is required");
    }
    if (typeof source.content !== "string") {
      throw new Error(`source content is required: ${source.path}`);
    }

    const sourcePath = normalizeSourcePath(source.path);
    if (sources.has(sourcePath)) {
      throw new Error(`duplicate source path: ${source.path}`);
    }
    sources.set(sourcePath, source.content);
  }

  return sources;
}

export function normalizeSourcePath(sourcePath) {
  if (typeof sourcePath !== "string" || sourcePath.length === 0) {
    throw new Error("source path is required");
  }
  if (sourcePath.includes("\\")) {
    throw new Error(`source path must use '/' separators: ${sourcePath}`);
  }
  if (path.posix.isAbsolute(sourcePath) || sourcePath.split("/").includes("..")) {
    throw new Error(`invalid source path: ${sourcePath}`);
  }

  const normalized = path.posix.normalize(sourcePath);
  if (
    normalized === "." ||
    normalized === ".." ||
    normalized.startsWith("../") ||
    path.posix.isAbsolute(normalized)
  ) {
    throw new Error(`invalid source path: ${sourcePath}`);
  }

  return normalized;
}

export function compileSourcesByPath(inputSources) {
  return Object.fromEntries(
    inputSources.map((source) => [normalizeSourcePath(source.path), source.content]),
  );
}

export function bocBase64CodeHashHex(bocBase64) {
  return Cell.fromBoc(Buffer.from(bocBase64, "base64"))[0].hash().toString("hex");
}

export function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
