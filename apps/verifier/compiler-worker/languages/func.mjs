import { FuncCompiler } from "@ton-community/func-js";

import { bocBase64CodeHashHex, compileSourcesByPath, normalizeSourcePath } from "./common.mjs";
import { importFunc } from "./registry.mjs";

export async function compileFunc(input) {
  const funcModule = await importFunc(input.compiler_version);
  const targets = funcTargets(input.sources);
  if (targets.length === 0) {
    throw new Error("FunC requires at least one target source");
  }

  const result = await new FuncCompiler(funcModule.object).compileFunc({
    sources: compileSourcesByPath(input.sources),
    targets,
  });

  if (result.status === "error") {
    return { status: "compile_error", error: result.message };
  }

  return {
    status: "ok",
    code_hash: bocBase64CodeHashHex(result.codeBoc),
  };
}

function funcTargets(sources) {
  const targets = sources
    .filter((source) => source.include_in_command ?? source.is_entrypoint)
    .map((source) => normalizeSourcePath(source.path));

  if (targets.length > 0) {
    return targets;
  }

  return sources.length === 0 ? [] : [normalizeSourcePath(sources[0].path)];
}
