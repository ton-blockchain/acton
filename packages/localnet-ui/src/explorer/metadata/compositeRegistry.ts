import type {ExtendedContractABI} from "../api/compilerAbi"
import type {VerificationSourceResponse} from "../api/types"
import {normalizeCodeHash} from "./codeHash"
import {NullMetadataRegistry, unverifiedSourceResponse} from "./nullRegistry"
import type {
  CompilerAbiRegistration,
  ExplorerMetadataRegistry,
  RegisteredCompilerAbi,
  RegisteredSource,
  SourceRegistration,
} from "./types"

export class CompositeMetadataRegistry implements ExplorerMetadataRegistry {
  readonly canWriteAddressNames: boolean
  readonly canWriteCompilerAbis: boolean
  readonly canWriteSources: boolean

  constructor(private readonly registries: readonly ExplorerMetadataRegistry[]) {
    this.canWriteAddressNames = registries.some(registry => registry.canWriteAddressNames)
    this.canWriteCompilerAbis = registries.some(registry => registry.canWriteCompilerAbis)
    this.canWriteSources = registries.some(registry => registry.canWriteSources)
  }

  async getAddressNames(addresses: readonly string[]): Promise<Record<string, string | undefined>> {
    const result: Record<string, string | undefined> = {}
    let unresolved = [...new Set(addresses.filter(Boolean))]

    for (const registry of this.registries) {
      if (unresolved.length === 0) {
        break
      }
      const names = await registry
        .getAddressNames(unresolved)
        .catch((): Record<string, string | undefined> => ({}))
      unresolved = unresolved.filter(address => {
        const name = names[address]
        if (name) {
          result[address] = name
          return false
        }
        return true
      })
    }

    for (const address of unresolved) {
      result[address] = undefined
    }
    return result
  }

  setAddressName(address: string, name: string | undefined): Promise<void> {
    return this.writable("canWriteAddressNames").setAddressName(address, name)
  }

  async getCompilerAbis(
    codeHashes: readonly string[],
  ): Promise<Record<string, ExtendedContractABI | null>> {
    const uniqueCodeHashes = [...new Set(codeHashes.filter(Boolean))]
    const result: Record<string, ExtendedContractABI | null> = {}
    let unresolved = uniqueCodeHashes

    for (const registry of this.registries) {
      if (unresolved.length === 0) {
        break
      }
      const abis = await registry
        .getCompilerAbis(unresolved)
        .catch((): Record<string, ExtendedContractABI | null> => ({}))
      unresolved = unresolved.filter(codeHash => {
        const abi = abis[codeHash] ?? null
        if (abi) {
          result[codeHash] = abi
          for (const alias of abi.code_hashes) {
            const normalizedAlias = normalizeCodeHash(alias)
            if (normalizedAlias) {
              result[normalizedAlias] = abi
            }
          }
          return false
        }
        return true
      })
    }

    for (const codeHash of unresolved) {
      result[codeHash] = null
    }
    return Object.fromEntries(codeHashes.map(codeHash => [codeHash, result[codeHash] ?? null]))
  }

  registerCompilerAbis(entries: readonly CompilerAbiRegistration[]): Promise<void> {
    return this.writable("canWriteCompilerAbis").registerCompilerAbis(entries)
  }

  listCompilerAbis(): Promise<readonly RegisteredCompilerAbi[]> {
    return this.writableOrNull("canWriteCompilerAbis").listCompilerAbis()
  }

  deleteCompilerAbi(codeHash: string): Promise<void> {
    return this.writable("canWriteCompilerAbis").deleteCompilerAbi(codeHash)
  }

  async getSource(options: {
    readonly address?: string
    readonly codeHash?: string
  }): Promise<VerificationSourceResponse> {
    for (const registry of this.registries) {
      const source = await registry.getSource(options).catch(() => undefined)
      if (source?.verified && source.bundles.length > 0) {
        return source
      }
    }
    return unverifiedSourceResponse(options)
  }

  registerSources(entries: readonly SourceRegistration[]): Promise<void> {
    return this.writable("canWriteSources").registerSources(entries)
  }

  listSources(): Promise<readonly RegisteredSource[]> {
    return this.writableOrNull("canWriteSources").listSources()
  }

  deleteSource(codeHash: string): Promise<void> {
    return this.writable("canWriteSources").deleteSource(codeHash)
  }

  private writable(capability: WritableCapability): ExplorerMetadataRegistry {
    const registry = this.registries.find(candidate => candidate[capability])
    if (!registry) {
      throw new Error("Metadata registry is read-only.")
    }
    return registry
  }

  private writableOrNull(capability: WritableCapability): ExplorerMetadataRegistry {
    return this.registries.find(candidate => candidate[capability]) ?? new NullMetadataRegistry()
  }
}

type WritableCapability = "canWriteAddressNames" | "canWriteCompilerAbis" | "canWriteSources"
