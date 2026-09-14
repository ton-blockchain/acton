import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  linkSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

test("deduplicates identical WASM payloads across nested packages repeatably", (t) => {
  const { modules, write, run } = fixture(t);
  const content = "exports.wasm = 'AAAA';\n";
  const original = write("func-a/dist/compiler.wasm.js", content);
  const duplicate = write("tact-b/node_modules/func-c/dist/compiler.wasm.js", content);
  const linked = path.join(modules, "already-linked.wasm.js");
  linkSync(original, linked);

  // Same size, different bytes must not be merged; ordinary JS is out of scope.
  const different = write("func-d/dist/compiler.wasm.js", "exports.wasm = 'BBBB';\n");
  const javascript = write("func-a/dist/compiler.js", content);
  const sourceMap = write("func-a/dist/compiler.wasm.js.map", content);
  const files = [original, duplicate, linked, different, javascript, sourceMap];
  const snapshots = files.map((file) => readFileSync(file));
  const differentInode = statSync(different).ino;
  const javascriptInode = statSync(javascript).ino;
  const sourceMapInode = statSync(sourceMap).ino;

  let inodes;
  for (let iteration = 0; iteration < 2; iteration++) {
    run();
    assert.equal(statSync(original).ino, statSync(duplicate).ino);
    assert.equal(statSync(original).ino, statSync(linked).ino);
    assert.equal(statSync(different).ino, differentInode);
    assert.equal(statSync(javascript).ino, javascriptInode);
    assert.equal(statSync(sourceMap).ino, sourceMapInode);
    files.forEach((file, index) => assert.deepEqual(readFileSync(file), snapshots[index]));
    const current = files.map((file) => statSync(file).ino);
    if (inodes) {
      assert.deepEqual(current, inodes);
    }
    inodes = current;
  }
});

test("preserves file permissions and skips symlinks to packages and payloads", (t) => {
  const { root, modules, write, run } = fixture(t);
  const content = "exports.wasm = 'AAAA';\n";
  const regular = write("func-a/compiler.wasm.js", content);
  const executable = write("func-b/compiler.wasm.js", content);
  chmodSync(regular, 0o644);
  chmodSync(executable, 0o755);
  const before = [regular, executable].map((file) => statSync(file).ino);

  const external = path.join(root, "external");
  mkdirSync(external);
  const externalFile = path.join(external, "compiler.wasm.js");
  writeFileSync(externalFile, content);
  const externalInode = statSync(externalFile).ino;
  const packageLink = path.join(modules, "linked-package");
  const fileLink = path.join(modules, "linked.wasm.js");
  symlinkSync(external, packageLink, "dir");
  symlinkSync(externalFile, fileLink);

  run();
  assert.deepEqual([regular, executable].map((file) => statSync(file).ino), before);
  assert.equal(statSync(regular).mode & 0o777, 0o644);
  assert.equal(statSync(executable).mode & 0o777, 0o755);
  assert.equal(statSync(externalFile).ino, externalInode);
  assert.equal(lstatSync(packageLink).isSymbolicLink(), true);
  assert.equal(lstatSync(fileLink).isSymbolicLink(), true);
});

function fixture(t) {
  const root = mkdtempSync(path.join(tmpdir(), "verifier-deduplicate-wasm-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const worker = path.join(root, "worker");
  const modules = path.join(worker, "node_modules");
  const script = path.join(worker, "scripts", "patch-deduplicate-wasm.mjs");
  mkdirSync(path.dirname(script), { recursive: true });
  copyFileSync(new URL("./patch-deduplicate-wasm.mjs", import.meta.url), script);

  return {
    root,
    modules,
    write(filename, content) {
      const file = path.join(modules, filename);
      mkdirSync(path.dirname(file), { recursive: true });
      writeFileSync(file, content);
      return file;
    },
    run() {
      const result = spawnSync(process.execPath, [script], { encoding: "utf8" });
      assert.equal(result.status, 0, result.stderr);
      assert.equal(
        readdirSync(modules, { recursive: true }).some((name) => name.includes(".deduplicate-wasm-")),
        false,
      );
    },
  };
}
