import type {ContractABI} from "@ton/tolk-abi-to-typescript"

import type {ExtendedContractABI} from "../api/compilerAbi"
import type {
  SourceBundle,
  SourceCompiler,
  SourceFile,
  VerificationSourceResponse,
} from "../api/types"
import {normalizeCodeHash} from "../metadata/codeHash"
import {sourceRegistrationFromResponse} from "../metadata/sourceRegistration"
import type {CompilerAbiRegistration, SourceRegistration} from "../metadata/types"

export interface AbiImportFile {
  readonly path: string
  readonly text: string
}

export interface AbiImportPlan {
  readonly registrations: readonly CompilerAbiRegistration[]
  readonly registeredNames: readonly string[]
  readonly warnings: readonly string[]
}

// A dropped acton project (or its build/ dir) carries plenty of JSON that is
// not a registrable artifact: `cache/` holds stale contract versions, and the
// rest are logs, run sessions and dependency/output trees.
const SKIPPED_DIRS: ReadonlySet<string> = new Set([
  "cache",
  "logs",
  "sessions",
  "node_modules",
  "target",
  "dist",
])

// Hidden directories are skipped (.git alone makes walking them prohibitive),
// but acton-studio legitimately keeps generated artifacts under .studio/.
const ALLOWED_HIDDEN_DIRS: ReadonlySet<string> = new Set([".studio"])

function isSkippedDirName(name: string): boolean {
  return SKIPPED_DIRS.has(name) || (name.startsWith(".") && !ALLOWED_HIDDEN_DIRS.has(name))
}

const MAX_IMPORT_FILES = 2000
const MAX_IMPORT_FILE_BYTES = 8 * 1024 * 1024

export function isCompilerAbi(value: unknown): value is ContractABI {
  if (!value || typeof value !== "object") {
    return false
  }

  const abi = value as Partial<ContractABI>
  return (
    typeof abi.contract_name === "string" &&
    Array.isArray(abi.get_methods) &&
    Array.isArray(abi.incoming_messages) &&
    Array.isArray(abi.incoming_external) &&
    Array.isArray(abi.outgoing_messages) &&
    Array.isArray(abi.emitted_events) &&
    Array.isArray(abi.declarations) &&
    Array.isArray(abi.thrown_errors)
  )
}

export function extendedAbiFromUpload(
  source: unknown,
  codeHashes: readonly string[],
  displayName: string,
): ExtendedContractABI {
  const record =
    source && typeof source === "object" ? (source as Partial<ExtendedContractABI>) : {}
  const compilerAbi =
    record.compiler_abi && typeof record.compiler_abi === "object"
      ? (record.compiler_abi as ContractABI)
      : (source as ContractABI)

  if (!isCompilerAbi(compilerAbi)) {
    throw new Error("Uploaded JSON must be a compiler ABI.")
  }

  return {
    compiler_abi: compilerAbi,
    display_name:
      displayName.trim() ||
      (typeof record.display_name === "string" ? record.display_name.trim() : "") ||
      compilerAbi.contract_name,
    code_hashes: codeHashes,
    links: Array.isArray(record.links) ? record.links : [],
  }
}

interface AbiCandidate {
  readonly dir: string
  readonly base: string
  readonly abi: ContractABI
  readonly source: unknown
}

interface CodeCandidate {
  readonly dir: string
  readonly base: string
  readonly hash: string
}

/**
 * Pairs the ABI JSONs of a dropped acton `build/` directory with their compiled
 * code hashes: `build/abi/<Name>.json` (raw compiler ABI) matches
 * `build/<Name>.json` (`{code_boc64, hash}`) by basename, falling back to the
 * ABI's `contract_name`. Files that already carry their own code hashes
 * (exported extended ABIs) register as-is.
 */
export function buildAbiImportPlan(files: readonly AbiImportFile[]): AbiImportPlan {
  const abiCandidates: AbiCandidate[] = []
  const codeCandidates: CodeCandidate[] = []
  const registrations: CompilerAbiRegistration[] = []
  const warnings: string[] = []

  for (const file of files) {
    const path = normalizePath(file.path)
    if (!path.toLowerCase().endsWith(".json") || isInSkippedDir(path)) {
      continue
    }

    let parsed: unknown
    try {
      parsed = JSON.parse(file.text)
    } catch {
      continue
    }
    if (!parsed || typeof parsed !== "object") {
      continue
    }

    const {dir, base} = splitJsonPath(path)
    const record = parsed as {
      readonly compiler_abi?: unknown
      readonly code_hashes?: unknown
    }

    if (record.compiler_abi && isCompilerAbi(record.compiler_abi)) {
      const inlineHashes = normalizeHashList(
        Array.isArray(record.code_hashes) ? record.code_hashes : [],
      )
      if (inlineHashes.length > 0) {
        registrations.push({abi: extendedAbiFromUpload(parsed, inlineHashes, "")})
      } else {
        abiCandidates.push({dir, base, abi: record.compiler_abi, source: parsed})
      }
      continue
    }

    if (isCompilerAbi(parsed)) {
      abiCandidates.push({dir, base, abi: parsed, source: parsed})
      continue
    }

    const hash = codeArtifactHash(parsed)
    if (hash) {
      codeCandidates.push({dir, base, hash})
    }
  }

  // Key on dir+base, not base alone: a drop spanning several build trees may
  // legitimately carry same-named contracts with different code hashes.
  const seenAbiPaths = new Set<string>()
  for (const candidate of abiCandidates) {
    const candidateKey = `${candidate.dir}/${candidate.base}`
    if (seenAbiPaths.has(candidateKey)) {
      continue
    }
    seenAbiPaths.add(candidateKey)

    const hashes = matchCodeHashes(candidate, codeCandidates)
    if (hashes.length === 0) {
      warnings.push(
        `${candidate.abi.contract_name}: no matching code hash (expected ${candidate.base}.json with a "hash" field next to the abi/ directory)`,
      )
      continue
    }
    registrations.push({abi: extendedAbiFromUpload(candidate.source, hashes, "")})
  }

  const uniqueRegistrations = dedupeByCodeHash(registrations)
  return {
    registrations: uniqueRegistrations,
    registeredNames: uniqueRegistrations.map(
      entry => entry.abi.display_name?.trim() || entry.abi.compiler_abi.contract_name,
    ),
    warnings,
  }
}

export interface SourceImportPlan {
  readonly registrations: readonly SourceRegistration[]
  readonly registeredNames: readonly string[]
  readonly warnings: readonly string[]
}

/**
 * Finds `acton build --output-sources` artifacts anywhere in a dropped
 * directory (typically the whole project root - the artifacts live in
 * `build/sources/<Name>.source.json`), so nothing needs to be cherry-picked
 * per contract. Recognizes both the current `bundle` artifact shape and the
 * legacy `bundles` array emitted by older CLIs.
 */
export function buildSourceImportPlan(files: readonly AbiImportFile[]): SourceImportPlan {
  const registrations: SourceRegistration[] = []
  const registeredNames: string[] = []
  const warnings: string[] = []
  const seenCodeHashes = new Set<string>()

  for (const file of files) {
    const path = normalizePath(file.path)
    if (!path.toLowerCase().endsWith(".json") || isInSkippedDir(path)) {
      continue
    }

    let parsed: unknown
    try {
      parsed = JSON.parse(file.text)
    } catch {
      continue
    }

    const source = sourceArtifactFromJson(parsed)
    if (!source) {
      continue
    }

    const registration = sourceRegistrationFromResponse(source)
    if (!registration) {
      continue
    }
    if (seenCodeHashes.has(registration.codeHash)) {
      continue
    }
    seenCodeHashes.add(registration.codeHash)
    registrations.push(registration)
    registeredNames.push(source.bundle?.entrypoint ?? splitJsonPath(path).base)
  }

  if (registrations.length === 0) {
    warnings.push(
      "No source artifacts found. Generate them with `acton build --output-sources build/sources` and drop the project again.",
    )
  }

  return {registrations, registeredNames, warnings}
}

// Accepts both the current artifact shape ({code_hash, verified, bundle}) and
// the legacy one ({code_hash, verified, bundles: [...]}) written by CLIs
// predating the single-bundle-per-code-hash change.
export function sourceArtifactFromJson(value: unknown): VerificationSourceResponse | undefined {
  if (!isRecord(value)) {
    return undefined
  }

  const codeHash =
    typeof value.code_hash === "string" ? normalizeCodeHash(value.code_hash) : undefined
  if (!codeHash || typeof value.verified !== "boolean") {
    return undefined
  }

  const bundle = isSourceBundle(value.bundle)
    ? value.bundle
    : Array.isArray(value.bundles)
      ? value.bundles.find(isSourceBundle)
      : undefined
  if (!bundle) {
    return undefined
  }

  return {code_hash: value.code_hash as string, verified: value.verified, bundle}
}

function isSourceBundle(value: unknown): value is SourceBundle {
  return (
    isRecord(value) &&
    typeof value.source_bundle_hash === "string" &&
    typeof value.verified_at === "number" &&
    typeof value.storage_revision === "string" &&
    typeof value.entrypoint === "string" &&
    isSourceCompiler(value.compiler) &&
    Array.isArray(value.files) &&
    value.files.length > 0 &&
    value.files.every(isSourceFile)
  )
}

function isSourceCompiler(value: unknown): value is SourceCompiler {
  return (
    isRecord(value) &&
    typeof value.language === "string" &&
    typeof value.version === "string" &&
    "params" in value
  )
}

function isSourceFile(value: unknown): value is SourceFile {
  return (
    isRecord(value) &&
    typeof value.path === "string" &&
    typeof value.content_hash === "string" &&
    isNullableBool(value.include_in_command) &&
    isNullableBool(value.is_stdlib) &&
    isNullableBool(value.has_include_directives) &&
    typeof value.content === "string"
  )
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function isNullableBool(value: unknown): value is boolean | null {
  return value === null || typeof value === "boolean"
}

export async function collectDroppedImportFiles(
  dataTransfer: DataTransfer,
): Promise<AbiImportFile[]> {
  const entries = [...dataTransfer.items]
    .map(item => (item.kind === "file" ? item.webkitGetAsEntry() : null))
    .filter((entry): entry is FileSystemEntry => Boolean(entry))

  if (entries.length === 0) {
    return collectPickedImportFiles(dataTransfer.files)
  }

  const files: AbiImportFile[] = []
  for (const entry of entries) {
    await walkEntry(entry, files)
  }
  return files
}

export async function collectPickedImportFiles(
  fileList: FileList | null,
): Promise<AbiImportFile[]> {
  const files: AbiImportFile[] = []
  for (const file of [...(fileList ?? [])]) {
    if (files.length >= MAX_IMPORT_FILES) {
      break
    }
    const path = normalizePath(file.webkitRelativePath || file.name)
    if (!isReadableJsonFile(path, file.size)) {
      continue
    }
    files.push({path, text: await file.text()})
  }
  return files
}

async function walkEntry(entry: FileSystemEntry, out: AbiImportFile[]): Promise<void> {
  if (out.length >= MAX_IMPORT_FILES) {
    return
  }

  if (entry.isFile) {
    const path = normalizePath(entry.fullPath || entry.name)
    if (!path.toLowerCase().endsWith(".json") || isInSkippedDir(path)) {
      return
    }
    try {
      const file = await new Promise<File>((resolve, reject) => {
        ;(entry as FileSystemFileEntry).file(resolve, reject)
      })
      if (!isReadableJsonFile(path, file.size)) {
        return
      }
      out.push({path, text: await file.text()})
    } catch {
      // Unreadable entries (permissions, vanished files) are skipped silently.
    }
    return
  }

  if (entry.isDirectory) {
    if (isSkippedDirName(entry.name)) {
      return
    }
    const reader = (entry as FileSystemDirectoryEntry).createReader()
    // readEntries returns results in chunks (Chrome caps at 100); drain it.
    for (;;) {
      const batch = await new Promise<FileSystemEntry[]>((resolve, reject) => {
        reader.readEntries(resolve, reject)
      }).catch(() => [] as FileSystemEntry[])
      if (batch.length === 0) {
        return
      }
      for (const child of batch) {
        await walkEntry(child, out)
      }
    }
  }
}

function isReadableJsonFile(path: string, size: number): boolean {
  return (
    path.toLowerCase().endsWith(".json") && !isInSkippedDir(path) && size <= MAX_IMPORT_FILE_BYTES
  )
}

function normalizePath(path: string): string {
  return path.replaceAll("\\", "/").replace(/^\/+/, "")
}

function isInSkippedDir(path: string): boolean {
  return path.split("/").slice(0, -1).some(isSkippedDirName)
}

function splitJsonPath(path: string): {readonly dir: string; readonly base: string} {
  const separatorIndex = path.lastIndexOf("/")
  const dir = separatorIndex >= 0 ? path.slice(0, separatorIndex) : ""
  const fileName = separatorIndex >= 0 ? path.slice(separatorIndex + 1) : path
  return {dir, base: fileName.replace(/\.json$/i, "")}
}

function parentDir(dir: string): string {
  const separatorIndex = dir.lastIndexOf("/")
  return separatorIndex >= 0 ? dir.slice(0, separatorIndex) : ""
}

function normalizeHashList(values: readonly unknown[]): readonly string[] {
  return [
    ...new Set(
      values
        .filter((value): value is string => typeof value === "string")
        .map(normalizeCodeHash)
        .filter((value): value is string => Boolean(value)),
    ),
  ]
}

function codeArtifactHash(parsed: object): string | undefined {
  const record = parsed as {
    readonly hash?: unknown
    readonly code_hash?: unknown
    readonly code_hash_hex?: unknown
  }
  const candidates = [record.hash, record.code_hash, record.code_hash_hex]
  for (const candidate of candidates) {
    if (typeof candidate === "string") {
      const normalized = normalizeCodeHash(candidate)
      if (normalized) {
        return normalized
      }
    }
  }
  return undefined
}

function matchCodeHashes(
  abiCandidate: AbiCandidate,
  codeCandidates: readonly CodeCandidate[],
): readonly string[] {
  const byBase = codeCandidates.filter(candidate => candidate.base === abiCandidate.base)
  // `build/abi/<Name>.json` pairs with `build/<Name>.json`: the code artifact
  // one level above the abi/ directory wins over same-named files elsewhere.
  const siblingDir = parentDir(abiCandidate.dir)
  const siblings = byBase.filter(candidate => candidate.dir === siblingDir)
  const matched =
    siblings.length > 0
      ? siblings
      : byBase.length > 0
        ? byBase
        : codeCandidates.filter(candidate => candidate.base === abiCandidate.abi.contract_name)
  return [...new Set(matched.map(candidate => candidate.hash))]
}

// Deduplicates on the COMPLETE code-hash sets: a later registration sharing any
// hash with an earlier one merges its remaining hashes into that entry instead
// of being dropped (or duplicating the shared hash in the registry).
function dedupeByCodeHash(
  registrations: readonly CompilerAbiRegistration[],
): readonly CompilerAbiRegistration[] {
  const merged: CompilerAbiRegistration[] = []
  const indexByHash = new Map<string, number>()
  for (const registration of registrations) {
    const hashes = registration.abi.code_hashes
    const existingIndex = hashes
      .map(hash => indexByHash.get(hash))
      .find((index): index is number => index !== undefined)
    if (existingIndex === undefined) {
      const index = merged.length
      merged.push(registration)
      for (const hash of hashes) {
        indexByHash.set(hash, index)
      }
      continue
    }

    const existing = merged[existingIndex]
    const union = [...new Set([...existing.abi.code_hashes, ...hashes])]
    if (union.length !== existing.abi.code_hashes.length) {
      merged[existingIndex] = {abi: {...existing.abi, code_hashes: union}}
    }
    for (const hash of hashes) {
      indexByHash.set(hash, existingIndex)
    }
  }
  return merged
}
