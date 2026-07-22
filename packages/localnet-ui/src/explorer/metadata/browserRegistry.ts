import {openDB, type DBSchema, type IDBPDatabase} from "idb"

import {addressKey} from "../api/compilerAbi"
import type {ExtendedContractABI} from "../api/compilerAbi"
import type {VerificationSourceResponse} from "../api/types"
import {normalizeCodeHash} from "./codeHash"
import {unverifiedSourceResponse} from "./nullRegistry"
import type {
  CompilerAbiRegistration,
  ExplorerMetadataRegistry,
  RegisteredCompilerAbi,
  RegisteredSource,
  SourceRegistration,
} from "./types"

interface StoredCompilerAbi extends RegisteredCompilerAbi {
  readonly id: string
  readonly namespace: string
}

interface StoredSource extends RegisteredSource {
  readonly id: string
  readonly namespace: string
}

type NormalizedCompilerAbi = Pick<RegisteredCompilerAbi, "codeHash" | "abi">

interface MetadataDb extends DBSchema {
  compilerAbis: {
    key: string
    value: StoredCompilerAbi
    indexes: {
      "by-namespace": string
    }
  }
  sources: {
    key: string
    value: StoredSource
    indexes: {
      "by-namespace": string
    }
  }
}

const DB_NAME = "acton-explorer-metadata"
const DB_VERSION = 1

export class BrowserMetadataRegistry implements ExplorerMetadataRegistry {
  readonly canWriteAddressNames = true
  readonly canWriteCompilerAbis = true
  readonly canWriteSources = true

  private readonly namespace: string
  private dbPromise: Promise<IDBPDatabase<MetadataDb>> | undefined

  constructor(namespace: string) {
    this.namespace = namespace
  }

  async getAddressNames(addresses: readonly string[]): Promise<Record<string, string | undefined>> {
    const names = readAddressNames(this.namespace)
    return Object.fromEntries(
      addresses.map(address => [address, names.get(addressKey(address)) ?? undefined]),
    )
  }

  async setAddressName(address: string, name: string | undefined): Promise<void> {
    const names = readAddressNames(this.namespace)
    const key = addressKey(address)
    const nextName = name?.trim()
    if (nextName) {
      names.set(key, nextName)
    } else {
      names.delete(key)
    }
    writeAddressNames(this.namespace, names)
  }

  async getCompilerAbis(
    codeHashes: readonly string[],
  ): Promise<Record<string, ExtendedContractABI | null>> {
    const db = await this.db()
    const entries = await db.getAllFromIndex("compilerAbis", "by-namespace", this.namespace)
    const result: Record<string, ExtendedContractABI | null> = {}
    for (const codeHash of codeHashes) {
      const normalized = normalizeCodeHash(codeHash)
      const entry = normalized
        ? entries.find(candidate => compilerAbiCodeHashes(candidate.abi).includes(normalized))
        : undefined
      result[codeHash] = entry?.abi ?? null
    }
    return result
  }

  async registerCompilerAbis(entries: readonly CompilerAbiRegistration[]): Promise<void> {
    if (entries.length === 0) {
      return
    }

    const db = await this.db()
    const existingEntries = await db.getAllFromIndex("compilerAbis", "by-namespace", this.namespace)
    const normalizedEntries = entries
      .map(entry => normalizeRegisteredCompilerAbi(entry.abi))
      .filter((entry): entry is NormalizedCompilerAbi => Boolean(entry))
    const staleIds = new Set(
      normalizedEntries.flatMap(entry => {
        const codeHashes = compilerAbiCodeHashes(entry.abi)
        return existingEntries
          .filter(existingEntry =>
            compilerAbiCodeHashes(existingEntry.abi).some(hash => codeHashes.includes(hash)),
          )
          .map(existingEntry => existingEntry.id)
      }),
    )
    const tx = db.transaction("compilerAbis", "readwrite")
    const savedAt = Date.now()
    await Promise.all([
      ...[...staleIds].map(id => tx.store.delete(id)),
      ...normalizedEntries.map(entry =>
        tx.store.put({
          ...entry,
          id: this.entryId(entry.codeHash),
          namespace: this.namespace,
          savedAt,
        }),
      ),
    ])
    await tx.done
  }

  async listCompilerAbis(): Promise<readonly RegisteredCompilerAbi[]> {
    const db = await this.db()
    const entries = await db.getAllFromIndex("compilerAbis", "by-namespace", this.namespace)
    return entries.sort((a, b) => b.savedAt - a.savedAt)
  }

  async deleteCompilerAbi(codeHash: string): Promise<void> {
    const normalized = normalizeCodeHash(codeHash)
    if (!normalized) {
      return
    }
    const db = await this.db()
    const entries = await db.getAllFromIndex("compilerAbis", "by-namespace", this.namespace)
    const entry = entries.find(candidate =>
      compilerAbiCodeHashes(candidate.abi).includes(normalized),
    )
    if (entry) {
      await db.delete("compilerAbis", entry.id)
    }
  }

  async getSource(options: {
    readonly address?: string
    readonly codeHash?: string
  }): Promise<VerificationSourceResponse> {
    const codeHash = normalizeCodeHash(options.codeHash)
    if (!codeHash) {
      return unverifiedSourceResponse(options)
    }

    const db = await this.db()
    const entry = await db.get("sources", this.entryId(codeHash))
    return entry?.source ?? unverifiedSourceResponse(options)
  }

  async registerSources(entries: readonly SourceRegistration[]): Promise<void> {
    if (entries.length === 0) {
      return
    }

    const db = await this.db()
    const tx = db.transaction(["compilerAbis", "sources"], "readwrite")
    const savedAt = Date.now()
    await Promise.all(
      entries.map(async entry => {
        const codeHash = normalizeCodeHash(entry.codeHash)
        if (!codeHash) {
          return
        }

        await tx.objectStore("sources").put({
          id: this.entryId(codeHash),
          namespace: this.namespace,
          codeHash,
          source: normalizeRegisteredSource(codeHash, entry.source),
          savedAt,
        })

        const compilerAbi = compilerAbiFromSource(entry.source)
        if (compilerAbi) {
          const abi: ExtendedContractABI = {
            compiler_abi: compilerAbi,
            display_name: compilerAbi.contract_name,
            code_hashes: [codeHash],
            links: [],
          }
          await tx.objectStore("compilerAbis").put({
            id: this.entryId(codeHash),
            namespace: this.namespace,
            codeHash,
            abi,
            savedAt,
          })
        }
      }),
    )
    await tx.done
  }

  async listSources(): Promise<readonly RegisteredSource[]> {
    const db = await this.db()
    const entries = await db.getAllFromIndex("sources", "by-namespace", this.namespace)
    return entries.sort((a, b) => b.savedAt - a.savedAt)
  }

  async deleteSource(codeHash: string): Promise<void> {
    const normalized = normalizeCodeHash(codeHash)
    if (!normalized) {
      return
    }
    const db = await this.db()
    await db.delete("sources", this.entryId(normalized))
  }

  private entryId(codeHash: string): string {
    return `${this.namespace}:${codeHash}`
  }

  private db(): Promise<IDBPDatabase<MetadataDb>> {
    this.dbPromise ??= openDB<MetadataDb>(DB_NAME, DB_VERSION, {
      upgrade(db) {
        if (!db.objectStoreNames.contains("compilerAbis")) {
          const store = db.createObjectStore("compilerAbis", {keyPath: "id"})
          store.createIndex("by-namespace", "namespace")
        }
        if (!db.objectStoreNames.contains("sources")) {
          const store = db.createObjectStore("sources", {keyPath: "id"})
          store.createIndex("by-namespace", "namespace")
        }
      },
    })
    return this.dbPromise
  }
}

function addressNamesStorageKey(namespace: string): string {
  return `acton:metadata:${namespace}:address-names:v1`
}

function readAddressNames(namespace: string): Map<string, string> {
  try {
    const raw = globalThis.localStorage?.getItem(addressNamesStorageKey(namespace))
    if (!raw) {
      return new Map()
    }
    const parsed = JSON.parse(raw) as unknown
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return new Map()
    }
    return new Map(
      Object.entries(parsed).filter(
        (entry): entry is [string, string] =>
          typeof entry[0] === "string" && typeof entry[1] === "string",
      ),
    )
  } catch {
    return new Map()
  }
}

function writeAddressNames(namespace: string, names: ReadonlyMap<string, string>): void {
  try {
    globalThis.localStorage?.setItem(
      addressNamesStorageKey(namespace),
      JSON.stringify(Object.fromEntries(names)),
    )
  } catch {
    // Ignore storage quota and privacy-mode errors; address names remain session-local.
  }
}

function normalizeRegisteredCompilerAbi(
  abi: ExtendedContractABI,
): NormalizedCompilerAbi | undefined {
  const codeHashes = compilerAbiCodeHashes(abi)
  const codeHash = codeHashes[0]
  if (!codeHash) {
    return undefined
  }
  return {
    codeHash,
    abi: {
      ...abi,
      display_name: abi.display_name?.trim() || abi.compiler_abi.contract_name,
      code_hashes: codeHashes,
      links: Array.isArray(abi.links) ? abi.links : [],
    },
  }
}

function compilerAbiCodeHashes(abi: ExtendedContractABI): string[] {
  return [
    ...new Set(
      abi.code_hashes.map(normalizeCodeHash).filter((value): value is string => Boolean(value)),
    ),
  ]
}

function normalizeRegisteredSource(
  codeHash: string,
  source: VerificationSourceResponse,
): VerificationSourceResponse {
  return {
    ...source,
    code_hash: normalizeCodeHash(source.code_hash) ?? codeHash,
    verified: source.verified || source.bundles.length > 0,
  }
}

function compilerAbiFromSource(
  source: VerificationSourceResponse,
): ExtendedContractABI["compiler_abi"] | undefined {
  for (const bundle of source.bundles) {
    const fromBundle = objectCompilerAbi(bundle)
    if (fromBundle) {
      return fromBundle
    }
    const fromFiles = bundle.files.map(file => objectCompilerAbi(file)).find(Boolean)
    if (fromFiles) {
      return fromFiles
    }
  }
  return undefined
}

function objectCompilerAbi(value: unknown): ExtendedContractABI["compiler_abi"] | undefined {
  if (!value || typeof value !== "object") {
    return undefined
  }

  const maybeCompilerAbi = (value as {readonly compiler_abi?: unknown}).compiler_abi
  if (maybeCompilerAbi && typeof maybeCompilerAbi === "object") {
    return maybeCompilerAbi as ExtendedContractABI["compiler_abi"]
  }

  const content = (value as {readonly content?: unknown}).content
  if (typeof content !== "string") {
    return undefined
  }

  try {
    const parsed = JSON.parse(content) as unknown
    return parsed && typeof parsed === "object"
      ? (parsed as ExtendedContractABI["compiler_abi"])
      : undefined
  } catch {
    return undefined
  }
}
