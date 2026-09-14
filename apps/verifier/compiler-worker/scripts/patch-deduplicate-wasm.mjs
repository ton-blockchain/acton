import { createHash } from "node:crypto";
import {
  linkSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const nodeModulesDir = path.resolve(scriptDir, "..", "node_modules");

function main() {
  const groups = new Map();
  collectWasmFiles(nodeModulesDir, groups);

  let linkedFiles = 0;
  for (const files of groups.values()) {
    if (files.length < 2) {
      continue;
    }
    linkedFiles += deduplicateFiles(files);
  }
  console.log(`WASM files deduplicated: ${linkedFiles}`);
}

function collectWasmFiles(directory, groups) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const filePath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      collectWasmFiles(filePath, groups);
    } else if (entry.isFile() && entry.name.endsWith(".wasm.js")) {
      const stat = statSync(filePath);
      // Hard links share ownership and permissions and cannot cross devices.
      // Only compare files with compatible metadata and identical sizes.
      const key = [stat.dev, stat.mode, stat.uid, stat.gid, stat.size].join(":");
      const files = groups.get(key) ?? [];
      files.push({ filePath, stat });
      groups.set(key, files);
    }
    // Skip symlinks so linked packages outside this node_modules stay untouched.
  }
}

function deduplicateFiles(files) {
  const originals = new Map();
  let linkedFiles = 0;

  for (const file of files) {
    const content = readFileSync(file.filePath);
    const digest = createHash("sha256").update(content).digest("hex");
    const original = originals.get(digest);
    if (!original) {
      originals.set(digest, file);
      continue;
    }
    if (original.stat.ino === file.stat.ino) {
      continue;
    }
    if (!content.equals(readFileSync(original.filePath))) {
      throw new Error(`Refusing to link different WASM files: ${file.filePath}`);
    }

    replaceWithHardLink(original.filePath, file.filePath);
    linkedFiles++;
  }
  return linkedFiles;
}

function replaceWithHardLink(original, duplicate) {
  const temporaryDir = mkdtempSync(
    path.join(path.dirname(duplicate), ".deduplicate-wasm-"),
  );
  try {
    const temporaryLink = path.join(temporaryDir, "module.wasm.js");
    linkSync(original, temporaryLink);
    // Replace only after the link exists; failures leave the original file intact.
    renameSync(temporaryLink, duplicate);
  } finally {
    rmSync(temporaryDir, { recursive: true, force: true });
  }
}

main();
