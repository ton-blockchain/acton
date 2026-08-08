import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const TOLK_SOURCE_MAP_VERSIONS = ["1.4.0", "1.4.1", "1.4.2"];
const TOLK_ALLOW_NO_ENTRYPOINT_VERSIONS = new Set(["1.4.2"]);

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const compilerWorkerDir = path.resolve(scriptDir, "..");

for (const version of TOLK_SOURCE_MAP_VERSIONS) {
  const packageDir = path.join(
    compilerWorkerDir,
    "node_modules",
    `tolk-${version}`,
    "dist",
  );
  patchJavaScript(path.join(packageDir, "index.js"), version);
  patchTypes(path.join(packageDir, "index.d.ts"), version);
}

function patchJavaScript(filePath, version) {
  replaceInFile(
    filePath,
    version,
    "        withSrcLineComments: compilerConfig.withSrcLineComments,\n",
    [
      "        withSrcLineComments: compilerConfig.withSrcLineComments,\n",
      "        withSymbolTypes: compilerConfig.withSourceMapData,\n",
      "        withDebugMarks: compilerConfig.withSourceMapData,\n",
    ].join(""),
  );
  if (TOLK_ALLOW_NO_ENTRYPOINT_VERSIONS.has(version)) {
    replaceInFile(
      filePath,
      version,
      "        entrypointFileName: compilerConfig.entrypointFileName,\n",
      [
        "        entrypointFileName: compilerConfig.entrypointFileName,\n",
        "        allowNoEntrypoint: compilerConfig.allowNoEntrypoint,\n",
      ].join(""),
    );
  }
}

function patchTypes(filePath, version) {
  replaceInFile(
    filePath,
    version,
    "    withSrcLineComments?: boolean;\n",
    [
      "    withSrcLineComments?: boolean;\n",
      "    withSourceMapData?: boolean;\n",
    ].join(""),
  );
  if (TOLK_ALLOW_NO_ENTRYPOINT_VERSIONS.has(version)) {
    replaceInFile(
      filePath,
      version,
      "    entrypointFileName: string;\n",
      [
        "    entrypointFileName: string;\n",
        "    allowNoEntrypoint?: boolean;\n",
      ].join(""),
    );
  }
  replaceInFile(
    filePath,
    version,
    "    stderr: string;\n",
    [
      "    stderr: string;\n",
      "    symbolTypesJson?: unknown;\n",
      "    debugMarksJson?: unknown;\n",
      "    debugMarksBase64?: string;\n",
    ].join(""),
  );
}

function replaceInFile(filePath, version, search, replacement) {
  const current = readFileSync(filePath, "utf8");
  if (current.includes(replacement)) {
    return;
  }
  if (!current.includes(search)) {
    throw new Error(
      `Could not patch tolk-${version}: expected fragment not found in ${filePath}`,
    );
  }
  writeFileSync(filePath, current.replace(search, replacement));
}
