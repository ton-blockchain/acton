import semver from "semver";

import { bocBase64CodeHashHex, normalizeSourcePath } from "./common.mjs";
import { importTact } from "./registry.mjs";
import { generatedTolkAbiSources } from "./tact-abi.mjs";

export async function compileTact(input) {
  const pkgSource = tactPkgSource(input);
  const pkg = pkgSource.content;
  const pkgJson = parsePkg(pkg);
  const packageVersion = pkgJson?.compiler?.version;
  if (typeof packageVersion !== "string" || packageVersion.length === 0) {
    throw new Error("Tact pkg compiler.version is required");
  }
  if (packageVersion !== input.compiler_version) {
    throw new Error(
      `Tact compiler_version mismatch: compile_params=${input.compiler_version}, pkg=${packageVersion}`,
    );
  }

  const importedTactModule = await importTact(input.compiler_version);
  const tactModule =
    typeof importedTactModule.verify === "function"
      ? importedTactModule
      : importedTactModule.default;
  if (typeof tactModule?.verify !== "function") {
    throw new Error(`Tact ${input.compiler_version} does not export verify()`);
  }
  const output = [];
  const verificationResult = await tactModule.verify({
    pkg,
    logger: tactLogger(tactModule, input.compiler_version, output),
  });

  if (!verificationResult.ok) {
    return {
      status: "compile_error",
      error: [String(verificationResult.error), ...output.map(String)].join(
        "\n",
      ),
    };
  }

  const generated = generatedSources(pkgSource.path, verificationResult.files);
  const generatedTolkAbi = await generatedTolkAbiSources(
    verificationResult.package,
    generated,
  );
  if (generatedTolkAbi !== undefined) {
    generated.push(...generatedTolkAbi);
  }

  return {
    status: "ok",
    code_hash: bocBase64CodeHashHex(verificationResult.package.code),
    generated_sources: generated,
  };
}

function tactPkgSource(input) {
  const pkgSource = input.sources
    .filter((source) => normalizeSourcePath(source.path).endsWith(".pkg"))
    .sort(
      (left, right) =>
        left.path.split("/").length - right.path.split("/").length,
    )[0];

  if (!pkgSource) {
    throw new Error("Tact requires a .pkg source");
  }

  return pkgSource;
}

function parsePkg(pkg) {
  try {
    return JSON.parse(pkg);
  } catch (error) {
    throw new Error(`invalid Tact pkg JSON: ${error.message}`);
  }
}

function tactLogger(tactModule, version, output) {
  if (semver.lte(version, "1.4.0")) {
    return {
      log: (message) => output.push(message),
      error: (message) => output.push(message),
    };
  }

  const Logger = tactModule.Logger;
  if (typeof Logger === "function") {
    return new (class extends Logger {
      debug(message) {
        output.push(message);
      }
      info(message) {
        output.push(message);
      }
      warn(message) {
        output.push(message);
      }
      error(message) {
        output.push(message);
      }
    })();
  }

  return {
    debug: (message) => output.push(message),
    info: (message) => output.push(message),
    warn: (message) => output.push(message),
    error: (message) => output.push(message),
  };
}

function generatedSources(originalPkgPath, files) {
  return Object.entries(files ?? {})
    .filter(([filename]) => {
      const normalized = normalizeSourcePath(filename);
      return (
        /\.(abi|pkg|tact)$/.test(normalized) &&
        normalized !== normalizeSourcePath(originalPkgPath)
      );
    })
    .map(([filename, contentBase64]) => {
      const path = normalizeSourcePath(filename);
      let content = Buffer.from(contentBase64, "base64").toString("utf8");
      if (path.endsWith(".abi")) {
        content = JSON.stringify(JSON.parse(content), null, 2);
      }
      return { path, content };
    });
}
