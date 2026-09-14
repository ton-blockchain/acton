import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

test("prunes development files while preserving runtime assets, metadata and symlinks", (t) => {
  const root = mkdtempSync(path.join(tmpdir(), "verifier-prune-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const worker = path.join(root, "worker");
  const modules = path.join(worker, "node_modules");
  const script = path.join(
    worker, "scripts", "patch-prune-runtime-dependencies.mjs",
  );
  mkdirSync(path.dirname(script), { recursive: true });
  copyFileSync(new URL("./patch-prune-runtime-dependencies.mjs", import.meta.url), script);

  // Include both npm aliases and dependencies nested below another package.
  for (const [directory, name, main] of [
    ["tolk-test", "@ton/tolk-js", "dist/index.js"],
    ["tact-test", "@tact-lang/compiler", "./dist/index.js"],
    ["tact-test/node_modules/@tact-lang/opcode", "@tact-lang/opcode", "dist/index.js"],
    ["tact-test/node_modules/@ton/sandbox", "@ton/sandbox", "dist/index.js"],
    ["tact-test/node_modules/core-alias", "@ton/core", "dist/index.js"],
    ["@ton/core", "@ton/core", "dist/index.js"],
    ["@ton/core/node_modules/other-package", "other-package", "index.js"],
    ["ton-core", "ton-core", "dist/index.js"],
    ["ohm-js", "ohm-js", "index.js"],
  ]) {
    write(path.join(directory, "package.json"), JSON.stringify({ name, main }));
    write(path.join(directory, main), "module.exports = {};\n");
  }

  const removed = [
    "tact-test/dist/index.d.ts",
    "tact-test/dist/index.js.map",
    "tact-test/node_modules/@tact-lang/opcode/dist/index.d.ts",
    "@types/node/index.d.ts",
    "@ton/core/dist/boc/Cell.spec.js",
    "@ton/core/dist/boc/Cell.test.js",
    "ton-core/dist/boc/Cell.spec.js",
    "ton-core/dist/boc/Cell.test.js",
    "tact-test/node_modules/core-alias/dist/boc/Cell.spec.js",
  ];
  const preserved = [
    "tolk-test/dist/index.d.ts",
    "tolk-test/dist/index.js.map",
    "tact-test/package.json",
    "tact-test/dist/index.js",
    "tact-test/dist/asm/coverage/index.js",
    "tact-test/dist/asm/coverage/index.d.ts",
    "tact-test/dist/asm/coverage/index.js.map",
    "tact-test/dist/func/funcfiftlib.wasm.js",
    "tact-test/stdlib/stdlib.tact",
    "tact-test/stdlib/stdlib.fc",
    "tact-test/LICENSE",
    "tact-test/LICENSE.map",
    "tact-test/NOTICE.d.ts",
    "@types/node/package.json",
    "@types/node/LICENSE",
    "assets/data.map/runtime.json",
    "@ton/core/package.json",
    "@ton/core/dist/index.js",
    "@ton/core/dist/boc/Cell.js",
    "@ton/core/LICENSE.spec.js",
    "@ton/core/node_modules/other-package/index.spec.js",
    "ton-core/package.json",
    "ton-core/dist/index.js",
    "ton-core/dist/boc/Cell.js",
    "tact-test/node_modules/core-alias/dist/boc/Cell.js",
    "tact-test/dist/index.spec.js",
    "ohm-js/src/main.test.js",
  ];
  for (const filename of [...removed, ...preserved]) {
    if (filename.endsWith("package.json")) {
      if (filename === "@types/node/package.json") {
        write(filename, JSON.stringify({ name: "@types/node" }));
      }
    } else {
      write(filename, "keep this content\n");
    }
  }
  const snapshots = new Map(
    preserved.map((filename) => [
      filename, readFileSync(path.join(modules, filename), "utf8"),
    ]),
  );

  const outside = path.join(root, "linked-package");
  mkdirSync(outside);
  writeFileSync(path.join(outside, "index.d.ts"), "linked declaration\n");
  writeFileSync(path.join(outside, "index.spec.js"), "linked test\n");
  symlinkSync(outside, path.join(modules, "linked-package"), "dir");
  symlinkSync(path.join(outside, "index.d.ts"), path.join(modules, "linked.d.ts"));
  symlinkSync(outside, path.join(modules, "@ton/core/dist/linked-tests"), "dir");
  symlinkSync(
    path.join(outside, "index.spec.js"),
    path.join(modules, "@ton/core/dist/linked.spec.js"),
  );

  for (let run = 0; run < 2; run++) {
    const result = spawnSync(process.execPath, [script], { encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
    for (const filename of removed) {
      assert.equal(existsSync(path.join(modules, filename)), false, filename);
    }
    for (const [filename, content] of snapshots) {
      assert.equal(readFileSync(path.join(modules, filename), "utf8"), content, filename);
    }
    assert.equal(
      readFileSync(path.join(modules, "linked-package/index.d.ts"), "utf8"),
      "linked declaration\n",
    );
    assert.equal(
      readFileSync(path.join(modules, "linked.d.ts"), "utf8"),
      "linked declaration\n",
    );
    for (const filename of [
      "@ton/core/dist/linked-tests/index.spec.js",
      "@ton/core/dist/linked.spec.js",
    ]) {
      assert.equal(readFileSync(path.join(modules, filename), "utf8"), "linked test\n");
    }
  }

  function write(filename, content) {
    const target = path.join(modules, filename);
    mkdirSync(path.dirname(target), { recursive: true });
    writeFileSync(target, content);
  }
});

test("Tolk patches update JavaScript and preserved declarations repeatably", (t) => {
  const root = mkdtempSync(path.join(tmpdir(), "verifier-tolk-patch-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const script = path.join(root, "scripts", "patch-tolk-source-map-data.mjs");
  mkdirSync(path.dirname(script), { recursive: true });
  copyFileSync(new URL("./patch-tolk-source-map-data.mjs", import.meta.url), script);

  const runtimeFiles = ["1.4.0", "1.4.1", "1.4.2"].map((version) => {
    const file = path.join(root, "node_modules", `tolk-${version}`, "dist", "index.js");
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, [
      "        withSrcLineComments: compilerConfig.withSrcLineComments,\n",
      "        entrypointFileName: compilerConfig.entrypointFileName,\n",
    ].join(""));
    writeFileSync(path.join(path.dirname(file), "index.d.ts"), [
      "    withSrcLineComments?: boolean;\n",
      "    entrypointFileName: string;\n",
      "    stderr: string;\n",
    ].join(""));
    return file;
  });

  let patched;
  for (let run = 0; run < 2; run++) {
    const result = spawnSync(process.execPath, [script], { encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
    const contents = runtimeFiles.map((file) => readFileSync(file, "utf8"));
    for (const content of contents) {
      assert.match(content, /withSymbolTypes: compilerConfig.withSourceMapData/);
      assert.match(content, /withDebugMarks: compilerConfig.withSourceMapData/);
    }
    assert.match(contents[2], /allowNoEntrypoint: compilerConfig.allowNoEntrypoint/);
    const declarations = runtimeFiles.map((file) =>
      readFileSync(path.join(path.dirname(file), "index.d.ts"), "utf8"),
    );
    for (const declaration of declarations) {
      assert.match(declaration, /withSourceMapData\?: boolean/);
      assert.match(declaration, /symbolTypesJson\?: unknown/);
      assert.match(declaration, /debugMarksJson\?: unknown/);
      assert.match(declaration, /debugMarksBase64\?: string/);
    }
    assert.match(declarations[2], /allowNoEntrypoint\?: boolean/);
    const patchedFiles = [...contents, ...declarations];
    if (patched) {
      assert.deepEqual(patchedFiles, patched);
    }
    patched = patchedFiles;
  }
});
